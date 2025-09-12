/*! The log panel shows the list of changes on the left side of the
log tab. */

use ansi_to_tui::IntoText;
use anyhow::Result;
use ratatui::{
    crossterm::event::{
        Event,
        /*
        KeyEventKind, MouseEvent, MouseEventKind
        */
    },
    layout::Rect,
    prelude::*,
    widgets::*,
};

use crate::{
    commander::{
        CommandError, Commander,
        log::{Head, LogOutput},
    },
    env::Config,
    keybinds::{LogTabEvent, LogTabKeybinds},
    ui::Component,
    ui::ComponentAction,
    ui::ComponentInputResult,
};

pub struct LogPanel<'a> {
    log_output: Result<LogOutput, CommandError>,
    log_output_text: Text<'a>,
    log_list_state: ListState,
    pub log_height: u16,

    /// The revision set to show in the log
    pub log_revset: Option<String>,

    /// Currently selected change
    pub head: Head,

    /// Rect used last time draw was called. Can be used to check if mouse clicks
    panel_rect: Rect,

    config: Config,
}

/*
pub enum LogPanelEvent {
    /* Commands to LogPanel */

    /// Refresh current state
    Refresh,
    /// Move selection down the given number of changes
    MoveRelative(isize),

    /* Notifications from LogPanel */

    /// Emitted when selection was changed
    SetHead(Head),
}
*/

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

impl LogPanel<'_> {
    pub fn new(commander: &mut Commander) -> Result<Self> {
        let log_revset = commander.env.default_revset.clone();
        let log_output = commander.get_log(&log_revset);
        let head = commander.get_current_head()?;

        let log_list_state = ListState::default().with_selected(get_head_index(&head, &log_output));

        let mut keybinds = LogTabKeybinds::default();
        if let Some(new_keybinds) = commander
            .env
            .config
            .keybinds()
            .and_then(|k| k.log_tab.clone())
        {
            keybinds.extend_from_config(&new_keybinds);
        }

        let log_output_text = match log_output.as_ref() {
            Ok(log_output) => log_output
                .graph
                .into_text()
                .unwrap_or(Text::from("Could not turn text into TUI text (coloring)")),
            Err(_) => Text::default(),
        };

        Ok(Self {
            log_output_text,
            log_output,
            log_list_state,
            log_height: 0,

            log_revset,

            head,

            panel_rect: Rect::ZERO,

            config: commander.env.config.clone(),
        })
    }
    pub fn refresh_log_output(&mut self, commander: &mut Commander) {
        self.log_output = commander.get_log(&self.log_revset);
        self.log_output_text = match self.log_output.as_ref() {
            Ok(log_output) => log_output
                .graph
                .into_text()
                .unwrap_or(Text::from("Could not turn text into TUI text (coloring)")),
            Err(_) => Text::default(),
        };
    }

    fn get_current_head_index(&self) -> Option<usize> {
        get_head_index(&self.head, &self.log_output)
    }

    pub fn set_head(&mut self, head: Head) {
        head.clone_into(&mut self.head);
    }

    /// Move selection relative to the current position.
    /// This will update self.head
    fn scroll_relative(&mut self, _commander: &mut Commander, scroll: isize) {
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
            self.set_head(next_head.clone());
        }
        // TODO Notify about change of head
    }

    pub fn handle_event(
        &mut self,
        commander: &mut Commander,
        log_tab_event: LogTabEvent,
    ) -> Result<ComponentInputResult> {
        match log_tab_event {
            LogTabEvent::ScrollDown => {
                self.scroll_relative(commander, 1);
            }
            LogTabEvent::ScrollUp => {
                self.scroll_relative(commander, -1);
            }
            LogTabEvent::ScrollDownHalf => {
                self.scroll_relative(commander, self.log_height as isize / 2 / 2);
            }
            LogTabEvent::ScrollUpHalf => {
                self.scroll_relative(
                    commander,
                    (self.log_height as isize / 2 / 2).saturating_neg(),
                );
            }
            _ => {
                return Ok(ComponentInputResult::NotHandled);
            }
        }
        Ok(ComponentInputResult::Handled)
    }
}

impl Component for LogPanel<'_> {
    // Called when switching to tab
    fn focus(&mut self, _commander: &mut Commander) -> Result<()> {
        Ok(())
    }

    fn update(&mut self, _commander: &mut Commander) -> Result<Option<ComponentAction>> {
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
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
                            .as_ref()
                            .is_some_and(|h| h == &self.head)
                    }));

                log_lines
            }
            Err(err) => err.into_text("Error getting log")?.lines,
        };

        let title = match &self.log_revset {
            Some(log_revset) => &format!(" Log for: {log_revset} "),
            None => " Log ",
        };

        let log_length: usize = log_lines.len();
        let log_block = Block::bordered()
            .title(title)
            .border_type(BorderType::Rounded);
        self.log_height = log_block.inner(area).height;
        let log = List::new(log_lines).block(log_block).scroll_padding(7);
        f.render_stateful_widget(log, area, &mut self.log_list_state);

        // Show scrollbar if lines don't fit the screen height
        if log_length > self.log_height.into() {
            let index = self.log_list_state.selected().unwrap_or(0);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
            let mut scrollbar_state = ScrollbarState::default()
                .content_length(log_length)
                .position(index);

            f.render_stateful_widget(
                scrollbar,
                area.inner(Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut scrollbar_state,
            );
        }

        Ok(())
    }

    fn input(&mut self, _commander: &mut Commander, _event: Event) -> Result<ComponentInputResult> {
        Ok(ComponentInputResult::NotHandled)
    }
}
