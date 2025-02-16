use std::collections::HashMap;

use crossterm::event::KeyEvent;

use super::Shortcut;

#[derive(Debug)]
pub struct KeybindsStore<A>
where
    A: Clone,
{
    shortcut_actions: HashMap<Shortcut, A>,
}

impl<A> KeybindsStore<A>
where
    A: Clone,
{
    pub fn match_event(&self, event: KeyEvent) -> Option<A> {
        self.shortcut_actions.get(&Shortcut::from_event(event)).map(ToOwned::to_owned)
    }
    pub fn add_action(&mut self, shortcut: Shortcut, action: A) {
        self.shortcut_actions.insert(shortcut, action);
    }
    pub fn len(&self) -> usize {
        self.shortcut_actions.len()
    }
}

impl<A> Default for KeybindsStore<A>
where
    A: Clone,
{
    fn default() -> Self {
        Self {
            shortcut_actions: HashMap::new(),
        }
    }
}
