use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Alignment,
    style::{Color, Style, Stylize},
    text::{Span, Text},
    widgets::{block::Title, BorderType, Borders},
};
use tui_confirm_dialog::PopupMessage;

pub struct MessagePopup<'a> {
    pub title: Title<'a>,
    pub messages: Text<'a>,
}

impl MessagePopup<'_> {
    /// Render the parent into the area.
    pub fn render(&self) -> PopupMessage {
        let mut title = self.title.clone();
        title.content.spans = [
            vec![Span::raw(" ")],
            title.content.spans,
            vec![Span::raw(" ")],
        ]
        .concat();

        title.content = title.content.fg(Color::Cyan).bold();

        let popup = tui_confirm_dialog::PopupMessage::new(title, self.messages.clone())
            .title_alignment(Alignment::Center)
            .text_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Green));

        popup
    }

    /// Handle input. Returns bool of if to close
    pub fn input(&self, key: KeyEvent) -> bool {
        matches!(
            key.code,
            KeyCode::Char('y') | KeyCode::Char('n') | KeyCode::Char('o') | KeyCode::Enter
        )
    }
}
