mod implementation;
pub mod reserved_state;
mod templates;
#[cfg(test)]
mod tests;

use super::*;
use eyre::Result;
use git2::{
    ApplyLocation, BranchType, DiffFormat, Email, EmailCreateOptions, IndexAddOption, ObjectType,
    Oid, Repository, RepositoryInitOptions, ResetType, Sort, Status, StatusOptions, StatusShow,
};
use implementation::RawRepositoryInner;
use simperby_common::reserved::ReservedState;
use std::convert::TryFrom;
use std::str;
use templates::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("git2 error: {0}")]
    Git2Error(git2::Error),
    /// The given git object doesn't exist.
    #[error("not found: {0}")]
    NotFound(String),
    /// The assumption of the method
    /// (e.g., there is no merge commit, there must be a merge base, ..) is violated.
    #[error("the repository is invalid: {0}")]
    InvalidRepository(String),
    #[error("unknown error: {0}")]
    Unknown(String),
}

impl From<git2::Error> for Error {
    fn from(e: git2::Error) -> Self {
        Error::Git2Error(e)
    }
}

/// A commit with abstracted diff. The committer is always the same as the author.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticCommit {
    pub title: String,
    pub body: String,
    pub diff: Diff,
    pub author: MemberName,
    pub timestamp: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawCommit {
    pub message: String,
    pub diff: Option<String>,
    pub author: String,
    pub email: String,
    pub timestamp: Timestamp,
}

#[derive(Debug)]
pub struct RawRepository {
    inner: tokio::sync::Mutex<Option<RawRepositoryInner>>,
}

impl RawRepository {
    /// Initialize the genesis repository from the genesis working tree.
    ///
    /// Fails if there is already a repository.
    pub async fn init(
        directory: &str,
        init_commit_message: &str,
        init_commit_branch: &Branch,
    ) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let repo = RawRepositoryInner::init(directory, init_commit_message, init_commit_branch)?;
        let inner = tokio::sync::Mutex::new(Some(repo));

        Ok(Self { inner })
    }

    /// Loads an exisitng repository.
    pub async fn open(directory: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let repo = RawRepositoryInner::open(directory)?;
        let inner = tokio::sync::Mutex::new(Some(repo));

        Ok(Self { inner })
    }

    /// Clones an exisitng repository.
    ///
    /// Fails if there is no repository with url.
    pub async fn clone(directory: &str, url: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let repo = RawRepositoryInner::clone(directory, url)?;
        let inner = tokio::sync::Mutex::new(Some(repo));

        Ok(Self { inner })
    }

    /// Returns the full commit hash from the revision selection string.
    ///
    /// See the [reference](https://git-scm.com/book/en/v2/Git-Tools-Revision-Selection).
    pub async fn retrieve_commit_hash(
        &self,
        revision_selection: String,
    ) -> Result<CommitHash, Error> {
        helper_1(
            self,
            RawRepositoryInner::retrieve_commit_hash,
            revision_selection,
        )
        .await
    }

    // ----------------------
    // Branch-related methods
    // ----------------------

    /// Returns the list of branches.
    pub async fn list_branches(&self) -> Result<Vec<Branch>, Error> {
        helper_0(self, RawRepositoryInner::list_branches).await
    }

    /// Creates a branch on the commit.
    pub async fn create_branch(
        &self,
        branch_name: Branch,
        commit_hash: CommitHash,
    ) -> Result<(), Error> {
        helper_2(
            self,
            RawRepositoryInner::create_branch,
            branch_name,
            commit_hash,
        )
        .await
    }

    /// Gets the commit that the branch points to.
    pub async fn locate_branch(&self, branch: Branch) -> Result<CommitHash, Error> {
        helper_1(self, RawRepositoryInner::locate_branch, branch).await
    }

    /// Gets the list of branches from the commit.
    pub async fn get_branches(&self, commit_hash: CommitHash) -> Result<Vec<Branch>, Error> {
        helper_1(self, RawRepositoryInner::get_branches, commit_hash).await
    }

    /// Moves the branch.
    pub async fn move_branch(
        &mut self,
        branch: Branch,
        commit_hash: CommitHash,
    ) -> Result<(), Error> {
        helper_2_mut(self, RawRepositoryInner::move_branch, branch, commit_hash).await
    }

    /// Deletes the branch.
    pub async fn delete_branch(&mut self, branch: Branch) -> Result<(), Error> {
        helper_1_mut(self, RawRepositoryInner::delete_branch, branch).await
    }

    // -------------------
    // Tag-related methods
    // -------------------

    /// Returns the list of tags.
    pub async fn list_tags(&self) -> Result<Vec<Tag>, Error> {
        helper_0(self, RawRepositoryInner::list_tags).await
    }

    /// Creates a tag on the given commit.
    pub async fn create_tag(&mut self, tag: Tag, commit_hash: CommitHash) -> Result<(), Error> {
        helper_2_mut(self, RawRepositoryInner::create_tag, tag, commit_hash).await
    }

    /// Gets the commit that the tag points to.
    pub async fn locate_tag(&self, tag: Tag) -> Result<CommitHash, Error> {
        helper_1(self, RawRepositoryInner::locate_tag, tag).await
    }

    /// Gets the tags on the given commit.
    pub async fn get_tag(&self, commit_hash: CommitHash) -> Result<Vec<Tag>, Error> {
        helper_1(self, RawRepositoryInner::get_tag, commit_hash).await
    }

    /// Removes the tag.
    pub async fn remove_tag(&mut self, tag: Tag) -> Result<(), Error> {
        helper_1_mut(self, RawRepositoryInner::remove_tag, tag).await
    }

    // ----------------------
    // Commit-related methods
    // ----------------------

    /// Creates a commit from the currently checked out branch.
    ///
    /// Committer will be the same as the author.
    pub async fn create_commit(&mut self, commit: RawCommit) -> Result<CommitHash, Error> {
        helper_1_mut(self, RawRepositoryInner::create_commit, commit).await
    }

    pub async fn read_commit(&self, commit_hash: CommitHash) -> Result<RawCommit, Error> {
        helper_1(self, RawRepositoryInner::read_commit, commit_hash).await
    }

    /// Creates a semantic commit from the currently checked out branch.
    ///
    /// It fails if the `diff` is not `Diff::Reserved` or `Diff::None`.
    pub async fn create_semantic_commit(
        &mut self,
        commit: SemanticCommit,
    ) -> Result<CommitHash, Error> {
        helper_1_mut(self, RawRepositoryInner::create_semantic_commit, commit).await
    }

    /// Reads the reserved state from the current working tree.
    pub async fn read_semantic_commit(
        &self,
        commit_hash: CommitHash,
    ) -> Result<SemanticCommit, Error> {
        helper_1(self, RawRepositoryInner::read_semantic_commit, commit_hash).await
    }

    /// Removes orphaned commits. Same as `git gc --prune=now --aggressive`
    pub async fn run_garbage_collection(&mut self) -> Result<(), Error> {
        helper_0_mut(self, RawRepositoryInner::run_garbage_collection).await
    }

    // ----------------------------
    // Working-tree-related methods
    // ----------------------------

    /// Checkouts and cleans the current working tree.
    /// This is same as `git checkout . && git clean -fd`.
    pub async fn checkout_clean(&mut self) -> Result<(), Error> {
        helper_0_mut(self, RawRepositoryInner::checkout_clean).await
    }

    /// Checkouts to the branch.
    pub async fn checkout(&mut self, branch: Branch) -> Result<(), Error> {
        helper_1_mut(self, RawRepositoryInner::checkout, branch).await
    }

    /// Checkouts to the commit and make `HEAD` in a detached mode.
    pub async fn checkout_detach(&mut self, commit_hash: CommitHash) -> Result<(), Error> {
        helper_1_mut(self, RawRepositoryInner::checkout_detach, commit_hash).await
    }

    /// Saves the local modifications to a new stash.
    pub async fn stash(&mut self) -> Result<(), Error> {
        helper_0_mut(self, RawRepositoryInner::stash).await
    }

    /// Pops the most recent stash.
    pub async fn stash_pop(&mut self) -> Result<(), Error> {
        helper_0_mut(self, RawRepositoryInner::stash_pop).await
    }

    /// Applys the most recent stash.
    pub async fn stash_apply(&mut self) -> Result<(), Error> {
        helper_0_mut(self, RawRepositoryInner::stash_apply).await
    }

    /// Removes the most recent stash.
    pub async fn stash_drop(&mut self) -> Result<(), Error> {
        helper_0_mut(self, RawRepositoryInner::stash_drop).await
    }

    /// Checks if there are no unstaged, staged and untracked files.
    pub async fn check_clean(&self) -> Result<(), Error> {
        helper_0(self, RawRepositoryInner::check_clean).await
    }

    // ---------------
    // Various queries
    // ---------------

    /// Returns the commit hash of the current HEAD.
    pub async fn get_head(&self) -> Result<CommitHash, Error> {
        helper_0(self, RawRepositoryInner::get_head).await
    }

    /// Returns the currently checked-out branch, if any.
    /// If the repository is in a detached HEAD state, it returns None.
    pub async fn get_currently_checkout_branch(&self) -> Result<Option<Branch>, Error> {
        helper_0(self, RawRepositoryInner::get_currently_checkout_branch).await
    }

    /// Returns the commit hash of the initial commit.
    ///
    /// Fails if the repository is empty.
    pub async fn get_initial_commit(&self) -> Result<CommitHash, Error> {
        helper_0(self, RawRepositoryInner::get_initial_commit).await
    }

    /// Returns the patch of the given commit.
    pub async fn get_patch(&self, commit_hash: CommitHash) -> Result<String, Error> {
        helper_1(self, RawRepositoryInner::get_patch, commit_hash).await
    }

    /// Returns the diff of the given commit.
    pub async fn show_commit(&self, commit_hash: CommitHash) -> Result<String, Error> {
        helper_1(self, RawRepositoryInner::show_commit, commit_hash).await
    }

    /// Lists the ancestor commits of the given commit (The first element is the direct parent).
    ///
    /// It fails if there is a merge commit.
    /// * `max`: the maximum number of entries to be returned.
    pub async fn list_ancestors(
        &self,
        commit_hash: CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error> {
        helper_2(self, RawRepositoryInner::list_ancestors, commit_hash, max).await
    }

    /// Queries the commits from the very next commit of `ancestor` to `descendant`.
    /// `ancestor` not included, `descendant` included.
    ///
    /// It fails if the two commits are the same.
    /// It fails if the `ancestor` is not the merge base of the two commits.
    pub async fn query_commit_path(
        &self,
        ancestor: CommitHash,
        descendant: CommitHash,
    ) -> Result<Vec<CommitHash>, Error> {
        helper_2(
            self,
            RawRepositoryInner::query_commit_path,
            ancestor,
            descendant,
        )
        .await
    }

    /// Returns the children commits of the given commit.
    pub async fn list_children(&self, commit_hash: CommitHash) -> Result<Vec<CommitHash>, Error> {
        helper_1(self, RawRepositoryInner::list_children, commit_hash).await
    }

    /// Returns the merge base of the two commits.
    pub async fn find_merge_base(
        &self,
        commit_hash1: CommitHash,
        commit_hash2: CommitHash,
    ) -> Result<CommitHash, Error> {
        helper_2(
            self,
            RawRepositoryInner::find_merge_base,
            commit_hash1,
            commit_hash2,
        )
        .await
    }

    /// Reads the reserved state from the currently checked out branch.
    pub async fn read_reserved_state(&self) -> Result<ReservedState, Error> {
        helper_0(self, RawRepositoryInner::read_reserved_state).await
    }

    /// Reads the reserved state at given commit hash.
    pub async fn read_reserved_state_at_commit(
        &self,
        commit_hash: CommitHash,
    ) -> Result<ReservedState, Error> {
        helper_1(
            self,
            RawRepositoryInner::read_reserved_state_at_commit,
            commit_hash,
        )
        .await
    }

    // ----------------------
    // Remote-related methods
    // ----------------------

    /// Adds a remote repository.
    pub async fn add_remote(
        &mut self,
        remote_name: String,
        remote_url: String,
    ) -> Result<(), Error> {
        helper_2_mut(
            self,
            RawRepositoryInner::add_remote,
            remote_name,
            remote_url,
        )
        .await
    }

    /// Removes a remote repository.
    pub async fn remove_remote(&mut self, remote_name: String) -> Result<(), Error> {
        helper_1_mut(self, RawRepositoryInner::remove_remote, remote_name).await
    }

    /// Fetches the remote repository. Same as `git fetch --all -j <LARGE NUMBER>`.
    pub async fn fetch_all(&mut self) -> Result<(), Error> {
        helper_0_mut(self, RawRepositoryInner::fetch_all).await
    }

    /// Pushes to the remote repository with the push option.
    /// This is same as `git push <remote_name> <branch_name> --push-option=<string>`.
    pub async fn push_option(
        &self,
        remote_name: String,
        branch: Branch,
        option: Option<String>,
    ) -> Result<(), Error> {
        helper_3(
            self,
            RawRepositoryInner::push_option,
            remote_name,
            branch,
            option,
        )
        .await
    }

    /// Lists all the remote repositories.
    ///
    /// Returns `(remote_name, remote_url)`.
    pub async fn list_remotes(&self) -> Result<Vec<(String, String)>, Error> {
        helper_0(self, RawRepositoryInner::list_remotes).await
    }

    /// Lists all the remote tracking branches.
    ///
    /// Returns `(remote_name, branch_name, commit_hash)`
    pub async fn list_remote_tracking_branches(
        &self,
    ) -> Result<Vec<(String, String, CommitHash)>, Error> {
        helper_0(self, RawRepositoryInner::list_remote_tracking_branches).await
    }

    /// Returns the commit of given remote branch.
    pub async fn locate_remote_tracking_branch(
        &self,
        remote_name: String,
        branch_name: String,
    ) -> Result<CommitHash, Error> {
        helper_2(
            self,
            RawRepositoryInner::locate_remote_tracking_branch,
            remote_name,
            branch_name,
        )
        .await
    }
}

#[cfg(target_os = "windows")]
pub fn run_command(command: impl AsRef<str>) -> Result<(), Error> {
    println!("> RUN: {}", command.as_ref());
    let mut child = std::process::Command::new("C:/Program Files/Git/bin/sh.exe")
        .arg("--login")
        .arg("-c")
        .arg(command.as_ref())
        .spawn()
        .map_err(|_| Error::Unknown("failed to execute process".to_string()))?;
    let ecode = child
        .wait()
        .map_err(|_| Error::Unknown("failed to wait on child".to_string()))?;

    if ecode.success() {
        Ok(())
    } else {
        Err(Error::Unknown("failed to run process".to_string()))
    }
}

#[cfg(not(target_os = "windows"))]
pub fn run_command(command: impl AsRef<str>) -> Result<(), Error> {
    println!("> RUN: {}", command.as_ref());
    let mut child = std::process::Command::new("sh")
        .arg("-c")
        .arg(command.as_ref())
        .spawn()
        .map_err(|_| Error::Unknown("failed to execute process".to_string()))?;

    let ecode = child
        .wait()
        .map_err(|_| Error::Unknown("failed to wait on child".to_string()))?;

    if ecode.success() {
        Ok(())
    } else {
        Err(Error::Unknown("failed to run process".to_string()))
    }
}
