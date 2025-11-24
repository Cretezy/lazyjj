#![expect(clippy::borrow_interior_mutable_const)]

use ansi_to_tui::IntoText;
use anyhow::Result;
use ratatui::{
    crossterm::event::{Event, KeyEventKind},
    layout::Rect,
    prelude::*,
    widgets::*,
};
use tracing::instrument;
use tui_confirm_dialog::{ButtonLabel, ConfirmDialog, ConfirmDialogState, Listener};
use tui_textarea::{CursorMove, TextArea};

use crate::{
    ComponentInputResult,
    commander::{CommandError, Commander, log::Head},
    env::{Config, DiffFormat},
    keybinds::{LogTabEvent, LogTabKeybinds},
    ui::{
        Component, ComponentAction,
        bookmark_set_popup::BookmarkSetPopup,
        help_popup::HelpPopup,
        message_popup::MessagePopup,
        panel::DetailsPanel,
        panel::LogPanel,
        rebase_popup::RebasePopup,
        utils::{centered_rect, centered_rect_line_height, tabs_to_spaces},
    },
};

const NEW_POPUP_ID: u16 = 1;
const EDIT_POPUP_ID: u16 = 2;
const ABANDON_POPUP_ID: u16 = 3;
const SQUASH_POPUP_ID: u16 = 4;

/// Log tab. Shows `jj log` in main panel and shows selected change details of in details panel.
pub struct LogTab<'a> {
    /// The revset filter to apply to jj log
    log_revset_textarea: Option<TextArea<'a>>,

    /// The list of changes shown to the left
    log_panel: LogPanel<'a>,

    /// The change content shown to the right
    head_panel: DetailsPanel,
    head_output: Result<String, CommandError>,

    /// The currently selected change. Indicates what to render
    /// in head_output. It is a copy of self.log_panel.head,
    /// so if these differ, we need to update self.head and
    /// self.head_output
    head: Head,

    // Location of panels on screen. [0] = log, [1] = details
    panel_rect: [Rect; 2],

    diff_format: DiffFormat,

    popup: ConfirmDialogState,
    popup_tx: std::sync::mpsc::Sender<Listener>,
    popup_rx: std::sync::mpsc::Receiver<Listener>,

    bookmark_set_popup_tx: std::sync::mpsc::Sender<bool>,
    bookmark_set_popup_rx: std::sync::mpsc::Receiver<bool>,

    describe_textarea: Option<TextArea<'a>>,
    describe_after_new: bool,

    rebase_popup: Option<RebasePopup>,

    squash_ignore_immutable: bool,

    edit_ignore_immutable: bool,

    config: Config,
    keybinds: LogTabKeybinds,
}

impl<'a> LogTab<'a> {
    #[instrument(level = "trace", skip(commander))]
    pub fn new(commander: &mut Commander) -> Result<Self> {
        let diff_format = commander.env.config.diff_format();

        let head = commander.get_current_head()?;

        let head_output = commander
            .get_commit_show(&head.commit_id, &diff_format, true)
            .map(|text| tabs_to_spaces(&text));

        let (popup_tx, popup_rx) = std::sync::mpsc::channel();
        let (bookmark_set_popup_tx, bookmark_set_popup_rx) = std::sync::mpsc::channel();

        let mut keybinds = LogTabKeybinds::default();
        if let Some(new_keybinds) = commander
            .env
            .config
            .keybinds()
            .and_then(|k| k.log_tab.clone())
        {
            keybinds.extend_from_config(&new_keybinds);
        }

        Ok(Self {
            log_revset_textarea: None,

            log_panel: LogPanel::new(commander)?,

            head,
            head_panel: DetailsPanel::new(),
            head_output,

            panel_rect: [Rect::ZERO, Rect::ZERO],

            diff_format,

            popup: ConfirmDialogState::default(),
            popup_tx,
            popup_rx,

            bookmark_set_popup_tx,
            bookmark_set_popup_rx,

            describe_textarea: None,
            describe_after_new: false,

            rebase_popup: None,

            squash_ignore_immutable: false,

            edit_ignore_immutable: false,

            config: commander.env.config.clone(),
            keybinds,
        })
    }

    /// Update change details panel if the selection has changed
    fn sync_head_output(&mut self, commander: &mut Commander) {
        if self.head == self.log_panel.head {
            // log panel and head panel agree on head
            return;
        }
        // Update head panel to show new head
        self.head = self.log_panel.head.clone();
        self.refresh_head_output(commander);
    }

    fn refresh_head_output(&mut self, commander: &mut Commander) {
        self.head_output = commander
            .get_commit_show(&self.head.commit_id, &self.diff_format, true)
            .map(|text| tabs_to_spaces(&text));
        self.head_panel.scroll_to(0);
    }

    pub fn set_head(&mut self, commander: &mut Commander, head: Head) {
        self.log_panel.set_head(head);
        self.log_panel.refresh_log_output(commander);
        self.sync_head_output(commander);
    }

    fn handle_event(
        &mut self,
        commander: &mut Commander,
        log_tab_event: LogTabEvent,
    ) -> Result<ComponentInputResult> {
        match log_tab_event {
            LogTabEvent::ScrollDown
            | LogTabEvent::ScrollUp
            | LogTabEvent::ScrollDownHalf
            | LogTabEvent::ScrollUpHalf => {
                self.log_panel.handle_event(commander, log_tab_event)?;
                self.sync_head_output(commander);
            }
            LogTabEvent::FocusCurrent => {
                self.set_head(commander, commander.get_current_head()?);
            }
            LogTabEvent::ToggleDiffFormat => {
                self.diff_format = self.diff_format.get_next(self.config.diff_tool());
                self.refresh_head_output(commander);
            }
            LogTabEvent::Refresh => {
                self.log_panel.refresh_log_output(commander);
                self.refresh_head_output(commander);
            }
            LogTabEvent::CreateNew { describe } => {
                self.popup = ConfirmDialogState::new(
                    NEW_POPUP_ID,
                    Span::styled(" New ", Style::new().bold().cyan()),
                    Text::from(vec![
                        Line::from("Are you sure you want to create a new change?"),
                        Line::from(format!("New parent: {}", self.head.change_id.as_str())),
                    ])
                    .fg(Color::default()),
                );
                self.popup
                    .with_yes_button(ButtonLabel::YES.clone())
                    .with_no_button(ButtonLabel::NO.clone())
                    .with_listener(Some(self.popup_tx.clone()))
                    .open();
                self.describe_after_new = describe;
            }
            LogTabEvent::Rebase => {
                let source_change = commander.get_current_head()?.commit_id;
                let target_change = &self.head.commit_id;
                self.rebase_popup = Some(RebasePopup::new(
                    source_change.clone(),
                    target_change.clone(),
                ));
            }
            LogTabEvent::Squash { ignore_immutable } => {
                if self.head.change_id == commander.get_current_head()?.change_id {
                    return Ok(ComponentInputResult::HandledAction(
                        ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                            title: "Squash".into(),
                            messages: "Cannot squash onto current change".into_text()?,
                            text_align: None,
                        }))),
                    ));
                }
                if self.head.immutable && !ignore_immutable {
                    return Ok(ComponentInputResult::HandledAction(
                        ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                            title: "Squash".into(),
                            messages: "Cannot squash onto immutable change".into_text()?,
                            text_align: None,
                        }))),
                    ));
                }

                let mut lines = vec![
                    Line::from("Are you sure you want to squash @ into this change?"),
                    Line::from(format!("Squash into {}", self.head.change_id.as_str())),
                ];
                if ignore_immutable {
                    lines.push(Line::from("This change is immutable."));
                }
                self.popup = ConfirmDialogState::new(
                    SQUASH_POPUP_ID,
                    Span::styled(" Squash ", Style::new().bold().cyan()),
                    Text::from(lines).fg(Color::default()),
                );
                self.popup
                    .with_yes_button(ButtonLabel::YES.clone())
                    .with_no_button(ButtonLabel::NO.clone())
                    .with_listener(Some(self.popup_tx.clone()))
                    .open();
                self.squash_ignore_immutable = ignore_immutable;
            }
            LogTabEvent::EditChange { ignore_immutable } => {
                if self.head.immutable && !ignore_immutable {
                    return Ok(ComponentInputResult::HandledAction(
                        ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                            title: " Edit ".into(),
                            messages: vec![
                                "The change cannot be edited because it is immutable.".into(),
                            ]
                            .into(),
                            text_align: None,
                        }))),
                    ));
                }

                let mut lines = vec![
                    Line::from("Are you sure you want to edit an existing change?"),
                    Line::from(format!("Change: {}", self.head.change_id.as_str())),
                ];
                if ignore_immutable {
                    lines.push(Line::from("This change is immutable."))
                }
                self.popup = ConfirmDialogState::new(
                    EDIT_POPUP_ID,
                    Span::styled(" Edit ", Style::new().bold().cyan()),
                    Text::from(lines).fg(Color::default()),
                );
                self.popup
                    .with_yes_button(ButtonLabel::YES.clone())
                    .with_no_button(ButtonLabel::NO.clone())
                    .with_listener(Some(self.popup_tx.clone()))
                    .open();
                self.edit_ignore_immutable = ignore_immutable;
            }
            LogTabEvent::Abandon => {
                if self.head.immutable {
                    return Ok(ComponentInputResult::HandledAction(
                        ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                            title: "Abandon".into(),
                            messages: vec![
                                "The change cannot be abandoned because it is immutable.".into(),
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
                        ])
                        .fg(Color::default()),
                    );
                    self.popup
                        .with_yes_button(ButtonLabel::YES.clone())
                        .with_no_button(ButtonLabel::NO.clone())
                        .with_listener(Some(self.popup_tx.clone()))
                        .open();
                }
            }
            LogTabEvent::Describe => {
                if self.head.immutable {
                    return Ok(ComponentInputResult::HandledAction(
                        ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                            title: "Describe".into(),
                            messages: vec![
                                "The change cannot be described because it is immutable.".into(),
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
            LogTabEvent::EditRevset => {
                let mut textarea = TextArea::new(
                    self.log_panel
                        .log_revset
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
            LogTabEvent::SetBookmark => {
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
            LogTabEvent::OpenFiles => {
                return Ok(ComponentInputResult::HandledAction(
                    ComponentAction::ViewFiles(self.head.clone()),
                ));
            }
            LogTabEvent::Push {
                all_bookmarks,
                allow_new,
            } => {
                match commander.git_push(all_bookmarks, allow_new, &self.head.commit_id) {
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

                self.log_panel.refresh_log_output(commander);
                self.refresh_head_output(commander);
            }
            LogTabEvent::Fetch { all_remotes } => {
                match commander.git_fetch(all_remotes) {
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

                self.log_panel.refresh_log_output(commander);
                self.refresh_head_output(commander);
            }
            LogTabEvent::OpenHelp => {
                return Ok(ComponentInputResult::HandledAction(
                    ComponentAction::SetPopup(Some(Box::new(HelpPopup::new(
                        self.keybinds.make_main_panel_help(),
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
                ));
            }
            LogTabEvent::Save
            | LogTabEvent::Cancel
            | LogTabEvent::ClosePopup
            | LogTabEvent::Unbound => return Ok(ComponentInputResult::NotHandled),
        };
        Ok(ComponentInputResult::Handled)
    }
}

impl Component for LogTab<'_> {
    fn focus(&mut self, commander: &mut Commander) -> Result<()> {
        let latest_head = commander.get_head_latest(&self.head)?;
        self.log_panel.set_head(latest_head);
        self.sync_head_output(commander);
        Ok(())
    }

    fn update(&mut self, commander: &mut Commander) -> Result<Option<ComponentAction>> {
        // Check for popup action
        if let Ok(res) = self.popup_rx.try_recv()
            && res.1.unwrap_or(false)
        {
            match res.0 {
                NEW_POPUP_ID => {
                    commander.run_new(self.head.commit_id.as_str())?;
                    self.set_head(commander, commander.get_current_head()?);
                    if self.describe_after_new {
                        self.describe_after_new = false;
                        let textarea = TextArea::default();
                        self.describe_textarea = Some(textarea);
                    }
                    return Ok(Some(ComponentAction::ChangeHead(self.head.clone())));
                }
                EDIT_POPUP_ID => {
                    commander.run_edit(self.head.commit_id.as_str(), self.edit_ignore_immutable)?;
                    self.log_panel.refresh_log_output(commander);
                    self.refresh_head_output(commander);
                    return Ok(Some(ComponentAction::ChangeHead(self.head.clone())));
                }
                ABANDON_POPUP_ID => {
                    if self.head == commander.get_current_head()? {
                        commander.run_abandon(&self.head.commit_id)?;
                        self.set_head(commander, commander.get_current_head()?);
                        return Ok(Some(ComponentAction::ChangeHead(self.head.clone())));
                    } else {
                        let head_parent = commander.get_commit_parent(&self.head.commit_id)?;
                        commander.run_abandon(&self.head.commit_id)?;
                        self.set_head(commander, head_parent);
                    }
                }
                SQUASH_POPUP_ID => {
                    commander
                        .run_squash(self.head.commit_id.as_str(), self.squash_ignore_immutable)?;
                    self.set_head(commander, commander.get_current_head()?);
                    return Ok(Some(ComponentAction::ChangeHead(self.head.clone())));
                }
                _ => {}
            }
        }

        if let Ok(true) = self.bookmark_set_popup_rx.try_recv() {
            self.log_panel.refresh_log_output(commander);
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
        self.panel_rect = [chunks[0], chunks[1]];

        // Draw log
        self.log_panel.draw(f, chunks[0])?;

        // Draw change details
        {
            let head_content = match self.head_output.as_ref() {
                Ok(head_output) => head_output.into_text()?.lines,
                Err(err) => err.into_text("Error getting head details")?.lines,
            };
            self.head_panel
                .render_context()
                .title(format!(" Details for {} ", self.head.change_id))
                .content(head_content)
                .draw(f, chunks[1])
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

        // Draw rebase popup
        {
            if let Some(log_rebase_popup) = &mut self.rebase_popup {
                log_rebase_popup.render_widget(f)
            }
        }

        Ok(())
    }

    fn input(&mut self, commander: &mut Commander, event: Event) -> Result<ComponentInputResult> {
        if let Some(describe_textarea) = self.describe_textarea.as_mut() {
            if let Event::Key(key) = event {
                match self.keybinds.match_event(key) {
                    LogTabEvent::Save => {
                        // TODO: Handle error
                        commander.run_describe(
                            self.head.commit_id.as_str(),
                            &describe_textarea.lines().join("\n"),
                        )?;
                        self.set_head(commander, commander.get_head_latest(&self.head)?);
                        self.describe_textarea = None;
                        return Ok(ComponentInputResult::Handled);
                    }
                    LogTabEvent::Cancel => {
                        self.describe_textarea = None;
                        return Ok(ComponentInputResult::Handled);
                    }
                    _ => (),
                }
            }
            describe_textarea.input(event);
            return Ok(ComponentInputResult::Handled);
        }

        if let Some(log_revset_textarea) = self.log_revset_textarea.as_mut() {
            if let Event::Key(key) = event {
                match self.keybinds.match_event(key) {
                    LogTabEvent::Save => {
                        let log_revset = log_revset_textarea.lines().join("\n");
                        self.log_panel.log_revset = if log_revset.trim().is_empty() {
                            None
                        } else {
                            Some(log_revset)
                        };
                        self.log_panel.refresh_log_output(commander);
                        self.log_revset_textarea = None;
                        return Ok(ComponentInputResult::Handled);
                    }
                    LogTabEvent::Cancel => {
                        self.log_revset_textarea = None;
                        return Ok(ComponentInputResult::Handled);
                    }
                    _ => (),
                }
            }
            log_revset_textarea.input(event);
            return Ok(ComponentInputResult::Handled);
        }

        if let Some(rebase_popup) = &mut self.rebase_popup {
            let handled = rebase_popup.handle_input(commander, event.clone());
            if handled.is_err() {
                // Close popup and show error message
                self.rebase_popup = None;
                let msg = handled.err().unwrap();
                let error_message = msg.to_string().into_text()?;
                return Ok(ComponentInputResult::HandledAction(
                    ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                        title: "Error".into(),
                        messages: error_message,
                        text_align: None,
                    }))),
                ));
            }
            if handled.ok() == Some(true) {
                // when handle_input returns true,
                // the popup should be closed
                self.rebase_popup = None;
                return Ok(ComponentInputResult::HandledAction(
                    ComponentAction::RefreshTab(),
                ));
            }
            return Ok(ComponentInputResult::Handled);
        }

        if let Event::Key(key) = &event {
            let key = *key;
            if key.kind != KeyEventKind::Press {
                return Ok(ComponentInputResult::Handled);
            }

            if self.popup.is_opened() {
                if matches!(
                    self.keybinds.match_event(key),
                    LogTabEvent::ClosePopup | LogTabEvent::Cancel
                ) {
                    self.popup = ConfirmDialogState::default();
                } else {
                    self.popup.handle(&key);
                }

                return Ok(ComponentInputResult::Handled);
            }

            if self.head_panel.input(key) {
                return Ok(ComponentInputResult::Handled);
            }

            let input_result = self.log_panel.input(commander, event)?;
            if input_result.is_handled() {
                self.sync_head_output(commander);
                return Ok(input_result);
            }

            let log_tab_event = self.keybinds.match_event(key);
            return self.handle_event(commander, log_tab_event);
        }

        if let Event::Mouse(mouse_event) = event {
            let input_result = self.log_panel.input(commander, event.clone())?;
            if input_result.is_handled() {
                self.sync_head_output(commander);
                return Ok(input_result);
            }
            if self.head_panel.input_mouse(mouse_event) {
                return Ok(ComponentInputResult::Handled);
            }
            return Ok(ComponentInputResult::NotHandled);
        }

        Ok(ComponentInputResult::Handled)
    }
}
