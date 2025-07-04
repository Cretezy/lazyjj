use std::vec;

use anyhow::Result;
use tracing::instrument;

use crate::{
    commander::{
        files::{Conflict, DiffType, File},
        log::Head,
        CommandError, Commander,
    },
    env::{Config, DiffFormat},
    ui::{
        details_panel::DetailsPanel, help_popup::HelpPopup, message_popup::MessagePopup,
        utils::tabs_to_spaces, Component, ComponentAction,
    },
    ComponentInputResult,
};

use ansi_to_tui::IntoText;
use ratatui::{
    crossterm::event::{Event, KeyCode, KeyEventKind},
    prelude::*,
    widgets::*,
};

/// Files tab. Shows files in selected change in main panel and selected file diff in details panel
pub struct FilesTab {
    head: Head,
    is_current_head: bool,

    files_output: Result<Vec<File>, CommandError>,
    conflicts_output: Vec<Conflict>,
    files_list_state: ListState,
    files_height: u16,

    pub file: Option<File>,
    diff_panel: DetailsPanel,
    diff_output: Result<Option<String>, CommandError>,
    diff_format: DiffFormat,

    config: Config,
}

fn get_current_file_index(
    current_file: Option<&File>,
    files_output: Result<&Vec<File>, &CommandError>,
) -> Option<usize> {
    if let (Some(current_file), Ok(files_output)) = (current_file, files_output) {
        if let Some(path) = current_file.path.as_ref() {
            return files_output
                .iter()
                .position(|file| file.path.as_ref() == Some(path));
        }
    }

    None
}

impl FilesTab {
    #[instrument(level = "trace", skip(commander))]
    pub fn new(commander: &mut Commander, head: &Head) -> Result<Self> {
        let head = head.clone();
        let is_current_head = head == commander.get_current_head()?;

        let diff_format = commander.env.config.diff_format();

        let files_output = commander.get_files(&head);
        let conflicts_output = commander.get_conflicts(&head.commit_id)?;
        let current_file = files_output
            .as_ref()
            .ok()
            .and_then(|files_output| files_output.first())
            .map(|file| file.to_owned());
        let diff_output = current_file
            .as_ref()
            .map(|current_change| {
                commander.get_file_diff(&head, current_change, &diff_format, true)
            })
            .map_or(Ok(None), |r| {
                r.map(|diff| diff.map(|diff| tabs_to_spaces(&diff)))
            });

        let files_list_state = ListState::default().with_selected(get_current_file_index(
            current_file.as_ref(),
            files_output.as_ref(),
        ));

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
            .as_ref()
            .ok()
            .and_then(|files_output| files_output.first())
            .map(|file| file.to_owned());
        self.refresh_diff(commander)?;

        Ok(())
    }

    pub fn get_current_file_index(&self) -> Option<usize> {
        get_current_file_index(self.file.as_ref(), self.files_output.as_ref())
    }

    pub fn refresh_files(&mut self, commander: &mut Commander) -> Result<()> {
        self.files_output = commander.get_files(&self.head);
        self.conflicts_output = commander.get_conflicts(&self.head.commit_id)?;
        Ok(())
    }

    pub fn refresh_diff(&mut self, commander: &mut Commander) -> Result<()> {
        self.diff_output = self
            .file
            .as_ref()
            .map(|current_file| {
                commander.get_file_diff(&self.head, current_file, &self.diff_format, true)
            })
            .map_or(Ok(None), |r| {
                r.map(|diff| diff.map(|diff| tabs_to_spaces(&diff)))
            });
        self.diff_panel.scroll = 0;
        Ok(())
    }

    pub fn untrack_file(&mut self, commander: &mut Commander) -> Result<()> {
        self.file
            .as_ref()
            .map(|current_file| commander.untrack_file(&current_file));
        Ok(())
    }

    fn scroll_files(&mut self, commander: &mut Commander, scroll: isize) -> Result<()> {
        if let Ok(files) = self.files_output.as_ref() {
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
            if let Some(next_file) = next_file {
                self.file = Some(next_file.to_owned());
                self.refresh_diff(commander)?;
            }
        }
        Ok(())
    }
}

impl Component for FilesTab {
    fn focus(&mut self, commander: &mut Commander) -> Result<()> {
        self.is_current_head = self.head == commander.get_current_head()?;
        self.head = commander.get_head_latest(&self.head)?;
        self.refresh_files(commander)?;
        self.refresh_diff(commander)?;
        Ok(())
    }

    fn draw(
        &mut self,
        f: &mut ratatui::prelude::Frame<'_>,
        area: ratatui::prelude::Rect,
    ) -> Result<()> {
        let chunks = Layout::default()
            .direction(self.config.layout().into())
            .constraints([
                Constraint::Percentage(self.config.layout_percent()),
                Constraint::Percentage(100 - self.config.layout_percent()),
            ])
            .split(area);

        // Draw files
        {
            let current_file_index = self.get_current_file_index();

            let mut lines: Vec<Line> = match self.files_output.as_ref() {
                Ok(files_output) => {
                    let files_lines = files_output
                        .iter()
                        .enumerate()
                        .flat_map(|(i, file)| {
                            file.line
                                .to_text()
                                .unwrap()
                                .iter()
                                .map(|line| {
                                    let mut line = line.to_owned();

                                    // Add padding at start
                                    line.spans.insert(0, Span::from(" "));

                                    if let Some(diff_type) = file.diff_type.as_ref() {
                                        line.spans = line
                                            .spans
                                            .iter_mut()
                                            .map(|span| span.to_owned().fg(diff_type.color()))
                                            .collect();
                                    }

                                    if current_file_index == Some(i) {
                                        line = line.bg(self.config.highlight_color());

                                        line.spans = line
                                            .spans
                                            .iter_mut()
                                            .map(|span| {
                                                span.to_owned().bg(self.config.highlight_color())
                                            })
                                            .collect();
                                    }

                                    line
                                })
                                .collect::<Vec<Line>>()
                        })
                        .collect::<Vec<Line>>();

                    if files_lines.is_empty() {
                        vec![Line::from(" No changed files in change")
                            .fg(Color::DarkGray)
                            .italic()]
                    } else {
                        files_lines
                    }
                }
                Err(err) => err.into_text("Error getting files")?.lines,
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
            f.render_stateful_widget(files, chunks[0], &mut self.files_list_state);
            self.files_height = chunks[0].height - 2;
        }

        // Draw diff
        {
            let diff_block = Block::bordered()
                .title(" Diff ")
                .border_type(BorderType::Rounded)
                .padding(Padding::horizontal(1));
            let diff_content = match self.diff_output.as_ref() {
                Ok(Some(diff_content)) => diff_content.into_text()?,
                Ok(None) => Text::default(),
                Err(err) => err.into_text("Error getting diff")?,
            };
            let diff = self
                .diff_panel
                .render(diff_content, diff_block.inner(chunks[1]))
                .block(diff_block);
            f.render_widget(diff, chunks[1]);
        }

        Ok(())
    }

    fn input(&mut self, commander: &mut Commander, event: Event) -> Result<ComponentInputResult> {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return Ok(ComponentInputResult::Handled);
            }

            if self.diff_panel.input(key) {
                return Ok(ComponentInputResult::Handled);
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
                KeyCode::Char('w') => {
                    self.diff_format = self.diff_format.get_next(self.config.diff_tool());
                    self.refresh_diff(commander)?;
                }
                KeyCode::Char('x') => {
                    let status = self
                        .file
                        .as_ref()
                        .map(|current_file| current_file.diff_type.clone());

                    if (status != Some(Some(DiffType::Deleted))) {
                        let prev_files = commander.get_files(&self.head)?;
                        self.untrack_file(commander)?;
                        let head = &commander.get_current_head()?;
                        self.set_head(commander, head)?;

                        let next_files = commander.get_files(&self.head)?;

                        if prev_files == next_files {
                            return Ok(ComponentInputResult::HandledAction(
                                ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                                    title: "untrack file".into(),
                                    messages: "make shure to ignore the file before untracking it. Otherwise it gets auto tracked again".into(),
                                    text_align: None,
                                }))),
                            ));
                        }
                    }
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
                KeyCode::Char('?') => {
                    return Ok(ComponentInputResult::HandledAction(
                        ComponentAction::SetPopup(Some(Box::new(HelpPopup::new(
                            vec![
                                ("j/k".to_owned(), "scroll down/up".to_owned()),
                                ("J/K".to_owned(), "scroll down by ½ page".to_owned()),
                                ("x".to_owned(), "untrack file".to_owned()),
                                ("@".to_owned(), "view current change files".to_owned()),
                            ],
                            vec![
                                ("Ctrl+e/Ctrl+y".to_owned(), "scroll down/up".to_owned()),
                                (
                                    "Ctrl+d/Ctrl+u".to_owned(),
                                    "scroll down/up by ½ page".to_owned(),
                                ),
                                (
                                    "Ctrl+f/Ctrl+b".to_owned(),
                                    "scroll down/up by page".to_owned(),
                                ),
                                ("w".to_owned(), "toggle diff format".to_owned()),
                                ("W".to_owned(), "toggle wrapping".to_owned()),
                            ],
                        )))),
                    ))
                }
                _ => return Ok(ComponentInputResult::NotHandled),
            };
        }

        Ok(ComponentInputResult::Handled)
    }
}
