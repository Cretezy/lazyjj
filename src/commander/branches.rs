use crate::{
    commander::{CommandError, Commander, RemoveEndLine},
    env::DiffFormat,
};
use ansi_to_tui::IntoText;
use anyhow::Result;
use lazy_static::lazy_static;
use ratatui::text::Text;
use regex::Regex;
use std::fmt::Display;
use tracing::instrument;

#[derive(Clone, Debug, PartialEq)]
pub struct Branch {
    pub name: String,
    pub remote: Option<String>,
    pub present: bool,
}

impl Display for Branch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut text = self.name.clone();
        if let Some(remote) = self.remote.as_ref() {
            text.push('@');
            text.push_str(remote);
        }
        write!(f, "{}", text)
    }
}

// Template which outputs `[name@remote]`. Used to parse data from branch list
const BRANCH_TEMPLATE: &str = r#""[" ++ name ++ "@" ++ remote ++ "|" ++ present ++ "]""#;
lazy_static! {
    // Regex to parse branch
    static ref BRANCH_REGEX: Regex = Regex::new(r"^\[(.*)@(.*)\|(.*)\]$").unwrap();
}

fn parse_branch(text: &str) -> Option<Branch> {
    let captured = BRANCH_REGEX.captures(text);
    captured.as_ref().and_then(|captured| {
        let name = captured.get(1);
        let remote = captured.get(2);
        let present = captured.get(3);
        if let (Some(name), Some(remote), Some(present)) = (name, remote, present) {
            let remote = remote.as_str().to_owned();
            Some(Branch {
                remote: if remote.is_empty() {
                    None
                } else {
                    Some(remote)
                },
                name: name.as_str().to_owned(),
                present: present.as_str() == "true",
            })
        } else {
            None
        }
    })
}

#[derive(Clone, Debug)]
pub enum BranchLine {
    Unparsable(String),
    Parsed { text: String, branch: Branch },
}

impl BranchLine {
    pub fn to_text(&self) -> Result<Text, ansi_to_tui::Error> {
        match self {
            BranchLine::Unparsable(text) => text.to_text(),
            BranchLine::Parsed { text, .. } => text.to_text(),
        }
    }
}

impl Commander {
    /// Get branches.
    /// Maps to `jj branch list`
    #[instrument(level = "trace", skip(self))]
    pub fn get_branches(&mut self, show_all: bool) -> Result<Vec<BranchLine>, CommandError> {
        let mut args = vec![];
        if show_all {
            args.push("--all-remotes");
        }
        let branches_colored = self.execute_jj_command(
            [
                vec![
                    "branch",
                    "list",
                    "--config-toml",
                    // Override format_ref_targets to not list conflicts
                    r#"
                            template-aliases.'format_ref_targets(ref)' = '''
                                if(ref.conflict(),
                                  " " ++ label("conflict", "(conflicted)"),
                                  ": " ++ format_commit_summary_with_refs(ref.normal_target(), ""),
                                )
                            '''
                        "#,
                ],
                args.clone(),
            ]
            .concat(),
            true,
            true,
        )?;

        let branches: Vec<BranchLine> = self
            .execute_jj_command(
                [
                    vec![
                        "branch",
                        "list",
                        "-T",
                        &format!(r#"{} ++ "\n""#, BRANCH_TEMPLATE),
                    ],
                    args,
                ]
                .concat(),
                false,
                true,
            )?
            .lines()
            .zip(branches_colored.lines())
            .map(|(line, line_colored)| match parse_branch(line) {
                Some(branch) => BranchLine::Parsed {
                    text: line_colored.to_owned(),
                    branch,
                },
                None => BranchLine::Unparsable(line_colored.to_owned()),
            })
            .collect();

        Ok(branches)
    }

    #[instrument(level = "trace", skip(self))]
    pub fn get_branches_list(&mut self, show_all: bool) -> Result<Vec<Branch>, CommandError> {
        let mut args = vec![
            "branch".to_owned(),
            "list".to_owned(),
            "-T".to_owned(),
            format!(r#"if(present, {} ++ "\n", "")"#, BRANCH_TEMPLATE),
        ];
        if show_all {
            args.push("--all-remotes".to_owned());
        }

        let branches: Vec<Branch> = self
            .execute_jj_command(args, false, true)?
            .lines()
            .filter_map(parse_branch)
            .collect();

        Ok(branches)
    }

    /// Get branch details.
    /// Maps to `jj show <branch>`
    #[instrument(level = "trace", skip(self))]
    pub fn get_branch_show(
        &mut self,
        branch: &Branch,
        diff_format: &DiffFormat,
    ) -> Result<String, CommandError> {
        Ok(self
            .execute_jj_command(
                vec!["show", &branch.to_string(), diff_format.get_arg()],
                true,
                true,
            )?
            .remove_end_line())
    }
}

#[cfg(test)]
mod tests {

    use insta::assert_debug_snapshot;

    use crate::commander::tests::TestRepo;

    use super::*;

    #[test]
    fn get_branches() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let branch = test_repo.commander.create_branch("test")?;
        let branches = test_repo.commander.get_branches(false)?;

        assert_eq!(branches.len(), 1);
        assert_eq!(
            branches.first().and_then(|branch| match branch {
                BranchLine::Parsed { branch, .. } => Some(branch),
                _ => None,
            }),
            Some(&branch)
        );

        Ok(())
    }

    #[test]
    fn get_branches_list() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let branch = test_repo.commander.create_branch("test")?;
        let branches = test_repo.commander.get_branches_list(false)?;

        assert_eq!(branches, [branch]);

        Ok(())
    }

    #[test]
    fn get_branch_show() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let branch = test_repo.commander.create_branch("test")?;
        let branch_show = test_repo
            .commander
            .get_branch_show(&branch, &DiffFormat::default())?;

        let mut settings = insta::Settings::clone_current();
        settings.add_filter(r"Commit ID: [0-9a-fA-F]{40}", "Commit ID: [COMMIT_ID]");
        settings.add_filter(r"Change ID: [k-z]{32}", "Change ID: [Change ID]");
        settings.add_filter(r"(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})", "([DATE_TIME])");
        let _bound = settings.bind_to_scope();

        assert_debug_snapshot!(branch_show);

        Ok(())
    }
}
