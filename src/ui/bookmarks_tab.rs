#![allow(clippy::borrow_interior_mutable_const)]
use crate::{
    commander::{bookmarks::BookmarkLine, ids::ChangeId, CommandError, Commander},
    env::{Config, DiffFormat, JJLayout},
    ui::{
        details_panel::DetailsPanel,
        help_popup::HelpPopup,
        message_popup::MessagePopup,
        utils::{centered_rect, centered_rect_line_height, tabs_to_spaces},
        Component, ComponentAction,
    },
    ComponentInputResult,
};
use ansi_to_tui::IntoText;
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{prelude::*, widgets::*};
use tracing::instrument;
use tui_confirm_dialog::{ButtonLabel, ConfirmDialog, ConfirmDialogState, Listener};
use tui_textarea::{CursorMove, TextArea};

struct CreateBookmark<'a> {
    textarea: TextArea<'a>,
    error: Option<anyhow::Error>,
}

struct RenameBookmark<'a> {
    textarea: TextArea<'a>,
    name: String,
    error: Option<anyhow::Error>,
}

struct DeleteBookmark {
    name: String,
}

struct ForgetBookmark {
    name: String,
}

const DELETE_BRANCH_POPUP_ID: u16 = 1;
const FORGET_BRANCH_POPUP_ID: u16 = 2;
const NEW_POPUP_ID: u16 = 3;
const EDIT_POPUP_ID: u16 = 4;

/// Bookmarks tab. Shows bookmarks in main panel and selected bookmark current change in details panel.
pub struct BookmarksTab<'a> {
    bookmarks_output: Result<Vec<BookmarkLine>, CommandError>,
    bookmarks_list_state: ListState,
    bookmarks_height: u16,

    show_all: bool,

    bookmark: Option<BookmarkLine>,

    bookmark_panel: DetailsPanel,
    bookmark_output: Option<Result<String, CommandError>>,

    create: Option<CreateBookmark<'a>>,
    rename: Option<RenameBookmark<'a>>,
    delete: Option<DeleteBookmark>,
    forget: Option<ForgetBookmark>,

    describe_textarea: Option<TextArea<'a>>,
    describe_after_new: bool,
    describe_after_new_change: Option<ChangeId>,

    popup: ConfirmDialogState,
    popup_tx: std::sync::mpsc::Sender<Listener>,
    popup_rx: std::sync::mpsc::Receiver<Listener>,

    diff_format: DiffFormat,
    layout_direction: Direction,
    layout_percent: u16,

    config: Config,
}

fn get_current_bookmark_index(
    current_bookmark: Option<&BookmarkLine>,
    bookmarks_output: &Result<Vec<BookmarkLine>, CommandError>,
) -> Option<usize> {
    match bookmarks_output {
        Ok(bookmarks_output) => current_bookmark.as_ref().and_then(|current_bookmark| {
            bookmarks_output
                .iter()
                .position(|bookmark| match (current_bookmark, bookmark) {
                    (
                        BookmarkLine::Parsed {
                            bookmark: current_bookmark,
                            ..
                        },
                        BookmarkLine::Parsed { bookmark, .. },
                    ) => {
                        current_bookmark.name == bookmark.name
                            && current_bookmark.remote == bookmark.remote
                    }
                    (
                        BookmarkLine::Unparsable(current_bookmark),
                        BookmarkLine::Unparsable(bookmark),
                    ) => current_bookmark == bookmark,
                    _ => false,
                })
        }),
        Err(_) => None,
    }
}

impl BookmarksTab<'_> {
    #[instrument(level = "trace", skip(commander))]
    pub fn new(commander: &mut Commander) -> Result<Self> {
        let diff_format = commander.env.config.diff_format();

        let show_all = false;

        let bookmarks_output = commander.get_bookmarks(show_all);
        let bookmark = bookmarks_output
            .as_ref()
            .ok()
            .and_then(|bookmarks_output| bookmarks_output.first())
            .map(|bookmarks_output| bookmarks_output.to_owned());

        let bookmarks_list_state = ListState::default().with_selected(get_current_bookmark_index(
            bookmark.as_ref(),
            &bookmarks_output,
        ));

        let bookmark_output = bookmark.as_ref().and_then(|bookmark| match bookmark {
            BookmarkLine::Parsed { bookmark, .. } => Some(
                commander
                    .get_bookmark_show(bookmark, &diff_format)
                    .map(|diff| tabs_to_spaces(&diff)),
            ),
            _ => None,
        });

        let (popup_tx, popup_rx) = std::sync::mpsc::channel();

        let layout_direction = if commander.env.config.layout() == JJLayout::Horizontal {
            Direction::Horizontal
        } else {
            Direction::Vertical
        };
        let layout_percent = commander.env.config.layout_percent();

        Ok(Self {
            bookmarks_output,
            bookmark,
            bookmarks_list_state,
            bookmarks_height: 0,

            show_all,

            bookmark_panel: DetailsPanel::new(),
            bookmark_output,

            create: None,
            rename: None,
            delete: None,
            forget: None,

            describe_after_new: false,
            describe_textarea: None,
            describe_after_new_change: None,

            popup: ConfirmDialogState::default(),
            popup_tx,
            popup_rx,

            diff_format,
            layout_direction,
            layout_percent,

            config: commander.env.config.clone(),
        })
    }

    pub fn get_current_bookmark_index(&self) -> Option<usize> {
        get_current_bookmark_index(self.bookmark.as_ref(), &self.bookmarks_output)
    }

    pub fn refresh_bookmarks(&mut self, commander: &mut Commander) {
        self.bookmarks_output = commander.get_bookmarks(self.show_all);
    }

    pub fn refresh_bookmark(&mut self, commander: &mut Commander) {
        self.bookmark_output = self.bookmark.as_ref().and_then(|bookmark| match bookmark {
            BookmarkLine::Parsed { bookmark, .. } => Some(
                commander
                    .get_bookmark_show(bookmark, &self.diff_format)
                    .map(|diff| tabs_to_spaces(&diff)),
            ),
            _ => None,
        });

        self.bookmark_panel.scroll = 0;
    }

    fn scroll_bookmarks(&mut self, commander: &mut Commander, scroll: isize) {
        let bookmarks = Vec::new();
        let bookmarks = self.bookmarks_output.as_ref().unwrap_or(&bookmarks);
        let current_bookmark_index = self.get_current_bookmark_index();
        let next_bookmark = match current_bookmark_index {
            Some(current_bookmark_index) => bookmarks.get(
                current_bookmark_index
                    .saturating_add_signed(scroll)
                    .min(bookmarks.len() - 1),
            ),
            None => bookmarks.first(),
        }
        .map(|x| x.to_owned());

        if let Some(next_bookmark) = next_bookmark {
            self.bookmark = Some(next_bookmark);
            self.refresh_bookmark(commander);
        }
    }
}

impl Component for BookmarksTab<'_> {
    fn switch(&mut self, commander: &mut Commander) -> Result<()> {
        self.refresh_bookmarks(commander);
        self.refresh_bookmark(commander);
        Ok(())
    }

    fn update(&mut self, commander: &mut Commander) -> Result<Option<ComponentAction>> {
        // Check for popup action
        if let Ok(res) = self.popup_rx.try_recv() {
            if res.1.unwrap_or(false) {
                match res.0 {
                    DELETE_BRANCH_POPUP_ID => {
                        if let Some(delete) = self.delete.as_ref() {
                            match commander.delete_bookmark(&delete.name) {
                                Ok(_) => {
                                    self.refresh_bookmarks(commander);
                                    let bookmarks = Vec::new();
                                    let bookmarks =
                                        self.bookmarks_output.as_ref().unwrap_or(&bookmarks);
                                    self.bookmark =
                                        bookmarks.first().map(|bookmark| bookmark.to_owned());
                                    self.refresh_bookmark(commander);
                                }
                                Err(err) => {
                                    return Ok(Some(ComponentAction::SetPopup(Some(Box::new(
                                        MessagePopup {
                                            title: "Delete error".into(),
                                            messages: err.to_string().into_text()?,
                                        },
                                    )))));
                                }
                            }
                        }
                    }
                    FORGET_BRANCH_POPUP_ID => {
                        if let Some(forget) = self.forget.as_ref() {
                            match commander.forget_bookmark(&forget.name) {
                                Ok(_) => {
                                    self.refresh_bookmarks(commander);
                                    let bookmarks = Vec::new();
                                    let bookmarks =
                                        self.bookmarks_output.as_ref().unwrap_or(&bookmarks);
                                    self.bookmark =
                                        bookmarks.first().map(|bookmark| bookmark.to_owned());
                                    self.refresh_bookmark(commander);
                                }
                                Err(err) => {
                                    return Ok(Some(ComponentAction::SetPopup(Some(Box::new(
                                        MessagePopup {
                                            title: "Forget error".into(),
                                            messages: err.to_string().into_text()?,
                                        },
                                    )))));
                                }
                            }
                        }
                    }
                    NEW_POPUP_ID => {
                        if let Some(BookmarkLine::Parsed { bookmark, .. }) = self.bookmark.as_ref()
                        {
                            commander.run_new(&bookmark.to_string())?;
                            let head = commander.get_current_head()?;
                            if self.describe_after_new {
                                self.describe_after_new_change = Some(head.change_id);
                                self.describe_after_new = false;
                                let textarea = TextArea::default();
                                self.describe_textarea = Some(textarea);
                                return Ok(None);
                            } else {
                                return Ok(Some(ComponentAction::ViewLog(head)));
                            }
                        }
                    }
                    EDIT_POPUP_ID => {
                        if let Some(BookmarkLine::Parsed { bookmark, .. }) = self.bookmark.as_ref()
                        {
                            commander.run_edit(&bookmark.to_string())?;
                            let head = commander.get_current_head()?;
                            return Ok(Some(ComponentAction::ViewLog(head)));
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(None)
    }

    fn draw(
        &mut self,
        f: &mut ratatui::prelude::Frame<'_>,
        area: ratatui::prelude::Rect,
    ) -> Result<()> {
        let chunks = Layout::default()
            .direction(self.layout_direction)
            .constraints([
                Constraint::Percentage(self.layout_percent),
                Constraint::Percentage(100 - self.layout_percent),
            ])
            .split(area);

        // Draw bookmarks
        {
            let current_bookmark_index = self.get_current_bookmark_index();

            let bookmark_lines: Vec<Line> = match self.bookmarks_output.as_ref() {
                Ok(bookmarks_output) => bookmarks_output
                    .iter()
                    .enumerate()
                    .map(|(i, bookmark)| -> Result<Vec<Line>, ansi_to_tui::Error> {
                        let bookmark_text = bookmark.to_text()?;
                        Ok(bookmark_text
                            .iter()
                            .map(|line| {
                                let mut line = line.to_owned();

                                // Add padding at start
                                line.spans.insert(0, Span::from(" "));

                                if current_bookmark_index.map_or(false, |current_bookmark_index| {
                                    i == current_bookmark_index
                                }) {
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
                            .collect::<Vec<Line>>())
                    })
                    .collect::<Result<Vec<Vec<Line>>, ansi_to_tui::Error>>()?
                    .into_iter()
                    .flatten()
                    .collect(),
                Err(err) => [
                    vec![Line::raw("Error getting bookmarks").bold().fg(Color::Red)],
                    // TODO: Remove when jj 0.20 is released
                    if let CommandError::Status(output, _) = err {
                        if output.contains("unexpected argument '-T' found") {
                            vec![
                                Line::raw(""),
                                Line::raw("Please update jj to >0.18 for -T support to bookmarks")
                                    .bold()
                                    .fg(Color::Red),
                            ]
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    },
                    vec![Line::raw(""), Line::raw("")],
                    err.to_string().into_text()?.lines,
                ]
                .concat(),
            };

            let lines = if bookmark_lines.is_empty() {
                vec![Line::from(" No bookmarks").fg(Color::DarkGray).italic()]
            } else {
                bookmark_lines
            };

            let bookmarks_block = Block::bordered()
                .title(" Bookmarks ")
                .border_type(BorderType::Rounded);
            self.bookmarks_height = bookmarks_block.inner(chunks[0]).height;
            let bookmarks = List::new(lines).block(bookmarks_block).scroll_padding(3);
            *self.bookmarks_list_state.selected_mut() = current_bookmark_index;
            f.render_stateful_widget(bookmarks, chunks[0], &mut self.bookmarks_list_state);
        }

        // Draw bookmark
        {
            let title = if let Some(BookmarkLine::Parsed { bookmark, .. }) = self.bookmark.as_ref()
            {
                format!(" Bookmark {} ", bookmark)
            } else {
                " Bookmark ".to_owned()
            };

            let bookmark_block = Block::bordered()
                .title(title)
                .border_type(BorderType::Rounded)
                .padding(Padding::horizontal(1));
            let bookmark_content: Vec<Line> = match self.bookmark_output.as_ref() {
                Some(Ok(bookmark_output)) => bookmark_output.into_text()?.lines,
                Some(Err(err)) => err.into_text("Error getting bookmark")?.lines,
                None => vec![],
            };
            let bookmark = self
                .bookmark_panel
                .render(bookmark_content, bookmark_block.inner(chunks[1]))
                .block(bookmark_block);
            f.render_widget(bookmark, chunks[1]);
        }

        // Draw popup
        if self.popup.is_opened() {
            let popup = ConfirmDialog::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Green))
                .selected_button_style(
                    Style::default()
                        .bg(self.config.highlight_color())
                        .underlined(),
                );
            f.render_stateful_widget(popup, area, &mut self.popup);
        }

        // Draw create textarea
        {
            if let Some(create) = self.create.as_mut() {
                let block = Block::bordered()
                    .title(Span::styled(
                        " Create bookmark ",
                        Style::new().bold().cyan(),
                    ))
                    .title_alignment(Alignment::Center)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Green));
                let error_lines = create
                    .error
                    .as_ref()
                    .map(|error| error.to_string().into_text().unwrap().lines);
                let error_height = if let Some(error_lines) = error_lines.as_ref() {
                    error_lines.len() + 1
                } else {
                    0
                };
                let area = centered_rect_line_height(area, 30, 5 + error_height as u16);
                f.render_widget(Clear, area);
                f.render_widget(&block, area);

                let popup_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Fill(1),
                        Constraint::Length(error_height as u16),
                        Constraint::Length(2),
                    ])
                    .split(block.inner(area));

                f.render_widget(&create.textarea, popup_chunks[0]);

                if let Some(error_lines) = error_lines {
                    let help = Paragraph::new(error_lines).block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    );

                    f.render_widget(help, popup_chunks[1]);
                }

                let help = Paragraph::new(vec!["Ctrl+s: save | Escape: cancel".into()])
                    .fg(Color::DarkGray)
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    );

                f.render_widget(help, popup_chunks[2]);
            }
        }

        // Draw rename textarea
        {
            if let Some(rename) = self.rename.as_mut() {
                let block = Block::bordered()
                    .title(Span::styled(
                        " Rename bookmark ",
                        Style::new().bold().cyan(),
                    ))
                    .title_alignment(Alignment::Center)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Green));
                let error_lines = rename
                    .error
                    .as_ref()
                    .map(|error| error.to_string().into_text().unwrap().lines);
                let error_height = if let Some(error_lines) = error_lines.as_ref() {
                    error_lines.len() + 1
                } else {
                    0
                };
                let area = centered_rect_line_height(area, 30, 5 + error_height as u16);
                f.render_widget(Clear, area);
                f.render_widget(&block, area);

                let popup_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Fill(1),
                        Constraint::Length(error_height as u16),
                        Constraint::Length(2),
                    ])
                    .split(block.inner(area));

                f.render_widget(&rename.textarea, popup_chunks[0]);

                if let Some(error_lines) = error_lines {
                    let help = Paragraph::new(error_lines).block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    );

                    f.render_widget(help, popup_chunks[1]);
                }

                let help = Paragraph::new(vec!["Ctrl+s: save | Escape: cancel".into()])
                    .fg(Color::DarkGray)
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    );

                f.render_widget(help, popup_chunks[2]);
            }
        }

        // Draw describe textarea
        {
            if let Some(describe_textarea) = self.describe_textarea.as_mut() {
                let block = Block::bordered()
                    .title(Span::styled(" Describe ", Style::new().bold().cyan()))
                    .title_alignment(Alignment::Center)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Green));
                let area = centered_rect(area, 50, 50);
                f.render_widget(Clear, area);
                f.render_widget(&block, area);

                let popup_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Fill(1), Constraint::Length(2)])
                    .split(block.inner(area));

                f.render_widget(&*describe_textarea, popup_chunks[0]);

                let help = Paragraph::new(vec!["Ctrl+s: save | Escape: cancel".into()])
                    .fg(Color::DarkGray)
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    );

                f.render_widget(help, popup_chunks[1]);
            }
        }

        Ok(())
    }

    fn input(&mut self, commander: &mut Commander, event: Event) -> Result<ComponentInputResult> {
        if let Some(create) = self.create.as_mut() {
            if let Event::Key(key) = event {
                match key.code {
                    _ if (key.code == KeyCode::Char('s')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                        || (key.code == KeyCode::Enter) =>
                    {
                        let name = create.textarea.lines().join("\n");

                        if name.trim().is_empty() {
                            create.error =
                                Some(anyhow::Error::msg("Bookmark name cannot be empty"));
                            return Ok(ComponentInputResult::Handled);
                        }

                        if let Err(err) = commander.create_bookmark(&name) {
                            create.error = Some(anyhow::Error::new(err));
                            return Ok(ComponentInputResult::Handled);
                        }

                        self.create = None;
                        self.refresh_bookmarks(commander);

                        // Select new bookmark
                        if let Some(bookmark) =
                            self.bookmarks_output
                                .as_ref()
                                .ok()
                                .and_then(|bookmarks_output| {
                                    bookmarks_output.iter().find(|bookmark| match bookmark {
                                        BookmarkLine::Unparsable(_) => false,
                                        BookmarkLine::Parsed { bookmark, .. } => {
                                            bookmark.name == name
                                        }
                                    })
                                })
                        {
                            self.bookmark = Some(bookmark.clone());
                        }

                        self.refresh_bookmark(commander);

                        return Ok(ComponentInputResult::Handled);
                    }
                    KeyCode::Esc => {
                        self.create = None;
                        return Ok(ComponentInputResult::Handled);
                    }
                    _ => {}
                }
            }
            create.textarea.input(event);
            return Ok(ComponentInputResult::Handled);
        }

        if let Some(rename) = self.rename.as_mut() {
            if let Event::Key(key) = event {
                match key.code {
                    _ if (key.code == KeyCode::Char('s')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                        || (key.code == KeyCode::Enter) =>
                    {
                        let new = rename.textarea.lines().join("\n");

                        if new.trim().is_empty() {
                            rename.error =
                                Some(anyhow::Error::msg("Bookmark name cannot be empty"));
                            return Ok(ComponentInputResult::Handled);
                        }

                        let old = rename.name.clone();

                        if let Err(err) = commander.rename_bookmark(&old, &new) {
                            rename.error = Some(anyhow::Error::new(err));
                            return Ok(ComponentInputResult::Handled);
                        }
                        self.rename = None;
                        self.refresh_bookmarks(commander);

                        // Select new bookmark
                        if let Some(bookmark) =
                            self.bookmarks_output
                                .as_ref()
                                .ok()
                                .and_then(|bookmarks_output| {
                                    bookmarks_output.iter().find(|bookmark| match bookmark {
                                        BookmarkLine::Unparsable(_) => false,
                                        BookmarkLine::Parsed { bookmark, .. } => {
                                            bookmark.name == new
                                        }
                                    })
                                })
                        {
                            self.bookmark = Some(bookmark.clone());
                        }

                        self.refresh_bookmark(commander);

                        return Ok(ComponentInputResult::Handled);
                    }
                    KeyCode::Esc => {
                        self.rename = None;
                        return Ok(ComponentInputResult::Handled);
                    }
                    _ => {}
                }
            }
            rename.textarea.input(event);
            return Ok(ComponentInputResult::Handled);
        }

        if let (Some(describe_textarea), Some(describe_after_new_change)) = (
            self.describe_textarea.as_mut(),
            self.describe_after_new_change.as_ref(),
        ) {
            if let Event::Key(key) = event {
                match key.code {
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // TODO: Handle error
                        commander.run_describe(
                            describe_after_new_change.as_str(),
                            &describe_textarea.lines().join("\n"),
                        )?;
                        self.describe_textarea = None;
                        self.describe_after_new_change = None;
                        return Ok(ComponentInputResult::HandledAction(
                            ComponentAction::ViewLog(commander.get_current_head()?),
                        ));
                    }
                    KeyCode::Esc => {
                        self.describe_textarea = None;
                        self.describe_after_new_change = None;
                        return Ok(ComponentInputResult::Handled);
                    }
                    _ => {}
                }
            }
            describe_textarea.input(event);
            return Ok(ComponentInputResult::Handled);
        }

        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return Ok(ComponentInputResult::Handled);
            }
            if self.popup.is_opened() {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    self.popup = ConfirmDialogState::default();
                } else {
                    self.popup.handle(key);
                }

                return Ok(ComponentInputResult::Handled);
            }

            if self.bookmark_panel.input(key) {
                return Ok(ComponentInputResult::Handled);
            }

            match key.code {
                KeyCode::Char('j') | KeyCode::Down => self.scroll_bookmarks(commander, 1),
                KeyCode::Char('k') | KeyCode::Up => self.scroll_bookmarks(commander, -1),
                KeyCode::Char('J') => {
                    self.scroll_bookmarks(commander, self.bookmarks_height as isize / 2);
                }
                KeyCode::Char('K') => {
                    self.scroll_bookmarks(
                        commander,
                        (self.bookmarks_height as isize / 2).saturating_neg(),
                    );
                }
                KeyCode::Char('w') => {
                    self.diff_format = match self.diff_format {
                        DiffFormat::ColorWords => DiffFormat::Git,
                        _ => DiffFormat::ColorWords,
                    };
                    self.refresh_bookmark(commander);
                }
                KeyCode::Char('R') | KeyCode::F(5) => {
                    self.refresh_bookmarks(commander);
                    self.refresh_bookmark(commander);
                }
                KeyCode::Char('a') => {
                    self.show_all = !self.show_all;
                    self.refresh_bookmarks(commander);
                }
                KeyCode::Char('c') => {
                    let textarea = TextArea::default();
                    self.create = Some(CreateBookmark {
                        textarea,
                        error: None,
                    });
                    return Ok(ComponentInputResult::Handled);
                }
                KeyCode::Char('r') => {
                    if let Some(BookmarkLine::Parsed { bookmark, .. }) = self.bookmark.as_ref() {
                        let mut textarea = TextArea::new(vec![bookmark.name.clone()]);
                        textarea.move_cursor(CursorMove::End);
                        self.rename = Some(RenameBookmark {
                            textarea,
                            name: bookmark.name.clone(),
                            error: None,
                        });
                        return Ok(ComponentInputResult::Handled);
                    }
                }
                KeyCode::Char('d') => {
                    if let Some(BookmarkLine::Parsed { bookmark, .. }) = self.bookmark.as_ref() {
                        self.delete = Some(DeleteBookmark {
                            name: bookmark.name.clone(),
                        });
                        self.popup = ConfirmDialogState::new(
                            DELETE_BRANCH_POPUP_ID,
                            Span::styled(" Delete ", Style::new().bold().cyan()),
                            Text::from(vec![Line::from(format!(
                                "Are you sure you want to delete the {} bookmark?",
                                bookmark.name
                            ))]),
                        )
                        .with_yes_button(ButtonLabel::YES.clone())
                        .with_no_button(ButtonLabel::NO.clone())
                        .with_listener(Some(self.popup_tx.clone()))
                        .open();
                    }
                }
                KeyCode::Char('f') => {
                    if let Some(BookmarkLine::Parsed { bookmark, .. }) = self.bookmark.as_ref() {
                        self.forget = Some(ForgetBookmark {
                            name: bookmark.name.clone(),
                        });
                        self.popup = ConfirmDialogState::new(
                            FORGET_BRANCH_POPUP_ID,
                            Span::styled(" Forget ", Style::new().bold().cyan()),
                            Text::from(vec![Line::from(format!(
                                "Are you sure you want to forget the {} bookmark?",
                                bookmark.name
                            ))]),
                        )
                        .with_yes_button(ButtonLabel::YES.clone())
                        .with_no_button(ButtonLabel::NO.clone())
                        .with_listener(Some(self.popup_tx.clone()))
                        .open();
                    }
                }
                // TODO: Ask for confirmation?
                KeyCode::Char('t') => {
                    if let Some(BookmarkLine::Parsed { bookmark, .. }) = self.bookmark.as_ref() {
                        if bookmark.remote.is_some() && bookmark.present {
                            commander.track_bookmark(bookmark)?;
                            self.refresh_bookmarks(commander);
                            self.refresh_bookmark(commander);
                        }
                    }
                }
                KeyCode::Char('T') => {
                    if let Some(BookmarkLine::Parsed { bookmark, .. }) = self.bookmark.as_ref() {
                        if bookmark.remote.is_some() && bookmark.present {
                            commander.untrack_bookmark(bookmark)?;
                            self.refresh_bookmarks(commander);
                            self.refresh_bookmark(commander);
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    if let Some(BookmarkLine::Parsed { bookmark, .. }) = self.bookmark.as_ref() {
                        if bookmark.present {
                            self.popup = ConfirmDialogState::new(
                                NEW_POPUP_ID,
                                Span::styled(" New ", Style::new().bold().cyan()),
                                Text::from(vec![
                                    Line::from("Are you sure you want to create a new change?"),
                                    Line::from(format!("Bookmark: {}", bookmark)),
                                ]),
                            )
                            .with_yes_button(ButtonLabel::YES.clone())
                            .with_no_button(ButtonLabel::NO.clone())
                            .with_listener(Some(self.popup_tx.clone()))
                            .open();

                            self.describe_after_new = key.code == KeyCode::Char('N');
                        }
                    }
                }
                KeyCode::Char('e') => {
                    if let Some(BookmarkLine::Parsed { bookmark, .. }) = self.bookmark.as_ref() {
                        if bookmark.present {
                            if commander.check_revision_immutable(&bookmark.to_string())? {
                                return Ok(ComponentInputResult::HandledAction(
                                    ComponentAction::SetPopup(Some(Box::new(MessagePopup {
                                        title: "Edit".into(),
                                        messages: vec![
                                            "The change cannot be edited because it is immutable."
                                                .into(),
                                        ]
                                        .into(),
                                    }))),
                                ));
                            } else {
                                self.popup = ConfirmDialogState::new(
                                    EDIT_POPUP_ID,
                                    Span::styled(" Edit ", Style::new().bold().cyan()),
                                    Text::from(vec![
                                        Line::from(
                                            "Are you sure you want to edit an existing change?",
                                        ),
                                        Line::from(format!("Bookmark: {}", bookmark)),
                                    ]),
                                )
                                .with_yes_button(ButtonLabel::YES.clone())
                                .with_no_button(ButtonLabel::NO.clone())
                                .with_listener(Some(self.popup_tx.clone()))
                                .open();
                            }
                        }
                    }
                }
                KeyCode::Enter => {
                    if let Some(BookmarkLine::Parsed { bookmark, .. }) = self.bookmark.as_ref() {
                        if bookmark.present {
                            return Ok(ComponentInputResult::HandledAction(
                                ComponentAction::ViewLog(commander.get_bookmark_head(bookmark)?),
                            ));
                        }
                    }
                }
                KeyCode::Char('h') | KeyCode::Char('?') => {
                    return Ok(ComponentInputResult::HandledAction(
                        ComponentAction::SetPopup(Some(Box::new(HelpPopup::new(
                            vec![
                                ("j/k".to_owned(), "scroll down/up".to_owned()),
                                ("J/K".to_owned(), "scroll down by ½ page".to_owned()),
                                ("a".to_owned(), "show all remotes".to_owned()),
                                ("c".to_owned(), "create bookmark".to_owned()),
                                ("r".to_owned(), "rename bookmark".to_owned()),
                                ("d/f".to_owned(), "delete/forget bookmark".to_owned()),
                                ("t/T".to_owned(), "track/untrack bookmark".to_owned()),
                                ("Enter".to_owned(), "view in log".to_owned()),
                                ("n".to_owned(), "new from bookmark".to_owned()),
                                ("N".to_owned(), "new and describe".to_owned()),
                                ("e".to_owned(), "edit bookmark".to_owned()),
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
