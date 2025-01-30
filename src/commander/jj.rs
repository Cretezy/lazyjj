use crate::commander::{bookmarks::Bookmark, ids::CommitId, CommandError, Commander};

use anyhow::{Context, Result};
use tracing::instrument;

impl Commander {
    /// Create a new change after revision. Maps to `jj new <revision>`
    #[instrument(level = "trace", skip(self))]
    pub fn run_new(&mut self, revision: &str) -> Result<()> {
        self.execute_void_jj_command(vec!["new", revision])
            .context("Failed executing jj new")
    }

    /// Edit change. Maps to `jj edit <commit>`
    #[instrument(level = "trace", skip(self))]
    pub fn run_edit(&mut self, revision: &str) -> Result<()> {
        self.execute_void_jj_command(vec!["edit", revision])
            .context("Failed executing jj edit")
    }

    /// Abandon change. Maps to `jj abandon <revision>`
    #[instrument(level = "trace", skip(self))]
    pub fn run_abandon(&mut self, commit_id: &CommitId) -> Result<()> {
        self.execute_void_jj_command(vec!["abandon", commit_id.as_str()])
            .context("Failed executing jj abandon")
    }

    /// Describe change. Maps to `jj describe <revision> -m <message>`
    #[instrument(level = "trace", skip(self))]
    pub fn run_describe(&mut self, revision: &str, message: &str) -> Result<()> {
        self.execute_void_jj_command(vec!["describe", revision, "-m", message])
            .context("Failed executing jj describe")
    }

    /// Create bookmark. Maps to `jj bookmark create <name>`
    #[instrument(level = "trace", skip(self))]
    pub fn create_bookmark(&mut self, name: &str) -> Result<Bookmark, CommandError> {
        self.execute_void_jj_command(vec!["bookmark", "create", name])?;
        // jj only creates local bookmarks
        Ok(Bookmark {
            name: name.to_owned(),
            remote: None,
            present: true,
        })
    }

    /// Create bookmark pointing to commit. Maps to `jj bookmark create <name> -r <revision>`
    #[instrument(level = "trace", skip(self))]
    pub fn create_bookmark_commit(
        &mut self,
        name: &str,
        commit_id: &CommitId,
    ) -> Result<Bookmark, CommandError> {
        self.execute_void_jj_command(vec!["bookmark", "create", name, "-r", commit_id.as_str()])?;
        // jj only creates local bookmarks
        Ok(Bookmark {
            name: name.to_owned(),
            remote: None,
            present: true,
        })
    }

    /// Set bookmark pointing to commit. Maps to `jj bookmark set <name> -r <revision>`
    #[instrument(level = "trace", skip(self))]
    pub fn set_bookmark_commit(
        &mut self,
        name: &str,
        commit_id: &CommitId,
    ) -> Result<(), CommandError> {
        // TODO: Maybe don't do --allow-backwards by default?
        self.execute_void_jj_command(vec![
            "bookmark",
            "set",
            name,
            "-r",
            commit_id.as_str(),
            "--allow-backwards",
        ])
    }

    /// Rename bookmark. Maps to `jj bookmark rename <old> <new>`
    #[instrument(level = "trace", skip(self))]
    pub fn rename_bookmark(&mut self, old: &str, new: &str) -> Result<(), CommandError> {
        self.execute_void_jj_command(vec!["bookmark", "rename", old, new])
    }

    /// Delete bookmark. Maps to `jj bookmark delete <name>`
    #[instrument(level = "trace", skip(self))]
    pub fn delete_bookmark(&mut self, name: &str) -> Result<(), CommandError> {
        self.execute_void_jj_command(vec!["bookmark", "delete", name])
    }

    /// Forget bookmark. Maps to `jj bookmark forget <name>`
    #[instrument(level = "trace", skip(self))]
    pub fn forget_bookmark(&mut self, name: &str) -> Result<(), CommandError> {
        self.execute_void_jj_command(vec!["bookmark", "forget", name])
    }

    /// Track bookmark. Maps to `jj bookmark track <bookmark>@<remote>`
    #[instrument(level = "trace", skip(self))]
    pub fn track_bookmark(&mut self, bookmark: &Bookmark) -> Result<(), CommandError> {
        self.execute_void_jj_command(vec!["bookmark", "track", &bookmark.to_string()])
    }

    /// Untrack bookmark. Maps to `jj bookmark untrack <bookmark>@<remote>`
    #[instrument(level = "trace", skip(self))]
    pub fn untrack_bookmark(&mut self, bookmark: &Bookmark) -> Result<(), CommandError> {
        self.execute_void_jj_command(vec!["bookmark", "untrack", &bookmark.to_string()])
    }

    /// Git push. Maps to `jj git push`
    #[instrument(level = "trace", skip(self))]
    pub fn git_push(
        &mut self,
        all_bookmarks: bool,
        allow_new: bool,
        commit_id: &CommitId,
    ) -> Result<String, CommandError> {
        let mut args = vec!["git", "push"];
        if allow_new {
            args.push("--allow-new");
        }
        if all_bookmarks {
            args.push("--all");
        } else {
            args.push("-r");
            args.push(commit_id.as_str());
        }

        self.execute_jj_command(args, true, true)
    }

    /// Git fetch. Maps to `jj git fetch`
    #[instrument(level = "trace", skip(self))]
    pub fn git_fetch(&mut self, all_remotes: bool) -> Result<String, CommandError> {
        let mut args = vec!["git", "fetch"];
        if all_remotes {
            args.push("--all-remotes");
        }

        self.execute_jj_command(args, true, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commander::tests::TestRepo;

    #[test]
    fn run_new() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let head = test_repo.commander.get_current_head()?;
        test_repo.commander.run_new(head.commit_id.as_str())?;
        assert_eq!(
            test_repo
                .commander
                .command_history
                .last()
                .unwrap()
                .args
                .first()
                .unwrap(),
            "new"
        );
        assert_ne!(head, test_repo.commander.get_current_head()?);

        Ok(())
    }

    #[test]
    fn run_edit() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let head = test_repo.commander.get_current_head()?;
        test_repo.commander.run_new(head.commit_id.as_str())?;
        assert_ne!(head, test_repo.commander.get_current_head()?);
        test_repo.commander.run_edit(head.commit_id.as_str())?;
        assert_eq!(
            test_repo
                .commander
                .command_history
                .last()
                .unwrap()
                .args
                .first()
                .unwrap(),
            "edit"
        );
        assert_eq!(head, test_repo.commander.get_current_head()?);

        Ok(())
    }

    #[test]
    fn run_abandon() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let head = test_repo.commander.get_current_head()?;
        test_repo.commander.run_abandon(&head.commit_id)?;
        assert_eq!(
            test_repo
                .commander
                .command_history
                .last()
                .unwrap()
                .args
                .first()
                .unwrap(),
            "abandon"
        );
        assert_ne!(head, test_repo.commander.get_current_head()?);

        Ok(())
    }

    #[test]
    fn run_describe() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let head = test_repo.commander.get_current_head()?;
        test_repo
            .commander
            .run_describe(head.commit_id.as_str(), "AAA")?;
        assert_eq!(
            test_repo
                .commander
                .command_history
                .last()
                .unwrap()
                .args
                .first()
                .unwrap(),
            "describe"
        );

        let head = test_repo.commander.get_current_head()?.commit_id;
        assert_eq!(test_repo.commander.get_commit_description(&head)?, "AAA");

        Ok(())
    }

    #[test]
    fn create_bookmark() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let bookmark = test_repo.commander.create_bookmark("test")?;
        let bookmarks = test_repo.commander.get_bookmarks_list(false)?;

        assert_eq!(bookmarks, [bookmark]);

        Ok(())
    }

    #[test]
    fn create_bookmark_commit() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        // Create new change, since by default `jj bookmark create` uses current change
        let head = test_repo.commander.get_current_head()?;
        test_repo.commander.run_new(head.commit_id.as_str())?;
        assert_ne!(head, test_repo.commander.get_current_head()?);

        let bookmark = test_repo
            .commander
            .create_bookmark_commit("test", &head.commit_id)?;

        let log = test_repo.commander.execute_jj_command(
            [
                "log",
                "--limit",
                "1",
                "--no-graph",
                "-T",
                "commit_id",
                "-r",
                &bookmark.name,
            ],
            false,
            true,
        )?;

        assert_eq!(head.commit_id.to_string(), log);

        Ok(())
    }

    #[test]
    fn set_bookmark_commit() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        // Create new change, since by default `jj bookmark create` uses current change
        let old_head = test_repo.commander.get_current_head()?;
        test_repo.commander.run_new(old_head.commit_id.as_str())?;
        let new_head = test_repo.commander.get_current_head()?;
        assert_ne!(old_head, new_head);

        let bookmark = test_repo.commander.create_bookmark("test")?;

        let log = test_repo.commander.execute_jj_command(
            [
                "log",
                "--limit",
                "1",
                "--no-graph",
                "-T",
                "commit_id",
                "-r",
                &bookmark.name,
            ],
            false,
            true,
        )?;

        assert_eq!(new_head.commit_id.to_string(), log);

        test_repo
            .commander
            .set_bookmark_commit(&bookmark.name, &old_head.commit_id)?;

        let log = test_repo.commander.execute_jj_command(
            [
                "log",
                "--limit",
                "1",
                "--no-graph",
                "-T",
                "commit_id",
                "-r",
                &bookmark.name,
            ],
            false,
            true,
        )?;

        assert_eq!(old_head.commit_id.to_string(), log);

        Ok(())
    }

    #[test]
    fn rename_bookmark() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let bookmark = test_repo.commander.create_bookmark("test1")?;

        let bookmarks = test_repo.commander.get_bookmarks_list(false)?;
        assert_eq!(bookmarks, [bookmark.clone()]);

        test_repo
            .commander
            .rename_bookmark(&bookmark.name, "test2")?;

        let bookmarks = test_repo.commander.get_bookmarks_list(false)?;
        assert_eq!(
            bookmarks,
            [Bookmark {
                name: "test2".to_owned(),
                remote: None,
                present: true,
            }]
        );

        Ok(())
    }

    #[test]
    fn delete_bookmark() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let bookmark = test_repo.commander.create_bookmark("test")?;

        let bookmarks = test_repo.commander.get_bookmarks_list(false)?;
        assert_eq!(bookmarks, [bookmark.clone()]);

        test_repo.commander.delete_bookmark(&bookmark.name)?;

        let bookmarks = test_repo.commander.get_bookmarks_list(false)?;
        assert_eq!(bookmarks, []);

        Ok(())
    }

    #[test]
    fn forget_bookmark() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let bookmark = test_repo.commander.create_bookmark("test")?;

        let bookmarks = test_repo.commander.get_bookmarks_list(false)?;
        assert_eq!(bookmarks, [bookmark.clone()]);

        test_repo.commander.forget_bookmark(&bookmark.name)?;

        let bookmarks = test_repo.commander.get_bookmarks_list(false)?;
        assert_eq!(bookmarks, []);

        Ok(())
    }
}
