use std::sync::LazyLock;

use ratatui::{
    layout::Alignment,
    style::{Color, Style, Stylize},
    text::Span,
    widgets::{Block, BorderType, Padding},
};

pub static POPUP_BLOCK: LazyLock<Block<'static>> = LazyLock::new(|| {
    Block::<'static>::bordered()
        .padding(Padding::horizontal(1))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Green))
});
pub static POPUP_BLOCK_TITLE_STYLE: LazyLock<Style> = LazyLock::new(|| Style::new().bold().cyan());

pub fn create_popup_block(title: &str) -> Block<'_> {
    POPUP_BLOCK
        .clone()
        .title(Span::styled(format!(" {title} "), *POPUP_BLOCK_TITLE_STYLE))
        .title_alignment(Alignment::Center)
}
