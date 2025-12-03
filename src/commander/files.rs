/*!
[Commander] member functions related to jj diff.

This module has features to parse the diff output.
It is mostly used in the [files_tab][crate::ui::files_tab] module.
*/
use std::sync::LazyLock;

use crate::{
    commander::{CommandError, Commander, ids::CommitId, log::Head},
    env::DiffFormat,
};

use anyhow::{Context, Result};
use ratatui::style::Color;
use regex::Regex;
use tracing::instrument;

#[derive(Clone, Debug, PartialEq)]
pub struct File {
    pub line: String,
    pub path: Option<String>,
    pub diff_type: Option<DiffType>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DiffType {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Conflict {
    pub path: String,
}

impl DiffType {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "A" => Some(DiffType::Added),
            "M" => Some(DiffType::Modified),
            "D" => Some(DiffType::Deleted),
            "R" => Some(DiffType::Renamed),
            _ => None,
        }
    }

    pub fn color(&self) -> Color {
        match self {
            DiffType::Added => Color::Green,
            DiffType::Modified => Color::Cyan,
            DiffType::Renamed => Color::Cyan,
            DiffType::Deleted => Color::Red,
        }
    }
}

// Example line: `A README.md`, `M src/main.rs`, `D Hello World`
static FILES_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(.) (.*)").unwrap());
static RENAME_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{(.*?) => (.*?)\}").unwrap());
static CONFLICTS_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(.*)    .*").unwrap());

impl Commander {
    /// Get list of changes files in a change. Parses the output.
    /// Maps to `jj diff --summary -r <revision>`
    #[instrument(level = "trace", skip(self))]
    pub fn get_files(&self, head: &Head) -> Result<Vec<File>, CommandError> {
        Ok(self
            .execute_jj_command(
                vec!["diff", "-r", head.commit_id.as_str(), "--summary"],
                false,
                true,
            )?
            .lines()
            .map(|line| {
                let captured = FILES_REGEX.captures(line);
                let diff_type = captured
                    .as_ref()
                    .and_then(|captured| captured.get(1))
                    .and_then(|inner_text| DiffType::parse(inner_text.as_str()));
                let path = captured
                    .as_ref()
                    .and_then(|captured| captured.get(2))
                    .map(|inner_text| inner_text.as_str().to_owned());

                File {
                    line: line.to_string(),
                    path,
                    diff_type,
                }
            })
            .collect())
    }

    /// Get list of changes files in a change. Parses the output.
    /// Maps to `jj diff --summary -r <revision>`
    #[instrument(level = "trace", skip(self))]
    pub fn get_conflicts(&self, commit_id: &CommitId) -> Result<Vec<Conflict>> {
        let output = self.execute_jj_command(
            vec!["resolve", "--list", "-r", commit_id.as_str()],
            false,
            true,
        );

        match output {
            Ok(output) => Ok(output
                .lines()
                .filter_map(|line| {
                    let captured = CONFLICTS_REGEX.captures(line);
                    captured
                        .as_ref()
                        .and_then(|captured| captured.get(1))
                        .map(|inner_text| Conflict {
                            path: inner_text.as_str().to_owned(),
                        })
                })
                .collect()),
            Err(CommandError::Status(_, Some(2))) => {
                // No conflicts
                Ok(vec![])
            }
            Err(err) => Err(err).context("Failed getting conflicts"),
        }
    }

    /// Get diff for file change in a change.
    /// Maps to `jj diff -r <revision> <path>`
    #[instrument(level = "trace", skip(self))]
    pub fn get_file_diff(
        &self,
        head: &Head,
        current_file: &File,
        diff_format: &DiffFormat,
        ignore_working_copy: bool,
    ) -> Result<Option<String>, CommandError> {
        let Some(path) = current_file.path.as_ref() else {
            return Ok(None);
        };

        let path = if let (true, Some(captures)) = (
            current_file.diff_type == Some(DiffType::Renamed),
            RENAME_REGEX.captures(path),
        ) {
            match captures.get(2) {
                Some(path) => path.as_str(),
                None => return Ok(None),
            }
        } else {
            path
        };

        let fileset = format!("file:\"{}\"", path.replace('"', "\\\""));
        let mut args = vec!["diff", "-r", head.commit_id.as_str(), &fileset];
        args.append(&mut diff_format.get_args());
        if ignore_working_copy {
            args.push("--ignore-working-copy");
        }

        self.execute_jj_command(args, true, true).map(Some)
    }

    #[instrument(level = "trace", skip(self))]
    pub fn untrack_file(&self, current_file: &File) -> Result<Option<String>, CommandError> {
        let Some(path) = current_file.path.as_ref() else {
            return Ok(None);
        };

        let path = if let Some(DiffType::Renamed) = current_file.diff_type
            && let Some(captures) = RENAME_REGEX.captures(path)
        {
            match captures.get(2) {
                Some(path) => path.as_str(),
                None => return Ok(None),
            }
        } else {
            path
        };

        let fileset = format!("file:\"{}\"", path.replace('"', "\\\""));
        Ok(Some(self.execute_jj_command(
            vec!["file", "untrack", &fileset],
            false,
            true,
        )?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commander::tests::TestRepo;
    use insta::assert_debug_snapshot;
    use std::fs;

    #[test]
    fn get_files() -> Result<()> {
        let test_repo = TestRepo::new()?;
        let file_path = test_repo.directory.path().join("README");

        // Initial state
        {
            let head = test_repo.commander.get_current_head()?;
            let files = test_repo.commander.get_files(&head)?;
            assert_eq!(files, vec![]);
        }

        // Add file
        {
            fs::write(&file_path, b"AAA")?;

            let head = test_repo.commander.get_current_head()?;
            let files = test_repo.commander.get_files(&head)?;
            assert_eq!(
                files,
                vec![File {
                    line: "A README".to_owned(),
                    path: Some("README".to_owned(),),
                    diff_type: Some(DiffType::Added,),
                },]
            );
        }

        // Commit
        test_repo.commander.execute_void_jj_command(vec!["new"])?;

        // Modify file
        {
            fs::write(&file_path, b"BBB")?;

            let head = test_repo.commander.get_current_head()?;
            let files = test_repo.commander.get_files(&head)?;
            assert_eq!(
                files,
                vec![File {
                    line: "M README".to_owned(),
                    path: Some("README".to_owned()),
                    diff_type: Some(DiffType::Modified)
                },]
            );
        }

        // Delete file
        {
            fs::remove_file(&file_path)?;

            let head = test_repo.commander.get_current_head()?;
            let files = test_repo.commander.get_files(&head)?;
            assert_eq!(
                files,
                vec![File {
                    line: "D README".to_owned(),
                    path: Some("README".to_owned()),
                    diff_type: Some(DiffType::Deleted)
                },]
            );
        }

        Ok(())
    }

    #[test]
    fn get_file_diff() -> Result<()> {
        let test_repo = TestRepo::new()?;

        let mut file_path = test_repo.directory.path().join("README");

        // Add file
        {
            fs::write(&file_path, b"AAA")?;
            let file = File {
                path: Some("README".to_string()),
                diff_type: Some(DiffType::Added),
                line: "A README".to_string(),
            };

            let head = test_repo.commander.get_current_head()?;
            assert_debug_snapshot!(test_repo.commander.get_file_diff(
                &head,
                &file,
                &DiffFormat::ColorWords,
                false
            )?);
            assert_debug_snapshot!(test_repo.commander.get_file_diff(
                &head,
                &file,
                &DiffFormat::Git,
                false
            )?);
        }

        // Commit
        test_repo.commander.execute_void_jj_command(vec!["new"])?;

        // Modify file
        {
            fs::write(&file_path, b"BBB")?;
            let file = File {
                path: Some("README".to_string()),
                diff_type: Some(DiffType::Modified),
                line: "M README".to_string(),
            };

            let head = test_repo.commander.get_current_head()?;
            assert_debug_snapshot!(test_repo.commander.get_file_diff(
                &head,
                &file,
                &DiffFormat::ColorWords,
                true
            )?);
            assert_debug_snapshot!(test_repo.commander.get_file_diff(
                &head,
                &file,
                &DiffFormat::Git,
                true
            )?);
        }

        // Commit
        test_repo.commander.execute_void_jj_command(vec!["new"])?;

        // Rename file
        {
            let file_path_new = test_repo.directory.path().join("README2");
            fs::rename(file_path, &file_path_new)?;
            file_path = file_path_new;

            let file = File {
                path: Some("{README => README2}".to_string()),
                diff_type: Some(DiffType::Renamed),
                line: "R {README => README2}".to_string(),
            };

            let head = test_repo.commander.get_current_head()?;
            assert_debug_snapshot!(test_repo.commander.get_file_diff(
                &head,
                &file,
                &DiffFormat::ColorWords,
                true
            )?);
            assert_debug_snapshot!(test_repo.commander.get_file_diff(
                &head,
                &file,
                &DiffFormat::Git,
                true
            )?);
        }

        // Commit
        test_repo.commander.execute_void_jj_command(vec!["new"])?;

        // Delete file
        {
            fs::remove_file(&file_path)?;
            let file = File {
                path: Some("README2".to_string()),
                diff_type: Some(DiffType::Deleted),
                line: "D README2".to_string(),
            };

            let head = test_repo.commander.get_current_head()?;
            assert_debug_snapshot!(test_repo.commander.get_file_diff(
                &head,
                &file,
                &DiffFormat::ColorWords,
                true
            )?);
            assert_debug_snapshot!(test_repo.commander.get_file_diff(
                &head,
                &file,
                &DiffFormat::Git,
                true
            )?);
        }

        Ok(())
    }

    #[test]
    fn get_conflicts() -> Result<()> {
        let test_repo = TestRepo::new()?;

        let file_path = test_repo.directory.path().join("README");

        let head0 = test_repo.commander.get_current_head()?;

        // First change
        test_repo.commander.run_new(head0.commit_id.as_str())?;
        let head1 = test_repo.commander.get_current_head()?;
        fs::write(&file_path, b"AAA")?;

        test_repo.commander.run_new(head0.commit_id.as_str())?;
        let head2 = test_repo.commander.get_current_head()?;
        fs::write(&file_path, b"BBB")?;

        test_repo.commander.execute_void_jj_command([
            "rebase",
            "-s",
            head2.change_id.as_str(),
            "-d",
            head1.change_id.as_str(),
        ])?;

        let head = test_repo.commander.get_current_head()?;

        let conflicts = test_repo.commander.get_conflicts(&head.commit_id)?;

        assert_eq!(
            conflicts,
            [Conflict {
                path: "README".to_owned()
            }]
        );

        Ok(())
    }
}
