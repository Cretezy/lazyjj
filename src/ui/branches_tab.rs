#![allow(clippy::borrow_interior_mutable_const)]
use crate::{
    commander::{branches::BranchLine, ids::ChangeId, CommandError, Commander},
    env::{Config, DiffFormat},
    ui::{
        details_panel::DetailsPanel,
        help_popup::HelpPopup,
        message_popup::MessagePopup,
        utils::{centered_rect, centered_rect_line_height, tabs_to_spaces},
        Component, ComponentAction,
    },
    ComponentInputResult,
};
use ansi_to_tui::IntoText;
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{prelude::*, widgets::*};
use tracing::instrument;
use tui_confirm_dialog::{ButtonLabel, ConfirmDialog, ConfirmDialogState, Listener};
use tui_textarea::{CursorMove, TextArea};

struct CreateBranch<'a> {
    textarea: TextArea<'a>,
    error: Option<anyhow::Error>,
}

struct RenameBranch<'a> {
    textarea: TextArea<'a>,
    name: String,
    error: Option<anyhow::Error>,
}

struct DeleteBranch {
    name: String,
}

struct ForgetBranch {
    name: String,
}

const DELETE_BRANCH_POPUP_ID: u16 = 1;
const FORGET_BRANCH_POPUP_ID: u16 = 2;
const NEW_POPUP_ID: u16 = 3;
const EDIT_POPUP_ID: u16 = 4;

/// Branches tab. Shows branches in left panel and selected branch current change in right panel.
pub struct BranchesTab<'a> {
    branches_output: Result<Vec<BranchLine>, CommandError>,
    branches_list_state: ListState,
    branches_height: u16,

    show_all: bool,

    branch: Option<BranchLine>,

    branch_panel: DetailsPanel,
    branch_output: Option<Result<String, CommandError>>,

    create: Option<CreateBranch<'a>>,
    rename: Option<RenameBranch<'a>>,
    delete: Option<DeleteBranch>,
    forget: Option<ForgetBranch>,

    describe_textarea: Option<TextArea<'a>>,
    describe_after_new: bool,
    describe_after_new_change: Option<ChangeId>,

    popup: ConfirmDialogState,
    popup_tx: std::sync::mpsc::Sender<Listener>,
    popup_rx: std::sync::mpsc::Receiver<Listener>,

    diff_format: DiffFormat,

    config: Config,
}

fn get_current_branch_index(
    current_branch: Option<&BranchLine>,
    branches_output: &Result<Vec<BranchLine>, CommandError>,
) -> Option<usize> {
    match branches_output {
        Ok(branches_output) => current_branch.as_ref().and_then(|current_branch| {
            branches_output
                .iter()
                .position(|branch| match (current_branch, branch) {
                    (
                        BranchLine::Parsed {
                            branch: current_branch,
                            ..
                        },
                        BranchLine::Parsed { branch, .. },
                    ) => {
                        current_branch.name == branch.name && current_branch.remote == branch.remote
                    }
                    (BranchLine::Unparsable(current_branch), BranchLine::Unparsable(branch)) => {
                        current_branch == branch
                    }
                    _ => false,
                })
        }),
        Err(_) => None,
    }
}

impl BranchesTab<'_> {
    #[instrument(level = "trace", skip(commander))]
    pub fn new(commander: &mut Commander) -> Result<Self> {
        let diff_format = commander.env.config.diff_format();

        let show_all = false;

        let branches_output = commander.get_branches(show_all);
        let branch = branches_output
            .as_ref()
            .ok()
            .and_then(|branches_output| branches_output.first())
            .map(|branches_output| branches_output.to_owned());

        let branches_list_state = ListState::default()
            .with_selected(get_current_branch_index(branch.as_ref(), &branches_output));

        let branch_output = branch.as_ref().and_then(|branch| match branch {
            BranchLine::Parsed { branch, .. } => Some(
                commander
                    .get_branch_show(branch, &diff_format)
                    .map(|diff| tabs_to_spaces(&diff)),
            ),
            _ => None,
        });

        let (popup_tx, popup_rx) = std::sync::mpsc::channel();

        Ok(Self {
            branches_output,
            branch,
            branches_list_state,
            branches_height: 0,

            show_all,

            branch_panel: DetailsPanel::new(),
            branch_output,

            create: None,
            rename: None,
            delete: None,
            forget: None,

            describe_after_new: false,
            describe_textarea: None,
            describe_after_new_change: None,

            popup: ConfirmDialogState::default(),
            popup_tx,
            popup_rx,

            diff_format,

            config: commander.env.config.clone(),
        })
    }

    pub fn get_current_branch_index(&self) -> Option<usize> {
        get_current_branch_index(self.branch.as_ref(), &self.branches_output)
    }

    pub fn refresh_branches(&mut self, commander: &mut Commander) {
        self.branches_output = commander.get_branches(self.show_all);
    }

    pub fn refresh_branch(&mut self, commander: &mut Commander) {
        self.branch_output = self.branch.as_ref().and_then(|branch| match branch {
            BranchLine::Parsed { branch, .. } => Some(
                commander
                    .get_branch_show(branch, &self.diff_format)
                    .map(|diff| tabs_to_spaces(&diff)),
            ),
            _ => None,
        });

        self.branch_panel.scroll = 0;
    }

    fn scroll_branches(&mut self, commander: &mut Commander, scroll: isize) {
        let branches = Vec::new();
        let branches = self.branches_output.as_ref().unwrap_or(&branches);
        let current_branch_index = self.get_current_branch_index();
        let next_branch = match current_branch_index {
            Some(current_branch_index) => branches.get(
                current_branch_index
                    .saturating_add_signed(scroll)
                    .min(branches.len() - 1),
            ),
            None => branches.first(),
        }
        .map(|x| x.to_owned());

        if let Some(next_branch) = next_branch {
            self.branch = Some(next_branch);
            self.refresh_branch(commander);
        }
    }
}

impl Component for BranchesTab<'_> {
    fn switch(&mut self, commander: &mut Commander) -> Result<()> {
        self.refresh_branches(commander);
        self.refresh_branch(commander);
        Ok(())
    }

    fn update(&mut self, commander: &mut Commander) -> Result<Option<ComponentAction>> {
        // Check for popup action
        if let Ok(res) = self.popup_rx.try_recv() {
            if res.1.unwrap_or(false) {
                match res.0 {
                    DELETE_BRANCH_POPUP_ID => {
                        if let Some(delete) = self.delete.as_ref() {
                            match commander.delete_branch(&delete.name) {
                                Ok(_) => {
                                    self.refresh_branches(commander);
                                    let branches = Vec::new();
                                    let branches =
                                        self.branches_output.as_ref().unwrap_or(&branches);
                                    self.branch = branches.first().map(|branch| branch.to_owned());
                                    self.refresh_branch(commander);
                                }
                                Err(err) => {
                                    return Ok(Some(ComponentAction::SetPopup(Some(Box::new(
                                        MessagePopup {
                                            title: "Delete error".into(),
                                            messages: err.to_string().into_text()?,
                                        },
                                    )))));
                                }
                            }
                        }
                    }
                    FORGET_BRANCH_POPUP_ID => {
                        if let Some(forget) = self.forget.as_ref() {
                            match commander.forget_branch(&forget.name) {
                                Ok(_) => {
                                    self.refresh_branches(commander);
                                    let branches = Vec::new();
                                    let branches =
                                        self.branches_output.as_ref().unwrap_or(&branches);
                                    self.branch = branches.first().map(|branch| branch.to_owned());
                                    self.refresh_branch(commander);
                                }
                                Err(err) => {
                                    return Ok(Some(ComponentAction::SetPopup(Some(Box::new(
                                        MessagePopup {
                                            title: "Forget error".into(),
                                            messages: err.to_string().into_text()?,
                                        },
                                    )))));
                                }
                            }
                        }
                    }
                    NEW_POPUP_ID => {
                        if let Some(BranchLine::Parsed { branch, .. }) = self.branch.as_ref() {
                            commander.run_new(&branch.to_string())?;
                            let head = commander.get_current_head()?;
                            if self.describe_after_new {
                                self.describe_after_new_change = Some(head.change_id);
                                self.describe_after_new = false;
                                let textarea = TextArea::default();
                                self.describe_textarea = Some(textarea);
                                return Ok(None);
                            } else {
                                return Ok(Some(ComponentAction::ViewLog(head)));
                            }
                        }
                    }
                    EDIT_POPUP_ID => {
                        if let Some(BranchLine::Parsed { branch, .. }) = self.branch.as_ref() {
                            commander.run_edit(&branch.to_string())?;
                            let head = commander.get_current_head()?;
                            return Ok(Some(ComponentAction::ViewLog(head)));
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(None)
    }

    fn draw(
        &mut self,
        f: &mut ratatui::prelude::Frame<'_>,
        area: ratatui::prelude::Rect,
    ) -> Result<()> {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Draw branches
        {
            let current_branch_index = self.get_current_branch_index();

            let branch_lines: Vec<Line> = match self.branches_output.as_ref() {
                Ok(branches_output) => branches_output
                    .iter()
                    .enumerate()
                    .map(|(i, branch)| -> Result<Vec<Line>, ansi_to_tui::Error> {
                        let branch_text = branch.to_text()?;
                        Ok(branch_text
                            .iter()
                            .map(|line| {
                                let mut line = line.to_owned();

                                // Add padding at start
                                line.spans.insert(0, Span::from(" "));

                                if current_branch_index
                                    .map_or(false, |current_branch_index| i == current_branch_index)
                                {
                                    line = line.bg(self.config.highlight_color());

                                    line.spans = line
                                        .spans
                                        .iter_mut()
                                        .map(|span| {
                                            span.to_owned().bg(self.config.highlight_color())
                                        })
                                        .collect();
                                }

                                line
                            })
                            .collect::<Vec<Line>>())
                    })
                    .collect::<Result<Vec<Vec<Line>>, ansi_to_tui::Error>>()?
                    .into_iter()
                    .flatten()
                    .collect(),
                Err(err) => [
                    vec![Line::raw("Error getting branches").bold().fg(Color::Red)],
                    // TODO: Remove when jj 0.20 is released
                    if let CommandError::Status(output, _) = err {
                        if output.contains("unexpected argument '-T' found") {
                            vec![
                                Line::raw(""),
                                Line::raw("Please update jj to >0.18 for -T support to branches")
                                    .bold()
                                    .fg(Color::Red),
                            ]
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    },
                    vec![Line::raw(""), Line::raw("")],
                    err.to_string().into_text()?.lines,
                ]
                .concat(),
            };

            let lines = if branch_lines.is_empty() {
                vec![Line::from(" No branches").fg(Color::DarkGray).italic()]
            } else {
                branch_lines
            };

            let branches_block = Block::bordered()
                .title(" Branches ")
                .border_type(BorderType::Rounded);
            self.branches_height = branches_block.inner(chunks[0]).height;
            let branches = List::new(lines).block(branches_block).scroll_padding(3);
            *self.branches_list_state.selected_mut() = current_branch_index;
            f.render_stateful_widget(branches, chunks[0], &mut self.branches_list_state);
        }

        // Draw branch
        {
            let title = if let Some(BranchLine::Parsed { branch, .. }) = self.branch.as_ref() {
                format!(" Branch {} ", branch)
            } else {
                " Branch ".to_owned()
            };

            let branch_block = Block::bordered()
                .title(title)
                .border_type(BorderType::Rounded)
                .padding(Padding::horizontal(1));
            let branch_content: Vec<Line> = match self.branch_output.as_ref() {
                Some(Ok(branch_output)) => branch_output.into_text()?.lines,
                Some(Err(err)) => err.into_text("Error getting branch")?.lines,
                None => vec![],
            };
            let branch = self
                .branch_panel
                .render(branch_content, branch_block.inner(chunks[1]))
                .block(branch_block);
            f.render_widget(branch, chunks[1]);
        }

        // Draw popup
        if self.popup.is_opened() {
            let popup = ConfirmDialog::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Green))
                .selected_button_style(
                    Style::default()
                        .bg(self.config.highlight_color())
                        .underlined(),
                );
            f.render_stateful_widget(popup, area, &mut self.popup);
        }

        // Draw create textarea
        {
            if let Some(create) = self.create.as_mut() {
                let block = Block::bordered()
                    .title(Span::styled(" Create branch ", Style::new().bold().cyan()))
                    .title_alignment(Alignment::Center)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Green));
                let error_lines = create
                    .error
                    .as_ref()
                    .map(|error| error.to_string().into_text().unwrap().lines);
                let error_height = if let Some(error_lines) = error_lines.as_ref() {
                    error_lines.len() + 1
                } else {
                    0
                };
                let area = centered_rect_line_height(area, 30, 5 + error_height as u16);
                f.render_widget(Clear, area);
                f.render_widget(&block, area);

                let popup_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Fill(1),
                        Constraint::Length(error_height as u16),
                        Constraint::Length(2),
                    ])
                    .split(block.inner(area));

                f.render_widget(create.textarea.widget(), popup_chunks[0]);

                if let Some(error_lines) = error_lines {
                    let help = Paragraph::new(error_lines).block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    );

                    f.render_widget(help, popup_chunks[1]);
                }

                let help = Paragraph::new(vec!["Ctrl+s: save | Escape: cancel".into()])
                    .fg(Color::DarkGray)
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    );

                f.render_widget(help, popup_chunks[2]);
            }
        }

        // Draw rename textarea
        {
            if let Some(rename) = self.rename.as_mut() {
                let block = Block::bordered()
                    .title(Span::styled(" Rename branch ", Style::new().bold().cyan()))
                    .title_alignment(Alignment::Center)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Green));
                let error_lines = rename
                    .error
                    .as_ref()
                    .map(|error| error.to_string().into_text().unwrap().lines);
                let error_height = if let Some(error_lines) = error_lines.as_ref() {
                    error_lines.len() + 1
                } else {
                    0
                };
                let area = centered_rect_line_height(area, 30, 5 + error_height as u16);
                f.render_widget(Clear, area);
                f.render_widget(&block, area);

                let popup_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Fill(1),
                        Constraint::Length(error_height as u16),
                        Constraint::Length(2),
                    ])
                    .split(block.inner(area));

                f.render_widget(rename.textarea.widget(), popup_chunks[0]);

                if let Some(error_lines) = error_lines {
                    let help = Paragraph::new(error_lines).block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    );

                    f.render_widget(help, popup_chunks[1]);
                }

                let help = Paragraph::new(vec!["Ctrl+s: save | Escape: cancel".into()])
                    .fg(Color::DarkGray)
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    );

                f.render_widget(help, popup_chunks[2]);
            }
        }

        // Draw describe textarea
        {
            if let Some(describe_textarea) = self.describe_textarea.as_mut() {
                let block = Block::bordered()
                    .title(Span::styled(" Describe ", Style::new().bold().cyan()))
                    .title_alignment(Alignment::Center)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Green));
                let area = centered_rect(area, 50, 50);
                f.render_widget(Clear, area);
                f.render_widget(&block, area);

                let popup_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Fill(1), Constraint::Length(2)])
                    .split(block.inner(area));

                f.render_widget(describe_textarea.widget(), popup_chunks[0]);

                let help = Paragraph::new(vec!["Ctrl+s: save | Escape: cancel".into()])
                    .fg(Color::DarkGray)
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    );

                f.render_widget(help, popup_chunks[1]);
            }
        }

        Ok(())
    }

    fn input(&mut self, commander: &mut Commander, event: Event) -> Result<ComponentInputResult> {
        if let Some(create) = self.create.as_mut() {
            if let Event::Key(key) = event {
                match key.code {
                    _ if (key.code == KeyCode::Char('s')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                        || (key.code == KeyCode::Enter) =>
                    {
                        let name = create.textarea.lines().join("\n");

                        if name.trim().is_empty() {
                            create.error = Some(anyhow::Error::msg("Branch name cannot be empty"));
                            return Ok(ComponentInputResult::Handled);
                        }

                        if let Err(err) = commander.create_branch(&name) {
                            create.error = Some(anyhow::Error::new(err));
                            return Ok(ComponentInputResult::Handled);
                        }

                        self.create = None;
                        self.refresh_branches(commander);

                        // Select new branch
                        if let Some(branch) =
                            self.branches_output
                                .as_ref()
                                .ok()
                                .and_then(|branches_output| {
                                    branches_output.iter().find(|branch| match branch {
                                        BranchLine::Unparsable(_) => false,
                                        BranchLine::Parsed { branch, .. } => branch.name == name,
                                    })
                                })
                        {
                            self.branch = Some(branch.clone());
                        }

                        self.refresh_branch(commander);

                        return Ok(ComponentInputResult::Handled);
                    }
                    KeyCode::Esc => {
                        self.create = None;
                        return Ok(ComponentInputResult::Handled);
                    }
                    _ => {}
                }
            }
            create.textarea.input(event);
            return Ok(ComponentInputResult::Handled);
        }

        if let Some(rename) = self.rename.as_mut() {
            if let Event::Key(key) = event {
                match key.code {
                    _ if (key.code == KeyCode::Char('s')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                        || (key.code == KeyCode::Enter) =>
                    {
                        let new = rename.textarea.lines().join("\n");

                        if new.trim().is_empty() {
                            rename.error = Some(anyhow::Error::msg("Branch name cannot be empty"));
                            return Ok(ComponentInputResult::Handled);
                        }

                        let old = rename.name.clone();

                        if let Err(err) = commander.rename_branch(&old, &new) {
                            rename.error = Some(anyhow::Error::new(err));
                            return Ok(ComponentInputResult::Handled);
                        }
                        self.rename = None;
                        self.refresh_branches(commander);

                        // Select new branch
                        if let Some(branch) =
                            self.branches_output
                                .as_ref()
                                .ok()
                                .and_then(|branches_output| {
                                    branches_output.iter().find(|branch| match branch {
                                        BranchLine::Unparsable(_) => false,
                                        BranchLine::Parsed { branch, .. } => branch.name == new,
                                    })
                                })
                        {
                            self.branch = Some(branch.clone());
                        }

                        self.refresh_branch(commander);

                        return Ok(ComponentInputResult::Handled);
                    }
                    KeyCode::Esc => {
                        self.rename = None;
                        return Ok(ComponentInputResult::Handled);
                    }
                    _ => {}
                }
            }
            rename.textarea.input(event);
            return Ok(ComponentInputResult::Handled);
        }

        if let (Some(describe_textarea), Some(describe_after_new_change)) = (
            self.describe_textarea.as_mut(),
            self.describe_after_new_change.as_ref(),
        ) {
            if let Event::Key(key) = event {
                match key.code {
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // TODO: Handle error
                        commander.run_describe(
                            describe_after_new_change.as_str(),
                            &describe_textarea.lines().join("\n"),
                        )?;
                        self.describe_textarea = None;
                        self.describe_after_new_change = None;
                        return Ok(ComponentInputResult::HandledAction(
                            ComponentAction::ViewLog(commander.get_current_head()?),
                        ));
                    }
                    KeyCode::Esc => {
                        self.describe_textarea = None;
                        self.describe_after_new_change = None;
                        return Ok(ComponentInputResult::Handled);
                    }
                    _ => {}
                }
            }
            describe_textarea.input(event);
            return Ok(ComponentInputResult::Handled);
        }

        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return Ok(ComponentInputResult::Handled);
            }
            if self.popup.is_opened() {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    self.popup = ConfirmDialogState::default();
                } else {
                    self.popup.handle(key);
                }

                return Ok(ComponentInputResult::Handled);
            }

            if self.branch_panel.input(key) {
                return Ok(ComponentInputResult::Handled);
            }

            match key.code {
                KeyCode::Char('j') | KeyCode::Down => self.scroll_branches(commander, 1),
                KeyCode::Char('k') | KeyCode::Up => self.scroll_branches(commander, -1),
                KeyCode::Char('J') => {
                    self.scroll_branches(commander, self.branches_height as isize / 2);
                }
                KeyCode::Char('K') => {
                    self.scroll_branches(
                        commander,
                        (self.branches_height as isize / 2).saturating_neg(),
                    );
                }
                KeyCode::Char('w') => {
                    self.diff_format = match self.diff_format {
                        DiffFormat::ColorWords => DiffFormat::Git,
                        _ => DiffFormat::ColorWords,
                    };
                    self.refresh_branch(commander);
                }
                KeyCode::Char('R') | KeyCode::F(5) => {
                    self.refresh_branches(commander);
                    self.refresh_branch(commander);
                }
                KeyCode::Char('a') => {
                    self.show_all = !self.show_all;
                    self.refresh_branches(commander);
                }
                KeyCode::Char('c') => {
                    let textarea = TextArea::default();
                    self.create = Some(CreateBranch {
                        textarea,
                        error: None,
                    });
                    return Ok(ComponentInputResult::Handled);
                }
                KeyCode::Char('r') => {
                    if let Some(BranchLine::Parsed { branch, .. }) = self.branch.as_ref() {
                        let mut textarea = TextArea::new(vec![branch.name.clone()]);
                        textarea.move_cursor(CursorMove::End);
                        self.rename = Some(RenameBranch {
                            textarea,
                            name: branch.name.clone(),
                            error: None,
                        });
                        return Ok(ComponentInputResult::Handled);
                    }
                }
                KeyCode::Char('d') => {
                    if let Some(BranchLine::Parsed { branch, .. }) = self.branch.as_ref() {
                        self.delete = Some(DeleteBranch {
                            name: branch.name.clone(),
                        });
                        self.popup = ConfirmDialogState::new(
                            DELETE_BRANCH_POPUP_ID,
                            Span::styled(" Delete ", Style::new().bold().cyan()),
                            Text::from(vec![Line::from(format!(
                                "Are you sure you want to delete the {} branch?",
                                branch.name
                            ))]),
                        )
                        .with_yes_button(ButtonLabel::YES.clone())
                        .with_no_button(ButtonLabel::NO.clone())
                        .with_listener(Some(self.popup_tx.clone()))
                        .open();
                    }
                }
                KeyCode::Char('f') => {
                    if let Some(BranchLine::Parsed { branch, .. }) = self.branch.as_ref() {
                        self.forget = Some(ForgetBranch {
                            name: branch.name.clone(),
                        });
                        self.popup = ConfirmDialogState::new(
                            FORGET_BRANCH_POPUP_ID,
                            Span::styled(" Forget ", Style::new().bold().cyan()),
                            Text::from(vec![Line::from(format!(
                                "Are you sure you want to forget the {} branch?",
                                branch.name
                            ))]),
                        )
                        .with_yes_button(ButtonLabel::YES.clone())
                        .with_no_button(ButtonLabel::NO.clone())
                        .with_listener(Some(self.popup_tx.clone()))
                        .open();
                    }
                }
                // TODO: Ask for confirmation?
                KeyCode::Char('t') => {
                    if let Some(BranchLine::Parsed { branch, .. }) = self.branch.as_ref() {
                        if branch.remote.is_some() && branch.present {
                            commander.track_branch(branch)?;
                            self.refresh_branches(commander);
                            self.refresh_branch(commander);
                        }
                    }
                }
                KeyCode::Char('T') => {
                    if let Some(BranchLine::Parsed { branch, .. }) = self.branch.as_ref() {
                        if branch.remote.is_some() && branch.present {
                            commander.untrack_branch(branch)?;
                            self.refresh_branches(commander);
                            self.refresh_branch(commander);
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    if let Some(BranchLine::Parsed { branch, .. }) = self.branch.as_ref() {
                        if branch.present {
                            self.popup = ConfirmDialogState::new(
                                NEW_POPUP_ID,
                                Span::styled(" New ", Style::new().bold().cyan()),
                                Text::from(vec![
                                    Line::from("Are you sure you want to create a new change?"),
                                    Line::from(format!("Branch: {}", branch)),
                                ]),
                            )
                            .with_yes_button(ButtonLabel::YES.clone())
                            .with_no_button(ButtonLabel::NO.clone())
                            .with_listener(Some(self.popup_tx.clone()))
                            .open();

                            self.describe_after_new = key.code == KeyCode::Char('N');
                        }
                    }
                }
                KeyCode::Char('e') => {
                    if let Some(BranchLine::Parsed { branch, .. }) = self.branch.as_ref() {
                        if branch.present {
                            if commander.check_revision_immutable(&branch.to_string())? {
                                return Ok(ComponentInputResult::HandledAction(
                                    ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                                        title: "Edit".into(),
                                        messages: vec![
                                            "The change cannot be edited because it is immutable."
                                                .into(),
                                        ]
                                        .into(),
                                    }))),
                                ));
                            } else {
                                self.popup = ConfirmDialogState::new(
                                    EDIT_POPUP_ID,
                                    Span::styled(" Edit ", Style::new().bold().cyan()),
                                    Text::from(vec![
                                        Line::from(
                                            "Are you sure you want to edit an existing change?",
                                        ),
                                        Line::from(format!("Branch: {}", branch)),
                                    ]),
                                )
                                .with_yes_button(ButtonLabel::YES.clone())
                                .with_no_button(ButtonLabel::NO.clone())
                                .with_listener(Some(self.popup_tx.clone()))
                                .open();
                            }
                        }
                    }
                }
                KeyCode::Enter => {
                    if let Some(BranchLine::Parsed { branch, .. }) = self.branch.as_ref() {
                        if branch.present {
                            return Ok(ComponentInputResult::HandledAction(
                                ComponentAction::ViewLog(commander.get_branch_head(branch)?),
                            ));
                        }
                    }
                }
                KeyCode::Char('h') | KeyCode::Char('?') => {
                    return Ok(ComponentInputResult::HandledAction(
                        ComponentAction::SetPopup(Some(Box::new(HelpPopup::new(
                            vec![
                                ("j/k".to_owned(), "scroll down/up".to_owned()),
                                ("J/K".to_owned(), "scroll down by ½ page".to_owned()),
                                ("a".to_owned(), "show all remotes".to_owned()),
                                ("c".to_owned(), "create branch".to_owned()),
                                ("r".to_owned(), "rename branch".to_owned()),
                                ("d/f".to_owned(), "delete/forget branch".to_owned()),
                                ("t/T".to_owned(), "track/untrack branch".to_owned()),
                                ("Enter".to_owned(), "view in log".to_owned()),
                                ("n".to_owned(), "new from branch".to_owned()),
                                ("N".to_owned(), "new and describe".to_owned()),
                                ("e".to_owned(), "edit branch".to_owned()),
                            ],
                            vec![
                                ("Ctrl+e/Ctrl+y".to_owned(), "scroll down/up".to_owned()),
                                (
                                    "Ctrl+d/Ctrl+u".to_owned(),
                                    "scroll down/up by ½ page".to_owned(),
                                ),
                                (
                                    "Ctrl+f/Ctrl+b".to_owned(),
                                    "scroll down/up by page".to_owned(),
                                ),
                                ("w".to_owned(), "toggle diff format".to_owned()),
                                ("W".to_owned(), "toggle wrapping".to_owned()),
                            ],
                        )))),
                    ))
                }
                _ => return Ok(ComponentInputResult::NotHandled),
            };
        }

        Ok(ComponentInputResult::Handled)
    }
}
