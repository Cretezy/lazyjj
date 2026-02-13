use super::Shortcut;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct KeybindsConfig {
    pub log_tab: Option<LogTabKeybindsConfig>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
pub enum Keybind {
    Single(Shortcut),
    Multiple(Vec<Shortcut>),
    Enable(bool),
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LogTabKeybindsConfig {
    pub save: Option<Keybind>,
    pub cancel: Option<Keybind>,

    pub close_popup: Option<Keybind>,

    pub scroll_down: Option<Keybind>,
    pub scroll_up: Option<Keybind>,
    pub scroll_down_half: Option<Keybind>,
    pub scroll_up_half: Option<Keybind>,

    pub focus_current: Option<Keybind>,
    pub toggle_diff_format: Option<Keybind>,

    pub refresh: Option<Keybind>,
    pub duplicate: Option<Keybind>,
    pub create_new: Option<Keybind>,
    pub create_new_describe: Option<Keybind>,
    pub squash: Option<Keybind>,
    pub squash_ignore_immutable: Option<Keybind>,
    pub edit_change: Option<Keybind>,
    pub edit_change_ignore_immutable: Option<Keybind>,
    pub abandon: Option<Keybind>,
    pub describe: Option<Keybind>,
    pub edit_revset: Option<Keybind>,
    pub set_bookmark: Option<Keybind>,
    pub open_files: Option<Keybind>,
    pub rebase: Option<Keybind>,

    pub push: Option<Keybind>,
    pub push_new: Option<Keybind>,
    pub push_all: Option<Keybind>,
    pub push_all_new: Option<Keybind>,
    pub fetch: Option<Keybind>,
    pub fetch_all: Option<Keybind>,

    pub open_help: Option<Keybind>,
}
