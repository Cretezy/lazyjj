use anyhow::Result;
use ratatui::{
    crossterm::event::Event,
    layout::{Alignment, Rect},
    style::{Color, Style, Stylize},
    text::{Span, Text},
    widgets::{block::Title, BorderType, Borders},
    Frame,
};
use tui_confirm_dialog::PopupMessage;

use crate::{commander::Commander, ui::Component, ComponentInputResult};

pub struct MessagePopup<'a> {
    pub title: Title<'a>,
    pub messages: Text<'a>,
    pub text_align: Option<Alignment>,
}

impl Component for MessagePopup<'_> {
    /// Render the parent into the area.
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let mut title = self.title.clone();
        title.content.spans = [
            vec![Span::raw(" ")],
            title.content.spans,
            vec![Span::raw(" ")],
        ]
        .concat();

        title.content = title.content.fg(Color::Cyan).bold();

        let text_align = match self.text_align {
            Some(align) => align,
            None => Alignment::Center,
        };

        // TODO: Support scrolling long messages
        let popup = PopupMessage::new(title, self.messages.clone())
            .title_alignment(Alignment::Center)
            .text_alignment(text_align)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Green));

        f.render_widget(popup, area);

        Ok(())
    }

    fn input(&mut self, _commander: &mut Commander, _event: Event) -> Result<ComponentInputResult> {
        Ok(ComponentInputResult::NotHandled)
    }
}
