use anyhow::Result;
use tui_confirm_dialog::{ButtonLabel, ConfirmDialog, ConfirmDialogState, Listener};

use ansi_to_tui::IntoText;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::{prelude::*, widgets::*};
use tui_textarea::{CursorMove, TextArea};

use crate::{
    commander::{
        log::{Head, LogOutput},
        Commander,
    },
    env::{DiffFormat, JjConfig},
    ui::{
        details_panel::DetailsPanel,
        message_popup::MessagePopup,
        utils::{centered_rect, centered_rect_line_height},
        Component, ComponentAction,
    },
};

const NEW_POPUP_ID: u16 = 1;
const EDIT_POPUP_ID: u16 = 2;
const ABANDON_POPUP_ID: u16 = 3;

/// Log tab. Shows `jj log` in left panel and shows selected change details of in right panel.
pub struct Log<'a> {
    log_output: Result<LogOutput>,
    log_list_state: ListState,
    log_height: u16,

    log_revset: Option<String>,
    log_revset_textarea: Option<TextArea<'a>>,

    head_panel: DetailsPanel,
    head_output: Result<String>,
    head: Head,

    diff_format: DiffFormat,

    popup: ConfirmDialogState,
    popup_tx: std::sync::mpsc::Sender<Listener>,
    popup_rx: std::sync::mpsc::Receiver<Listener>,

    message_popup: Option<MessagePopup<'a>>,

    describe_textarea: Option<TextArea<'a>>,
    describe_after_new: bool,

    config: JjConfig,
}

fn get_head_index(head: &Head, log_output: &Result<LogOutput>) -> Option<usize> {
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

impl Log<'_> {
    pub fn new(commander: &mut Commander) -> Result<Self> {
        let diff_format = commander.env.config.diff_format();

        let log_output = commander.get_log(&None);
        let head = commander.get_current_head()?;

        let log_list_state = ListState::default().with_selected(get_head_index(&head, &log_output));

        let head_output = commander.get_commit_show(&head.commit_id, &diff_format);

        let (popup_tx, popup_rx) = std::sync::mpsc::channel();

        Ok(Self {
            log_output,
            log_list_state,
            log_height: 0,

            log_revset: None,
            log_revset_textarea: None,

            head,
            head_panel: DetailsPanel::new(),
            head_output,

            diff_format,

            popup: ConfirmDialogState::default(),
            popup_tx,
            popup_rx,

            message_popup: None,

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
    }

    fn refresh_head_output(&mut self, commander: &mut Commander) {
        self.head_output = commander.get_commit_show(&self.head.commit_id, &self.diff_format);
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
            next_head.clone_into(&mut self.head);
            self.refresh_head_output(commander);
        }
    }
}

#[allow(clippy::invisible_characters)]
impl Component for Log<'_> {
    fn update(&mut self, commander: &mut Commander) -> Result<Option<ComponentAction>> {
        let latest_head = commander.get_head_latest(&self.head)?;
        if latest_head != self.head {
            self.head = latest_head;
            self.refresh_log_output(commander);
            self.refresh_head_output(commander);
        }

        // Check for popup action
        if let Ok(res) = self.popup_rx.try_recv()
            && res.1.unwrap_or(false)
        {
            match res.0 {
                NEW_POPUP_ID => {
                    commander.run_new(&self.head.commit_id)?;
                    self.head = commander.get_current_head()?;
                    self.refresh_log_output(commander);
                    self.refresh_head_output(commander);
                    let mut actions = vec![ComponentAction::ChangeHead(self.head.clone())];
                    if self.describe_after_new {
                        self.describe_after_new = false;
                        let mut textarea = TextArea::default();
                        textarea.move_cursor(CursorMove::End);
                        self.describe_textarea = Some(textarea);
                        actions.push(ComponentAction::SetTextAreaActive(true));
                    }
                    return Ok(Some(ComponentAction::Multiple(actions)));
                }
                EDIT_POPUP_ID => {
                    // TODO: Handle error
                    commander.run_edit(&self.head.commit_id)?;
                    self.refresh_log_output(commander);
                    self.refresh_head_output(commander);
                    return Ok(Some(ComponentAction::ChangeHead(self.head.clone())));
                }
                ABANDON_POPUP_ID => {
                    if self.head == commander.get_current_head()? {
                        // TODO: Handle error
                        commander.run_abandon(&self.head.commit_id)?;
                        self.refresh_log_output(commander);
                        self.head = commander.get_current_head()?;
                        self.refresh_head_output(commander);
                        return Ok(Some(ComponentAction::ChangeHead(self.head.clone())));
                    } else {
                        let head_parent = commander.get_commit_parent(&self.head.commit_id)?;
                        // TODO: Handle error
                        commander.run_abandon(&self.head.commit_id)?;
                        self.refresh_log_output(commander);
                        self.head = head_parent;
                        self.refresh_head_output(commander);
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

        // Draw log
        {
            let panel_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Fill(1), Constraint::Length(2)])
                .split(chunks[0]);

            let mut scroll_offset = 0;
            let log_lines = match self.log_output.as_ref() {
                Ok(log_output) => {
                    let log_lines: Vec<Line> = log_output
                        .graph
                        .to_text()?
                        .iter()
                        .enumerate()
                        .map(|(i, line)| {
                            let mut line = line.to_owned();

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
                Err(err) => {
                    format!(
                        "{}\n\n\n{}",
                        &err.to_string(),
                        err.source()
                            .map_or("".to_string(), |source| source.to_string())
                    )
                    .into_text()?
                    .lines
                }
            };

            let title = match &self.log_revset {
                Some(log_revset) => &format!(" Log for: {} ", log_revset),
                None => " Log ",
            };

            let log = List::new(log_lines)
                .block(
                    Block::bordered()
                        .title(title)
                        .border_type(BorderType::Rounded),
                )
                .scroll_padding(7);
            f.render_stateful_widget(log, panel_chunks[0], &mut self.log_list_state);
            self.log_height = panel_chunks[0].height.saturating_sub(2);

            let help = Paragraph::new(vec![
                "j/k: scroll down/up | J/K: scroll down by ½ page | Enter: see files | @: current change | r: revset"
                    .into(),
                "d: describe change | e: edit change | n: new change | N: new with message | a: abandon change".into(),
            ]).fg(Color::DarkGray);
            f.render_widget(help, panel_chunks[1]);
        }

        // Draw change details
        {
            let panel_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Fill(1), Constraint::Length(2)])
                .split(chunks[1]);

            let head_content = match self.head_output.as_ref() {
                Ok(head_output) => head_output,
                Err(err) => &format!(
                    "{}\n\n\n{}",
                    &err.to_string(),
                    err.source()
                        .map_or("".to_string(), |source| source.to_string())
                ),
            };
            let head_block = Block::bordered()
                .title(format!(" Details for {} ", self.head.change_id))
                .border_type(BorderType::Rounded);
            let head = self
                .head_panel
                .render(head_content.into_text()?, head_block.inner(chunks[1]))
                .block(head_block);

            f.render_widget(head, panel_chunks[0]);

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

                f.render_widget(log_revset_textarea.widget(), popup_chunks[0]);

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
    fn input(
        &mut self,
        commander: &mut Commander,
        event: Event,
    ) -> Result<Option<ComponentAction>> {
        if let Some(describe_textarea) = self.describe_textarea.as_mut() {
            if let Event::Key(key) = event {
                match key.code {
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // TODO: Handle error
                        commander.run_describe(
                            &self.head.commit_id,
                            &describe_textarea.lines().join("\n"),
                        )?;
                        self.refresh_log_output(commander);
                        self.refresh_head_output(commander);
                        self.describe_textarea = None;
                        return Ok(Some(ComponentAction::SetTextAreaActive(false)));
                    }
                    KeyCode::Esc => {
                        self.describe_textarea = None;
                        return Ok(Some(ComponentAction::SetTextAreaActive(false)));
                    }
                    _ => {}
                }
            }
            describe_textarea.input(event);
            return Ok(None);
        }

        if let Some(log_revset_textarea) = self.log_revset_textarea.as_mut() {
            if let Event::Key(key) = event {
                match key.code {
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let log_revset = log_revset_textarea.lines().join("");
                        self.log_revset = if log_revset.trim().is_empty() {
                            None
                        } else {
                            Some(log_revset)
                        };
                        self.refresh_log_output(commander);
                        self.log_revset_textarea = None;
                        return Ok(Some(ComponentAction::SetTextAreaActive(false)));
                    }
                    KeyCode::Esc => {
                        self.log_revset_textarea = None;
                        return Ok(Some(ComponentAction::SetTextAreaActive(false)));
                    }
                    _ => {}
                }
            }
            log_revset_textarea.input(event);
            return Ok(None);
        }

        if let Event::Key(key) = event {
            if self.popup.is_opened() && self.popup.handle(key) {
                return Ok(None);
            }
            if let Some(message_popup) = &self.message_popup {
                if message_popup.input(key) {
                    self.message_popup = None;
                }
                return Ok(None);
            }

            if self.head_panel.input(key) {
                return Ok(None);
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
                KeyCode::Char('p') => {
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
                        ]),
                    )
                    .modal(true)
                    .with_yes_button(ButtonLabel::YES.clone())
                    .with_no_button(ButtonLabel::NO.clone())
                    .with_listener(Some(self.popup_tx.clone()))
                    .open();

                    if key.code == KeyCode::Char('N') {
                        self.describe_after_new = true;
                    }
                }
                KeyCode::Char('e') => {
                    if self.head.immutable {
                        self.message_popup = Some(MessagePopup {
                            title: "Edit".into(),
                            messages: vec![
                                "The change cannot be edited because it is immutable.".into()
                            ]
                            .into(),
                        });
                    } else {
                        self.popup = ConfirmDialogState::new(
                            EDIT_POPUP_ID,
                            Span::styled(" Edit ", Style::new().bold().cyan()),
                            Text::from(vec![
                                Line::from("Are you sure you want to edit an existing change?"),
                                Line::from(format!("Change: {}", self.head.change_id.as_str())),
                            ]),
                        )
                        .modal(true)
                        .with_yes_button(ButtonLabel::YES.clone())
                        .with_no_button(ButtonLabel::NO.clone())
                        .with_listener(Some(self.popup_tx.clone()))
                        .open();
                    }
                }
                KeyCode::Char('a') => {
                    if self.head.immutable {
                        self.message_popup = Some(MessagePopup {
                            title: "Abandon".into(),
                            messages: vec![
                                "The change cannot be abandoned because it is immutable.".into(),
                            ]
                            .into(),
                        });
                    } else {
                        self.popup = ConfirmDialogState::new(
                            ABANDON_POPUP_ID,
                            Span::styled(" Abandon ", Style::new().bold().cyan()),
                            Text::from(vec![
                                Line::from("Are you sure you want to abandon this change?"),
                                Line::from(format!("Change: {}", self.head.change_id.as_str())),
                            ]),
                        )
                        .modal(true)
                        .with_yes_button(ButtonLabel::YES.clone())
                        .with_no_button(ButtonLabel::NO.clone())
                        .with_listener(Some(self.popup_tx.clone()))
                        .open();
                    }
                }
                KeyCode::Char('d') => {
                    if self.head.immutable {
                        self.message_popup = Some(MessagePopup {
                            title: "Describe".into(),
                            messages: vec![
                                "The change cannot be described because it is immutable.".into(),
                            ]
                            .into(),
                        });
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
                        return Ok(Some(ComponentAction::SetTextAreaActive(true)));
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
                    return Ok(Some(ComponentAction::SetTextAreaActive(true)));
                }
                KeyCode::Enter => {
                    return Ok(Some(ComponentAction::ViewFiles(self.head.clone())));
                }
                _ => {}
            };
        }

        Ok(None)
    }
}
