use std::collections::HashMap;

use crossterm::event::KeyEvent;

use super::{Keybind, Shortcut};

#[derive(Debug)]
pub struct KeybindsStore<A> {
    shortcut_actions: HashMap<Shortcut, A>,
}

impl<A> KeybindsStore<A>
where
    A: Clone + Eq,
{
    pub fn match_event(&self, event: KeyEvent) -> Option<A> {
        self.shortcut_actions
            .get(&Shortcut::from_event(event))
            .map(ToOwned::to_owned)
    }
    pub fn add_action(&mut self, shortcut: Shortcut, action: A) {
        self.shortcut_actions.insert(shortcut, action);
    }
    pub fn get_shortcuts(&self, action: A) -> Vec<Shortcut> {
        self.shortcut_actions
            .iter()
            .filter(|(_, a)| **a == action)
            .map(|(s, _)| *s)
            .collect()
    }
    pub fn replace_action_from_config(&mut self, action: A, key: &Keybind) {
        // just ignore this case
        if matches!(key, Keybind::Enable(true)) {
            return;
        }

        self.remove_action(action.clone());
        match key {
            Keybind::Single(s) => self.add_action(*s, action),
            Keybind::Multiple(list) => {
                for s in list {
                    self.add_action(*s, action.clone());
                }
            }
            // in case Enable(false) action is only removed
            Keybind::Enable(_) => (),
        }
    }
    /// Remove all shortcuts for specified action
    fn remove_action(&mut self, action: A) {
        self.shortcut_actions.retain(|_, a| action != *a);
    }
    pub fn len(&self) -> usize {
        self.shortcut_actions.len()
    }
}

impl<A> Default for KeybindsStore<A> {
    fn default() -> Self {
        Self {
            shortcut_actions: HashMap::new(),
        }
    }
}
