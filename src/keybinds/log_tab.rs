use std::str::FromStr;

use crossterm::event::KeyEvent;

use crate::{make_keybinds_help, set_keybinds, update_keybinds};

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
    Squash {
        ignore_immutable: bool,
    },
    EditChange {
        ignore_immutable: bool,
    },
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
            // todo: move to DetailsKeybindings
            LogTabEvent::ToggleDiffFormat => "w",
            LogTabEvent::Refresh => "shift+r",
            LogTabEvent::Refresh => "f5",
            LogTabEvent::CreateNew { describe: false } => "n",
            LogTabEvent::CreateNew { describe: true } => "shift+n",
            LogTabEvent::Squash { ignore_immutable: false } => "s",
            LogTabEvent::Squash { ignore_immutable: true } => "shift+s",
            LogTabEvent::EditChange { ignore_immutable: false } => "e",
            LogTabEvent::EditChange { ignore_immutable: true } => "shift+e",
            LogTabEvent::Abandon => "a",
            LogTabEvent::Describe => "d",
            LogTabEvent::EditRevset => "r",
            LogTabEvent::SetBookmark => "b",
            LogTabEvent::OpenFiles => "enter",
            event_push(false, false) => "p",
            event_push(false, true) => "ctrl+p",
            event_push(true, false) => "shift+p",
            event_push(true, true) => "ctrl+shift+p",
            LogTabEvent::Fetch { all_remotes: false } => "f",
            LogTabEvent::Fetch { all_remotes: true } => "shift+f",
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
        update_keybinds!(
            self.keys,
            LogTabEvent::Save => config.save,
            LogTabEvent::Cancel => config.cancel,
            LogTabEvent::ClosePopup => config.close_popup,
            LogTabEvent::ScrollDown => config.scroll_down,
            LogTabEvent::ScrollUp => config.scroll_up,
            LogTabEvent::ScrollDownHalf => config.scroll_down_half,
            LogTabEvent::ScrollUpHalf => config.scroll_up_half,
            LogTabEvent::FocusCurrent => config.focus_current,
            LogTabEvent::ToggleDiffFormat => config.toggle_diff_format,
            LogTabEvent::Refresh => config.refresh,
            LogTabEvent::CreateNew { describe: false } => config.create_new,
            LogTabEvent::CreateNew { describe: true } => config.create_new_describe,
            LogTabEvent::Squash { ignore_immutable: false } => config.squash,
            LogTabEvent::Squash { ignore_immutable: true } => config.squash_ignore_immutable,
            LogTabEvent::EditChange { ignore_immutable: false } => config.edit_change,
            LogTabEvent::EditChange { ignore_immutable: true } => config.edit_change_ignore_immutable,
            LogTabEvent::Abandon => config.abandon,
            LogTabEvent::Describe => config.describe,
            LogTabEvent::EditRevset => config.edit_revset,
            LogTabEvent::SetBookmark => config.set_bookmark,
            LogTabEvent::OpenFiles => config.open_files,
            event_push(false, false) => config.push,
            event_push(false, true) => config.push_new,
            event_push(true, false) => config.push_all,
            event_push(true, true) => config.push_all_new,
            LogTabEvent::Fetch { all_remotes: false } => config.fetch,
            LogTabEvent::Fetch { all_remotes: true } => config.fetch_all,
            LogTabEvent::OpenHelp => config.open_help,
        );
    }
    pub fn make_main_panel_help(&self) -> Vec<(String, String)> {
        make_keybinds_help!(
            self.keys,
            LogTabEvent::ScrollDown => "scroll down",
            LogTabEvent::ScrollUp => "scroll up",
            LogTabEvent::ScrollDownHalf => "scroll down by ½ page",
            LogTabEvent::ScrollUpHalf => "scroll up by ½ page",
            LogTabEvent::OpenFiles => "see files",
            LogTabEvent::FocusCurrent => "current change",
            LogTabEvent::EditRevset => "set revset",
            LogTabEvent::Describe => "describe change",
            LogTabEvent::EditChange { ignore_immutable: false } => "edit change",
            LogTabEvent::EditChange { ignore_immutable: true } => "edit change ignoring immutability",
            LogTabEvent::CreateNew { describe: false } => "new change",
            LogTabEvent::CreateNew { describe: true } => "new with message",
            LogTabEvent::Abandon => "abandon change",
            LogTabEvent::Squash { ignore_immutable: false } => "squash @ into the selected change",
            LogTabEvent::Squash { ignore_immutable: true } => "squash @ into the selected change ignoring immutability",
            LogTabEvent::SetBookmark => "set bookmark",
            LogTabEvent::Fetch { all_remotes: false } => "git fetch",
            LogTabEvent::Fetch { all_remotes: true } => "git fetch all remotes",
            event_push(false, false) => "git push",
            event_push(false, true) => "git push with new bookmarks",
            event_push(true, false) => "git push all bookmarks, except new",
            event_push(true, true) => "git push all bookmarks",
        )
    }
}

fn event_push(all_bookmarks: bool, allow_new: bool) -> LogTabEvent {
    LogTabEvent::Push {
        all_bookmarks,
        allow_new,
    }
}

#[test]
fn test_log_tab_keybinds_default() {
    let _ = LogTabKeybinds::default();
}
