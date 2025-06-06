use std::str::FromStr;

use ratatui::crossterm::event::KeyEvent;

use crate::{make_keybinds_help, set_keybinds, update_keybinds};

use super::{config::FilesTabKeybindsConfig, keybinds_store::KeybindsStore, Shortcut};

#[derive(Debug)]
pub struct FilesTabKeybinds {
    // todo: probably split keys for different contexts, e.g when describe_textarea is opened
    keys: KeybindsStore<FilesTabEvent>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FilesTabEvent {
    ScrollDown,
    ScrollUp,
    ScrollDownHalf,
    ScrollUpHalf,

    FocusCurrent,
    ToggleDiffFormat,

    Refresh,
    OpenHelp,

    Unbound,
}

impl Default for FilesTabKeybinds {
    fn default() -> Self {
        let mut keys = KeybindsStore::<FilesTabEvent>::default();
        set_keybinds!(
            keys,
            FilesTabEvent::ScrollDown => "j",
            FilesTabEvent::ScrollDown => "down",
            FilesTabEvent::ScrollUp => "k",
            FilesTabEvent::ScrollUp => "up",
            FilesTabEvent::ScrollDownHalf => "shift+j",
            FilesTabEvent::ScrollUpHalf => "shift+k",
            FilesTabEvent::FocusCurrent => "@",
            // todo: move to DetailsKeybindings
            FilesTabEvent::ToggleDiffFormat => "w",
            FilesTabEvent::Refresh => "shift+r",
            FilesTabEvent::Refresh => "f5",
            FilesTabEvent::OpenHelp => "?",
        );

        Self { keys }
    }
}

impl FilesTabKeybinds {
    pub fn match_event(&self, event: KeyEvent) -> FilesTabEvent {
        if let Some(action) = self.keys.match_event(event) {
            action
        } else {
            FilesTabEvent::Unbound
        }
    }
    pub fn extend_from_config(&mut self, config: &FilesTabKeybindsConfig) {
        update_keybinds!(
            self.keys,
            FilesTabEvent::ScrollDown => config.scroll_down,
            FilesTabEvent::ScrollUp => config.scroll_up,
            FilesTabEvent::ScrollDownHalf => config.scroll_down_half,
            FilesTabEvent::ScrollUpHalf => config.scroll_up_half,
            FilesTabEvent::FocusCurrent => config.focus_current,
            FilesTabEvent::ToggleDiffFormat => config.toggle_diff_format,
            FilesTabEvent::Refresh => config.refresh,
            FilesTabEvent::OpenHelp => config.open_help,
        );
    }
    pub fn make_main_panel_help(&self) -> Vec<(String, String)> {
        make_keybinds_help!(
            self.keys,
            FilesTabEvent::ScrollDown => "scroll down",
            FilesTabEvent::ScrollUp => "scroll up",
            FilesTabEvent::ScrollDownHalf => "scroll down by ½ page",
            FilesTabEvent::ScrollUpHalf => "scroll up by ½ page",
            FilesTabEvent::FocusCurrent => "view files from current change",
        )
    }
}

#[test]
fn test_files_tab_keybinds_default() {
    let _ = FilesTabKeybinds::default();
}
