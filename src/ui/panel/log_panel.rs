/*! The log panel shows the list of changes on the left side of the
log tab. */

use ansi_to_tui::IntoText;
use anyhow::Result;
use ratatui::{
    crossterm::event::{Event, MouseEvent, MouseEventKind},
    layout::Rect,
    prelude::*,
    text::ToText,
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

/**
    A panel that displays the output of jj log.
    This panel is used on the left side of the log tab.
    It shows a selected change, which is expanded
    on the right side of the log tab.

    The log operates with two index:
    - line index (into self.log_output.text)
    - head index (into self.log_output.heads)

    The line index is used for scrolling at the display leve.

    The head index is used for scrolling at the user level
    as well as for selecting which lines to highlight.
*/
pub struct LogPanel<'a> {
    log_output: Result<LogOutput, CommandError>,
    log_output_text: Text<'a>,
    log_list_state: ListState,
    pub log_rect: Rect,

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

impl<'a> LogPanel<'a> {
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
            log_rect: Rect::ZERO,

            log_revset,

            head,

            panel_rect: Rect::ZERO,

            config: commander.env.config.clone(),
        })
    }

    //
    //  Handle jj log output
    //

    /// Run jj log and store output for display
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
                if let Some(line_change) = line_head
                    && line_change == &self.head
                {
                    set_bg(&mut line, self.config.highlight_color());
                };

                line
            })
            .collect()
    }

    /// Get lines to show in log list
    fn log_lines(&self) -> Vec<Line<'a>> {
        match self.log_output.as_ref() {
            Ok(log_output) => self.output_to_lines(log_output),
            Err(err) => err.into_text("Error getting log").unwrap().lines,
        }
    }

    //
    //  Selected head and the special head index
    //

    /// Find the line in self.log_output that match self.head
    fn selected_log_line(&self) -> Option<usize> {
        let log_output = self.log_output.as_ref().ok()?;

        log_output
            .graph_heads
            .iter()
            .position(|opt_h| opt_h.as_ref().is_some_and(|h| h == &self.head))
    }

    /// Find head of the provided log_output line
    fn head_at_log_line(&mut self, log_line: usize) -> Option<Head> {
        let log_output = self.log_output.as_ref().ok()?;

        let graph_head = log_output.graph_heads.get(log_line)?;

        graph_head.clone()
    }

    // Return the head-index for the selection
    fn get_current_head_index(&self) -> Option<usize> {
        get_head_index(&self.head, &self.log_output)
    }

    /// Number of log list items that fit on screen. Think of this as
    /// in unit head-index. Moving the head-index this much causes a
    /// full page scroll.
    fn visible_heads(&self) -> u16 {
        // Every item in the log list is 2 lines high, so divide screen rows
        // by 2 to get the number of log items that fit in it.
        self.log_rect.height / 2
    }

    /// Move selection to a specific head. This may cause the next draw to
    /// scroll to a different line.
    pub fn set_head(&mut self, head: Head) {
        head.clone_into(&mut self.head);
    }

    /// Move selection relative to the current position.
    /// The scroll is relative to head-index, not line-index.
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

    //
    //  Event handling
    //

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
                self.scroll_relative(commander, self.visible_heads() as isize / 2);
            }
            LogTabEvent::ScrollUpHalf => {
                self.scroll_relative(
                    commander,
                    (self.visible_heads() as isize / 2).saturating_neg(),
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
        self.panel_rect = area;

        let title = match &self.log_revset {
            Some(log_revset) => &format!(" Log for: {log_revset} "),
            None => " Log ",
        };

        let log_lines = self.log_lines();
        let log_length: usize = log_lines.len();
        let log_block = Block::bordered()
            .title(title)
            .border_type(BorderType::Rounded);
        self.log_rect = log_block.inner(area);
        self.log_list_state.select(self.selected_log_line());
        let log = List::new(log_lines).block(log_block).scroll_padding(7);
        f.render_stateful_widget(log, area, &mut self.log_list_state);

        // Show scrollbar if lines don't fit the screen height
        if log_length > self.log_rect.height.into() {
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

    fn input(&mut self, commander: &mut Commander, event: Event) -> Result<ComponentInputResult> {
        if let Event::Mouse(mouse_event) = event {
            // Determine if mouse event is inside log-view
            let mouse_pos = Position::new(mouse_event.column, mouse_event.row);
            if !self.panel_rect.contains(mouse_pos) {
                return Ok(ComponentInputResult::NotHandled);
            }

            // Execute command dependent on panel and event kind
            match mouse_event.kind {
                MouseEventKind::ScrollUp => {
                    self.handle_event(commander, LogTabEvent::ScrollUp)?;
                    return Ok(ComponentInputResult::Handled);
                }
                MouseEventKind::ScrollDown => {
                    self.handle_event(commander, LogTabEvent::ScrollDown)?;
                    return Ok(ComponentInputResult::Handled);
                }
                MouseEventKind::Up(_) => {
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
                    ) && let Some(head) = self.head_at_log_line(inx)
                    {
                        self.set_head(head);
                    }
                }
                _ => {} // Handle other mouse events if necessary
            }
        }

        Ok(ComponentInputResult::NotHandled)
    }
}

// Determine which list item a mouse event is related to
fn list_item_from_mouse_event(
    list: &[ListItem],
    list_rect: Rect,
    list_state: &ListState,
    mouse_event: &MouseEvent,
) -> Option<usize> {
    let mouse_pos = Position::new(mouse_event.column, mouse_event.row);
    if !list_rect.contains(mouse_pos) {
        return None;
    }

    // Assume that each item is exactly one line.
    // This is not true in the general case, but it is in this module.
    let mouse_offset = mouse_pos.y - list_rect.y;
    let item_index = list_state.offset() + mouse_offset as usize;
    if item_index >= list.len() {
        return None;
    }
    Some(item_index)
}
