use std::str::FromStr;

use crossterm::event::KeyEvent;

use crate::{set_keybinds, update_keybinds};

use super::{config::LogTabKeybindsConfig, keybinds_store::KeybindsStore, Shortcut};

#[derive(Debug)]
pub struct LogTabKeybinds {
    // todo: probably split keys for different contexts, e.g when describe_textarea is opened
    keys: KeybindsStore<LogTabEvent>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LogTabEvent {
    Save,
    Cancel,

    ClosePopup,

    ScrollDown,
    ScrollUp,
    ScrollDownHalf,
    ScrollUpHalf,

    FocusCurrent,
    ToggleDiffFormat,

    Refresh,
    CreateNew {
        describe: bool,
    },
    Squash,
    EditChange,
    Abandon,
    Describe,
    EditRevset,
    SetBookmark,
    OpenFiles,

    Push {
        all_bookmarks: bool,
        allow_new: bool,
    },
    Fetch {
        all_remotes: bool,
    },

    OpenHelp,

    Unbound,
}

impl Default for LogTabKeybinds {
    fn default() -> Self {
        let mut keys = KeybindsStore::<LogTabEvent>::default();

        let push = |all_bookmarks, allow_new| LogTabEvent::Push {
            all_bookmarks,
            allow_new,
        };
        set_keybinds!(
            keys,
            LogTabEvent::Save => "ctrl+s",
            LogTabEvent::Cancel => "esc",
            LogTabEvent::ClosePopup => "q",
            LogTabEvent::ScrollDown => "j",
            LogTabEvent::ScrollDown => "down",
            LogTabEvent::ScrollUp => "k",
            LogTabEvent::ScrollUp => "up",
            LogTabEvent::ScrollDownHalf => "shift+j",
            LogTabEvent::ScrollUpHalf => "shift+k",
            LogTabEvent::FocusCurrent => "@",
            LogTabEvent::ToggleDiffFormat => "w",
            LogTabEvent::Refresh => "shift+r",
            LogTabEvent::Refresh => "f5",
            LogTabEvent::CreateNew { describe: false } => "n",
            LogTabEvent::CreateNew { describe: true } => "shift+n",
            LogTabEvent::Squash => "s",
            LogTabEvent::EditChange => "e",
            LogTabEvent::Abandon => "a",
            LogTabEvent::Describe => "d",
            LogTabEvent::EditRevset => "r",
            LogTabEvent::SetBookmark => "b",
            LogTabEvent::OpenFiles => "enter",
            push(false, false) => "p",
            push(false, true) => "ctrl+p",
            push(true, false) => "shift+p",
            push(true, true) => "ctrl+shift+p",
            LogTabEvent::Fetch { all_remotes: false } => "f",
            LogTabEvent::Fetch { all_remotes: true } => "shift+f",
            LogTabEvent::OpenHelp => "h",
            LogTabEvent::OpenHelp => "?",
        );

        Self { keys }
    }
}

impl LogTabKeybinds {
    pub fn match_event(&self, event: KeyEvent) -> LogTabEvent {
        if let Some(action) = self.keys.match_event(event) {
            action
        } else {
            LogTabEvent::Unbound
        }
    }
    pub fn extend_from_config(&mut self, config: &LogTabKeybindsConfig) {
        let push = |all_bookmarks, allow_new| LogTabEvent::Push {
            all_bookmarks,
            allow_new,
        };
        update_keybinds!(
            self.keys,
            LogTabEvent::Save => config.save,
            LogTabEvent::Cancel => config.cancel,
            LogTabEvent::ClosePopup => config.close_popup,
            LogTabEvent::ScrollDown => config.scroll_down,
            LogTabEvent::ScrollDown => config.scroll_down,
            LogTabEvent::ScrollUp => config.scroll_up,
            LogTabEvent::ScrollUp => config.scroll_up,
            LogTabEvent::ScrollDownHalf => config.scroll_down_half,
            LogTabEvent::ScrollUpHalf => config.scroll_up_half,
            LogTabEvent::FocusCurrent => config.focus_current,
            LogTabEvent::ToggleDiffFormat => config.toggle_diff_format,
            LogTabEvent::Refresh => config.refresh,
            LogTabEvent::CreateNew { describe: false } => config.create_new,
            LogTabEvent::CreateNew { describe: true } => config.create_new_describe,
            LogTabEvent::Squash => config.squash,
            LogTabEvent::EditChange => config.edit_change,
            LogTabEvent::Abandon => config.abandon,
            LogTabEvent::Describe => config.describe,
            LogTabEvent::EditRevset => config.edit_revset,
            LogTabEvent::SetBookmark => config.set_bookmark,
            LogTabEvent::OpenFiles => config.open_files,
            push(false, false) => config.push,
            push(false, true) => config.push_new,
            push(true, false) => config.push_all,
            push(true, true) => config.push_all_new,
            LogTabEvent::Fetch { all_remotes: false } => config.fetch,
            LogTabEvent::Fetch { all_remotes: true } => config.fetch_all,
            LogTabEvent::OpenHelp => config.open_help,
        );
    }
}

#[test]
fn test_log_tab_keybinds_default() {
    let _ = LogTabKeybinds::default();
}
