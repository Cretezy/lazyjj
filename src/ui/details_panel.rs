use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Margin, Rect},
    text::{Line, Text},
    widgets::{
        Block, BorderType, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Wrap,
    },
};

/// Details panel used for the right side of each tab.
/// This handles scrolling and wrapping.
pub struct DetailsPanel {
    scroll: u16,
    height: u16,
    lines: u16,
    wrap: bool,
}

/// Transient object holding render data
pub struct DetailsPanelRenderContext<'a> {
    panel: &'a mut DetailsPanel,
    title: Option<Line<'a>>,
    content: Option<Text<'a>>,
}

/// Commands that can be handled by the details panel
pub enum DetailsPanelEvent {
    ScrollDown,
    ScrollUp,
    ScrollDownHalfPage,
    ScrollUpHalfPage,
    ScrollDownPage,
    ScrollUpPage,
    ToggleWrap,
}

impl<'a> DetailsPanelRenderContext<'a> {
    pub fn new(panel: &'a mut DetailsPanel) -> Self {
        Self {
            panel,
            title: None,
            content: None,
        }
    }
    /// Set the title on the frame that surrounds the content
    pub fn title<T>(&mut self, title: T) -> &mut Self
    where
        T: Into<Line<'a>>,
    {
        self.title = Some(title.into());
        self
    }
    /// Set the text inside the panel
    pub fn content<T>(&mut self, content: T) -> &mut Self
    where
        T: Into<Text<'a>>,
    {
        self.content = Some(content.into());
        self
    }

    pub fn draw(&mut self, f: &mut ratatui::prelude::Frame<'_>, area: ratatui::prelude::Rect) {
        // Define border block
        let mut border = Block::bordered()
            .border_type(BorderType::Rounded)
            .padding(Padding::horizontal(1));
        // Apply title if provided
        if let Some(title) = &self.title {
            border = border.title_top(title.clone());
        }

        // Find text inside border
        let content_text = match &self.content {
            Some(text) => text,
            None => &Text::raw(""),
        };
        // Create content widget that uses border
        let paragraph_area = border.inner(area);
        let paragraph = self
            .panel
            .render(content_text.clone(), paragraph_area)
            .block(border);

        // render content and border
        f.render_widget(paragraph, area);

        // render scrollbar on top of border
        if self.panel.lines > paragraph_area.height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);

            let mut scrollbar_state =
                ScrollbarState::new(self.panel.lines.into()).position(self.panel.scroll.into());

            f.render_stateful_widget(
                scrollbar,
                area.inner(Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut scrollbar_state,
            );
        }
    }
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

    pub fn render_context(&mut self) -> DetailsPanelRenderContext<'_> {
        DetailsPanelRenderContext::new(self)
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

    pub fn scroll_to(&mut self, line_no: u16) {
        self.scroll = line_no.min(self.lines.saturating_sub(1))
    }

    pub fn scroll(&mut self, scroll: isize) {
        self.scroll_to(self.scroll.saturating_add_signed(scroll as i16))
    }

    pub fn handle_event(&mut self, details_panel_event: DetailsPanelEvent) {
        match details_panel_event {
            DetailsPanelEvent::ScrollDown => self.scroll(1),
            DetailsPanelEvent::ScrollUp => self.scroll(-1),
            DetailsPanelEvent::ScrollDownHalfPage => self.scroll(self.height as isize / 2),
            DetailsPanelEvent::ScrollUpHalfPage => {
                self.scroll((self.height as isize / 2).saturating_neg())
            }
            DetailsPanelEvent::ScrollDownPage => self.scroll(self.height as isize),
            DetailsPanelEvent::ScrollUpPage => self.scroll((self.height as isize).saturating_neg()),
            DetailsPanelEvent::ToggleWrap => self.wrap = !self.wrap,
        }
    }

    /// Handle input. Returns bool of if event was handled
    pub fn input(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.handle_event(DetailsPanelEvent::ScrollDown)
            }
            KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.handle_event(DetailsPanelEvent::ScrollUp)
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.handle_event(DetailsPanelEvent::ScrollDownHalfPage)
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.handle_event(DetailsPanelEvent::ScrollUpHalfPage)
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.handle_event(DetailsPanelEvent::ScrollDownPage)
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.handle_event(DetailsPanelEvent::ScrollUpPage)
            }
            KeyCode::Char('W') => self.handle_event(DetailsPanelEvent::ToggleWrap),
            _ => return false,
        };

        true
    }
}
