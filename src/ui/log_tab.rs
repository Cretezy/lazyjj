#![allow(clippy::borrow_interior_mutable_const)]

use ansi_to_tui::IntoText;
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{prelude::*, widgets::*};
use tracing::instrument;
use tui_confirm_dialog::{ButtonLabel, ConfirmDialog, ConfirmDialogState, Listener};
use tui_textarea::{CursorMove, TextArea};

use crate::{
    commander::{
        log::{Head, LogOutput},
        CommandError, Commander,
    },
    env::{Config, DiffFormat},
    ui::{
        bookmark_set_popup::BookmarkSetPopup,
        details_panel::DetailsPanel,
        help_popup::HelpPopup,
        message_popup::MessagePopup,
        utils::{centered_rect, centered_rect_line_height, tabs_to_spaces},
        Component, ComponentAction,
    },
    ComponentInputResult,
};

const NEW_POPUP_ID: u16 = 1;
const EDIT_POPUP_ID: u16 = 2;
const ABANDON_POPUP_ID: u16 = 3;
const SQUASH_POPUP_ID: u16 = 4;

/// Log tab. Shows `jj log` in main panel and shows selected change details of in details panel.
pub struct LogTab<'a> {
    log_output: Result<LogOutput, CommandError>,
    log_output_text: Text<'a>,
    log_list_state: ListState,
    log_height: u16,

    log_revset: Option<String>,
    log_revset_textarea: Option<TextArea<'a>>,

    head_panel: DetailsPanel,
    head_output: Result<String, CommandError>,
    head: Head,

    diff_format: DiffFormat,

    popup: ConfirmDialogState,
    popup_tx: std::sync::mpsc::Sender<Listener>,
    popup_rx: std::sync::mpsc::Receiver<Listener>,

    bookmark_set_popup_tx: std::sync::mpsc::Sender<bool>,
    bookmark_set_popup_rx: std::sync::mpsc::Receiver<bool>,

    describe_textarea: Option<TextArea<'a>>,
    describe_after_new: bool,

    config: Config,
}

fn get_head_index(head: &Head, log_output: &Result<LogOutput, CommandError>) -> Option<usize> {
    match log_output {
        Ok(log_output) => log_output
            .heads
            .iter()
            .position(|heads| heads == head)
            .or_else(|| {
                log_output
                    .heads
                    .iter()
                    .position(|commit| commit.change_id == head.change_id)
            }),
        Err(_) => None,
    }
}

impl LogTab<'_> {
    #[instrument(level = "trace", skip(commander))]
    pub fn new(commander: &mut Commander) -> Result<Self> {
        let diff_format = commander.env.config.diff_format();

        let log_revset = commander.env.default_revset.clone();
        let log_output = commander.get_log(&log_revset);
        let head = commander.get_current_head()?;

        let log_list_state = ListState::default().with_selected(get_head_index(&head, &log_output));

        let head_output = commander
            .get_commit_show(&head.commit_id, &diff_format)
            .map(|text| tabs_to_spaces(&text));

        let (popup_tx, popup_rx) = std::sync::mpsc::channel();
        let (bookmark_set_popup_tx, bookmark_set_popup_rx) = std::sync::mpsc::channel();

        Ok(Self {
            log_output_text: match log_output.as_ref() {
                Ok(log_output) => log_output
                    .graph
                    .into_text()
                    .unwrap_or(Text::from("Could not turn text into TUI text (coloring)")),
                Err(_) => Text::default(),
            },
            log_output,
            log_list_state,
            log_height: 0,

            log_revset,
            log_revset_textarea: None,

            head,
            head_panel: DetailsPanel::new(),
            head_output,

            diff_format,

            popup: ConfirmDialogState::default(),
            popup_tx,
            popup_rx,

            bookmark_set_popup_tx,
            bookmark_set_popup_rx,

            describe_textarea: None,
            describe_after_new: false,

            config: commander.env.config.clone(),
        })
    }

    fn get_current_head_index(&self) -> Option<usize> {
        get_head_index(&self.head, &self.log_output)
    }

    fn refresh_log_output(&mut self, commander: &mut Commander) {
        self.log_output = commander.get_log(&self.log_revset);
        self.log_output_text = match self.log_output.as_ref() {
            Ok(log_output) => log_output
                .graph
                .into_text()
                .unwrap_or(Text::from("Could not turn text into TUI text (coloring)")),
            Err(_) => Text::default(),
        };
    }

    fn refresh_head_output(&mut self, commander: &mut Commander) {
        self.head_output = commander
            .get_commit_show(&self.head.commit_id, &self.diff_format)
            .map(|text| tabs_to_spaces(&text));
        self.head_panel.scroll = 0;
    }

    fn scroll_log(&mut self, commander: &mut Commander, scroll: isize) {
        let log_output = match self.log_output.as_ref() {
            Ok(log_output) => log_output,
            Err(_) => return,
        };

        let heads: &Vec<Head> = log_output.heads.as_ref();

        let current_head_index = self.get_current_head_index();
        let next_head = match current_head_index {
            Some(current_head_index) => heads.get(
                current_head_index
                    .saturating_add_signed(scroll)
                    .min(heads.len() - 1),
            ),
            None => heads.first(),
        };
        if let Some(next_head) = next_head {
            self.set_head(commander, next_head.clone());
        }
    }

    pub fn set_head(&mut self, commander: &mut Commander, head: Head) {
        head.clone_into(&mut self.head);
        self.refresh_head_output(commander);
    }
}

#[allow(clippy::invisible_characters)]
impl Component for LogTab<'_> {
    fn switch(&mut self, commander: &mut Commander) -> Result<()> {
        self.refresh_log_output(commander);
        self.refresh_head_output(commander);
        Ok(())
    }

    fn update(&mut self, commander: &mut Commander) -> Result<Option<ComponentAction>> {
        let latest_head = commander.get_head_latest(&self.head)?;
        if latest_head != self.head {
            self.head = latest_head;
            self.refresh_log_output(commander);
            self.refresh_head_output(commander);
        }

        // Check for popup action
        if let Ok(res) = self.popup_rx.try_recv() {
            if res.1.unwrap_or(false) {
                match res.0 {
                    NEW_POPUP_ID => {
                        commander.run_new(self.head.commit_id.as_str())?;
                        self.head = commander.get_current_head()?;
                        self.refresh_log_output(commander);
                        self.refresh_head_output(commander);
                        if self.describe_after_new {
                            self.describe_after_new = false;
                            let textarea = TextArea::default();
                            self.describe_textarea = Some(textarea);
                        }
                        return Ok(Some(ComponentAction::ChangeHead(self.head.clone())));
                    }
                    EDIT_POPUP_ID => {
                        commander.run_edit(self.head.commit_id.as_str())?;
                        self.refresh_log_output(commander);
                        self.refresh_head_output(commander);
                        return Ok(Some(ComponentAction::ChangeHead(self.head.clone())));
                    }
                    ABANDON_POPUP_ID => {
                        if self.head == commander.get_current_head()? {
                            commander.run_abandon(&self.head.commit_id)?;
                            self.refresh_log_output(commander);
                            self.head = commander.get_current_head()?;
                            self.refresh_head_output(commander);
                            return Ok(Some(ComponentAction::ChangeHead(self.head.clone())));
                        } else {
                            let head_parent = commander.get_commit_parent(&self.head.commit_id)?;
                            commander.run_abandon(&self.head.commit_id)?;
                            self.refresh_log_output(commander);
                            self.head = head_parent;
                            self.refresh_head_output(commander);
                        }
                    }
                    SQUASH_POPUP_ID => {
                        commander.run_squash(self.head.commit_id.as_str())?;
                        self.head = commander.get_current_head()?;
                        self.refresh_log_output(commander);
                        self.refresh_head_output(commander);
                        return Ok(Some(ComponentAction::ChangeHead(self.head.clone())));
                    }
                    _ => {}
                }
            }
        }

        if let Ok(true) = self.bookmark_set_popup_rx.try_recv() {
            self.refresh_log_output(commander);
            self.refresh_head_output(commander)
        }

        Ok(None)
    }

    fn draw(
        &mut self,
        f: &mut ratatui::prelude::Frame<'_>,
        area: ratatui::prelude::Rect,
    ) -> Result<()> {
        let chunks = Layout::default()
            .direction(self.config.layout().into())
            .constraints([
                Constraint::Percentage(self.config.layout_percent()),
                Constraint::Percentage(100 - self.config.layout_percent()),
            ])
            .split(area);

        // Draw log
        {
            let mut scroll_offset = 0;
            let log_lines = match self.log_output.as_ref() {
                Ok(log_output) => {
                    let log_lines: Vec<Line> = self
                        .log_output_text
                        .iter()
                        .enumerate()
                        .map(|(i, line)| {
                            let mut line = line.to_owned();

                            // Add padding at start
                            line.spans.insert(0, Span::from(" "));

                            let line_head = log_output.graph_heads.get(i).unwrap_or(&None);

                            match line_head {
                                Some(line_change) => {
                                    if line_change == &self.head {
                                        line = line.bg(self.config.highlight_color());

                                        line.spans = line
                                            .spans
                                            .iter_mut()
                                            .map(|span| {
                                                span.to_owned().bg(self.config.highlight_color())
                                            })
                                            .collect();
                                    }
                                }
                                _ => scroll_offset += 1,
                            };

                            line
                        })
                        .collect();

                    self.log_list_state
                        .select(log_lines.iter().enumerate().position(|(i, _)| {
                            log_output
                                .graph_heads
                                .get(i)
                                .unwrap_or(&None)
                                .clone()
                                .map_or(false, |line_change| line_change == self.head)
                        }));

                    log_lines
                }
                Err(err) => err.into_text("Error getting log")?.lines,
            };

            let title = match &self.log_revset {
                Some(log_revset) => &format!(" Log for: {} ", log_revset),
                None => " Log ",
            };

            let log_block = Block::bordered()
                .title(title)
                .border_type(BorderType::Rounded);
            self.log_height = log_block.inner(chunks[0]).height;
            let log = List::new(log_lines).block(log_block).scroll_padding(7);
            f.render_stateful_widget(log, chunks[0], &mut self.log_list_state);
        }

        // Draw change details
        {
            let head_content = match self.head_output.as_ref() {
                Ok(head_output) => head_output.into_text()?.lines,
                Err(err) => err.into_text("Error getting head details")?.lines,
            };
            let head_block = Block::bordered()
                .title(format!(" Details for {} ", self.head.change_id))
                .border_type(BorderType::Rounded)
                .padding(Padding::horizontal(1));
            let head = self
                .head_panel
                .render(head_content, head_block.inner(chunks[1]))
                .block(head_block);

            f.render_widget(head, chunks[1]);
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

                f.render_widget(&*describe_textarea, popup_chunks[0]);

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

        // Draw revset textarea
        {
            if let Some(log_revset_textarea) = self.log_revset_textarea.as_mut() {
                let block = Block::bordered()
                    .title(Span::styled(" Revset ", Style::new().bold().cyan()))
                    .title_alignment(Alignment::Center)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Green));
                let area = centered_rect_line_height(area, 30, 7);
                f.render_widget(Clear, area);
                f.render_widget(&block, area);

                let popup_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Fill(1), Constraint::Length(2)])
                    .split(block.inner(area));

                f.render_widget(&*log_revset_textarea, popup_chunks[0]);

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

    #[allow(clippy::collapsible_if)]
    fn input(&mut self, commander: &mut Commander, event: Event) -> Result<ComponentInputResult> {
        if let Some(describe_textarea) = self.describe_textarea.as_mut() {
            if let Event::Key(key) = event {
                match key.code {
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // TODO: Handle error
                        commander.run_describe(
                            self.head.commit_id.as_str(),
                            &describe_textarea.lines().join("\n"),
                        )?;
                        self.refresh_log_output(commander);
                        self.refresh_head_output(commander);
                        self.describe_textarea = None;
                        return Ok(ComponentInputResult::Handled);
                    }
                    KeyCode::Esc => {
                        self.describe_textarea = None;
                        return Ok(ComponentInputResult::Handled);
                    }
                    _ => {}
                }
            }
            describe_textarea.input(event);
            return Ok(ComponentInputResult::Handled);
        }

        if let Some(log_revset_textarea) = self.log_revset_textarea.as_mut() {
            if let Event::Key(key) = event {
                match key.code {
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let log_revset = log_revset_textarea.lines().join("\n");
                        self.log_revset = if log_revset.trim().is_empty() {
                            None
                        } else {
                            Some(log_revset)
                        };
                        self.refresh_log_output(commander);
                        self.log_revset_textarea = None;
                        return Ok(ComponentInputResult::Handled);
                    }
                    KeyCode::Esc => {
                        self.log_revset_textarea = None;
                        return Ok(ComponentInputResult::Handled);
                    }
                    _ => {}
                }
            }
            log_revset_textarea.input(event);
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

            if self.head_panel.input(key) {
                return Ok(ComponentInputResult::Handled);
            }

            match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.scroll_log(commander, 1);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.scroll_log(commander, -1);
                }
                KeyCode::Char('J') => {
                    self.scroll_log(commander, self.log_height as isize / 2 / 2);
                }
                KeyCode::Char('K') => {
                    self.scroll_log(
                        commander,
                        (self.log_height as isize / 2 / 2).saturating_neg(),
                    );
                }
                KeyCode::Char('@') => {
                    self.head = commander.get_current_head()?;
                    self.refresh_head_output(commander);
                }
                KeyCode::Char('w') => {
                    self.diff_format = match self.diff_format {
                        DiffFormat::ColorWords => DiffFormat::Git,
                        _ => DiffFormat::ColorWords,
                    };
                    self.refresh_head_output(commander);
                }
                KeyCode::Char('R') | KeyCode::F(5) => {
                    self.refresh_log_output(commander);
                    self.refresh_head_output(commander);
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.popup = ConfirmDialogState::new(
                        NEW_POPUP_ID,
                        Span::styled(" New ", Style::new().bold().cyan()),
                        Text::from(vec![
                            Line::from("Are you sure you want to create a new change?"),
                            Line::from(format!("New parent: {}", self.head.change_id.as_str())),
                        ]).fg(Color::default()),
                    )
                    .with_yes_button(ButtonLabel::YES.clone())
                    .with_no_button(ButtonLabel::NO.clone())
                    .with_listener(Some(self.popup_tx.clone()))
                    .open();

                    self.describe_after_new = key.code == KeyCode::Char('N');
                }
                KeyCode::Char('s') => {
                    self.popup = ConfirmDialogState::new(
                        SQUASH_POPUP_ID,
                        Span::styled(" Squash ", Style::new().bold().cyan()),
                        Text::from(vec![
                            Line::from("Are you sure you want to squash @ into this change?"),
                            Line::from(format!("Squash into {}", self.head.change_id.as_str())),
                        ]).fg(Color::default()),
                    )
                    .with_yes_button(ButtonLabel::YES.clone())
                    .with_no_button(ButtonLabel::NO.clone())
                    .with_listener(Some(self.popup_tx.clone()))
                    .open();
                }
                KeyCode::Char('e') => {
                    if self.head.immutable {
                        return Ok(ComponentInputResult::HandledAction(
                            ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                                title: "Edit".into(),
                                messages: vec![
                                    "The change cannot be edited because it is immutable.".into(),
                                ]
                                .into(),
                                text_align: None,
                            }))),
                        ));
                    } else {
                        self.popup = ConfirmDialogState::new(
                            EDIT_POPUP_ID,
                            Span::styled(" Edit ", Style::new().bold().cyan()),
                            Text::from(vec![
                                Line::from("Are you sure you want to edit an existing change?"),
                                Line::from(format!("Change: {}", self.head.change_id.as_str())),
                            ]).fg(Color::default()),
                        )
                        .with_yes_button(ButtonLabel::YES.clone())
                        .with_no_button(ButtonLabel::NO.clone())
                        .with_listener(Some(self.popup_tx.clone()))
                        .open();
                    }
                }
                KeyCode::Char('a') => {
                    if self.head.immutable {
                        return Ok(ComponentInputResult::HandledAction(
                            ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                                title: "Abandon".into(),
                                messages: vec![
                                    "The change cannot be abandoned because it is immutable."
                                        .into(),
                                ]
                                .into(),
                                text_align: None,
                            }))),
                        ));
                    } else {
                        self.popup = ConfirmDialogState::new(
                            ABANDON_POPUP_ID,
                            Span::styled(" Abandon ", Style::new().bold().cyan()),
                            Text::from(vec![
                                Line::from("Are you sure you want to abandon this change?"),
                                Line::from(format!("Change: {}", self.head.change_id.as_str())),
                            ]).fg(Color::default()),
                        )
                        .with_yes_button(ButtonLabel::YES.clone())
                        .with_no_button(ButtonLabel::NO.clone())
                        .with_listener(Some(self.popup_tx.clone()))
                        .open();
                    }
                }
                KeyCode::Char('d') => {
                    if self.head.immutable {
                        return Ok(ComponentInputResult::HandledAction(
                            ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                                title: "Describe".into(),
                                messages: vec![
                                    "The change cannot be described because it is immutable."
                                        .into(),
                                ]
                                .into(),
                                text_align: None,
                            }))),
                        ));
                    } else {
                        let mut textarea = TextArea::new(
                            commander
                                .get_commit_description(&self.head.commit_id)?
                                .split("\n")
                                .map(|line| line.to_string())
                                .collect(),
                        );
                        textarea.move_cursor(CursorMove::End);
                        self.describe_textarea = Some(textarea);
                        return Ok(ComponentInputResult::Handled);
                    }
                }
                KeyCode::Char('r') => {
                    let mut textarea = TextArea::new(
                        self.log_revset
                            .as_ref()
                            .unwrap_or(&"".to_owned())
                            .lines()
                            .map(String::from)
                            .collect(),
                    );
                    textarea.move_cursor(CursorMove::End);
                    self.log_revset_textarea = Some(textarea);
                    return Ok(ComponentInputResult::Handled);
                }
                KeyCode::Char('b') => {
                    return Ok(ComponentInputResult::HandledAction(
                        ComponentAction::SetPopup(Some(Box::new(BookmarkSetPopup::new(
                            self.config.clone(),
                            commander,
                            Some(self.head.change_id.clone()),
                            self.head.commit_id.clone(),
                            self.bookmark_set_popup_tx.clone(),
                        )))),
                    ));
                }
                KeyCode::Enter => {
                    return Ok(ComponentInputResult::HandledAction(
                        ComponentAction::ViewFiles(self.head.clone()),
                    ));
                }
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    match commander.git_push(
                        key.code == KeyCode::Char('P'),
                        key.modifiers.contains(KeyModifiers::CONTROL),
                        &self.head.commit_id,
                    ) {
                        Ok(result) if !result.is_empty() => {
                            return Ok(ComponentInputResult::HandledAction(
                                ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                                    title: "Push message".into(),
                                    messages: result.into_text()?,
                                    text_align: None,
                                }))),
                            ));
                        }
                        Err(err) => {
                            return Ok(ComponentInputResult::HandledAction(
                                ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                                    title: "Push error".into(),
                                    messages: err.into_text("")?,
                                    text_align: None,
                                }))),
                            ));
                        }
                        _ => (),
                    }

                    self.refresh_log_output(commander);
                    self.refresh_head_output(commander);
                }
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    match commander.git_fetch(key.code == KeyCode::Char('F')) {
                        Ok(result) if !result.is_empty() => {
                            return Ok(ComponentInputResult::HandledAction(
                                ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                                    title: "Fetch message".into(),
                                    messages: result.into_text()?,
                                    text_align: None,
                                }))),
                            ));
                        }
                        Err(err) => {
                            return Ok(ComponentInputResult::HandledAction(
                                ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                                    title: "Fetch error".into(),
                                    messages: err.into_text("")?,
                                    text_align: None,
                                }))),
                            ));
                        }
                        _ => (),
                    }

                    self.refresh_log_output(commander);
                    self.refresh_head_output(commander);
                }
                KeyCode::Char('h') | KeyCode::Char('?') => {
                    return Ok(ComponentInputResult::HandledAction(
                        ComponentAction::SetPopup(Some(Box::new(HelpPopup::new(
                            vec![
                                ("j/k".to_owned(), "scroll down/up".to_owned()),
                                ("J/K".to_owned(), "scroll down by ½ page".to_owned()),
                                ("Enter".to_owned(), "see files".to_owned()),
                                ("@".to_owned(), "current change".to_owned()),
                                ("r".to_owned(), "revset".to_owned()),
                                ("d".to_owned(), "describe change".to_owned()),
                                ("e".to_owned(), "edit change".to_owned()),
                                ("n".to_owned(), "new change".to_owned()),
                                ("N".to_owned(), "new with message".to_owned()),
                                ("a".to_owned(), "abandon change".to_owned()),
                                ("s".to_owned(), "squash @ into the selected change".to_owned()),
                                ("b".to_owned(), "set bookmark".to_owned()),
                                ("f".to_owned(), "git fetch".to_owned()),
                                ("F".to_owned(), "git fetch all remotes".to_owned()),
                                (
                                    "p".to_owned(),
                                    "git push (+Ctrl to include new bookmarks)".to_owned(),
                                ),
                                (
                                    "P".to_owned(),
                                    "git push all bookmarks (+Ctrl to include new bookmarks)"
                                        .to_owned(),
                                ),
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
