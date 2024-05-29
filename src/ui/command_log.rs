use std::borrow::Borrow;

use anyhow::Result;

use ansi_to_tui::IntoText;
use crossterm::event::{Event, KeyCode};
use ratatui::{prelude::*, widgets::*};

use crate::{
    commander::{CommandLogItem, Commander},
    env::Config,
    ui::{details_panel::DetailsPanel, Component},
    ComponentInputResult,
};

/// Command log tab. Shows list of commands exectured by lazyjj in left panel and selected command
/// output in right panel
pub struct CommandLog {
    command_history: Vec<CommandLogItem>,
    commands_list_state: ListState,
    commands_height: u16,

    output_panel: DetailsPanel,

    config: Config,
}

impl CommandLog {
    pub fn new(commander: &mut Commander) -> Result<Self> {
        let command_history = commander.command_history.clone();
        let selected_index = command_history.first().map(|_| 0);
        let commands_list_state = ListState::default().with_selected(selected_index);

        Ok(Self {
            commands_height: 0,
            commands_list_state,
            command_history,
            output_panel: DetailsPanel::new(),
            config: commander.env.config.clone(),
        })
    }

    pub fn get_output_lines<'a>(&self) -> Result<Vec<Line<'a>>> {
        let mut output_lines = vec![];

        if let Some(selected_index) = self.commands_list_state.selected()
            && let Some(command) = self.command_history.iter().rev().nth(selected_index)
        {
            match command.output.clone().borrow() {
                Ok(output) => {
                    output_lines.push(Line::default().spans([
                        "Command: ".into(),
                        Span::raw(command.program.to_owned()).fg(Color::Blue),
                        " ".into(),
                        command.args.join(" ").fg(Color::Blue),
                    ]));
                    output_lines.push(Line::default().spans([
                        ("Status code: ").into(),
                        output.status.code().map_or("?".into(), |code| {
                            Span::raw(code.to_string()).fg(if code > 0 {
                                Color::Red
                            } else {
                                Color::Yellow
                            })
                        }),
                    ]));
                    output_lines.push(
                        Line::default().spans([
                            Span::raw("Time: "),
                            Span::raw(command.time.format("%Y-%m-%d %H:%M:%S").to_string())
                                .fg(Color::Cyan),
                        ]),
                    );
                    output_lines.push(
                        Line::default().spans([
                            Span::raw("Duration: "),
                            Span::raw(format!("{}ms", command.duration.num_milliseconds()))
                                .fg(Color::Cyan),
                        ]),
                    );
                    output_lines.push(Line::default());

                    let mut has_output = false;

                    let stdout = &mut String::from_utf8_lossy(&output.stdout);
                    if !(stdout.is_empty()) {
                        output_lines.push(
                            Line::default().spans([Span::raw("Output:").fg(Color::Green).bold()]),
                        );
                        output_lines.push(Line::default());
                        output_lines.append(&mut stdout.as_ref().into_text()?.lines);
                        has_output = true;
                    }

                    let stderr = &mut String::from_utf8_lossy(&output.stderr);
                    if !stdout.is_empty() && !stderr.is_empty() {
                        output_lines.push(Line::default());
                        output_lines.push(Line::default());
                    }

                    if !(stderr.is_empty()) {
                        output_lines.push(
                            Line::default()
                                .spans([Span::raw("Error output:").fg(Color::Green).bold()]),
                        );
                        output_lines.push(Line::default());
                        output_lines.append(&mut stderr.as_ref().into_text()?.lines);
                        has_output = true;
                    }

                    if !has_output {
                        output_lines.push(
                            Line::default()
                                .spans([Span::raw("No output").fg(Color::DarkGray).italic()]),
                        );
                    }
                }
                Err(err) => {
                    output_lines.push(Line::default().spans(["Error: ".into(), err.to_string()]))
                }
            }
        }

        Ok(output_lines)
    }

    fn scroll_commands(&mut self, scroll: isize) {
        *self.commands_list_state.selected_mut() = Some(
            (self
                .commands_list_state
                .selected()
                .map(|selected_index| selected_index.saturating_add_signed(scroll))
                .unwrap_or(0))
            .min(self.command_history.len() - 1)
            .max(0),
        );
        self.output_panel.scroll = 0;
    }
}

#[allow(clippy::invisible_characters)]
impl Component for CommandLog {
    fn switch(&mut self, commander: &mut Commander) -> Result<()> {
        let command_history = commander.command_history.clone();
        let selected_index = command_history.first().map(|_| 0);
        self.commands_list_state.select(selected_index);
        self.command_history = command_history;
        Ok(())
    }

    fn draw(
        &mut self,
        f: &mut ratatui::prelude::Frame<'_>,
        area: ratatui::prelude::Rect,
    ) -> Result<()> {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Draw commands
        {
            let panel_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Fill(1), Constraint::Length(2)])
                .split(chunks[0]);

            let command_lines = self
                .command_history
                .iter()
                .rev()
                .enumerate()
                .map(|(i, command)| {
                    let mut line = Line::default()
                        .spans([
                            Span::raw(command.program.clone()),
                            " ".into(),
                            command.args.join(" ").into(),
                        ])
                        .fg(
                            if command
                                .output
                                .as_ref()
                                .as_ref()
                                .map_or(false, |output| output.status.success())
                            {
                                Color::Blue
                            } else {
                                Color::Red
                            },
                        );

                    if let Some(selected_index) = self.commands_list_state.selected()
                        && i == selected_index
                    {
                        line = line.bg(self.config.highlight_color());
                    }

                    line
                })
                .collect::<Vec<Line>>();

            let commands = List::new(command_lines)
                .block(
                    Block::bordered()
                        .title(" Commands ")
                        .border_type(BorderType::Rounded),
                )
                .scroll_padding(3);

            f.render_stateful_widget(commands, panel_chunks[0], &mut self.commands_list_state);
            self.commands_height = chunks[0].height.saturating_sub(2);

            let help = Paragraph::new(vec![
                "j/k: scroll down/up | J/K: scroll down by ½ page".into(),
                "@: latest command".into(),
            ])
            .fg(Color::DarkGray);
            f.render_widget(help, panel_chunks[1]);
        }

        // Draw output
        {
            let panel_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Fill(1), Constraint::Length(2)])
                .split(chunks[1]);

            let output_block = Block::bordered()
                .title(" Output ")
                .border_type(BorderType::Rounded);
            let output = self
                .output_panel
                .render(self.get_output_lines()?, output_block.inner(chunks[1]))
                .block(output_block);

            f.render_widget(output, panel_chunks[0]);

            let help = Paragraph::new(vec![
                "Ctrl+e/Ctrl+y: scroll down/up | Ctrl+d/Ctrl+u: scroll down/up by ½ page".into(),
                "Ctrl+f/Ctrl+b: scroll down/up by page | W: toggle wrapping".into(),
            ])
            .fg(Color::DarkGray);
            f.render_widget(help, panel_chunks[1]);
        }

        Ok(())
    }

    #[allow(clippy::collapsible_if)]
    fn input(&mut self, _commander: &mut Commander, event: Event) -> Result<ComponentInputResult> {
        if let Event::Key(key) = event {
            if self.output_panel.input(key) {
                return Ok(ComponentInputResult::Handled);
            }

            match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.scroll_commands(1);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.scroll_commands(-1);
                }
                KeyCode::Char('J') => {
                    self.scroll_commands(self.commands_height as isize / 2);
                }
                KeyCode::Char('K') => {
                    self.scroll_commands((self.commands_height as isize / 2).saturating_neg());
                }
                KeyCode::Char('@') => {
                    self.scroll_commands(isize::MIN);
                }
                _ => return Ok(ComponentInputResult::NotHandled),
            };
        }

        Ok(ComponentInputResult::Handled)
    }
}
