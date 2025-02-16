use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{keybinds_store::KeybindsStore, Shortcut};

#[derive(Debug)]
pub struct LogTabKeybinds {
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
        let ctrl = KeyModifiers::CONTROL;
        let shift = KeyModifiers::SHIFT;
        let ctrl_shift = ctrl | shift;

        let mut keys = KeybindsStore::<LogTabEvent>::default();

        keys.add_action(
            Shortcut::new_mod_char(KeyModifiers::CONTROL, 's'),
            LogTabEvent::Save,
        );
        keys.add_action(Shortcut::new_key(KeyCode::Esc), LogTabEvent::Cancel);
        keys.add_action(Shortcut::new_char('q'), LogTabEvent::ClosePopup);

        keys.add_shortcuts(
            [Shortcut::new_char('j'), Shortcut::new_key(KeyCode::Down)],
            LogTabEvent::ScrollDown,
        );
        keys.add_shortcuts(
            [Shortcut::new_char('k'), Shortcut::new_key(KeyCode::Up)],
            LogTabEvent::ScrollUp,
        );
        keys.add_shortcuts(
            [Shortcut::new_mod_char(shift, 'j')],
            LogTabEvent::ScrollDownHalf,
        );
        keys.add_shortcuts(
            [Shortcut::new_mod_char(shift, 'k')],
            LogTabEvent::ScrollUpHalf,
        );

        keys.add_action(Shortcut::new_char('@'), LogTabEvent::FocusCurrent);
        keys.add_action(Shortcut::new_char('w'), LogTabEvent::ToggleDiffFormat);
        keys.add_shortcuts(
            [
                Shortcut::new_mod_char(shift, 'r'),
                Shortcut::new_key(KeyCode::F(5)),
            ],
            LogTabEvent::Refresh,
        );
        keys.add_action(
            Shortcut::new_char('n'),
            LogTabEvent::CreateNew { describe: false },
        );
        keys.add_action(
            Shortcut::new_mod_char(shift, 'n'),
            LogTabEvent::CreateNew { describe: true },
        );
        keys.add_action(Shortcut::new_char('s'), LogTabEvent::Squash);
        keys.add_action(Shortcut::new_char('e'), LogTabEvent::EditChange);
        keys.add_action(Shortcut::new_char('a'), LogTabEvent::Abandon);
        keys.add_action(Shortcut::new_char('d'), LogTabEvent::Describe);
        keys.add_action(Shortcut::new_char('r'), LogTabEvent::EditRevset);
        keys.add_action(Shortcut::new_char('b'), LogTabEvent::SetBookmark);
        keys.add_action(Shortcut::new_key(KeyCode::Enter), LogTabEvent::OpenFiles);
        keys.add_action(
            Shortcut::new_char('p'),
            LogTabEvent::Push {
                all_bookmarks: false,
                allow_new: false,
            },
        );
        keys.add_action(
            Shortcut::new_mod_char(ctrl, 'p'),
            LogTabEvent::Push {
                all_bookmarks: false,
                allow_new: false,
            },
        );
        keys.add_action(
            Shortcut::new_mod_char(shift, 'p'),
            LogTabEvent::Push {
                all_bookmarks: false,
                allow_new: false,
            },
        );
        keys.add_action(
            Shortcut::new_mod_char(ctrl_shift, 'p'),
            LogTabEvent::Push {
                all_bookmarks: false,
                allow_new: false,
            },
        );
        keys.add_action(
            Shortcut::new_char('f'),
            LogTabEvent::Fetch { all_remotes: false },
        );
        keys.add_action(
            Shortcut::new_mod_char(shift, 'f'),
            LogTabEvent::Fetch { all_remotes: true },
        );
        keys.add_shortcuts(
            [Shortcut::new_char('h'), Shortcut::new_char('?')],
            LogTabEvent::OpenHelp,
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
}
