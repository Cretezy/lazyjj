/*! The rebase popup allows the user to pick a rebase configuration and
 start rebase, or cancel the opreation.

 The UI looks like this
 ~~~
    Source   (zsztoxlv)
    ( ) -s this and descendants
    ( ) -b whole branch
    (*) -r only one change moves
    Target @ (umrpslui)
    (*) -d rebase onto @ as new branch
    ( ) -A rebase after @
    ( ) -B rebase before @

    Esc: Cancel    Enter: Rebase
~~~
It has keyboard shortcuts s, b, r, d, shift+a, shift+b for selecting
a radiobutton, and shortcuts Enter, Esc, q for closing the popup.


*/

use anyhow::Result;
use ratatui::{
    Frame,
    crossterm::event::Event,
    layout::{Alignment, Rect},
    prelude::{Buffer, Constraint, Direction, Layout, Size},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Clear, Paragraph, StatefulWidget},
};

use crate::{
    ComponentInputResult,
    commander::{Commander, ids::CommitId},
    keybinds::rebase_popup::{CutOption, PasteOption, PopupAction},
    ui::Component,
};

type Keybinds = crate::keybinds::rebase_popup::Keybinds;

/// A transient popup for configuring a rebase command
pub struct RebasePopup {
    pub keybinds: Keybinds,

    pub source_change: CommitId,
    pub target_change: CommitId,

    pub source_mode: CutOption,
    pub target_mode: PasteOption,
}

impl RebasePopup {
    pub fn new(source_change: CommitId, target_change: CommitId) -> Self {
        Self {
            keybinds: Keybinds::default(),
            source_change,
            target_change,
            source_mode: CutOption::SingleRevision,
            target_mode: PasteOption::NewBranch,
        }
    }

    /// Collect all the rendering code that would have been in
    /// log_tab.rs/draw
    pub fn render_widget(&mut self, frame: &mut Frame) {
        let area = center_rect(
            frame.area(),
            Size {
                width: 32,
                height: 12,
            },
        );
        self.draw(frame, area)
            .expect("Expected drawing without failues");
    }

    /// Map an event to a popup action
    // TODO: This should be done by a custom keybinds object
    fn match_event(&self, event: Event) -> PopupAction {
        if let Event::Key(key) = event {
            return self.keybinds.match_event(key);
        }
        PopupAction::None
    }

    /// Run the command that the popup is currently configured to do
    fn run_command(&self, commander: &mut Commander) {
        let src_rev = self.source_change.as_str();
        let tgt_rev = self.target_change.as_str();
        let src_mode = match self.source_mode {
            CutOption::IncludeDescendants => "-s",
            CutOption::IncludeBranch => "-b",
            CutOption::SingleRevision => "-r",
        };
        let tgt_mode = match self.target_mode {
            PasteOption::NewBranch => "-d",
            PasteOption::InsertAfter => "-A",
            PasteOption::InsertBefore => "-B",
        };
        commander
            .run_rebase(src_mode, src_rev, tgt_mode, tgt_rev)
            .expect("jj rebase  should run without errors");
    }

    /// Process the input event. If this function returns true,
    /// then the popup should be closed. Either a rebase was executed
    /// or the operation was cancelled.
    pub fn handle_input(&mut self, commander: &mut Commander, event: Event) -> bool {
        match self.match_event(event) {
            PopupAction::Ok => {
                self.run_command(commander);
                return true;
            }
            PopupAction::Cancel => return true,
            PopupAction::SetSourceMode(m) => self.source_mode = m,
            PopupAction::SetTargetMode(m) => self.target_mode = m,
            PopupAction::None => (),
        }
        false
    }
}

impl Component for RebasePopup {
    /// Render the dialog into the area.
    fn draw(&mut self, frame: &mut Frame<'_>, area: Rect) -> Result<()> {
        // The border of the dialog
        let block = Block::bordered()
            .title(Span::styled(" Rebase ", Style::new().bold().cyan()))
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Green));
        frame.render_widget(Clear, area);
        frame.render_widget(&block, area);

        // Split area into chunks. Even though the area size is constant,
        // we pretend it can change in the future.
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .vertical_margin(1)
            .horizontal_margin(2)
            .constraints(
                [
                    Constraint::Length(1), // title "Source"
                    Constraint::Min(3),    // buttons for source mode
                    Constraint::Length(1), // title "Target"
                    Constraint::Min(3),    // buttons for target mode
                    Constraint::Length(2), // help text
                ]
                .as_ref(),
            )
            .split(area);

        // Radio buttons for source
        let src_rev: String = self.source_change.as_str().chars().take(8).collect();
        let src_options = vec![
            "-s this and descendants",
            "-b whole branch",
            "-r only one change moves",
        ];
        let mut src_select: usize = match self.source_mode {
            CutOption::IncludeDescendants => 0,
            CutOption::IncludeBranch => 1,
            CutOption::SingleRevision => 2,
        };
        frame.render_widget(
            Paragraph::new(Span::raw(format!("Source @ ({src_rev})"))),
            chunks[0],
        );
        frame.render_stateful_widget(RadioButton::new(src_options), chunks[1], &mut src_select);

        // Radio buttons for target
        let tgt_rev: String = self.target_change.as_str().chars().take(8).collect();
        let tgt_options = vec![
            "-d rebase as new branch",
            "-A rebase after",
            "-B rebase before",
        ];
        let mut tgt_select: usize = match self.target_mode {
            PasteOption::NewBranch => 0,
            PasteOption::InsertAfter => 1,
            PasteOption::InsertBefore => 2,
        };
        frame.render_widget(
            Paragraph::new(Span::raw(format!("Target ({tgt_rev})"))),
            chunks[2],
        );
        frame.render_stateful_widget(RadioButton::new(tgt_options), chunks[3], &mut tgt_select);

        // Help on terminating dialog
        frame.render_widget(
            Paragraph::new(Text::from(vec![
                Line::raw(""),
                Line::raw("Esc: Cancel    Enter: Rebase"),
            ])),
            chunks[4],
        );

        Ok(())
    }

    fn input(&mut self, _commander: &mut Commander, _event: Event) -> Result<ComponentInputResult> {
        unreachable!();
        //return Ok(ComponentInputResult::Handled);
    }
}

/****************************************************************/
// TODO: Move this function to ui::utils

/// Find a rect of the given size at the center of an outside rect
fn center_rect(outside: Rect, area: Size) -> Rect {
    Rect {
        x: outside.x + (outside.width - area.width) / 2,
        y: outside.y + (outside.height - area.height) / 2,
        width: area.width,
        height: area.height,
    }
}

/****************************************************************/
// TODO: Move this widget to a separate file

/** A widget for a group of radio buttons.

It is a stateful widget.
The state is an usize number that indicates which label is
selected.

Example:
~~~
( ) apples
( ) bananas
(*) lemons
~~~
*/
struct RadioButton {
    /// Button labels
    pub labels: Vec<String>,
    /// Button style can be modified before drawing
    pub button_style: Style,
    /// Label style can be modified before drawing
    pub label_style: Style,
}

impl RadioButton {
    pub fn new(labels: Vec<&str>) -> Self {
        let button_style = Style::default().fg(Color::White);
        let label_style = Style::default().fg(Color::White);
        Self {
            labels: labels.iter().map(|s| s.to_string()).collect(),
            button_style,
            label_style,
        }
    }
}

impl StatefulWidget for RadioButton {
    type State = usize;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        for (row, label) in self.labels.iter().enumerate() {
            let button = if row == *state { "(*)" } else { "( )" };
            buf.set_string(
                area.left(),
                area.top() + row as u16,
                button,
                self.button_style,
            );
            buf.set_string(
                area.left() + 4_u16,
                area.top() + row as u16,
                label,
                self.label_style,
            );
        }
    }
}
