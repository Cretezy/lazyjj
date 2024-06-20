use crate::{
    commander::{ids::CommitId, log::Head, CommandError, Commander},
    env::DiffFormat,
};

use anyhow::{Context, Result};
use lazy_static::lazy_static;
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
            _ => None,
        }
    }

    pub fn color(&self) -> Color {
        match self {
            DiffType::Added => Color::Green,
            DiffType::Modified => Color::Cyan,
            DiffType::Deleted => Color::Red,
        }
    }
}

lazy_static! {
    // Example line: `A README.md`, `M src/main.rs`, `D Hello World`
    static ref FILES_REGEX: Regex = Regex::new(r"(.) (.*)").unwrap();
    static ref CONFLICTS_REGEX: Regex = Regex::new(r"(.*)    .*").unwrap();
}

impl Commander {
    /// Get list of changes files in a change. Parses the output.
    /// Maps to `jj diff --summary -r <revision>`
    #[instrument(level = "trace", skip(self))]
    pub fn get_files(&mut self, head: &Head) -> Result<Vec<File>, CommandError> {
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
    pub fn get_conflicts(&mut self, commit_id: &CommitId) -> Result<Vec<Conflict>> {
        let output = self.execute_jj_command(
            vec!["resolve", "--list", "-r", commit_id.as_str()],
            false,
            true,
        );

        match output {
            Ok(output) => {
                return Ok(output
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
                    .collect())
            }
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
        &mut self,
        head: &Head,
        current_file: &str,
        diff_format: &DiffFormat,
    ) -> Result<String, CommandError> {
        self.execute_jj_command(
            vec![
                "diff",
                "-r",
                head.commit_id.as_str(),
                current_file,
                diff_format.get_arg(),
            ],
            true,
            true,
        )
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
        let mut test_repo = TestRepo::new()?;
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
        let mut test_repo = TestRepo::new()?;

        let file_path = test_repo.directory.path().join("README");

        // Add file
        {
            fs::write(&file_path, b"AAA")?;

            let head = test_repo.commander.get_current_head()?;
            assert_debug_snapshot!(test_repo.commander.get_file_diff(
                &head,
                "README",
                &DiffFormat::ColorWords
            )?);
            assert_debug_snapshot!(test_repo.commander.get_file_diff(
                &head,
                "README",
                &DiffFormat::Git
            )?);
        }

        // Commit
        test_repo.commander.execute_void_jj_command(vec!["new"])?;

        // Modify file
        {
            fs::write(&file_path, b"BBB")?;

            let head = test_repo.commander.get_current_head()?;
            assert_debug_snapshot!(test_repo.commander.get_file_diff(
                &head,
                "README",
                &DiffFormat::ColorWords
            )?);
            assert_debug_snapshot!(test_repo.commander.get_file_diff(
                &head,
                "README",
                &DiffFormat::Git
            )?);
        }

        // Delete file
        {
            fs::remove_file(&file_path)?;

            let head = test_repo.commander.get_current_head()?;
            assert_debug_snapshot!(test_repo.commander.get_file_diff(
                &head,
                "README",
                &DiffFormat::ColorWords
            )?);
            assert_debug_snapshot!(test_repo.commander.get_file_diff(
                &head,
                "README",
                &DiffFormat::Git
            )?);
        }

        Ok(())
    }

    #[test]
    fn get_conflicts() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

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
