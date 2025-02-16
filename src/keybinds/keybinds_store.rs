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
    pub fn add_shortcuts<I: IntoIterator<Item = Shortcut>>(&mut self, shortcuts: I, action: A) {
        for s in shortcuts {
            self.add_action(s, action.clone());
        }
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
