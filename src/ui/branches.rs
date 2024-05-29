#![allow(clippy::borrow_interior_mutable_const)]
use crate::{
    commander::{branches::BranchLine, CommandError, Commander},
    env::{Config, DiffFormat},
    ui::{
        details_panel::DetailsPanel, message_popup::MessagePopup, utils::centered_rect_line_height,
        Component, ComponentAction,
    },
    ComponentInputResult,
};
use ansi_to_tui::IntoText;
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::{prelude::*, widgets::*};
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

/// Branches tab. Shows branches in left panel and selected branch current change in right panel.
pub struct Branches<'a> {
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

    popup: ConfirmDialogState,
    popup_tx: std::sync::mpsc::Sender<Listener>,
    popup_rx: std::sync::mpsc::Receiver<Listener>,

    message_popup: Option<MessagePopup<'a>>,

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

impl Branches<'_> {
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
            BranchLine::Parsed { branch, .. } => {
                Some(commander.get_branch_show(branch, &diff_format))
            }
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

            popup: ConfirmDialogState::default(),
            popup_tx,
            popup_rx,

            message_popup: None,

            diff_format,

            config: commander.env.config.clone(),
        })
    }

    // pub fn set_head(&mut self, commander: &mut Commander, head: &Head) -> Result<()> {
    //     self.head = head.clone();
    //     self.is_current_head = self.head == commander.get_current_head()?;
    //
    //     self.refresh_files(commander)?;
    //     self.file = self
    //         .files_output
    //         .first()
    //         .and_then(|change| change.path.clone());
    //     self.refresh_diff(commander)?;
    //
    //     Ok(())
    // }

    pub fn get_current_branch_index(&self) -> Option<usize> {
        get_current_branch_index(self.branch.as_ref(), &self.branches_output)
    }

    pub fn refresh_branches(&mut self, commander: &mut Commander) {
        self.branches_output = commander.get_branches(self.show_all);
    }

    pub fn refresh_branch(&mut self, commander: &mut Commander) {
        self.branch_output = self.branch.as_ref().and_then(|branch| match branch {
            BranchLine::Parsed { branch, .. } => {
                Some(commander.get_branch_show(branch, &self.diff_format))
            }
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

impl Component for Branches<'_> {
    fn switch(&mut self, commander: &mut Commander) -> Result<()> {
        self.refresh_branches(commander);
        self.refresh_branch(commander);
        Ok(())
    }

    fn update(&mut self, commander: &mut Commander) -> Result<Option<ComponentAction>> {
        // Check for popup action
        if let Ok(res) = self.popup_rx.try_recv()
            && res.1.unwrap_or(false)
        {
            match res.0 {
                DELETE_BRANCH_POPUP_ID => {
                    if let Some(delete) = self.delete.as_ref() {
                        match commander.delete_branch(&delete.name) {
                            Ok(_) => {
                                self.refresh_branches(commander);
                                let branches = Vec::new();
                                let branches = self.branches_output.as_ref().unwrap_or(&branches);
                                self.branch = branches.first().map(|branch| branch.to_owned());
                                self.refresh_branch(commander);
                            }
                            Err(err) => {
                                self.message_popup = Some(MessagePopup {
                                    title: "Delete error".into(),
                                    messages: err.to_string().into_text()?,
                                });
                            }
                        }
                        return Ok(None);
                    }
                }
                FORGET_BRANCH_POPUP_ID => {
                    if let Some(forget) = self.forget.as_ref() {
                        match commander.delete_branch(&forget.name) {
                            Ok(_) => {
                                self.refresh_branches(commander);
                                let branches = Vec::new();
                                let branches = self.branches_output.as_ref().unwrap_or(&branches);
                                self.branch = branches.first().map(|branch| branch.to_owned());
                                self.refresh_branch(commander);
                            }
                            Err(err) => {
                                self.message_popup = Some(MessagePopup {
                                    title: "Forget error".into(),
                                    messages: err.to_string().into_text()?,
                                });
                            }
                        }
                        return Ok(None);
                    }
                }
                _ => {}
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
            let panel_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Fill(1), Constraint::Length(2)])
                .split(chunks[0]);

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
                    if let CommandError::Status(output, _) = err
                        && output.contains("unexpected argument '-T' found")
                    {
                        vec![
                            Line::raw(""),
                            Line::raw("Please update jj to >0.18 for -T support to branches")
                                .bold()
                                .fg(Color::Red),
                        ]
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

            let files = List::new(lines)
                .block(
                    Block::bordered()
                        .title(" Branches ")
                        .border_type(BorderType::Rounded),
                )
                .scroll_padding(3);
            *self.branches_list_state.selected_mut() = current_branch_index;
            f.render_stateful_widget(files, panel_chunks[0], &mut self.branches_list_state);
            self.branches_height = panel_chunks[0].height - 2;

            let help = Paragraph::new(vec![
                "j/k: scroll down/up | J/K: scroll down by ½ page | a: show all remotes".into(),
                "c: create branch | r: rename branch | d/f: delete/forget branch | t/T: track/untrack branch".into(),
            ])
            .fg(Color::DarkGray);
            f.render_widget(help, panel_chunks[1]);
        }

        // Draw branch
        {
            let panel_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Fill(1), Constraint::Length(2)])
                .split(chunks[1]);

            let title = if let Some(BranchLine::Parsed { branch, .. }) = self.branch.as_ref() {
                format!(" Branch {} ", branch)
            } else {
                " Branch ".to_owned()
            };

            let branch_block = Block::bordered()
                .title(title)
                .border_type(BorderType::Rounded);
            let branch_content: Vec<Line> = match self.branch_output.as_ref() {
                Some(Ok(branch_output)) => branch_output.into_text()?.lines,
                Some(Err(err)) => err.into_text("Error getting branch")?.lines,
                None => vec![],
            };
            let branch = self
                .branch_panel
                .render(branch_content, branch_block.inner(chunks[1]))
                .block(branch_block);
            f.render_widget(branch, panel_chunks[0]);

            let help = Paragraph::new(vec![
                "Ctrl+e/Ctrl+y: scroll down/up | Ctrl+d/Ctrl+u: scroll down/up by ½ page".into(),
                "Ctrl+f/Ctrl+b: scroll down/up by page | p: toggle diff format | w: toggle wrapping".into(),
            ]).fg(Color::DarkGray);
            f.render_widget(help, panel_chunks[1]);
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

        // Draw messge popup
        if let Some(message_popup) = &self.message_popup {
            f.render_widget(message_popup.render(), area);
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

                        return Ok(ComponentInputResult::HandledAction(
                            ComponentAction::SetTextAreaActive(false),
                        ));
                    }
                    KeyCode::Esc => {
                        self.create = None;
                        return Ok(ComponentInputResult::HandledAction(
                            ComponentAction::SetTextAreaActive(false),
                        ));
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

                        return Ok(ComponentInputResult::HandledAction(
                            ComponentAction::SetTextAreaActive(false),
                        ));
                    }
                    KeyCode::Esc => {
                        self.rename = None;
                        return Ok(ComponentInputResult::HandledAction(
                            ComponentAction::SetTextAreaActive(false),
                        ));
                    }
                    _ => {}
                }
            }
            rename.textarea.input(event);
            return Ok(ComponentInputResult::Handled);
        }

        if let Event::Key(key) = event {
            if self.popup.is_opened() {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    self.popup = ConfirmDialogState::default();
                } else {
                    self.popup.handle(key);
                }

                return Ok(ComponentInputResult::Handled);
            }
            if let Some(message_popup) = &self.message_popup {
                if key.code == KeyCode::Char('q')
                    || key.code == KeyCode::Esc
                    || message_popup.input(key)
                {
                    self.message_popup = None;
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
                KeyCode::Char('p') => {
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
                    return Ok(ComponentInputResult::HandledAction(
                        ComponentAction::SetTextAreaActive(true),
                    ));
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
                        return Ok(ComponentInputResult::HandledAction(
                            ComponentAction::SetTextAreaActive(true),
                        ));
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
                KeyCode::Char('t') => {
                    if let Some(BranchLine::Parsed { branch, .. }) = self.branch.as_ref()
                        && branch.remote.is_some()
                    {
                        commander.track_branch(branch)?;
                        self.refresh_branches(commander);
                        self.refresh_branch(commander);
                    }
                }
                KeyCode::Char('T') => {
                    if let Some(BranchLine::Parsed { branch, .. }) = self.branch.as_ref()
                        && branch.remote.is_some()
                    {
                        commander.untrack_branch(branch)?;
                        self.refresh_branches(commander);
                        self.refresh_branch(commander);
                    }
                }
                _ => return Ok(ComponentInputResult::NotHandled),
            };
        }

        Ok(ComponentInputResult::Handled)
    }
}
