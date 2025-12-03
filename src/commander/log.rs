/*!
[Commander] member functions related to jj log.

This module has features to parse the log output to extract change id and commit id.
It is mostly used in the [log_tab][crate::ui::log_tab] module.
*/

use crate::{
    commander::{
        CommandError, Commander, RemoveEndLine,
        bookmarks::Bookmark,
        ids::{ChangeId, CommitId},
    },
    env::DiffFormat,
};

use anyhow::{Context, Result, anyhow, bail};
use itertools::Itertools;
use regex::Regex;
use std::{fmt::Display, sync::LazyLock};
use thiserror::Error;
use tracing::instrument;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Head {
    pub change_id: ChangeId,
    pub commit_id: CommitId,
    pub divergent: bool,
    pub immutable: bool,
}

#[derive(Clone, Debug)]
pub struct LogOutput {
    pub graph: String,
    // Maps graph line -> heads
    pub graph_heads: Vec<Option<Head>>,
    pub heads: Vec<Head>,
}

#[derive(Error, Debug)]
pub struct HeadParseError(String);

impl Display for HeadParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Head parse error: {}", self.0)
    }
}

// Template which outputs `[change_id|commit_id|divergent]`. Used to parse data from log and other
// commands which supports templating.
const HEAD_TEMPLATE: &str =
    r#""[" ++ change_id ++ "|" ++ commit_id ++ "|" ++ divergent ++ "|" ++ immutable ++ "]""#;
// Regex to parse HEAD_TEMPLATE
static HEAD_TEMPLATE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[(.*)\|(.*)\|(.*)\|(.*)\]").unwrap());

// Parse a head with HEAD_TEMPLATE.
fn parse_head(text: &str) -> Result<Head> {
    let captured = HEAD_TEMPLATE_REGEX.captures(text);
    captured
        .as_ref()
        .map_or(Err(anyhow!(HeadParseError(text.to_owned()))), |captured| {
            if let (Some(change_id), Some(commit_id), Some(divergent), Some(immutable)) = (
                captured.get(1),
                captured.get(2),
                captured.get(3),
                captured.get(4),
            ) {
                Ok(Head {
                    change_id: ChangeId(change_id.as_str().to_string()),
                    commit_id: CommitId(commit_id.as_str().to_string()),
                    divergent: divergent.as_str() == "true",
                    immutable: immutable.as_str() == "true",
                })
            } else {
                bail!(HeadParseError(text.to_owned()))
            }
        })
}

impl Commander {
    /// Get log. Returns human readable log and mapping to log line to head.
    /// Maps to `jj log`
    #[instrument(level = "trace", skip(self))]
    pub fn get_log(&self, revset: &Option<String>) -> Result<LogOutput, CommandError> {
        let mut args = vec![];

        if let Some(revset) = revset {
            args.push("-r");
            args.push(revset);
        }

        // Force builtin_log_compact which uses 2 lines per change
        let graph = self.execute_jj_command(
            [
                vec!["log", "--template", "builtin_log_compact"],
                args.clone(),
            ]
            .concat(),
            true,
            true,
        )?;

        // Extract the log one more time, but this time use a template
        // where each line begins with Head information. Since jj has
        // 2 lines per change, there will also be two lines with head info.
        // The number of lines in graph and the number of items in graph_heads
        // should be identical.
        let graph_heads: Vec<Option<Head>> = self
            .execute_jj_command(
                [
                    vec![
                        "log",
                        "--template",
                        // Match builtin_log_compact with 2 lines per change
                        &format!(
                            r#"{HEAD_TEMPLATE} ++ " " ++ bookmarks ++"\n" ++ {HEAD_TEMPLATE}"#
                        ),
                    ],
                    args,
                ]
                .concat(),
                false,
                true,
            )?
            .lines()
            .map(|line| parse_head(line).ok())
            .collect();

        let heads = graph_heads.clone().into_iter().flatten().unique().collect();

        Ok(LogOutput {
            graph,
            graph_heads,
            heads,
        })
    }

    /// Get commit details.
    /// Maps to `jj show <commit>`
    #[instrument(level = "trace", skip(self))]
    pub fn get_commit_show(
        &self,
        commit_id: &CommitId,
        diff_format: &DiffFormat,
        ignore_working_copy: bool,
    ) -> Result<String, CommandError> {
        let mut args = vec!["show", commit_id.as_str()];
        args.append(&mut diff_format.get_args());
        if ignore_working_copy {
            args.push("--ignore-working-copy");
        }

        Ok(self.execute_jj_command(args, true, true)?.remove_end_line())
    }

    /// Get the current head.
    /// Maps to `jj log -r @`
    #[instrument(level = "trace", skip(self))]
    pub fn get_current_head(&self) -> Result<Head> {
        parse_head(
            &self
                .execute_jj_command(
                    vec![
                        "log",
                        "--no-graph",
                        "--template",
                        &format!(r#"{HEAD_TEMPLATE} ++ "\n""#),
                        "-r",
                        "@",
                        "--limit",
                        "1",
                    ],
                    false,
                    true,
                )
                .context("Failed getting current head")?
                .remove_end_line(),
        )
    }

    /// Get the latest version of a head. Can detect evolution of divergent head.
    #[instrument(level = "trace", skip(self))]
    pub fn get_head_latest(&self, head: &Head) -> Result<Head> {
        // Get all heads which point to the same change ID
        let latest_heads_res = self.execute_jj_command(
            vec![
                "log",
                "--no-graph",
                "-r",
                &format!(r#"change_id({})"#, head.change_id.as_str()),
                "--template",
                &format!(r#"{HEAD_TEMPLATE} ++ "\n""#),
            ],
            false,
            true,
        );
        let Ok(latest_heads_res) = latest_heads_res else {
            return self.get_head_latest(&self.get_current_head()?);
        };
        if latest_heads_res.is_empty() {
            return self.get_head_latest(&self.get_current_head()?);
        }
        let latest_heads: Vec<Head> = latest_heads_res
            .lines()
            .map(parse_head)
            .collect::<Result<Vec<Head>>>()?;

        // If the current head exist, that means it wasn't updated
        if let Some(head) = latest_heads.iter().find(|latest_head| latest_head == &head) {
            return Ok(head.to_owned());
        }

        // Check obslog for each head. If the obslog contains the head's commit, it means
        // there's a new commit for the head
        for latest_head in latest_heads.iter() {
            let parent_commits: Vec<ChangeId> = self
                .execute_jj_command(
                    vec![
                        "obslog",
                        "--no-graph",
                        "--template",
                        r#"commit.change_id() ++ "\n""#,
                        "-r",
                        latest_head.commit_id.as_str(),
                    ],
                    false,
                    true,
                )
                .context("Failed getting latest head parent commits")?
                .lines()
                .map(|line| ChangeId(line.to_owned()))
                .collect();

            if parent_commits
                .iter()
                .any(|parent_commit| parent_commit == &head.change_id)
            {
                return Ok(latest_head.to_owned());
            }
        }

        bail!(
            "Could not find head latest: {} {} {:?}",
            head.change_id,
            head.commit_id,
            latest_heads
        );
    }

    /// Get a commit's parent.
    /// Maps to `jj log -r <revision>-`
    #[instrument(level = "trace", skip(self))]
    pub fn get_commit_parent(&self, commit_id: &CommitId) -> Result<Head> {
        parse_head(
            &self
                .execute_jj_command(
                    vec![
                        "log",
                        "--no-graph",
                        "--template",
                        &format!(r#"{HEAD_TEMPLATE} ++ "\n""#),
                        "-r",
                        &format!("{commit_id}-"),
                        "--limit",
                        "1",
                    ],
                    false,
                    true,
                )
                .with_context(|| format!("Failed getting commit parent: {commit_id}"))?
                .remove_end_line(),
        )
    }

    /// Get commit's description.
    /// Maps to `jj log -r <revision> -T description`
    #[instrument(level = "trace", skip(self))]
    pub fn get_commit_description(&self, commit_id: &CommitId) -> Result<String> {
        Ok(self
            .execute_jj_command(
                vec![
                    "log",
                    "--no-graph",
                    "--template",
                    "description",
                    "-r",
                    commit_id.as_str(),
                    "--limit",
                    "1",
                ],
                false,
                true,
            )
            .with_context(|| format!("Failed getting commit description: {commit_id}"))?
            .remove_end_line())
    }

    /// Check if a revision is immutable
    /// Maps to `jj log -r <revision> -T immutable`
    #[instrument(level = "trace", skip(self))]
    pub fn check_revision_immutable(&self, revision: &str) -> Result<bool> {
        Ok(self
            .execute_jj_command(
                vec![
                    "log",
                    "--no-graph",
                    "--template",
                    "immutable",
                    "-r",
                    revision,
                    "--limit",
                    "1",
                ],
                false,
                true,
            )
            .with_context(|| format!("Failed checking if revision is immutable: {revision}"))?
            .remove_end_line()
            == "true")
    }

    /// Get bookmark head
    /// Maps to `jj log -r <bookmark>[@<remote>]`
    #[instrument(level = "trace", skip(self))]
    pub fn get_bookmark_head(&self, bookmark: &Bookmark) -> Result<Head> {
        parse_head(
            &self
                .execute_jj_command(
                    vec![
                        "log",
                        "--no-graph",
                        "--template",
                        &format!(r#"{HEAD_TEMPLATE} ++ "\n""#),
                        "-r",
                        &bookmark.to_string(),
                        "--limit",
                        "1",
                    ],
                    false,
                    true,
                )
                .context("Failed getting bookmark head")?
                .remove_end_line(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::commander::tests::TestRepo;
    use insta::assert_debug_snapshot;

    #[test]
    fn get_log() -> Result<()> {
        let test_repo = TestRepo::new()?;

        let log = test_repo.commander.get_log(&None)?;

        let mut settings = insta::Settings::clone_current();
        settings.add_filter(r"[k-z]{8} .*? [0-9a-fA-F]{8}", "[LINE]");
        let _bound = settings.bind_to_scope();

        assert_debug_snapshot!(log.graph);

        assert!(log.graph_heads.iter().all(|graph_head| {
            graph_head
                .as_ref()
                .is_none_or(|graph_head| log.heads.contains(graph_head))
        }));

        Ok(())
    }

    #[test]
    fn get_commit_show() -> Result<()> {
        let test_repo = TestRepo::new()?;

        fs::write(test_repo.directory.path().join("README"), b"AAA")?;

        let head = test_repo.commander.get_current_head()?;
        let show =
            test_repo
                .commander
                .get_commit_show(&head.commit_id, &DiffFormat::ColorWords, false)?;

        let mut settings = insta::Settings::clone_current();
        settings.add_filter(r"Commit ID: [0-9a-fA-F]{40}", "Commit ID: [COMMIT_ID]");
        settings.add_filter(r"Change ID: [k-z]{32}", "Change ID: [Change ID]");
        settings.add_filter(r"(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})", "([DATE_TIME])");
        let _bound = settings.bind_to_scope();

        assert_debug_snapshot!(show);

        Ok(())
    }

    #[test]
    fn get_commit_parent() -> Result<()> {
        let test_repo = TestRepo::new()?;

        let head = test_repo.commander.get_current_head()?;

        assert_eq!(
            test_repo.commander.get_commit_parent(&head.commit_id)?,
            Head {
                commit_id: CommitId("0000000000000000000000000000000000000000".to_owned()),
                change_id: ChangeId("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz".to_owned()),
                divergent: false,
                immutable: true,
            }
        );

        Ok(())
    }

    #[test]
    fn get_head_latest() -> Result<()> {
        let test_repo = TestRepo::new()?;

        let old_head = test_repo.commander.get_current_head()?;

        fs::write(test_repo.directory.path().join("README"), b"AAA")?;

        let new_head = test_repo.commander.get_current_head()?;

        assert_ne!(old_head, new_head);

        assert_eq!(new_head, test_repo.commander.get_head_latest(&old_head)?);

        Ok(())
    }

    #[test]
    fn check_revision_immutable() -> Result<()> {
        let test_repo = TestRepo::new()?;

        assert!(!(test_repo.commander.check_revision_immutable("@")?));

        Ok(())
    }

    #[test]
    fn get_bookmark_head() -> Result<()> {
        let test_repo = TestRepo::new()?;

        let head = test_repo.commander.get_current_head()?;
        // Git doesn't support bookmark pointing to root commit, so it will advance
        let bookmark = test_repo.commander.create_bookmark("main")?;

        assert_eq!(test_repo.commander.get_bookmark_head(&bookmark)?, head);

        Ok(())
    }
}
