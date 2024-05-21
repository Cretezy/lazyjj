use crate::commander::{ids::CommitId, Commander};

use anyhow::{Context, Result};

impl Commander {
    /// Create a new change after change. Maps to `jj new <revision>`
    pub fn run_new(&mut self, commit_id: &CommitId) -> Result<()> {
        self.execute_void_jj_command(vec!["new", commit_id.as_str()])
            .context("Failed executing jj new")
    }

    /// Edit change. Maps to `jj edit <revision>`
    pub fn run_edit(&mut self, commit_id: &CommitId) -> Result<()> {
        self.execute_void_jj_command(vec!["edit", commit_id.as_str()])
            .context("Failed executing jj edit")
    }

    /// Abandon change. Maps to `jj abandon <revision>`
    pub fn run_abandon(&mut self, commit_id: &CommitId) -> Result<()> {
        self.execute_void_jj_command(vec!["abandon", commit_id.as_str()])
            .context("Failed executing jj abandon")
    }

    /// Describe change. Maps to `jj describe <revision> -m <message>`
    pub fn run_describe(&mut self, commit_id: &CommitId, message: &str) -> Result<()> {
        self.execute_void_jj_command(vec!["describe", commit_id.as_str(), "-m", message])
            .context("Failed executing jj describe")
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
        test_repo.commander.run_new(&head.commit_id)?;
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
        test_repo.commander.run_new(&head.commit_id)?;
        assert_ne!(head, test_repo.commander.get_current_head()?);
        test_repo.commander.run_edit(&head.commit_id)?;
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
        test_repo.commander.run_describe(&head.commit_id, "AAA")?;
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
}
