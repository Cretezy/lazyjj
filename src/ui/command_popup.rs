use crossterm::event::{Event, KeyCode};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    text::Span,
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};
use tui_textarea::TextArea;

use crate::{
    commander::Commander,
    ui::{
        message_popup::MessagePopup, utils::centered_rect_line_height, Component, ComponentAction,
    },
    ComponentInputResult,
};

pub struct CommandPopup<'a> {
    command_textarea: TextArea<'a>,
}

impl CommandPopup<'_> {
    pub fn new() -> Self {
        Self {
            command_textarea: TextArea::new(vec![]),
        }
    }
}

impl Component for CommandPopup<'_> {
    fn draw(
        &mut self,
        f: &mut ratatui::Frame<'_>,
        area: ratatui::prelude::Rect,
    ) -> anyhow::Result<()> {
        let block = Block::bordered()
            .title(Span::styled(" Command ", Style::new().bold().cyan()))
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

        f.render_widget(&self.command_textarea, popup_chunks[0]);

        let help = Paragraph::new(vec!["Enter: run | Escape: cancel".into()])
            .fg(Color::DarkGray)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::DarkGray)),
            );

        f.render_widget(help, popup_chunks[1]);
        Ok(())
    }

    fn input(
        &mut self,
        commander: &mut Commander,
        event: crossterm::event::Event,
    ) -> anyhow::Result<ComponentInputResult> {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Enter => {
                    let command_input = self.command_textarea.lines().join(" ");
                    let command = command_input.split_whitespace().collect::<Vec<&str>>();
                    let res = commander.execute_jj_command(command, false, false);
                    let message = match res {
                        Ok(str) => str,
                        Err(err) => [
                            format!("Failed to execute jj command: jj {}", command_input,),
                            err.to_string(),
                        ]
                        .join("\n"),
                    };

                    if message.trim().is_empty() {
                        return Ok(ComponentInputResult::HandledAction(
                            ComponentAction::Multiple(vec![
                                ComponentAction::SetPopup(None),
                                ComponentAction::RefreshTab(),
                            ]),
                        ));
                    }

                    return Ok(ComponentInputResult::HandledAction(
                        ComponentAction::Multiple(vec![
                            ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                                title: format!("jj {}", command_input).into(),
                                messages: message.into(),
                                text_align: Alignment::Left.into(),
                            }))),
                            ComponentAction::RefreshTab(),
                        ]),
                    ));
                }
                KeyCode::Esc => {
                    return Ok(ComponentInputResult::HandledAction(
                        ComponentAction::SetPopup(None),
                    ));
                }
                _ => {}
            }
        };
        self.command_textarea.input(event);
        Ok(ComponentInputResult::Handled)
    }
}
