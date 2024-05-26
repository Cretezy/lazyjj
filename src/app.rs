use crate::{
    commander::Commander,
    env::Env,
    ui::{branches::Branches, command_log::CommandLog, files::Files, log::Log, ComponentAction},
};
use anyhow::Result;
use core::fmt;

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
    pub log: Log<'a>,
    pub files: Files,
    pub branches: Branches<'a>,
    pub command_log: CommandLog,
    pub textarea_active: bool,
}

impl App<'_> {
    pub fn new<'a>(env: Env, commander: &mut Commander) -> Result<App<'a>> {
        let current_head = &commander.get_current_head()?;
        // TODO: Lazy load tabs on open
        Ok(App {
            env,
            current_tab: Tab::Log,
            log: Log::new(commander)?,
            files: Files::new(commander, current_head)?,
            branches: Branches::new(commander)?,
            command_log: CommandLog::new(commander)?,
            textarea_active: false,
        })
    }

    pub fn handle_action(
        &mut self,
        component_action: ComponentAction,
        commander: &mut Commander,
    ) -> Result<()> {
        match component_action {
            ComponentAction::ViewFiles(head) => {
                self.files.set_head(commander, &head)?;
                self.current_tab = Tab::Files;
                self.get_current_component_mut().switch(commander)?;
            }
            ComponentAction::ChangeHead(head) => {
                self.files.set_head(commander, &head)?;
            }
            ComponentAction::SetTextAreaActive(textarea_active) => {
                self.textarea_active = textarea_active;
            }
            ComponentAction::Multiple(component_actions) => {
                for component_action in component_actions.into_iter() {
                    self.handle_action(component_action, commander)?;
                }
            }
        }

        Ok(())
    }
}
