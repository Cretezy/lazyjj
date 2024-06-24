use ansi_to_tui::IntoText;
use anyhow::bail;
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListState, Paragraph},
};
use tui_textarea::TextArea;

use crate::{
    commander::{
        branches::Branch,
        ids::{ChangeId, CommitId},
        Commander,
    },
    env::Config,
    ui::{
        styles::create_popup_block,
        utils::{centered_rect, centered_rect_line_height},
        Component, ComponentAction,
    },
    ComponentInputResult,
};

enum BranchSetOption {
    CreateBranch,
    // Name, exists
    GeneratedName(String, bool),
    Branch(Branch),
    Error(String),
}

pub struct BranchSetPopup<'a> {
    pub change_id: Option<ChangeId>,
    commit_id: CommitId,
    options: Vec<BranchSetOption>,
    list_state: ListState,
    list_height: u16,
    config: Config,
    creating: Option<TextArea<'a>>,
    tx: std::sync::mpsc::Sender<bool>,
}

fn generate_options(
    commander: &mut Commander,
    change_id: Option<&ChangeId>,
) -> Vec<BranchSetOption> {
    let branches = commander.get_branches_list(false).map(|branches| {
        branches
            .into_iter()
            .filter(|branch| branch.remote.is_none())
            .collect::<Vec<Branch>>()
    });
    let mut options = vec![BranchSetOption::CreateBranch];

    if let Some(change_id) = change_id {
        let generated_name = generate_name(&commander.env.config.branch_prefix(), change_id);
        let exists = if let Ok(branches) = branches.as_ref() {
            branches.iter().any(|branch| branch.name == generated_name)
        } else {
            false
        };
        options.push(BranchSetOption::GeneratedName(generated_name, exists));
    }

    match branches.as_ref() {
        Ok(branches) => {
            for branch in branches.iter().filter(|branch| branch.remote.is_none()) {
                options.push(BranchSetOption::Branch(branch.clone()))
            }
        }
        Err(err) => options.push(BranchSetOption::Error(err.to_string())),
    }

    options
}

fn generate_name(git_push_branch_prefix: &str, change_id: &ChangeId) -> String {
    let mut change_id = change_id.to_string();
    change_id.truncate(12);
    format!("{git_push_branch_prefix}{change_id}",)
}

impl BranchSetPopup<'_> {
    pub fn new(
        config: Config,
        commander: &mut Commander,
        change_id: Option<ChangeId>,
        commit_id: CommitId,
        tx: std::sync::mpsc::Sender<bool>,
    ) -> Self {
        Self {
            options: generate_options(commander, change_id.as_ref()),
            change_id,
            list_state: ListState::default().with_selected(Some(0)),
            list_height: 0,
            config,
            commit_id,
            creating: None,
            tx,
        }
    }

    fn scroll(&mut self, scroll: isize) {
        self.list_state.select(Some(
            self.list_state
                .selected()
                .map(|selected| selected.saturating_add_signed(scroll))
                .unwrap_or(0)
                .min(self.options.len().saturating_sub(1)),
        ));
    }

    fn on_creating(&mut self) {
        self.creating = Some(TextArea::default());
    }

    fn create_branch(&self, commander: &mut Commander, name: &str) -> Result<()> {
        if commander
            .get_branches_list(false)?
            .iter()
            .any(|branch| branch.name == name)
        {
            commander.set_branch_commit(name, &self.commit_id)?;
        } else {
            commander.create_branch_commit(name, &self.commit_id)?;
        }
        Ok(())
    }
    fn generate_branch(&self, commander: &mut Commander) -> Result<()> {
        if let Some(change_id) = self.change_id.as_ref() {
            let generated_name = generate_name(&commander.env.config.branch_prefix(), change_id);
            if commander
                .get_branches_list(false)?
                .iter()
                .any(|branch| branch.name == generated_name)
            {
                commander.set_branch_commit(&generated_name, &self.commit_id)?;
            } else {
                commander.create_branch_commit(&generated_name, &self.commit_id)?;
            }
            Ok(())
        } else {
            bail!("No change ID");
        }
    }
}

impl Component for BranchSetPopup<'_> {
    fn draw(&mut self, f: &mut ratatui::prelude::Frame<'_>, area: Rect) -> Result<()> {
        if let Some(creating) = self.creating.as_ref() {
            let block = create_popup_block("Create branch");
            let area = centered_rect_line_height(area, 30, 5);
            f.render_widget(Clear, area);
            f.render_widget(&block, area);

            let popup_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Fill(1), Constraint::Length(2)])
                .split(block.inner(area));

            f.render_widget(creating.widget(), popup_chunks[0]);

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
        } else {
            let block = Block::bordered()
                .title(Span::styled(" Select branch ", Style::new().bold().cyan()))
                .title_alignment(Alignment::Center)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Green));
            let area = centered_rect(area, 40, 60);
            f.render_widget(Clear, area);
            f.render_widget(&block, area);

            let popup_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Fill(1), Constraint::Length(2)])
                .split(block.inner(area));

            let list_items = self.options.iter().map(|option| match option {
                BranchSetOption::CreateBranch => Text::raw("(C)reate branch").fg(Color::Yellow),
                BranchSetOption::GeneratedName(generated_name, exists) => {
                    let mut text = format!("(G)enerate branch: {}", generated_name);
                    if *exists {
                        text.push_str(" (exists)");
                    }
                    Text::raw(text).fg(Color::Yellow)
                }
                BranchSetOption::Branch(branch) => Text::raw(branch.to_string()).fg(Color::Magenta),
                BranchSetOption::Error(err) => err.into_text().unwrap(),
            });

            let list = List::new(list_items)
                .scroll_padding(3)
                .highlight_style(Style::default().bg(self.config.highlight_color()));

            f.render_stateful_widget(list, popup_chunks[0], &mut self.list_state);
            self.list_height = popup_chunks[0].height;

            let help = Paragraph::new(vec!["j/k: scroll down/up | Escape: cancel".into()])
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

        Ok(())
    }

    /// Handle input. Returns bool of if to close
    fn input(
        &mut self,
        commander: &mut Commander,
        event: Event,
    ) -> anyhow::Result<crate::ComponentInputResult> {
        if let Some(creating) = self.creating.as_mut() {
            if let Event::Key(key) = event {
                match key.code {
                    _ if (key.code == KeyCode::Char('s')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                        || (key.code == KeyCode::Enter) =>
                    {
                        let name = &creating.lines().join("\n");
                        if name.trim().is_empty() {
                            return Ok(ComponentInputResult::Handled);
                        }

                        self.create_branch(commander, name)?;
                        self.tx.send(true)?;
                        return Ok(ComponentInputResult::HandledAction(
                            ComponentAction::SetPopup(None),
                        ));
                    }
                    KeyCode::Esc => {
                        return Ok(ComponentInputResult::Handled);
                    }
                    _ => {}
                }
            }

            creating.input(event);
            return Ok(ComponentInputResult::Handled);
        }

        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.scroll(1);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.scroll(-1);
                }
                KeyCode::Char('J') => {
                    self.scroll(self.list_height as isize / 2);
                }
                KeyCode::Char('K') => {
                    self.scroll((self.list_height as isize / 2).saturating_neg());
                }
                KeyCode::Char('g') => {
                    self.generate_branch(commander)?;
                    self.tx.send(true)?;
                    return Ok(ComponentInputResult::HandledAction(
                        ComponentAction::SetPopup(None),
                    ));
                }
                KeyCode::Char('c') => {
                    self.on_creating();
                }
                KeyCode::Enter => {
                    if let Some(action) = self
                        .list_state
                        .selected()
                        .and_then(|index| self.options.get(index))
                    {
                        match action {
                            BranchSetOption::CreateBranch => {
                                self.on_creating();
                            }
                            BranchSetOption::GeneratedName(_, _) => {
                                self.generate_branch(commander)?;
                                self.tx.send(true)?;
                                return Ok(ComponentInputResult::HandledAction(
                                    ComponentAction::SetPopup(None),
                                ));
                            }
                            BranchSetOption::Branch(branch) => {
                                commander.set_branch_commit(&branch.name, &self.commit_id)?;
                                self.tx.send(true)?;
                                return Ok(ComponentInputResult::HandledAction(
                                    ComponentAction::SetPopup(None),
                                ));
                            }
                            BranchSetOption::Error(_) => {
                                self.options = generate_options(commander, self.change_id.as_ref());
                            }
                        }
                    }
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.tx.send(false)?;
                    return Ok(ComponentInputResult::HandledAction(
                        ComponentAction::SetPopup(None),
                    ));
                }
                _ => return Ok(ComponentInputResult::NotHandled),
            }

            return Ok(ComponentInputResult::Handled);
        }

        Ok(ComponentInputResult::NotHandled)
    }
}
