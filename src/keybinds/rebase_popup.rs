/*! Key bindings specific for rebase popup */

use ratatui::crossterm::event::KeyEvent;
use std::str::FromStr; // used by set_keybinds macro

use super::{Shortcut, keybinds_store::KeybindsStore};
use crate::set_keybinds;

/// How should rebase cut revisions from source
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CutOption {
    IncludeDescendants, // -s
    IncludeBranch,      // -b
    SingleRevision,     // -r
}

/// How should rebase paste revisions at target
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PasteOption {
    NewBranch,    // -d
    InsertAfter,  // -A
    InsertBefore, // -B
}

/// Actions available inside a RebasePopup
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PopupAction {
    None,
    Ok,
    Cancel,
    SetSourceMode(CutOption),
    SetTargetMode(PasteOption),
}

fn default_keybinds() -> KeybindsStore<PopupAction> {
    let mut keys = KeybindsStore::<PopupAction>::default();
    set_keybinds!(
        keys,
        PopupAction::Ok => "enter",
        PopupAction::Cancel => "esc",
        PopupAction::Cancel => "q",
        PopupAction::SetSourceMode(CutOption::IncludeDescendants) => "s",
        PopupAction::SetSourceMode(CutOption::IncludeBranch) => "b",
        PopupAction::SetSourceMode(CutOption::SingleRevision) => "r",
        PopupAction::SetTargetMode(PasteOption::NewBranch) => "d",
        PopupAction::SetTargetMode(PasteOption::InsertAfter) => "shift+a",
        PopupAction::SetTargetMode(PasteOption::InsertBefore) => "shift+b",
    );
    keys
}

#[derive(Debug)]
pub struct Keybinds {
    keys: KeybindsStore<PopupAction>,
}

impl Default for Keybinds {
    fn default() -> Self {
        Self {
            keys: default_keybinds(),
        }
    }
}

impl Keybinds {
    pub fn match_event(&self, event: KeyEvent) -> PopupAction {
        if let Some(action) = self.keys.match_event(event) {
            action
        } else {
            PopupAction::None
        }
    }
}
