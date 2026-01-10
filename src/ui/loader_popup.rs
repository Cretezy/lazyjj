use ansi_to_tui::IntoText;
use anyhow::Result;
use ratatui::{
    Frame,
    crossterm::event::Event,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, BorderType, Clear},
};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};
use throbber_widgets_tui::{Throbber, ThrobberState};

use crate::{
    ComponentInputResult,
    commander::{CommandError, Commander},
    ui::{Component, ComponentAction, message_popup::MessagePopup, utils::centered_rect_fixed},
};

type OperationResult = Result<String, CommandError>;

pub struct LoaderPopup {
    operation_name: String,
    result_rx: Receiver<OperationResult>,
    throbber_state: ThrobberState,
    completed: bool,
    last_animation_update: Instant,
}

impl LoaderPopup {
    pub fn new<F>(operation_name: String, operation: F) -> Self
    where
        F: FnOnce() -> OperationResult + Send + 'static,
    {
        let (tx, rx): (Sender<OperationResult>, Receiver<OperationResult>) = mpsc::channel();

        // Spawn thread to run the operation
        thread::spawn(move || {
            let result = operation();
            tx.send(result)
        });

        Self {
            operation_name,
            result_rx: rx,
            throbber_state: ThrobberState::default(),
            completed: false,
            last_animation_update: Instant::now(),
        }
    }
}

impl Component for LoaderPopup {
    fn update(&mut self, _commander: &mut Commander) -> Result<Option<ComponentAction>> {
        if let Ok(result) = self.result_rx.try_recv() {
            self.completed = true;

            match result {
                Ok(output) if !output.is_empty() => {
                    return Ok(Some(ComponentAction::Multiple(vec![
                        ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                            title: format!("{} message", self.operation_name).into(),
                            messages: output.into_text()?,
                            text_align: None,
                        }))),
                        ComponentAction::RefreshTab(),
                    ])));
                }
                Ok(_) => {
                    return Ok(Some(ComponentAction::Multiple(vec![
                        ComponentAction::SetPopup(None),
                        ComponentAction::RefreshTab(),
                    ])));
                }
                Err(err) => {
                    return Ok(Some(ComponentAction::SetPopup(Some(Box::new(
                        MessagePopup {
                            title: format!("{} error", self.operation_name).into(),
                            messages: err.into_text("")?,
                            text_align: None,
                        },
                    )))));
                }
            }
        }

        if self.last_animation_update.elapsed() >= Duration::from_millis(100) {
            self.throbber_state.calc_next();
            self.last_animation_update = Instant::now();
        }

        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Green));

        let label = format!("{}...", self.operation_name);
        let content_width = 2 + label.len() as u16;
        let content_height = 1;

        let popup_width = content_width + 2;
        let popup_height = content_height + 2;

        let popup_area = centered_rect_fixed(area, popup_width, popup_height);
        f.render_widget(Clear, popup_area);
        f.render_widget(&block, popup_area);

        let inner = block.inner(popup_area);

        let throbber = Throbber::default().label(label).style(Style::default());
        f.render_stateful_widget(throbber, inner, &mut self.throbber_state);

        Ok(())
    }

    fn input(&mut self, _commander: &mut Commander, _event: Event) -> Result<ComponentInputResult> {
        // Block all input while loading
        Ok(ComponentInputResult::Handled)
    }
}
