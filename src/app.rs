use crate::{
    commander::Commander,
    env::Env,
    ui::{
        branches_tab::BranchesTab, command_log_tab::CommandLogTag, files_tab::FilesTab,
        log_tab::LogTab, Component, ComponentAction,
    },
    ComponentInputResult,
};
use anyhow::Result;
use core::fmt;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};

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
    pub log: LogTab<'a>,
    pub files: FilesTab,
    pub branches: BranchesTab<'a>,
    pub command_log: CommandLogTag,
    pub popup: Option<Box<dyn Component>>,
}

impl App<'_> {
    pub fn new<'a>(env: Env, commander: &mut Commander) -> Result<App<'a>> {
        let current_head = &commander.get_current_head()?;
        // TODO: Lazy load tabs on open
        Ok(App {
            env,
            current_tab: Tab::Log,
            log: LogTab::new(commander)?,
            files: FilesTab::new(commander, current_head)?,
            branches: BranchesTab::new(commander)?,
            command_log: CommandLogTag::new(commander)?,
            popup: None,
        })
    }

    pub fn set_tab(&mut self, commander: &mut Commander, tab: Tab) -> Result<()> {
        self.current_tab = tab;
        self.get_current_component_mut().switch(commander)?;
        Ok(())
    }

    pub fn handle_action(
        &mut self,
        component_action: ComponentAction,
        commander: &mut Commander,
    ) -> Result<()> {
        match component_action {
            ComponentAction::ViewFiles(head) => {
                self.files.set_head(commander, &head)?;
                self.set_tab(commander, Tab::Files)?;
            }
            ComponentAction::ViewLog(head) => {
                self.log.set_head(commander, head);
                self.set_tab(commander, Tab::Log)?;
            }
            ComponentAction::ChangeHead(head) => {
                self.files.set_head(commander, &head)?;
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
            }
        } else {
            match self
                .get_current_component_mut()
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
