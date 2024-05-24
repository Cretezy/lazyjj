use std::{path::PathBuf, process::Command};

use anyhow::{bail, Context, Result};
use ratatui::style::Color;
use serde::Deserialize;

use crate::commander::{get_output_args, RemoveEndLine};

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Config {
    #[serde(rename = "lazyjj.highlight-color")]
    lazyjj_highlight_color: Option<Color>,
    #[serde(rename = "lazyjj.diff-format")]
    lazyjj_diff_format: Option<DiffFormat>,
    #[serde(rename = "ui.diff.format")]
    ui_diff_format: Option<DiffFormat>,
}

impl Config {
    pub fn diff_format(&self) -> DiffFormat {
        self.lazyjj_diff_format
            .unwrap_or(self.ui_diff_format.unwrap_or(DiffFormat::ColorWords))
    }

    pub fn highlight_color(&self) -> Color {
        self.lazyjj_highlight_color
            .unwrap_or(Color::Rgb(50, 50, 150))
    }
}

#[derive(Debug, Clone)]
pub struct Env {
    pub config: Config,
    pub root: String,
}

impl Env {
    pub fn new(path: PathBuf) -> Result<Env> {
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
                .arg("--include-defaults")
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
                        .arg("--include-defaults")
                        .args(get_output_args(false, true))
                        .current_dir(&root)
                        .output()
                        .context("Failed to get jj config")?
                        .stdout,
                )?;
                toml::from_str::<Config>(&config_toml).context("Failed to parse jj config")?
            }
        };

        Ok(Env { root, config })
    }
}

#[derive(Clone, Debug, Deserialize, Default, Copy, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum DiffFormat {
    #[default]
    ColorWords,
    Git,
    Summary,
}
