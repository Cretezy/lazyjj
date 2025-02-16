use std::{fmt::Display, str::FromStr};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub use config::{Keybind, KeybindsConfig};
pub use log_tab::{LogTabEvent, LogTabKeybinds};

mod config;
mod keybinds_store;
mod log_tab;

/*#[derive(Debug)]
pub struct Keybinds {
    log_tab: LogTabKeybinds,
}*/

#[macro_export]
macro_rules! set_keybinds {
    () => {};
    ($keys:ident, $($action:expr => $shortcut:literal),* $(,)?) => {
        let mut __shortcuts_count = 0;
        $(
            $keys.add_action(Shortcut::from_str($shortcut).unwrap(), $action);
            __shortcuts_count += 1;
        )*
        assert_eq!(__shortcuts_count, $keys.len(), "shortcuts should not duplicate");
    };
}

#[macro_export]
macro_rules! update_keybinds {
    () => {};
    ($keys:expr, $($action:expr => $config:expr),* $(,)?) => {
        $(
            if let Some(ref k) = $config {
                $keys.replace_action_from_config($action, k);
            }
        )*
    };
}

#[macro_export]
macro_rules! make_keybinds_help {
    () => {};
    ($keys:expr, $($action:expr => $desc:literal),* $(,)?) => {
        #[allow(clippy::vec_init_then_push)]
        {
            let mut res = vec![];
            $(
                let shortcuts = $keys.get_shortcuts($action)
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join("/");
                if shortcuts.is_empty() {
                    res.push(("[disabled]".to_string(), $desc.to_string()));
                } else {
                    res.push((shortcuts, $desc.to_string()));
                }
            )*
            res
        }
    };
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, serde_with::DeserializeFromStr)]
pub struct Shortcut {
    key: KeyCode,
    modifiers: KeyModifiers,
}

impl Shortcut {
    pub fn new_mod_key(modifiers: KeyModifiers, key: KeyCode) -> Self {
        Self { key, modifiers }
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

impl FromStr for Shortcut {
    type Err = ShortcutParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut modifiers = KeyModifiers::empty();
        let mut key = None;
        for s in s.to_lowercase().split('+').map(|s| s.trim()) {
            match s {
                "ctrl" => modifiers |= KeyModifiers::CONTROL,
                "shift" => modifiers |= KeyModifiers::SHIFT,
                "enter" => key = Some(KeyCode::Enter),
                "esc" => key = Some(KeyCode::Esc),
                "left" => key = Some(KeyCode::Left),
                "right" => key = Some(KeyCode::Right),
                "up" => key = Some(KeyCode::Up),
                "down" => key = Some(KeyCode::Down),
                s if s.starts_with('f') && s.chars().count() > 1 => {
                    let s = s.trim_start_matches('f');
                    match s.parse::<u8>() {
                        Ok(k) => key = Some(KeyCode::F(k)),
                        Err(_) => return Err(ShortcutParseError::InvalidF),
                    }
                }
                s if s.chars().count() == 1 => {
                    let s = s.chars().last().unwrap();
                    key = Some(KeyCode::Char(s));
                }
                _ => (),
            }
        }

        if let Some(key) = key {
            Ok(Self::new_mod_key(modifiers, key))
        } else {
            Err(ShortcutParseError::NoKey)
        }
    }
}

impl Display for Shortcut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::with_capacity(3);
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("Control".to_string());
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push("Shift".to_string());
        }
        let k = match self.key {
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Left => "Left".to_string(),
            KeyCode::Right => "Right".to_string(),
            KeyCode::Up => "Up".to_string(),
            KeyCode::Down => "Down".to_string(),
            KeyCode::F(n) => format!("F{n}"),
            KeyCode::Char(c) => c.to_ascii_uppercase().to_string(),
            KeyCode::Esc => "Esc".to_string(),
            _ => "Unknown".to_string(),
        };
        parts.push(k);

        parts.join("+").fmt(f)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ShortcutParseError {
    #[error("invalid number after f")]
    InvalidF,
    #[error("no key specified")]
    NoKey,
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Shortcut {
        pub fn new_mod_char(modifiers: KeyModifiers, key: char) -> Self {
            Self::new_mod_key(modifiers, KeyCode::Char(key))
        }
        pub fn new_char(key: char) -> Self {
            Self::new_mod_key(KeyModifiers::empty(), KeyCode::Char(key))
        }
        pub fn new_key(key: KeyCode) -> Self {
            Self::new_mod_key(KeyModifiers::empty(), key)
        }
    }

    #[test]
    fn test_shortcut_from_str() {
        let ctrl = KeyModifiers::CONTROL;
        let shift = KeyModifiers::SHIFT;
        let ctrl_shift = ctrl | shift;

        let table = [
            ("q", Ok(Shortcut::new_char('q'))),
            ("Q", Ok(Shortcut::new_char('q'))),
            ("f", Ok(Shortcut::new_char('f'))),
            ("@", Ok(Shortcut::new_char('@'))),
            ("super+q", Ok(Shortcut::new_char('q'))),
            ("ctrl+q", Ok(Shortcut::new_mod_char(ctrl, 'q'))),
            ("ctrl+a+q", Ok(Shortcut::new_mod_char(ctrl, 'q'))),
            ("ctrl+Q", Ok(Shortcut::new_mod_char(ctrl, 'q'))),
            ("ctrl+ctrl+q", Ok(Shortcut::new_mod_char(ctrl, 'q'))),
            ("ctrl+shift+q", Ok(Shortcut::new_mod_char(ctrl_shift, 'q'))),
            (
                "ctrl+shift+f5",
                Ok(Shortcut::new_mod_key(ctrl_shift, KeyCode::F(5))),
            ),
            (
                "ctrl+shift+f25",
                Ok(Shortcut::new_mod_key(ctrl_shift, KeyCode::F(25))),
            ),
            ("enter", Ok(Shortcut::new_key(KeyCode::Enter))),
            (
                "ctrl+enter",
                Ok(Shortcut::new_mod_key(ctrl, KeyCode::Enter)),
            ),
            ("esc", Ok(Shortcut::new_key(KeyCode::Esc))),
            ("left", Ok(Shortcut::new_key(KeyCode::Left))),
            ("right", Ok(Shortcut::new_key(KeyCode::Right))),
            ("up", Ok(Shortcut::new_key(KeyCode::Up))),
            ("down", Ok(Shortcut::new_key(KeyCode::Down))),
            ("ctrl+ff", Err(ShortcutParseError::InvalidF)),
            ("qq", Err(ShortcutParseError::NoKey)),
            ("", Err(ShortcutParseError::NoKey)),
        ];

        for (s, expected) in table {
            assert_eq!(
                Shortcut::from_str(s),
                expected,
                "Shortcut::from_str(\"{s}\")"
            );
        }
    }
}
