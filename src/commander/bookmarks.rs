use crate::{
    commander::{CommandError, Commander, RemoveEndLine},
    env::DiffFormat,
};
use ansi_to_tui::IntoText;
use anyhow::Result;
use ratatui::text::Text;
use regex::Regex;
use std::{fmt::Display, sync::LazyLock};
use tracing::instrument;

#[derive(Clone, Debug, PartialEq)]
pub struct Bookmark {
    pub name: String,
    pub remote: Option<String>,
    pub present: bool,
}

impl Display for Bookmark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut text = self.name.clone();
        if let Some(remote) = self.remote.as_ref() {
            text.push('@');
            text.push_str(remote);
        }
        write!(f, "{}", text)
    }
}

// Template which outputs `[name@remote]`. Used to parse data from bookmark list
const BRANCH_TEMPLATE: &str = r#""[" ++ name ++ "@" ++ remote ++ "|" ++ present ++ "]""#;
// Regex to parse bookmark
static BRANCH_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[(.*)@(.*)\|(.*)\]$").unwrap());

fn parse_bookmark(text: &str) -> Option<Bookmark> {
    let captured = BRANCH_REGEX.captures(text);
    captured.as_ref().and_then(|captured| {
        let name = captured.get(1);
        let remote = captured.get(2);
        let present = captured.get(3);
        if let (Some(name), Some(remote), Some(present)) = (name, remote, present) {
            let remote = remote.as_str().to_owned();
            Some(Bookmark {
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
pub enum BookmarkLine {
    Unparsable(String),
    Parsed { text: String, bookmark: Bookmark },
}

impl BookmarkLine {
    pub fn to_text(&self) -> Result<Text, ansi_to_tui::Error> {
        match self {
            BookmarkLine::Unparsable(text) => text.to_text(),
            BookmarkLine::Parsed { text, .. } => text.to_text(),
        }
    }
}

impl Commander {
    /// Get bookmarks.
    /// Maps to `jj bookmark list`
    #[instrument(level = "trace", skip(self))]
    pub fn get_bookmarks(&mut self, show_all: bool) -> Result<Vec<BookmarkLine>, CommandError> {
        let mut args = vec![];
        if show_all {
            args.push("--all-remotes");
        }
        let bookmarks_colored = self.execute_jj_command(
            [
                vec![
                    "bookmark",
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

        let bookmarks: Vec<BookmarkLine> = self
            .execute_jj_command(
                [
                    vec![
                        "bookmark",
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
            .zip(bookmarks_colored.lines())
            .map(|(line, line_colored)| match parse_bookmark(line) {
                Some(bookmark) => BookmarkLine::Parsed {
                    text: line_colored.to_owned(),
                    bookmark,
                },
                None => BookmarkLine::Unparsable(line_colored.to_owned()),
            })
            .collect();

        Ok(bookmarks)
    }

    #[instrument(level = "trace", skip(self))]
    pub fn get_bookmarks_list(&mut self, show_all: bool) -> Result<Vec<Bookmark>, CommandError> {
        let mut args = vec![
            "bookmark".to_owned(),
            "list".to_owned(),
            "-T".to_owned(),
            format!(r#"if(present, {} ++ "\n", "")"#, BRANCH_TEMPLATE),
        ];
        if show_all {
            args.push("--all-remotes".to_owned());
        }

        let bookmarks: Vec<Bookmark> = self
            .execute_jj_command(args, false, true)?
            .lines()
            .filter_map(parse_bookmark)
            .collect();

        Ok(bookmarks)
    }

    /// Get bookmark details.
    /// Maps to `jj show <bookmark>`
    #[instrument(level = "trace", skip(self))]
    pub fn get_bookmark_show(
        &mut self,
        bookmark: &Bookmark,
        diff_format: &DiffFormat,
    ) -> Result<String, CommandError> {
        Ok(self
            .execute_jj_command(
                vec!["show", &bookmark.to_string(), diff_format.get_arg()],
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
    fn get_bookmarks() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let bookmark = test_repo.commander.create_bookmark("test")?;
        let bookmarks = test_repo.commander.get_bookmarks(false)?;

        assert_eq!(bookmarks.len(), 1);
        assert_eq!(
            bookmarks.first().and_then(|bookmark| match bookmark {
                BookmarkLine::Parsed { bookmark, .. } => Some(bookmark),
                _ => None,
            }),
            Some(&bookmark)
        );

        Ok(())
    }

    #[test]
    fn get_bookmarks_list() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let bookmark = test_repo.commander.create_bookmark("test")?;
        let bookmarks = test_repo.commander.get_bookmarks_list(false)?;

        assert_eq!(bookmarks, [bookmark]);

        Ok(())
    }

    #[test]
    fn get_bookmark_show() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let bookmark = test_repo.commander.create_bookmark("test")?;
        let bookmark_show = test_repo
            .commander
            .get_bookmark_show(&bookmark, &DiffFormat::default())?;

        let mut settings = insta::Settings::clone_current();
        settings.add_filter(r"Commit ID: [0-9a-fA-F]{40}", "Commit ID: [COMMIT_ID]");
        settings.add_filter(r"Change ID: [k-z]{32}", "Change ID: [Change ID]");
        settings.add_filter(r"(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})", "([DATE_TIME])");
        let _bound = settings.bind_to_scope();

        assert_debug_snapshot!(bookmark_show);

        Ok(())
    }
}
