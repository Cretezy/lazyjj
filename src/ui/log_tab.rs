#![expect(clippy::borrow_interior_mutable_const)]

use ansi_to_tui::IntoText;
use anyhow::Result;
use ratatui::{
    crossterm::event::{Event, KeyEventKind, MouseEvent, MouseEventKind},
    layout::Rect,
    prelude::*,
    text::ToText,
    widgets::*,
};
use tracing::instrument;
use tui_confirm_dialog::{ButtonLabel, ConfirmDialog, ConfirmDialogState, Listener};
use tui_textarea::{CursorMove, TextArea};

use crate::{
    ComponentInputResult,
    commander::{
        CommandError, Commander,
        log::{Head, LogOutput},
    },
    env::{Config, DiffFormat},
    keybinds::{LogTabEvent, LogTabKeybinds},
    ui::{
        Component, ComponentAction,
        bookmark_set_popup::BookmarkSetPopup,
        details_panel::DetailsPanel,
        details_panel::DetailsPanelEvent,
        help_popup::HelpPopup,
        message_popup::MessagePopup,
        utils::{centered_rect, centered_rect_line_height, tabs_to_spaces},
    },
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
    log_rect: Rect,
    log_height: u16,

    log_revset: Option<String>,
    log_revset_textarea: Option<TextArea<'a>>,

    head_panel: DetailsPanel,
    head_output: Result<String, CommandError>,
    head: Head,

    // Rect of panels [0] = log, [1] = details
    panel_rect: [Rect; 2],

    diff_format: DiffFormat,

    popup: ConfirmDialogState,
    popup_tx: std::sync::mpsc::Sender<Listener>,
    popup_rx: std::sync::mpsc::Receiver<Listener>,

    bookmark_set_popup_tx: std::sync::mpsc::Sender<bool>,
    bookmark_set_popup_rx: std::sync::mpsc::Receiver<bool>,

    describe_textarea: Option<TextArea<'a>>,
    describe_after_new: bool,

    squash_ignore_immutable: bool,

    edit_ignore_immutable: bool,

    config: Config,
    keybinds: LogTabKeybinds,
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

impl<'a> LogTab<'a> {
    #[instrument(level = "trace", skip(commander))]
    pub fn new(commander: &mut Commander) -> Result<Self> {
        let diff_format = commander.env.config.diff_format();

        let log_revset = commander.env.default_revset.clone();
        let log_output = commander.get_log(&log_revset);
        let head = commander.get_current_head()?;

        let log_list_state = ListState::default().with_selected(get_head_index(&head, &log_output));

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
            log_rect: Rect::ZERO,

            log_revset,
            log_revset_textarea: None,

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

            squash_ignore_immutable: false,

            edit_ignore_immutable: false,

            config: commander.env.config.clone(),
            keybinds,
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
            .get_commit_show(&self.head.commit_id, &self.diff_format, true)
            .map(|text| tabs_to_spaces(&text));
        self.head_panel.scroll_to(0);
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

    /// Convert log output to a list of formatted lines
    fn output_to_lines(&self, log_output: &LogOutput) -> Vec<Line<'a>> {
        // Set the background color of the line
        fn set_bg(line: &mut Line, bg_color: Color) {
            // Set background to use when no Span is present
            // This makes the highlight continue beyond the last Span
            line.style = line.style.patch(Style::default().bg(bg_color));

            for span in line.spans.iter_mut() {
                span.style = span.style.bg(bg_color)
            }
        }

        self.log_output_text
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let mut line = line.to_owned();

                // Add padding at start
                line.spans.insert(0, Span::from(" "));

                // Highlight lines that correspond to self.head
                let line_head = log_output.graph_heads.get(i).unwrap_or(&None);
                if let Some(line_change) = line_head {
                    if line_change == &self.head {
                        set_bg(&mut line, self.config.highlight_color());
                    }
                };

                line
            })
            .collect()
    }

    /// Find the line in self.log_output that match self.head
    fn selected_log_line(&self) -> Option<usize> {
        let Ok(log_output) = self.log_output.as_ref() else {
            return None;
        };

        log_output
            .graph_heads
            .iter()
            .position(|opt_h| opt_h.as_ref().is_some_and(|h| h == &self.head))
    }

    /// Find head of the provided log_output line
    fn head_at_log_line(&mut self, log_line: usize) -> Option<Head> {
        let Ok(log_output) = self.log_output.as_ref() else {
            return None;
        };

        let graph_head = log_output.graph_heads.get(log_line)?;

        graph_head.clone()
    }

    /// Get lines to show in log list
    fn log_lines(&self) -> Vec<Line<'a>> {
        match self.log_output.as_ref() {
            Ok(log_output) => self.output_to_lines(log_output),
            Err(err) => err.into_text("Error getting log").unwrap().lines,
        }
    }

    /// Number of log list items that fit on screen
    fn log_visible_items(&self) -> u16 {
        // Every item in the log list is 2 lines high, so divide screen rows
        // by 2 to get the number of log items that fit in it.
        self.log_height / 2
    }

    fn handle_event(
        &mut self,
        commander: &mut Commander,
        log_tab_event: LogTabEvent,
    ) -> Result<ComponentInputResult> {
        match log_tab_event {
            LogTabEvent::ScrollDown => {
                self.scroll_log(commander, 1);
            }
            LogTabEvent::ScrollUp => {
                self.scroll_log(commander, -1);
            }
            LogTabEvent::ScrollDownHalf => {
                self.scroll_log(commander, self.log_visible_items() as isize / 2);
            }
            LogTabEvent::ScrollUpHalf => {
                self.scroll_log(
                    commander,
                    (self.log_visible_items() as isize / 2).saturating_neg(),
                );
            }
            LogTabEvent::FocusCurrent => {
                self.head = commander.get_current_head()?;
                self.refresh_head_output(commander);
            }
            LogTabEvent::ToggleDiffFormat => {
                self.diff_format = self.diff_format.get_next(self.config.diff_tool());
                self.refresh_head_output(commander);
            }
            LogTabEvent::Refresh => {
                self.refresh_log_output(commander);
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

                self.refresh_log_output(commander);
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

                self.refresh_log_output(commander);
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
        if latest_head != self.head {
            self.head = latest_head;
        }
        self.refresh_log_output(commander);
        self.refresh_head_output(commander);
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
                    commander.run_edit(self.head.commit_id.as_str(), self.edit_ignore_immutable)?;
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
                    commander
                        .run_squash(self.head.commit_id.as_str(), self.squash_ignore_immutable)?;
                    self.head = commander.get_current_head()?;
                    self.refresh_log_output(commander);
                    self.refresh_head_output(commander);
                    return Ok(Some(ComponentAction::ChangeHead(self.head.clone())));
                }
                _ => {}
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
        self.panel_rect = [chunks[0], chunks[1]];

        // Draw log
        {
            let title = match &self.log_revset {
                Some(log_revset) => &format!(" Log for: {log_revset} "),
                None => " Log ",
            };

            let log_lines = self.log_lines();
            let log_length: usize = log_lines.len();
            let log_block = Block::bordered()
                .title(title)
                .border_type(BorderType::Rounded);
            self.log_rect = log_block.inner(chunks[0]);
            self.log_height = log_block.inner(chunks[0]).height;
            self.log_list_state.select(self.selected_log_line());
            let log = List::new(log_lines)
                .block(log_block)
                .scroll_padding(7);
            f.render_stateful_widget(log, chunks[0], &mut self.log_list_state);

            // Show scrollbar if lines don't fit the screen height
            if log_length > self.log_height.into() {
                let index = self.log_list_state.selected().unwrap_or(0);
                let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
                let mut scrollbar_state = ScrollbarState::default()
                    .content_length(log_length)
                    .position(index);

                f.render_stateful_widget(
                    scrollbar,
                    chunks[0].inner(Margin {
                        vertical: 1,
                        horizontal: 0,
                    }),
                    &mut scrollbar_state,
                );
            }
        }

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
                        self.head = commander.get_head_latest(&self.head)?;
                        self.refresh_log_output(commander);
                        self.refresh_head_output(commander);
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
                        self.log_revset = if log_revset.trim().is_empty() {
                            None
                        } else {
                            Some(log_revset)
                        };
                        self.refresh_log_output(commander);
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

        if let Event::Key(key) = event {
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

            let log_tab_event = self.keybinds.match_event(key);
            return self.handle_event(commander, log_tab_event);
        }

        if let Event::Mouse(mouse_event) = event {
            // Determine if mouse event is inside log-view or details-view
            fn contains(rect: &Rect, mouse_event: &MouseEvent) -> bool {
                rect.x <= mouse_event.column
                    && mouse_event.column < rect.x + rect.width
                    && rect.y <= mouse_event.row
                    && mouse_event.row < rect.y + rect.height
            }
            let find_panel = || -> Option<usize> {
                for (i, rect) in self.panel_rect.iter().enumerate() {
                    if contains(rect, &mouse_event) {
                        return Some(i);
                    }
                }
                None
            };
            let panel = find_panel();
            // Execute command dependent on panel and event kind
            const LOG_PANEL: Option<usize> = Some(0);
            const DETAILS_PANEL: Option<usize> = Some(1);
            match (panel, mouse_event.kind) {
                (LOG_PANEL, MouseEventKind::ScrollUp) => {
                    self.handle_event(commander, LogTabEvent::ScrollUp)?;
                }
                (LOG_PANEL, MouseEventKind::ScrollDown) => {
                    self.handle_event(commander, LogTabEvent::ScrollDown)?;
                }
                (LOG_PANEL, MouseEventKind::Up(_)) => {
                    // Check all items in list

                    // TODO make a function that constructs the log list
                    let log_lines = self.log_lines();
                    let log_items: Vec<ListItem> = log_lines
                        .iter()
                        .map(|line| ListItem::from(line.to_text()))
                        .collect();

                    // Select the clicked change
                    if let Some(inx) = list_item_from_mouse_event(
                        &log_items,
                        self.log_rect,
                        &self.log_list_state,
                        &mouse_event,
                    ) {
                        if let Some(head) = self.head_at_log_line(inx) {
                            self.set_head(commander, head);
                        }
                    }
                }
                (DETAILS_PANEL, MouseEventKind::ScrollUp) => {
                    self.head_panel.handle_event(DetailsPanelEvent::ScrollUp);
                    self.head_panel.handle_event(DetailsPanelEvent::ScrollUp);
                    self.head_panel.handle_event(DetailsPanelEvent::ScrollUp);
                }
                (DETAILS_PANEL, MouseEventKind::ScrollDown) => {
                    self.head_panel.handle_event(DetailsPanelEvent::ScrollDown);
                    self.head_panel.handle_event(DetailsPanelEvent::ScrollDown);
                    self.head_panel.handle_event(DetailsPanelEvent::ScrollDown);
                }
                _ => {} // Handle other mouse events if necessary
            }
        }

        Ok(ComponentInputResult::Handled)
    }
}

// Determine which list item a mouse event is related to
fn list_item_from_mouse_event(
    list: &[ListItem],
    list_rect: Rect,
    list_state: &ListState,
    mouse_event: &MouseEvent,
) -> Option<usize> {
    fn contains(rect: &Rect, mouse_event: &MouseEvent) -> bool {
        rect.x <= mouse_event.column
            && mouse_event.column < rect.x + rect.width
            && rect.y <= mouse_event.row
            && mouse_event.row < rect.y + rect.height
    }

    if !contains(&list_rect, mouse_event) {
        return None;
    }

    // for each item on screen check if it contains the mouse cursor

    let mut item_row = list_rect.y;
    let mut item_inx = list_state.offset();
    while item_row <= mouse_event.row {
        let next_row = item_row + list[item_inx].height() as u16;
        if mouse_event.row < next_row {
            return Some(item_inx);
        }
        item_row = next_row;
        item_inx += 1;
        if item_row >= list_rect.bottom() {
            return None;
        }
        if item_inx >= list.len() {
            return None;
        }
    }
    None
}
