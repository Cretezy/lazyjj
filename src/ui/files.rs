use anyhow::Result;

use crate::{
    commander::{
        files::{Conflict, File},
        log::Head,
        Commander,
    },
    env::{Config, DiffFormat},
    ui::{details_panel::DetailsPanel, Component, ComponentAction},
};

use ansi_to_tui::IntoText;
use crossterm::event::{Event, KeyCode};
use ratatui::{prelude::*, widgets::*};

/// Files tab. Shows files in selected change in left panel and selected file diff in right panel
pub struct Files {
    head: Head,
    is_current_head: bool,

    files_output: Vec<File>,
    conflicts_output: Vec<Conflict>,
    files_list_state: ListState,
    files_height: u16,

    pub file: Option<String>,
    diff_panel: DetailsPanel,
    diff_output: Option<String>,
    diff_format: DiffFormat,

    config: Config,
}

fn get_current_file_index(current_file: &Option<String>, files_output: &[File]) -> Option<usize> {
    current_file.as_ref().and_then(|current_file| {
        files_output.iter().position(|file| {
            file.path
                .as_ref()
                .map_or(false, |path| path == current_file)
        })
    })
}

impl Files {
    pub fn new(commander: &mut Commander, head: &Head) -> Result<Self> {
        let head = head.clone();
        let is_current_head = head == commander.get_current_head()?;

        let diff_format = commander.env.config.diff_format();

        let files_output = commander.get_files(&head)?;
        let conflicts_output = commander.get_conflicts(&head)?;
        let current_file = files_output.first().and_then(|change| change.path.clone());
        let diff_output = current_file
            .as_ref()
            .map(|current_change| commander.get_file_diff(&head, current_change, &diff_format))
            .map_or(Ok(None), |r| r.map(Some))?;

        let files_list_state = ListState::default()
            .with_selected(get_current_file_index(&current_file, &files_output));

        Ok(Self {
            head,
            is_current_head,

            files_output,
            file: current_file,
            files_list_state,
            files_height: 0,

            conflicts_output,

            diff_output,
            diff_format,
            diff_panel: DetailsPanel::new(),

            config: commander.env.config.clone(),
        })
    }

    pub fn set_head(&mut self, commander: &mut Commander, head: &Head) -> Result<()> {
        self.head = head.clone();
        self.is_current_head = self.head == commander.get_current_head()?;

        self.refresh_files(commander)?;
        self.file = self
            .files_output
            .first()
            .and_then(|change| change.path.clone());
        self.refresh_diff(commander)?;

        Ok(())
    }

    pub fn get_current_file_index(&self) -> Option<usize> {
        get_current_file_index(&self.file, &self.files_output)
    }

    pub fn refresh_files(&mut self, commander: &mut Commander) -> Result<()> {
        self.files_output = commander.get_files(&self.head)?;
        self.conflicts_output = commander.get_conflicts(&self.head)?;
        Ok(())
    }

    pub fn refresh_diff(&mut self, commander: &mut Commander) -> Result<()> {
        self.diff_output = self
            .file
            .as_ref()
            .map(|current_file| {
                commander.get_file_diff(&self.head, current_file, &self.diff_format)
            })
            .map_or(Ok(None), |r| r.map(Some))?;
        self.diff_panel.scroll = 0;
        Ok(())
    }

    fn scroll_files(&mut self, commander: &mut Commander, scroll: isize) -> Result<()> {
        let files: &Vec<File> = self.files_output.as_ref();
        let current_file_index = self.get_current_file_index();
        let next_file = match current_file_index {
            Some(current_file_index) => files.get(
                current_file_index
                    .saturating_add_signed(scroll)
                    .min(files.len() - 1),
            ),
            None => files.first(),
        }
        .map(|x| x.to_owned());
        if let Some(next_file) = next_file
            && next_file.path.is_some()
        {
            self.file.clone_from(&next_file.path);
            self.refresh_diff(commander)?;
        }

        Ok(())
    }
}

impl Component for Files {
    fn update(&mut self, commander: &mut Commander) -> Result<Option<ComponentAction>> {
        self.is_current_head = self.head == commander.get_current_head()?;
        Ok(None)
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

        // Draw files
        {
            let panel_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Fill(1), Constraint::Length(2)])
                .split(chunks[0]);

            let current_file_index = self.get_current_file_index();

            let files_lines: Vec<Line> = self
                .files_output
                .iter()
                .enumerate()
                .flat_map(|(i, file)| {
                    file.line
                        .to_text()
                        .unwrap()
                        .iter()
                        .map(|line| {
                            let mut line = line.to_owned();

                            if let Some(diff_type) = file.diff_type.as_ref() {
                                line.spans = line
                                    .spans
                                    .iter_mut()
                                    .map(|span| span.to_owned().fg(diff_type.color()))
                                    .collect();
                            }

                            if current_file_index
                                .map_or(false, |current_file_index| i == current_file_index)
                            {
                                line = line.bg(self.config.highlight_color());

                                line.spans = line
                                    .spans
                                    .iter_mut()
                                    .map(|span| span.to_owned().bg(self.config.highlight_color()))
                                    .collect();
                            }

                            line
                        })
                        .collect::<Vec<Line>>()
                })
                .collect();

            let mut lines = if files_lines.is_empty() {
                vec![Line::from(" No changed files in change")
                    .fg(Color::DarkGray)
                    .italic()]
            } else {
                files_lines
            };

            let title_change = if self.is_current_head {
                format!("@ ({})", self.head.change_id)
            } else {
                self.head.change_id.as_string()
            };

            if !self.conflicts_output.is_empty() {
                lines.push(Line::default());

                for conflict in &self.conflicts_output {
                    lines.push(Line::raw(format!("C {}", &conflict.path)).fg(Color::Red));
                }
            }

            let files = List::new(lines)
                .block(
                    Block::bordered()
                        .title(" Files for ".to_owned() + &title_change + " ")
                        .border_type(BorderType::Rounded),
                )
                .scroll_padding(3);
            *self.files_list_state.selected_mut() = current_file_index;
            f.render_stateful_widget(files, panel_chunks[0], &mut self.files_list_state);
            self.files_height = panel_chunks[0].height - 2;

            let help = Paragraph::new(vec![
                "j/j: scroll down/up | J/K: scroll down by ½ page".into(),
                "@: view current change files".into(),
            ])
            .fg(Color::DarkGray);
            f.render_widget(help, panel_chunks[1]);
        }

        // Draw diff
        {
            let panel_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Fill(1), Constraint::Length(2)])
                .split(chunks[1]);

            let diff_block = Block::bordered()
                .title(" Diff ")
                .border_type(BorderType::Rounded);
            let diff = self
                .diff_panel
                .render(
                    self.diff_output
                        .as_ref()
                        .map_or(Text::from(""), |diff_output| {
                            diff_output.into_text().unwrap()
                        }),
                    diff_block.inner(chunks[1]),
                )
                .block(diff_block);
            f.render_widget(diff, panel_chunks[0]);

            let help = Paragraph::new(vec![
                "Ctrl+e/Ctrl+y: scroll down/up | Ctrl+d/Ctrl+u: scroll down/up by ½ page".into(),
                "Ctrl+f/Ctrl+b: scroll down/up by page | p: toggle diff format | w: toggle wrapping".into(),
            ]).fg(Color::DarkGray);
            f.render_widget(help, panel_chunks[1]);
        }

        Ok(())
    }

    fn input(
        &mut self,
        commander: &mut Commander,
        event: Event,
    ) -> Result<Option<ComponentAction>> {
        if let Event::Key(key) = event {
            if self.diff_panel.input(key) {
                return Ok(None);
            }

            match key.code {
                KeyCode::Char('j') | KeyCode::Down => self.scroll_files(commander, 1)?,
                KeyCode::Char('k') | KeyCode::Up => self.scroll_files(commander, -1)?,
                KeyCode::Char('J') => {
                    self.scroll_files(commander, self.files_height as isize / 2)?;
                }
                KeyCode::Char('K') => {
                    self.scroll_files(
                        commander,
                        (self.files_height as isize / 2).saturating_neg(),
                    )?;
                }
                KeyCode::Char('p') => {
                    self.diff_format = match self.diff_format {
                        DiffFormat::ColorWords => DiffFormat::Git,
                        _ => DiffFormat::ColorWords,
                    };
                    self.refresh_diff(commander)?;
                }
                KeyCode::Char('R') | KeyCode::F(5) => {
                    self.head = commander.get_head_latest(&self.head)?;
                    self.refresh_files(commander)?;
                    self.refresh_diff(commander)?;
                }
                KeyCode::Char('@') => {
                    let head = &commander.get_current_head()?;
                    self.set_head(commander, head)?;
                }
                _ => {}
            };
        }

        Ok(None)
    }
}
