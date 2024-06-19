use lazy_static::lazy_static;
use ratatui::{
    layout::Alignment,
    style::{Color, Style, Stylize},
    text::Span,
    widgets::{Block, BorderType, Padding},
};

lazy_static! {
    pub static ref POPUP_BLOCK: Block<'static> = Block::bordered()
        .padding(Padding::horizontal(1))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Green));
    pub static ref POPUP_BLOCK_TITLE_STYLE: Style = Style::new().bold().cyan();
}

pub fn create_popup_block(title: &str) -> Block {
    return POPUP_BLOCK
        .clone()
        .title(Span::styled(format!(" {title} "), *POPUP_BLOCK_TITLE_STYLE))
        .title_alignment(Alignment::Center);
}
