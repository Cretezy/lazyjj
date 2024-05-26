use std::{path::PathBuf, process::Command};

use anyhow::{bail, Context, Result};
use ratatui::style::Color;
use serde::Deserialize;

use crate::commander::{get_output_args, RemoveEndLine};

// TODO: After 0.18, remove Config and replace with JjConfig
#[derive(Deserialize, Debug, Clone, Default)]
pub struct Config {
    #[serde(rename = "lazyjj.highlight-color")]
    lazyjj_highlight_color: Option<Color>,
    #[serde(rename = "lazyjj.diff-format")]
    lazyjj_diff_format: Option<DiffFormat>,
    #[serde(rename = "lazyjj.branch-prefix")]
    lazyjj_branch_prefix: Option<String>,
    #[serde(rename = "ui.diff.format")]
    ui_diff_format: Option<DiffFormat>,
    #[serde(rename = "git.push-branch-prefix")]
    git_push_branch_prefix: Option<String>,
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
    branch_prefix: Option<String>,
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
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct JjConfigGit {
    push_branch_prefix: Option<String>,
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

    pub fn branch_prefix(&self) -> String {
        self.lazyjj_branch_prefix.clone().unwrap_or(
            self.git_push_branch_prefix
                .clone()
                .unwrap_or("push-".to_owned()),
        )
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
                        lazyjj_branch_prefix: config
                            .lazyjj
                            .as_ref()
                            .and_then(|lazyjj| lazyjj.branch_prefix.clone()),
                        ui_diff_format: config
                            .ui
                            .and_then(|ui| ui.diff.and_then(|diff| diff.format)),
                        git_push_branch_prefix: config.git.and_then(|git| git.push_branch_prefix),
                    })?
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
    Stat,
}
