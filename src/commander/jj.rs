use crate::commander::{branches::Branch, ids::CommitId, CommandError, Commander};

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

    /// Create branch. Maps to `jj branch create <name>`
    #[instrument(level = "trace", skip(self))]
    pub fn create_branch(&mut self, name: &str) -> Result<Branch, CommandError> {
        self.execute_void_jj_command(vec!["branch", "create", name])?;
        // jj only creates local branches
        Ok(Branch {
            name: name.to_owned(),
            remote: None,
            present: true,
        })
    }

    /// Create branch pointing to commit. Maps to `jj branch create <name> -r <revision>`
    #[instrument(level = "trace", skip(self))]
    pub fn create_branch_commit(
        &mut self,
        name: &str,
        commit_id: &CommitId,
    ) -> Result<Branch, CommandError> {
        self.execute_void_jj_command(vec!["branch", "create", name, "-r", commit_id.as_str()])?;
        // jj only creates local branches
        Ok(Branch {
            name: name.to_owned(),
            remote: None,
            present: true,
        })
    }

    /// Set branch pointing to commit. Maps to `jj branch set <name> -r <revision>`
    #[instrument(level = "trace", skip(self))]
    pub fn set_branch_commit(
        &mut self,
        name: &str,
        commit_id: &CommitId,
    ) -> Result<(), CommandError> {
        // TODO: Maybe don't do --allow-backwards by default?
        self.execute_void_jj_command(vec![
            "branch",
            "set",
            name,
            "-r",
            commit_id.as_str(),
            "--allow-backwards",
        ])
    }

    /// Rename branch. Maps to `jj branch rename <old> <new>`
    #[instrument(level = "trace", skip(self))]
    pub fn rename_branch(&mut self, old: &str, new: &str) -> Result<(), CommandError> {
        self.execute_void_jj_command(vec!["branch", "rename", old, new])
    }

    /// Delete branch. Maps to `jj branch delete <name>`
    #[instrument(level = "trace", skip(self))]
    pub fn delete_branch(&mut self, name: &str) -> Result<(), CommandError> {
        self.execute_void_jj_command(vec!["branch", "delete", name])
    }

    /// Forget branch. Maps to `jj branch forget <name>`
    #[instrument(level = "trace", skip(self))]
    pub fn forget_branch(&mut self, name: &str) -> Result<(), CommandError> {
        self.execute_void_jj_command(vec!["branch", "forget", name])
    }

    /// Track branch. Maps to `jj branch track <branch>@<remote>`
    #[instrument(level = "trace", skip(self))]
    pub fn track_branch(&mut self, branch: &Branch) -> Result<(), CommandError> {
        self.execute_void_jj_command(vec!["branch", "track", &branch.to_string()])
    }

    /// Untrack branch. Maps to `jj branch untrack <branch>@<remote>`
    #[instrument(level = "trace", skip(self))]
    pub fn untrack_branch(&mut self, branch: &Branch) -> Result<(), CommandError> {
        self.execute_void_jj_command(vec!["branch", "untrack", &branch.to_string()])
    }

    /// Git push. Maps to `jj git push`
    #[instrument(level = "trace", skip(self))]
    pub fn git_push(
        &mut self,
        all_branches: bool,
        commit_id: &CommitId,
    ) -> Result<String, CommandError> {
        let mut args = vec!["git", "push"];
        if all_branches {
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
    fn create_branch() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let branch = test_repo.commander.create_branch("test")?;
        let branches = test_repo.commander.get_branches_list(false)?;

        assert_eq!(branches, [branch]);

        Ok(())
    }

    #[test]
    fn create_branch_commit() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        // Create new change, since by default `jj branch create` uses current change
        let head = test_repo.commander.get_current_head()?;
        test_repo.commander.run_new(head.commit_id.as_str())?;
        assert_ne!(head, test_repo.commander.get_current_head()?);

        let branch = test_repo
            .commander
            .create_branch_commit("test", &head.commit_id)?;

        let log = test_repo.commander.execute_jj_command(
            [
                "log",
                "--limit",
                "1",
                "--no-graph",
                "-T",
                "commit_id",
                "-r",
                &branch.name,
            ],
            false,
            true,
        )?;

        assert_eq!(head.commit_id.to_string(), log);

        Ok(())
    }

    #[test]
    fn set_branch_commit() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        // Create new change, since by default `jj branch create` uses current change
        let old_head = test_repo.commander.get_current_head()?;
        test_repo.commander.run_new(old_head.commit_id.as_str())?;
        let new_head = test_repo.commander.get_current_head()?;
        assert_ne!(old_head, new_head);

        let branch = test_repo.commander.create_branch("test")?;

        let log = test_repo.commander.execute_jj_command(
            [
                "log",
                "--limit",
                "1",
                "--no-graph",
                "-T",
                "commit_id",
                "-r",
                &branch.name,
            ],
            false,
            true,
        )?;

        assert_eq!(new_head.commit_id.to_string(), log);

        test_repo
            .commander
            .set_branch_commit(&branch.name, &old_head.commit_id)?;

        let log = test_repo.commander.execute_jj_command(
            [
                "log",
                "--limit",
                "1",
                "--no-graph",
                "-T",
                "commit_id",
                "-r",
                &branch.name,
            ],
            false,
            true,
        )?;

        assert_eq!(old_head.commit_id.to_string(), log);

        Ok(())
    }

    #[test]
    fn rename_branch() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let branch = test_repo.commander.create_branch("test1")?;

        let branches = test_repo.commander.get_branches_list(false)?;
        assert_eq!(branches, [branch.clone()]);

        test_repo.commander.rename_branch(&branch.name, "test2")?;

        let branches = test_repo.commander.get_branches_list(false)?;
        assert_eq!(
            branches,
            [Branch {
                name: "test2".to_owned(),
                remote: None,
                present: true,
            }]
        );

        Ok(())
    }

    #[test]
    fn delete_branch() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let branch = test_repo.commander.create_branch("test")?;

        let branches = test_repo.commander.get_branches_list(false)?;
        assert_eq!(branches, [branch.clone()]);

        test_repo.commander.delete_branch(&branch.name)?;

        let branches = test_repo.commander.get_branches_list(false)?;
        assert_eq!(branches, []);

        Ok(())
    }

    #[test]
    fn forget_branch() -> Result<()> {
        let mut test_repo = TestRepo::new()?;

        let branch = test_repo.commander.create_branch("test")?;

        let branches = test_repo.commander.get_branches_list(false)?;
        assert_eq!(branches, [branch.clone()]);

        test_repo.commander.forget_branch(&branch.name)?;

        let branches = test_repo.commander.get_branches_list(false)?;
        assert_eq!(branches, []);

        Ok(())
    }
}
