use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
    text::Text,
    widgets::{Paragraph, Wrap},
};

/// Details panel used for the right side of each tab.
/// This handles scrolling and wrapping.
pub struct DetailsPanel {
    pub scroll: u16,
    height: u16,
    lines: u16,
    wrap: bool,
}

impl DetailsPanel {
    pub fn new() -> Self {
        Self {
            scroll: 0,
            height: 0,
            lines: 0,
            wrap: true,
        }
    }

    /// Render the parent into the area.
    pub fn render<'a, T>(&mut self, content: T, area: Rect) -> Paragraph<'a>
    where
        T: Into<Text<'a>>,
    {
        let mut paragraph = Paragraph::new(content);

        if self.wrap {
            paragraph = paragraph.wrap(Wrap { trim: false });
        }

        self.height = area.height;
        self.lines = paragraph.line_count(area.width) as u16;

        paragraph = paragraph.scroll((self.scroll.min(self.lines.saturating_sub(1)), 0));

        paragraph
    }

    pub fn scroll(&mut self, scroll: isize) {
        self.scroll = (self.scroll.saturating_add_signed(scroll as i16)).min(self.lines - 1)
    }

    /// Handle input. Returns bool of if event was handled
    pub fn input(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => self.scroll(1),
            KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => self.scroll(-1),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll(self.height as isize / 2)
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll((self.height as isize / 2).saturating_neg())
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll(self.height as isize / 2)
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll((self.height as isize).saturating_neg())
            }
            KeyCode::Char('W') => {
                self.wrap = !self.wrap;
            }
            _ => return false,
        };

        true
    }
}
