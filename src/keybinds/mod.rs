use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub use log_tab::{LogTabEvent, LogTabKeybinds};

mod keybinds_store;
mod log_tab;

/*#[derive(Debug)]
pub struct Keybinds {
    log_tab: LogTabKeybinds,
}*/

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Shortcut {
    key: KeyCode,
    modifiers: KeyModifiers,
}

impl Shortcut {
    pub fn new_mod_key(modifiers: KeyModifiers, key: KeyCode) -> Self {
        Self { key, modifiers }
    }
    pub fn new_mod_char(modifiers: KeyModifiers, key: char) -> Self {
        Self::new_mod_key(modifiers, KeyCode::Char(key))
    }
    pub fn new_char(key: char) -> Self {
        Self::new_mod_key(KeyModifiers::empty(), KeyCode::Char(key))
    }
    pub fn new_key(key: KeyCode) -> Self {
        Self::new_mod_key(KeyModifiers::empty(), key)
    }
    pub fn from_event(event: KeyEvent) -> Self {
        Self {
            key: match event.code {
                KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
                c => c,
            },
            modifiers: event.modifiers,
        }
    }
}

/*impl FromStr for Shortcut {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}*/
