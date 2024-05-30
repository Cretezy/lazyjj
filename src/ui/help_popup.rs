use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Stylize,
    text::Span,
    widgets::{Block, Clear, Row, Table},
};

use crate::{
    ui::{styles::create_popup_block, utils::centered_rect, Component},
    ComponentInputResult,
};

pub struct HelpPopup {
    pub left_items: Vec<(String, String)>,
    pub right_items: Vec<(String, String)>,
    height: u16,
    scroll: usize,
}

impl HelpPopup {
    pub fn new(left_items: Vec<(String, String)>, right_items: Vec<(String, String)>) -> Self {
        Self {
            left_items,
            right_items,
            height: 0,
            // Can't use TableState as it's broken: https://github.com/ratatui-org/ratatui/issues/1179
            scroll: 0,
        }
    }

    fn create_table(&self, items: &[(String, String)], title: String) -> Table {
        let items: Vec<&(String, String)> = items.iter().skip(self.scroll).collect();

        let max_first_row_width = items.iter().map(|row| row.0.len()).max().unwrap_or(0);
        let rows: Vec<Row> = items
            .iter()
            .map(|row| Row::new([row.0.clone(), row.1.clone()]))
            .collect();
        let widths = [
            Constraint::Length(max_first_row_width as u16 + 2),
            Constraint::Fill(1),
        ];

        Table::new(rows, widths).block(Block::new().title(Span::from(title).bold()))
    }
}

impl Component for HelpPopup {
    fn draw(
        &mut self,
        f: &mut ratatui::prelude::Frame<'_>,
        area: ratatui::prelude::Rect,
    ) -> anyhow::Result<()> {
        let area = centered_rect(area, 60, 60);
        f.render_widget(Clear, area);

        let block = create_popup_block("Help");
        let block_inner = block.inner(area);
        self.height = block_inner.height;
        f.render_widget(&block, area);

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(2),
                Constraint::Fill(1),
            ])
            .split(block_inner);

        f.render_widget(
            self.create_table(&self.left_items, "Left panel".into()),
            chunks[0],
        );
        f.render_widget(
            self.create_table(&self.right_items, "Right panel".into()),
            chunks[2],
        );

        Ok(())
    }

    fn input(
        &mut self,
        _commander: &mut crate::commander::Commander,
        event: crossterm::event::Event,
    ) -> anyhow::Result<crate::ComponentInputResult> {
        if let Event::Key(key) = event
            && key.kind == event::KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('j') => {
                    let max = self.left_items.len().max(self.right_items.len());
                    self.scroll = (self.scroll + 1).min(max.saturating_sub(self.height as usize));
                }
                KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
                _ => return Ok(ComponentInputResult::NotHandled),
            }

            return Ok(ComponentInputResult::Handled);
        }

        Ok(ComponentInputResult::NotHandled)
    }
}
