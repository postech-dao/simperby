mod implementation;
pub mod reserved_state;
#[cfg(test)]
mod tests;

use super::*;
use async_trait::async_trait;
use eyre::Result;
use git2::{BranchType, ObjectType, Oid, Repository, RepositoryInitOptions};
use implementation::RawRepositoryImplInner;
use simperby_common::reserved::ReservedState;
use std::convert::TryFrom;
use std::fs;
use std::str;
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

/// A commit with abstracted diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticCommit {
    pub title: String,
    pub body: String,
    pub diff: Diff,
}

#[async_trait]
pub trait RawRepository: Send + Sync + 'static {
    /// Initialize the genesis repository from the genesis working tree.
    ///
    /// Fails if there is already a repository.
    async fn init(
        directory: &str,
        init_commit_message: &str,
        init_commit_branch: &Branch,
    ) -> Result<Self, Error>
    where
        Self: Sized;

    /// Loads an exisitng repository.
    async fn open(directory: &str) -> Result<Self, Error>
    where
        Self: Sized;

    /// Clones an exisitng repository.
    ///
    /// Fails if there is no repository with url.
    async fn clone(directory: &str, url: &str) -> Result<Self, Error>
    where
        Self: Sized;

    // ----------------------
    // Branch-related methods
    // ----------------------

    /// Returns the list of branches.
    async fn list_branches(&self) -> Result<Vec<Branch>, Error>;

    /// Creates a branch on the commit.
    async fn create_branch(
        &self,
        branch_name: Branch,
        commit_hash: CommitHash,
    ) -> Result<(), Error>;

    /// Gets the commit that the branch points to.
    async fn locate_branch(&self, branch: Branch) -> Result<CommitHash, Error>;

    /// Gets the list of branches from the commit.
    async fn get_branches(&self, commit_hash: CommitHash) -> Result<Vec<Branch>, Error>;

    /// Moves the branch.
    async fn move_branch(&mut self, branch: Branch, commit_hash: CommitHash) -> Result<(), Error>;

    /// Deletes the branch.
    async fn delete_branch(&mut self, branch: Branch) -> Result<(), Error>;

    // -------------------
    // Tag-related methods
    // -------------------

    /// Returns the list of tags.
    async fn list_tags(&self) -> Result<Vec<Tag>, Error>;

    /// Creates a tag on the given commit.
    async fn create_tag(&mut self, tag: Tag, commit_hash: CommitHash) -> Result<(), Error>;

    /// Gets the commit that the tag points to.
    async fn locate_tag(&self, tag: Tag) -> Result<CommitHash, Error>;

    /// Gets the tags on the given commit.
    async fn get_tag(&self, commit_hash: CommitHash) -> Result<Vec<Tag>, Error>;

    /// Removes the tag.
    async fn remove_tag(&mut self, tag: Tag) -> Result<(), Error>;

    // ----------------------
    // Commit-related methods
    // ----------------------

    /// Creates a commit from the currently checked out branch.
    async fn create_commit(
        &mut self,
        commit_message: String,
        diff: Option<String>,
    ) -> Result<CommitHash, Error>;

    /// Creates a semantic commit from the currently checked out branch.
    ///
    /// It fails if the `diff` is not `Diff::Reserved` or `Diff::None`.
    async fn create_semantic_commit(&mut self, commit: SemanticCommit)
        -> Result<CommitHash, Error>;

    /// Reads the reserved state from the current working tree.
    async fn read_semantic_commit(&self, commit_hash: CommitHash) -> Result<SemanticCommit, Error>;

    /// Removes orphaned commits. Same as `git gc --prune=now --aggressive`
    async fn run_garbage_collection(&mut self) -> Result<(), Error>;

    // ----------------------------
    // Working-tree-related methods
    // ----------------------------

    /// Checkouts and cleans the current working tree.
    /// This is same as `git checkout . && git clean -fd`.
    async fn checkout_clean(&mut self) -> Result<(), Error>;

    /// Checkouts to the branch.
    async fn checkout(&mut self, branch: Branch) -> Result<(), Error>;

    /// Checkouts to the commit and make `HEAD` in a detached mode.
    async fn checkout_detach(&mut self, commit_hash: CommitHash) -> Result<(), Error>;

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
    async fn show_commit(&self, commit_hash: CommitHash) -> Result<String, Error>;

    /// Lists the ancestor commits of the given commit (The first element is the direct parent).
    ///
    /// It fails if there is a merge commit.
    /// * `max`: the maximum number of entries to be returned.
    async fn list_ancestors(
        &self,
        commit_hash: CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error>;

    /// Queries the commits from the very next commit of `ancestor` to `descendant`.
    /// `ancestor` not included, `descendant` included.
    ///
    /// It fails if the two commits are the same.
    /// It fails if the `ancestor` is not the merge base of the two commits.
    async fn query_commit_path(
        &self,
        ancestor: CommitHash,
        descendant: CommitHash,
    ) -> Result<Vec<CommitHash>, Error>;

    /// Returns the children commits of the given commit.
    async fn list_children(&self, commit_hash: CommitHash) -> Result<Vec<CommitHash>, Error>;

    /// Returns the merge base of the two commits.
    async fn find_merge_base(
        &self,
        commit_hash1: CommitHash,
        commit_hash2: CommitHash,
    ) -> Result<CommitHash, Error>;

    /// Reads the reserved state from the currently checked out branch.
    async fn read_reserved_state(&self) -> Result<ReservedState, Error>;

    // ----------------------
    // Remote-related methods
    // ----------------------

    /// Adds a remote repository.
    async fn add_remote(&mut self, remote_name: String, remote_url: String) -> Result<(), Error>;

    /// Removes a remote repository.
    async fn remove_remote(&mut self, remote_name: String) -> Result<(), Error>;

    /// Fetches the remote repository. Same as `git fetch --all -j <LARGE NUMBER>`.
    async fn fetch_all(&mut self) -> Result<(), Error>;

    /// Lists all the remote repositories.
    ///
    /// Returns `(remote_name, remote_url)`.
    async fn list_remotes(&self) -> Result<Vec<(String, String)>, Error>;

    /// Lists all the remote tracking branches.
    ///
    /// Returns `(remote_name, branch_name, commit_hash)`
    async fn list_remote_tracking_branches(
        &self,
    ) -> Result<Vec<(String, String, CommitHash)>, Error>;

    /// Returns the commit of given remote branch.
    async fn locate_remote_tracking_branch(
        &self,
        remote_name: String,
        branch_name: String,
    ) -> Result<CommitHash, Error>;
}

#[derive(Debug)]
pub struct RawRepositoryImpl {
    inner: tokio::sync::Mutex<Option<RawRepositoryImplInner>>,
}

async fn helper_0<R: Send + Sync + 'static>(
    s: &RawRepositoryImpl,
    f: impl Fn(&RawRepositoryImplInner) -> R + Send + 'static,
) -> R {
    let mut lock = s.inner.lock().await;
    let inner = lock.take().expect("RawRepoImpl invariant violated");
    let (result, inner) = tokio::task::spawn_blocking(move || (f(&inner), inner))
        .await
        .unwrap();
    lock.replace(inner);
    result
}

async fn helper_0_mut<R: Send + Sync + 'static>(
    s: &mut RawRepositoryImpl,
    f: impl Fn(&mut RawRepositoryImplInner) -> R + Send + 'static,
) -> R {
    let mut lock = s.inner.lock().await;
    let mut inner = lock.take().expect("RawRepoImpl invariant violated");
    let (result, inner) = tokio::task::spawn_blocking(move || (f(&mut inner), inner))
        .await
        .unwrap();
    lock.replace(inner);
    result
}

async fn helper_1<T1: Send + Sync + 'static + Clone, R: Send + Sync + 'static>(
    s: &RawRepositoryImpl,
    f: impl Fn(&RawRepositoryImplInner, T1) -> R + Send + 'static,
    a1: T1,
) -> R {
    let mut lock = s.inner.lock().await;
    let inner = lock.take().expect("RawRepoImpl invariant violated");
    let (result, inner) = tokio::task::spawn_blocking(move || (f(&inner, a1), inner))
        .await
        .unwrap();
    lock.replace(inner);
    result
}

async fn helper_1_mut<T1: Send + Sync + 'static + Clone, R: Send + Sync + 'static>(
    s: &mut RawRepositoryImpl,
    f: impl Fn(&mut RawRepositoryImplInner, T1) -> R + Send + 'static,
    a1: T1,
) -> R {
    let mut lock = s.inner.lock().await;
    let mut inner = lock.take().expect("RawRepoImpl invariant violated");
    let (result, inner) = tokio::task::spawn_blocking(move || (f(&mut inner, a1), inner))
        .await
        .unwrap();
    lock.replace(inner);
    result
}

async fn helper_2<
    T1: Send + Sync + 'static + Clone,
    T2: Send + Sync + 'static + Clone,
    R: Send + Sync + 'static,
>(
    s: &RawRepositoryImpl,
    f: impl Fn(&RawRepositoryImplInner, T1, T2) -> R + Send + 'static,
    a1: T1,
    a2: T2,
) -> R {
    let mut lock = s.inner.lock().await;
    let inner = lock.take().expect("RawRepoImpl invariant violated");
    let (result, inner) = tokio::task::spawn_blocking(move || (f(&inner, a1, a2), inner))
        .await
        .unwrap();
    lock.replace(inner);
    result
}

async fn helper_2_mut<
    T1: Send + Sync + 'static + Clone,
    T2: Send + Sync + 'static + Clone,
    R: Send + Sync + 'static,
>(
    s: &mut RawRepositoryImpl,
    f: impl Fn(&mut RawRepositoryImplInner, T1, T2) -> R + Send + 'static,
    a1: T1,
    a2: T2,
) -> R {
    let mut lock = s.inner.lock().await;
    let mut inner = lock.take().expect("RawRepoImpl invariant violated");
    let (result, inner) = tokio::task::spawn_blocking(move || (f(&mut inner, a1, a2), inner))
        .await
        .unwrap();
    lock.replace(inner);
    result
}

#[async_trait]
impl RawRepository for RawRepositoryImpl {
    async fn init(
        directory: &str,
        init_commit_message: &str,
        init_commit_branch: &Branch,
    ) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let repo =
            RawRepositoryImplInner::init(directory, init_commit_message, init_commit_branch)?;
        let inner = tokio::sync::Mutex::new(Some(repo));

        Ok(Self { inner })
    }

    async fn open(directory: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let repo = RawRepositoryImplInner::open(directory)?;
        let inner = tokio::sync::Mutex::new(Some(repo));

        Ok(Self { inner })
    }

    async fn clone(directory: &str, url: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let repo = RawRepositoryImplInner::clone(directory, url)?;
        let inner = tokio::sync::Mutex::new(Some(repo));

        Ok(Self { inner })
    }

    async fn list_branches(&self) -> Result<Vec<Branch>, Error> {
        helper_0(self, RawRepositoryImplInner::list_branches).await
    }

    async fn create_branch(
        &self,
        branch_name: Branch,
        commit_hash: CommitHash,
    ) -> Result<(), Error> {
        helper_2(
            self,
            RawRepositoryImplInner::create_branch,
            branch_name,
            commit_hash,
        )
        .await
    }

    async fn locate_branch(&self, branch: Branch) -> Result<CommitHash, Error> {
        helper_1(self, RawRepositoryImplInner::locate_branch, branch).await
    }

    async fn get_branches(&self, commit_hash: CommitHash) -> Result<Vec<Branch>, Error> {
        helper_1(self, RawRepositoryImplInner::get_branches, commit_hash).await
    }

    async fn move_branch(&mut self, branch: Branch, commit_hash: CommitHash) -> Result<(), Error> {
        helper_2_mut(
            self,
            RawRepositoryImplInner::move_branch,
            branch,
            commit_hash,
        )
        .await
    }

    async fn delete_branch(&mut self, branch: Branch) -> Result<(), Error> {
        helper_1_mut(self, RawRepositoryImplInner::delete_branch, branch).await
    }

    async fn list_tags(&self) -> Result<Vec<Tag>, Error> {
        helper_0(self, RawRepositoryImplInner::list_tags).await
    }

    async fn create_tag(&mut self, tag: Tag, commit_hash: CommitHash) -> Result<(), Error> {
        helper_2_mut(self, RawRepositoryImplInner::create_tag, tag, commit_hash).await
    }

    async fn locate_tag(&self, tag: Tag) -> Result<CommitHash, Error> {
        helper_1(self, RawRepositoryImplInner::locate_tag, tag).await
    }

    async fn get_tag(&self, commit_hash: CommitHash) -> Result<Vec<Tag>, Error> {
        helper_1(self, RawRepositoryImplInner::get_tag, commit_hash).await
    }

    async fn remove_tag(&mut self, tag: Tag) -> Result<(), Error> {
        helper_1_mut(self, RawRepositoryImplInner::remove_tag, tag).await
    }

    async fn create_commit(
        &mut self,
        commit_message: String,
        diff: Option<String>,
    ) -> Result<CommitHash, Error> {
        helper_2_mut(
            self,
            RawRepositoryImplInner::create_commit,
            commit_message,
            diff,
        )
        .await
    }

    async fn create_semantic_commit(
        &mut self,
        commit: SemanticCommit,
    ) -> Result<CommitHash, Error> {
        helper_1_mut(self, RawRepositoryImplInner::create_semantic_commit, commit).await
    }

    async fn read_semantic_commit(&self, commit_hash: CommitHash) -> Result<SemanticCommit, Error> {
        helper_1(
            self,
            RawRepositoryImplInner::read_semantic_commit,
            commit_hash,
        )
        .await
    }

    async fn run_garbage_collection(&mut self) -> Result<(), Error> {
        helper_0_mut(self, RawRepositoryImplInner::run_garbage_collection).await
    }

    async fn checkout_clean(&mut self) -> Result<(), Error> {
        helper_0_mut(self, RawRepositoryImplInner::checkout_clean).await
    }

    async fn checkout(&mut self, branch: Branch) -> Result<(), Error> {
        helper_1_mut(self, RawRepositoryImplInner::checkout, branch).await
    }

    async fn checkout_detach(&mut self, commit_hash: CommitHash) -> Result<(), Error> {
        helper_1_mut(self, RawRepositoryImplInner::checkout_detach, commit_hash).await
    }

    async fn get_head(&self) -> Result<CommitHash, Error> {
        helper_0(self, RawRepositoryImplInner::get_head).await
    }

    async fn get_initial_commit(&self) -> Result<CommitHash, Error> {
        helper_0(self, RawRepositoryImplInner::get_initial_commit).await
    }

    async fn show_commit(&self, commit_hash: CommitHash) -> Result<String, Error> {
        helper_1(self, RawRepositoryImplInner::show_commit, commit_hash).await
    }

    async fn list_ancestors(
        &self,
        commit_hash: CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error> {
        helper_2(
            self,
            RawRepositoryImplInner::list_ancestors,
            commit_hash,
            max,
        )
        .await
    }

    async fn query_commit_path(
        &self,
        ancestor: CommitHash,
        descendant: CommitHash,
    ) -> Result<Vec<CommitHash>, Error> {
        helper_2(
            self,
            RawRepositoryImplInner::query_commit_path,
            ancestor,
            descendant,
        )
        .await
    }

    async fn list_children(&self, commit_hash: CommitHash) -> Result<Vec<CommitHash>, Error> {
        helper_1(self, RawRepositoryImplInner::list_children, commit_hash).await
    }

    async fn find_merge_base(
        &self,
        commit_hash1: CommitHash,
        commit_hash2: CommitHash,
    ) -> Result<CommitHash, Error> {
        helper_2(
            self,
            RawRepositoryImplInner::find_merge_base,
            commit_hash1,
            commit_hash2,
        )
        .await
    }

    async fn read_reserved_state(&self) -> Result<ReservedState, Error> {
        helper_0(self, RawRepositoryImplInner::read_reserved_state).await
    }

    async fn add_remote(&mut self, remote_name: String, remote_url: String) -> Result<(), Error> {
        helper_2_mut(
            self,
            RawRepositoryImplInner::add_remote,
            remote_name,
            remote_url,
        )
        .await
    }

    async fn remove_remote(&mut self, remote_name: String) -> Result<(), Error> {
        helper_1_mut(self, RawRepositoryImplInner::remove_remote, remote_name).await
    }

    async fn fetch_all(&mut self) -> Result<(), Error> {
        helper_0_mut(self, RawRepositoryImplInner::fetch_all).await
    }

    async fn list_remotes(&self) -> Result<Vec<(String, String)>, Error> {
        helper_0(self, RawRepositoryImplInner::list_remotes).await
    }

    async fn list_remote_tracking_branches(
        &self,
    ) -> Result<Vec<(String, String, CommitHash)>, Error> {
        helper_0(self, RawRepositoryImplInner::list_remote_tracking_branches).await
    }

    async fn locate_remote_tracking_branch(
        &self,
        remote_name: String,
        branch_name: String,
    ) -> Result<CommitHash, Error> {
        helper_2(
            self,
            RawRepositoryImplInner::locate_remote_tracking_branch,
            remote_name,
            branch_name,
        )
        .await
    }
}
