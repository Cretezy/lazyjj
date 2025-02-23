pub mod bookmarks;
pub mod files;
pub mod ids;
pub mod jj;
pub mod log;

use crate::env::DiffFormat;
use crate::env::Env;

use ansi_to_tui::IntoText;
use anyhow::{Context, Result};
use chrono::{DateTime, Local, TimeDelta};
use ratatui::{
    style::{Color, Stylize},
    text::{Line, Text},
};
use std::sync::Mutex;
use std::{
    ffi::OsStr,
    io,
    process::{Command, Output},
    string::FromUtf8Error,
    sync::Arc,
};
use thiserror::Error;

impl DiffFormat {
    pub fn get_arg(&self) -> Option<&str> {
        match self {
            DiffFormat::ColorWords => Some("--color-words"),
            DiffFormat::Git => Some("--git"),
            DiffFormat::Summary => Some("--summary"),
            DiffFormat::Stat => Some("--stat"),
            DiffFormat::DiffTool => None,
        }
    }
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("Error getting output: {0}")]
    Output(#[from] io::Error),
    #[error("{0}")]
    Status(String, Option<i32>),
    #[error("Error parsing UTF-8 output: {0}")]
    FromUtf8(#[from] FromUtf8Error),
}

impl CommandError {
    #[expect(clippy::wrong_self_convention)]
    pub fn into_text<'a>(&self, title: &'a str) -> Result<Text<'a>, ansi_to_tui::Error> {
        let mut lines = vec![];
        if !title.is_empty() {
            lines.push(Line::raw(title).bold().fg(Color::Red));
            lines.append(&mut vec![Line::raw(""), Line::raw("")]);
        }
        lines.append(&mut self.to_string().into_text()?.lines);

        Ok(Text::from(lines))
    }
}

#[derive(Clone, Debug)]
pub struct CommandLogItem {
    pub program: String,
    pub args: Vec<String>,
    pub output: Arc<Result<Output>>,
    pub time: DateTime<Local>,
    pub duration: TimeDelta,
}

/// Struct used to interact with the jj cli using commanders.
///
/// Handles arguments and recording of history.
#[derive(Debug)]
pub struct Commander {
    pub env: Env,
    pub command_history: Arc<Mutex<Vec<CommandLogItem>>>,

    // Used for testing
    pub jj_config_toml: Option<String>,
    pub force_no_color: bool,
}

impl Commander {
    pub fn new(env: &Env) -> Self {
        Self {
            env: env.clone(),
            command_history: Arc::new(Mutex::new(Vec::new())),
            jj_config_toml: None,
            force_no_color: false,
        }
    }

    /// Execute a command and record to history.
    fn execute_command(&self, command: &mut Command) -> Result<String, CommandError> {
        // Set current directory to root
        command.current_dir(&self.env.root);

        let program = command.get_program().to_str().unwrap_or("").to_owned();
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_str().unwrap_or("").to_owned())
            .collect();

        let time = Local::now();
        let output = command.output();
        let duration = Local::now() - time;

        // unwrap is enough, because mutex can only poison in the case of push panic
        self.command_history.lock().unwrap().push(CommandLogItem {
            program,
            args,
            output: Arc::new(match output.as_ref() {
                Ok(value) => Ok(value.clone()),
                // Clone io::Error
                Err(err) => Err(anyhow::Error::new(io::Error::new(
                    err.kind(),
                    err.to_string(),
                ))),
            }),
            time,
            duration,
        });

        let output = output?;

        if !output.status.success() {
            // Return JjError if non-zero status code
            return Err(CommandError::Status(
                String::from_utf8_lossy(&output.stderr).to_string(),
                output.status.code(),
            ));
        }

        Ok(String::from_utf8(output.stdout)?)
    }

    /// Execute a jj command with color/quiet arguments.
    pub fn execute_jj_command<I, S>(
        &self,
        args: I,
        color: bool,
        quiet: bool,
    ) -> Result<String, CommandError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new("jj");
        command.args(args);
        command.args(get_output_args(!self.force_no_color && color, quiet));

        if let Some(jj_config_toml) = &self.jj_config_toml {
            command.args(vec!["--config-toml", jj_config_toml]);
        }

        self.execute_command(&mut command)
    }

    /// Execute a jj command without using the output.
    pub fn execute_void_jj_command<I, S>(&self, args: I) -> Result<(), CommandError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        // Since no result is used, enable color for command log
        self.execute_jj_command(args, true, true)?;
        Ok(())
    }

    pub fn init(&self) -> Result<()> {
        self.execute_void_jj_command(vec!["status"])
            .context("Failed getting initial status")
    }
}

pub trait RemoveEndLine {
    fn remove_end_line(self) -> Self;
}

impl RemoveEndLine for String {
    fn remove_end_line(mut self) -> Self {
        if self.ends_with('\n') {
            self.pop();
            if self.ends_with('\r') {
                self.pop();
            }
        }
        self
    }
}

pub fn get_output_args(color: bool, quiet: bool) -> Vec<String> {
    vec![
        "--no-pager",
        "--color",
        if color { "always" } else { "never" },
        if quiet { "--quiet" } else { "" },
    ]
    .into_iter()
    .map(String::from)
    .filter(|arg| !arg.is_empty())
    .collect()
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::env::{Config, Env};

    use tempdir::TempDir;

    macro_rules! apply_common_filters {
        {} => {
            let mut settings = insta::Settings::clone_current();
            // Change + commit IDs
            settings.add_filter(r"[k-z]{8} [0-9a-fA-F]{8}", "[CHANGE_ID + COMMIT_ID]");
            let _bound = settings.bind_to_scope();
        }
    }

    pub struct TestRepo {
        pub commander: Commander,
        pub directory: TempDir,
    }

    impl TestRepo {
        pub fn new() -> Result<Self> {
            let directory = TempDir::new("lazyjj")?;

            let jj_config_toml = r#"
                user.email = "lazyjj@example.com"
                user.name = "lazyjj"
                ui.color = "never"
            "#;

            Command::new("jj")
                .arg("git")
                .arg("init")
                .arg("--colocate")
                .arg("--config-toml")
                .arg(jj_config_toml)
                .current_dir(directory.path())
                .output()?;

            let env = Env {
                root: directory.path().to_string_lossy().to_string(),
                config: Config::default(),
                default_revset: None,
            };

            let mut commander = Commander::new(&env);

            commander.jj_config_toml = Some(jj_config_toml.to_owned());
            commander.force_no_color = true;

            Ok(Self {
                directory,
                commander,
            })
        }
    }

    #[test]
    fn test_repo() -> Result<()> {
        apply_common_filters!();

        let test_repo = TestRepo::new()?;

        test_repo
            .commander
            .execute_jj_command(vec!["status"], true, true)?;

        Ok(())
    }
}
