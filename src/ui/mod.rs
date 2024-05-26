pub mod command_log;
pub mod details_panel;
pub mod files;
pub mod log;
pub mod message_popup;
pub mod utils;

use anyhow::Result;

use crossterm::event::Event;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    symbols, Frame,
};
use ratatui::{prelude::*, widgets::*};

use crate::{
    app::{App, Tab},
    commander::{log::Head, Commander},
    ComponentInputResult,
};

pub enum ComponentAction {
    ViewFiles(Head),
    ChangeHead(Head),
    SetTextAreaActive(bool),
    Multiple(Vec<ComponentAction>),
}

pub trait Component {
    // Called when switching to tab
    fn reset(&mut self, _commander: &mut Commander) -> Result<()> {
        Ok(())
    }

    fn update(&mut self, _commander: &mut Commander) -> Result<Option<ComponentAction>> {
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()>;

    fn input(&mut self, commander: &mut Commander, event: Event) -> Result<ComponentInputResult>;
}

impl App<'_> {
    // pub fn get_current_component(&self) -> &dyn Component {
    //     match self.current_tab {
    //         Tab::Log => &self.log,
    //         Tab::Files => &self.files,
    //         Tab::CommandLog => &self.command_log,
    //     }
    // }

    pub fn get_current_component_mut(&mut self) -> &mut dyn Component {
        match self.current_tab {
            Tab::Log => &mut self.log,
            Tab::Files => &mut self.files,
            Tab::CommandLog => &mut self.command_log,
        }
    }
}

pub fn ui(f: &mut Frame, app: &mut App) -> Result<()> {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(f.size());

    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    {
        let tabs = Tabs::new(
            Tab::VALUES
                .iter()
                .enumerate()
                .map(|(i, tab)| format!("[{}] {}", i + 1, tab)),
        )
        .block(
            Block::bordered()
                .title(" Tabs ")
                .border_type(BorderType::Rounded),
        )
        .highlight_style(Style::default().bg(app.env.config.highlight_color()))
        .select(
            Tab::VALUES
                .iter()
                .position(|tab| tab == &app.current_tab)
                .unwrap_or(0),
        )
        .divider(symbols::line::VERTICAL);

        f.render_widget(tabs, header_chunks[0]);
    }
    {
        let tabs = Paragraph::new("q: quit | R: refresh | 1/2/3: change tab")
            .fg(Color::DarkGray)
            .block(
                Block::bordered()
                    .title(" lazyjj ")
                    .border_type(BorderType::Rounded)
                    .fg(Color::default()),
            );

        f.render_widget(tabs, header_chunks[1]);
    }

    app.get_current_component_mut().draw(f, chunks[1])
}
