use std::{path::PathBuf, process::Command};

use anyhow::{bail, Context, Result};
use ratatui::style::Color;
use serde::Deserialize;

use crate::{
    commander::{get_output_args, RemoveEndLine},
    keybinds::KeybindsConfig,
};

// TODO: After 0.18, remove Config and replace with JjConfig
#[derive(Deserialize, Debug, Clone, Default)]
pub struct Config {
    #[serde(rename = "lazyjj.highlight-color")]
    lazyjj_highlight_color: Option<Color>,
    #[serde(rename = "lazyjj.diff-format")]
    lazyjj_diff_format: Option<DiffFormat>,
    #[serde(rename = "lazyjj.bookmark-prefix")]
    lazyjj_bookmark_prefix: Option<String>,
    #[serde(rename = "lazyjj.layout")]
    lazyjj_layout: Option<JJLayout>,
    #[serde(rename = "lazyjj.layout-percent")]
    lazyjj_layout_percent: Option<u16>,
    #[serde(rename = "lazyjj.keybinds")]
    lazyjj_keybinds: Option<KeybindsConfig>,
    #[serde(rename = "ui.diff.format")]
    ui_diff_format: Option<DiffFormat>,
    #[serde(rename = "ui.diff.tool")]
    ui_diff_tool: Option<()>,
    #[serde(rename = "git.push-bookmark-prefix")]
    git_push_bookmark_prefix: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct JjConfig {
    lazyjj: Option<JjConfigLazyjj>,
    ui: Option<JjConfigUi>,
    git: Option<JjConfigGit>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct JjConfigLazyjj {
    highlight_color: Option<Color>,
    diff_format: Option<DiffFormat>,
    bookmark_prefix: Option<String>,
    layout: Option<JJLayout>,
    layout_percent: Option<u16>,
    keybinds: Option<KeybindsConfig>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct JjConfigUi {
    diff: Option<JjConfigUiDiff>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct JjConfigUiDiff {
    format: Option<DiffFormat>,
    tool: Option<toml::Value>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct JjConfigGit {
    push_bookmark_prefix: Option<String>,
}

impl Config {
    pub fn diff_format(&self) -> DiffFormat {
        let default = if self.has_diff_tool() {
            DiffFormat::DiffTool
        } else {
            DiffFormat::ColorWords
        };
        self.lazyjj_diff_format
            .unwrap_or(self.ui_diff_format.unwrap_or(default))
    }

    pub fn has_diff_tool(&self) -> bool {
        self.ui_diff_tool.is_some()
    }

    pub fn highlight_color(&self) -> Color {
        self.lazyjj_highlight_color
            .unwrap_or(Color::Rgb(50, 50, 150))
    }

    pub fn bookmark_prefix(&self) -> String {
        self.lazyjj_bookmark_prefix.clone().unwrap_or(
            self.git_push_bookmark_prefix
                .clone()
                .unwrap_or("push-".to_owned()),
        )
    }

    pub fn layout(&self) -> JJLayout {
        self.lazyjj_layout.unwrap_or(JJLayout::Horizontal)
    }

    pub fn layout_percent(&self) -> u16 {
        self.lazyjj_layout_percent.unwrap_or(50)
    }

    pub fn keybinds(&self) -> Option<&KeybindsConfig> {
        self.lazyjj_keybinds.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct Env {
    pub config: Config,
    pub root: String,
    pub default_revset: Option<String>,
}

impl Env {
    pub fn new(path: PathBuf, default_revset: Option<String>) -> Result<Env> {
        // Get jj repository root
        let root_output = Command::new("jj")
            .arg("root")
            .args(get_output_args(false, true))
            .current_dir(&path)
            .output()?;
        if !root_output.status.success() {
            bail!("No jj repository found in {}", path.to_str().unwrap_or(""))
        }
        let root = String::from_utf8(root_output.stdout)?.remove_end_line();

        // Read/parse jj config
        let config_toml = String::from_utf8(
            Command::new("jj")
                .arg("config")
                .arg("list")
                .arg("--template")
                .arg("'\"' ++ name ++ '\"' ++ '=' ++ value ++ '\n'")
                .args(get_output_args(false, true))
                .current_dir(&root)
                .output()
                .context("Failed to get jj config")?
                .stdout,
        )?;
        // Prior to https://github.com/martinvonz/jj/pull/3728, keys were not TOML-escaped.
        let config = match toml::from_str::<Config>(&config_toml) {
            Ok(config) => config,
            Err(_) => {
                let config_toml = String::from_utf8(
                    Command::new("jj")
                        .arg("config")
                        .arg("list")
                        .args(get_output_args(false, true))
                        .current_dir(&root)
                        .output()
                        .context("Failed to get jj config")?
                        .stdout,
                )?;
                toml::from_str::<JjConfig>(&config_toml)
                    .context("Failed to parse jj config")
                    .map(|config| Config {
                        lazyjj_highlight_color: config
                            .lazyjj
                            .as_ref()
                            .and_then(|lazyjj| lazyjj.highlight_color),
                        lazyjj_diff_format: config
                            .lazyjj
                            .as_ref()
                            .and_then(|lazyjj| lazyjj.diff_format),
                        lazyjj_bookmark_prefix: config
                            .lazyjj
                            .as_ref()
                            .and_then(|lazyjj| lazyjj.bookmark_prefix.clone()),
                        lazyjj_layout: config.lazyjj.as_ref().and_then(|lazyjj| lazyjj.layout),
                        lazyjj_layout_percent: config
                            .lazyjj
                            .as_ref()
                            .and_then(|lazyjj| lazyjj.layout_percent),
                        lazyjj_keybinds: config
                            .lazyjj
                            .as_ref()
                            .and_then(|lazyjj| lazyjj.keybinds.clone()),
                        ui_diff_format: config
                            .ui
                            .as_ref()
                            .and_then(|ui| ui.diff.as_ref().and_then(|diff| diff.format)),
                        ui_diff_tool: config.ui.as_ref().and_then(|ui| {
                            ui.diff
                                .as_ref()
                                .and_then(|diff| diff.tool.as_ref().map(|_| ()))
                        }),
                        git_push_bookmark_prefix: config
                            .git
                            .and_then(|git| git.push_bookmark_prefix),
                    })?
            }
        };

        Ok(Env {
            root,
            config,
            default_revset,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Default, Copy, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum DiffFormat {
    #[default]
    ColorWords,
    Git,
    DiffTool,
    // Unused
    Summary,
    Stat,
}

impl DiffFormat {
    pub fn get_next(&self, has_diff_tool: bool) -> DiffFormat {
        match self {
            DiffFormat::ColorWords => DiffFormat::Git,
            DiffFormat::Git => {
                if has_diff_tool {
                    DiffFormat::DiffTool
                } else {
                    DiffFormat::ColorWords
                }
            }
            _ => DiffFormat::ColorWords,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Default, Copy, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum JJLayout {
    #[default]
    Horizontal,
    Vertical,
}

// Impl into for JJLayout to ratatui's Direction
impl From<JJLayout> for ratatui::layout::Direction {
    fn from(layout: JJLayout) -> Self {
        match layout {
            JJLayout::Horizontal => ratatui::layout::Direction::Horizontal,
            JJLayout::Vertical => ratatui::layout::Direction::Vertical,
        }
    }
}
