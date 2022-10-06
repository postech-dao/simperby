use super::*;
use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("git2 error: {0}")]
    Git2Error(git2::Error),
    /// When the assumption of the method (e.g., there is no merge commit) is violated.
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

/// A raw handle for the local repository.
///
/// It automatically locks the repository once created.
#[async_trait]
pub trait RawRepository {
    /// Initialize the genesis repository from the genesis working tree.
    ///
    /// Fails if there is already a repository.
    async fn init(directory: &str) -> Result<Self, Error>
    where
        Self: Sized;

    // Loads an exisitng repository.
    async fn open(directory: &str) -> Result<Self, Error>
    where
        Self: Sized;

    // ----------------------
    // Branch-related methods
    // ----------------------

    /// Returns the list of branches.
    async fn list_branchs(&self) -> Result<Vec<Branch>, Error>;

    /// Creates a branch on the commit.
    async fn create_branch(
        &self,
        branch_name: &Branch,
        commit_hash: CommitHash,
    ) -> Result<(), Error>;

    /// Gets the commit that the branch points to.
    async fn locate_branch(&self, branch: &Branch) -> Result<CommitHash, Error>;

    /// Gets the list of branches from the commit.
    async fn get_branches(&self) -> Result<Vec<Branch>, Error>;

    /// Deletes the branch.
    async fn move_branch(&mut self, branch: &Branch, commit_hash: &CommitHash)
        -> Result<(), Error>;

    /// Deletes the branch.
    async fn delete_branch(&mut self, branch: &Branch) -> Result<(), Error>;

    // -------------------
    // Tag-related methods
    // -------------------

    /// Returns the list of branches.
    async fn list_tags(&self) -> Result<Vec<Tag>, Error>;

    /// Creates a tag on the given commit.
    async fn create_tag(&mut self, tag: &Tag, commit_hash: &CommitHash) -> Result<(), Error>;

    /// Gets the commit that the tag points to.
    async fn locate_tag(&self, tag: &Tag) -> Result<CommitHash, Error>;

    /// Gets the tags on the given commit.
    async fn get_tag(&self, commit_hash: &CommitHash) -> Result<Vec<Tag>, Error>;

    /// Removes the tag.
    async fn remove_tag(&mut self, tag: &Tag) -> Result<(), Error>;

    // ----------------------
    // Commit-related methods
    // ----------------------

    /// Create a commit with the branch.
    ///
    /// Same as `checkout_clean() && git checkout <branch> && git add -A && git commit`.
    async fn create_commit(
        &mut self,
        branch: &Branch,
        commit_message: &str,
        diff: Option<&str>,
    ) -> Result<(), Error>;

    /// Create a commit with the given list to overwrite files.
    ///
    /// * `files`: the list of `(file_path, file_content_to_overwrite)`.
    async fn create_commit_overwrite(
        &mut self,
        branch: &Branch,
        commit_message: &str,
        files: Vec<(String, String)>,
    ) -> Result<(), Error>;

    /// Removes orphaned commits. Same as `git gc --prune=now --aggressive`
    async fn run_garbage_collection(&mut self) -> Result<(), Error>;

    // ----------------------------
    // Working-tree-related methods
    // ----------------------------

    /// Checkouts and cleans the current working tree.
    /// This is same as `git checkout . && git clean -fd`.
    async fn checkout_clean(&mut self) -> Result<(), Error>;

    /// Checkouts to the branch.
    async fn checkout(&mut self, branch: &Branch) -> Result<(), Error>;

    /// Checkouts to the commit and make `HEAD` in a detached mode.
    async fn checkout_detach(&mut self, commit_hash: &CommitHash) -> Result<(), Error>;

    // ---------------
    // Various queries
    // ---------------

    /// Returns the commit hash of the current HEAD.
    async fn get_head(&self) -> Result<CommitHash, Error>;

    /// Returns the commit hash of the initial commit.
    ///
    /// Fails if the repository is empty.
    async fn get_initial_commit(&self) -> Result<CommitHash, Error>;

    /// Returns the diff of the given commit.
    async fn show_commit(&self, commit_hash: &CommitHash) -> Result<String, Error>;

    /// Lists the ancestor commits of the given commit (The first element is the direct parent).
    ///
    /// It fails if there is a merge commit.
    async fn list_ancestors(
        &self,
        commit_hash: &CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error>;

    /// Lists the descendant commits of the given commit (The first element is the direct child).
    ///
    /// It fails if there are diverged commits (i.e., having multiple children commit)
    async fn list_descendants(
        &self,
        commit_hash: &CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error>;

    /// Returns the children commits of the given commit.
    async fn list_children(&self, commit_hash: &CommitHash) -> Result<Vec<CommitHash>, Error>;

    /// Returns the merge base of the two commits.
    async fn find_merge_base(
        &self,
        commit_hash1: &CommitHash,
        commit_hash2: &CommitHash,
    ) -> Result<CommitHash, Error>;

    // ----------------------------
    // Remote-related methods
    // ----------------------------

    /// Adds a remote repository.
    async fn add_remote(&mut self, remote_name: &str, remote_url: &str) -> Result<(), Error>;

    /// Removes a remote repository.
    async fn remove_remote(&mut self, remote_name: &str) -> Result<(), Error>;

    /// Fetches the remote repository. Same as `git fetch --all -j <LARGE NUMBER>`.
    async fn fetch_all(&mut self) -> Result<(), Error>;

    /// Lists all the remote repositories.
    async fn list_remotes(&self) -> Result<Vec<(String, String)>, Error>;

    /// Lists all the remote tracking branches.
    async fn list_remote_tracking_branches(
        &self,
    ) -> Result<Vec<(String, String, CommitHash)>, Error>;
}
