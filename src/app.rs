use crate::{
    commander::Commander,
    env::Env,
    ui::{
        branches_tab::BranchesTab, command_log_tab::CommandLogTab, files_tab::FilesTab,
        log_tab::LogTab, Component, ComponentAction,
    },
    ComponentInputResult,
};
use anyhow::{anyhow, Result};
use core::fmt;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use tracing::{info, info_span};

#[derive(PartialEq, Copy, Clone)]
pub enum Tab {
    Log,
    Files,
    Branches,
    CommandLog,
}

impl fmt::Display for Tab {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Tab::Log => write!(f, "Log"),
            Tab::Files => write!(f, "Files"),
            Tab::Branches => write!(f, "Branches"),
            Tab::CommandLog => write!(f, "Command Log"),
        }
    }
}

impl Tab {
    pub const VALUES: [Self; 4] = [Tab::Log, Tab::Files, Tab::Branches, Tab::CommandLog];
}

pub struct App<'a> {
    pub env: Env,
    pub current_tab: Tab,
    pub log: Option<LogTab<'a>>,
    pub files: Option<FilesTab>,
    pub branches: Option<BranchesTab<'a>>,
    pub command_log: Option<CommandLogTab>,
    pub popup: Option<Box<dyn Component>>,
}

impl<'a> App<'a> {
    pub fn new(env: Env) -> Result<App<'a>> {
        Ok(App {
            env,
            current_tab: Tab::Log,
            log: None,
            files: None,
            branches: None,
            command_log: None,
            popup: None,
        })
    }

    pub fn get_or_init_current_tab(
        &mut self,
        commander: &mut Commander,
    ) -> Result<&mut dyn Component> {
        self.get_or_init_tab(commander, self.current_tab)
    }
    pub fn get_current_tab(&mut self) -> Option<&mut dyn Component> {
        self.get_tab(self.current_tab)
    }

    pub fn set_tab(&mut self, commander: &mut Commander, tab: Tab) -> Result<()> {
        info!("Setting tab to {}", tab);
        self.current_tab = tab;

        self.get_or_init_current_tab(commander)?.switch(commander)?;
        Ok(())
    }

    pub fn get_log_tab(&mut self, commander: &mut Commander) -> Result<&mut LogTab<'a>> {
        if self.log.is_none() {
            let span = info_span!("Initializing log tab");
            let log_tab = span.in_scope(|| LogTab::new(commander))?;
            self.log = Some(log_tab);
        }

        self.log
            .as_mut()
            .ok_or_else(|| anyhow!("Failed to get mutable reference to LogTab"))
    }

    pub fn get_files_tab(&mut self, commander: &mut Commander) -> Result<&mut FilesTab> {
        if self.files.is_none() {
            let span = info_span!("Initializing files tab");
            let files_tab = span.in_scope(|| {
                let current_head = commander.get_current_head()?;
                FilesTab::new(commander, &current_head)
            })?;
            self.files = Some(files_tab);
        }

        self.files
            .as_mut()
            .ok_or_else(|| anyhow!("Failed to get mutable reference to FilesTab"))
    }

    pub fn get_branches_tab(&mut self, commander: &mut Commander) -> Result<&mut BranchesTab<'a>> {
        if self.branches.is_none() {
            let span = info_span!("Initializing branches tab");
            let branches_tab = span.in_scope(|| BranchesTab::new(commander))?;
            self.branches = Some(branches_tab);
        }

        self.branches
            .as_mut()
            .ok_or_else(|| anyhow!("Failed to get mutable reference to BranchesTab"))
    }

    pub fn get_command_log_tab(&mut self, commander: &mut Commander) -> Result<&mut CommandLogTab> {
        if self.command_log.is_none() {
            let span = info_span!("Initializing command log tab");
            let command_log_tab = span.in_scope(|| CommandLogTab::new(commander))?;
            self.command_log = Some(command_log_tab);
        }

        self.command_log
            .as_mut()
            .ok_or_else(|| anyhow!("Failed to get mutable reference to CommandLogTab"))
    }

    pub fn get_or_init_tab(
        &mut self,
        commander: &mut Commander,
        tab: Tab,
    ) -> Result<&mut dyn Component> {
        Ok(match tab {
            Tab::Log => self.get_log_tab(commander)?,
            Tab::Files => self.get_files_tab(commander)?,
            Tab::Branches => self.get_branches_tab(commander)?,
            Tab::CommandLog => self.get_command_log_tab(commander)?,
        })
    }

    pub fn get_tab(&mut self, tab: Tab) -> Option<&mut dyn Component> {
        match tab {
            Tab::Log => self
                .log
                .as_mut()
                .map(|log_tab| log_tab as &mut dyn Component),
            Tab::Files => self
                .files
                .as_mut()
                .map(|files_tab| files_tab as &mut dyn Component),
            Tab::Branches => self
                .branches
                .as_mut()
                .map(|branches_tab| branches_tab as &mut dyn Component),
            Tab::CommandLog => self
                .command_log
                .as_mut()
                .map(|command_log_tab| command_log_tab as &mut dyn Component),
        }
    }

    pub fn handle_action(
        &mut self,
        component_action: ComponentAction,
        commander: &mut Commander,
    ) -> Result<()> {
        match component_action {
            ComponentAction::ViewFiles(head) => {
                self.set_tab(commander, Tab::Files)?;
                self.get_files_tab(commander)?.set_head(commander, &head)?;
            }
            ComponentAction::ViewLog(head) => {
                self.get_log_tab(commander)?.set_head(commander, head);
                self.set_tab(commander, Tab::Log)?;
            }
            ComponentAction::ChangeHead(head) => {
                self.get_files_tab(commander)?.set_head(commander, &head)?;
            }
            ComponentAction::SetPopup(popup) => {
                self.popup = popup;
            }
            ComponentAction::Multiple(component_actions) => {
                for component_action in component_actions.into_iter() {
                    self.handle_action(component_action, commander)?;
                }
            }
        }

        Ok(())
    }

    pub fn input(&mut self, event: Event, commander: &mut Commander) -> Result<bool> {
        if let Some(popup) = self.popup.as_mut() {
            match popup.input(commander, event.clone())? {
                ComponentInputResult::HandledAction(component_action) => {
                    self.handle_action(component_action, commander)?
                }
                ComponentInputResult::Handled => {}
                ComponentInputResult::NotHandled => {
                    if let Event::Key(key) = event {
                        if key.kind == event::KeyEventKind::Press {
                            // Close
                            if matches!(
                                key.code,
                                KeyCode::Char('y')
                                    | KeyCode::Char('n')
                                    | KeyCode::Char('o')
                                    | KeyCode::Enter
                                    | KeyCode::Char('q')
                                    | KeyCode::Esc
                            ) {
                                self.popup = None
                            }
                        }
                    }
                }
            };
        } else {
            match self
                .get_or_init_current_tab(commander)?
                .input(commander, event.clone())?
            {
                ComponentInputResult::HandledAction(component_action) => {
                    self.handle_action(component_action, commander)?
                }
                ComponentInputResult::Handled => {}
                ComponentInputResult::NotHandled => {
                    if let Event::Key(key) = event {
                        if key.kind == event::KeyEventKind::Press {
                            // Close
                            if key.code == KeyCode::Char('q')
                                || (key.modifiers.contains(KeyModifiers::CONTROL)
                                    && (key.code == KeyCode::Char('c')))
                                || key.code == KeyCode::Esc
                            {
                                return Ok(true);
                            }
                            //
                            // Tab switching
                            if let Some((_, tab)) = Tab::VALUES.iter().enumerate().find(|(i, _)| {
                                key.code
                                    == KeyCode::Char(
                                        char::from_digit((*i as u32) + 1u32, 10)
                                            .expect("Tab index could not be converted to digit"),
                                    )
                            }) {
                                self.set_tab(commander, *tab)?;
                            }
                        }
                    }
                }
            };
        }

        Ok(false)
    }
}
