#![allow(dead_code, unused)]
use super::*;
use anyhow::Result;
use async_trait::async_trait;
use git2::{BranchType, ObjectType, Oid, Repository, RepositoryInitOptions};
use simperby_common::reserved::ReservedState;
use std::convert::TryFrom;
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

/// A commit without any diff on non-reserved area.
#[derive(Debug, Clone)]
pub struct SemanticCommit {
    pub title: String,
    pub body: String,
    /// (If this commit made any change) the new reserved state.
    pub reserved_state: Option<ReservedState>,
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

    // Loads an exisitng repository.
    async fn open(directory: &str) -> Result<Self, Error>
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
        branch_name: &Branch,
        commit_hash: CommitHash,
    ) -> Result<(), Error>;

    /// Gets the commit that the branch points to.
    async fn locate_branch(&self, branch: &Branch) -> Result<CommitHash, Error>;

    /// Gets the list of branches from the commit.
    async fn get_branches(&self, commit_hash: &CommitHash) -> Result<Vec<Branch>, Error>;

    /// Moves the branch.
    async fn move_branch(&mut self, branch: &Branch, commit_hash: &CommitHash)
        -> Result<(), Error>;

    /// Deletes the branch.
    async fn delete_branch(&mut self, branch: &Branch) -> Result<(), Error>;

    // -------------------
    // Tag-related methods
    // -------------------

    /// Returns the list of tags.
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

    /// Creates a commit from the currently checked out branch.
    async fn create_commit(
        &mut self,
        commit_message: &str,
        diff: Option<&str>,
    ) -> Result<CommitHash, Error>;

    /// Creates a semantic commit from the currently checked out branch.
    async fn create_semantic_commit(&mut self, commit: SemanticCommit)
        -> Result<CommitHash, Error>;

    /// Reads the reserved state from the current working tree.
    async fn read_semantic_commit(&self, commit_hash: &CommitHash)
        -> Result<SemanticCommit, Error>;

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
    /// * `max`: the maximum number of entries to be returned.
    async fn list_ancestors(
        &self,
        commit_hash: &CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error>;

    /// Lists the descendant commits of the given commit (The first element is the direct child).
    ///
    /// It fails if there are diverged commits (i.e., having multiple children commit)
    /// * `max`: the maximum number of entries to be returned.
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

    // ----------------------
    // Remote-related methods
    // ----------------------

    /// Adds a remote repository.
    async fn add_remote(&mut self, remote_name: &str, remote_url: &str) -> Result<(), Error>;

    /// Removes a remote repository.
    async fn remove_remote(&mut self, remote_name: &str) -> Result<(), Error>;

    /// Fetches the remote repository. Same as `git fetch --all -j <LARGE NUMBER>`.
    async fn fetch_all(&mut self) -> Result<(), Error>;

    /// Lists all the remote repositories.
    ///
    /// Returns `(remote_name, remote_url)`.
    async fn list_remotes(&self) -> Result<Vec<(String, String)>, Error>;

    /// Lists all the remote tracking branches.
    ///
    /// Returns `(remote_name, remote_url, commit_hash)`
    async fn list_remote_tracking_branches(
        &self,
    ) -> Result<Vec<(String, String, CommitHash)>, Error>;
}

impl fmt::Debug for RawRepositoryImplInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?")
    }
}

pub struct RawRepositoryImplInner {
    repo: Repository,
}

/// TODO: Error handling and its messages
impl RawRepositoryImplInner {
    fn init(
        directory: &str,
        init_commit_message: &str,
        init_commit_branch: &Branch,
    ) -> Result<Self, Error>
    where
        Self: Sized,
    {
        match Repository::open(directory) {
            Ok(_repo) => Err(Error::InvalidRepository(
                "there is an already existing repository".to_string(),
            )),
            Err(_e) => {
                let mut opts = RepositoryInitOptions::new();
                opts.initial_head(init_commit_branch.as_str());
                let repo = Repository::init_opts(directory, &opts)?;
                {
                    // Create initial empty commit
                    let mut config = repo.config()?;
                    config.set_str("user.name", "name")?; // TODO: user.name value
                    config.set_str("user.email", "email")?; // TODO: user.email value
                    let mut index = repo.index()?;
                    let id = index.write_tree()?;
                    let sig = repo.signature()?;
                    let tree = repo.find_tree(id)?;

                    let _oid =
                        repo.commit(Some("HEAD"), &sig, &sig, init_commit_message, &tree, &[])?;
                }

                Ok(Self { repo })
            }
        }
    }

    fn open(directory: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let repo = Repository::open(directory)?;

        Ok(Self { repo })
    }

    fn list_branches(&self) -> Result<Vec<Branch>, Error> {
        let branches = self.repo.branches(Option::Some(BranchType::Local))?;

        branches
            .map(|branch| {
                let branch_name = branch?
                    .0
                    .name()?
                    .map(|name| name.to_string())
                    .ok_or_else(|| Error::Unknown("err".to_string()))?;

                Ok(branch_name)
            })
            .collect::<Result<Vec<Branch>, Error>>()
    }

    fn create_branch(&self, branch_name: &Branch, commit_hash: CommitHash) -> Result<(), Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let commit = self.repo.find_commit(oid)?;

        // TODO: Test if force true and verify new branch is created
        self.repo.branch(branch_name.as_str(), &commit, false)?;

        Ok(())
    }

    fn locate_branch(&self, branch: &Branch) -> Result<CommitHash, Error> {
        let branch = self.repo.find_branch(branch, BranchType::Local)?;
        let oid = branch
            .get()
            .target()
            .ok_or_else(|| Error::Unknown("err".to_string()))?;
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })
    }

    fn get_branches(&self, _commit_hash: &CommitHash) -> Result<Vec<Branch>, Error> {
        unimplemented!()
    }

    fn move_branch(&mut self, branch: &Branch, commit_hash: &CommitHash) -> Result<(), Error> {
        let mut git2_branch = self.repo.find_branch(branch, BranchType::Local)?;
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let reflog_msg = ""; // TODO: reflog_msg
        let reference = git2_branch.get_mut();
        let _set_branch = git2::Reference::set_target(reference, oid, reflog_msg)?;

        Ok(())
    }

    fn delete_branch(&mut self, branch: &Branch) -> Result<(), Error> {
        let mut git2_branch = self.repo.find_branch(branch, BranchType::Local)?;

        let current_branch = self
            .repo
            .head()?
            .shorthand()
            .ok_or_else(|| Error::Unknown("err".to_string()))?
            .to_string();

        if &current_branch == branch {
            Err(Error::InvalidRepository(
                ("given branch is currently checkout branch").to_string(),
            ))
        } else {
            git2_branch.delete().map_err(Error::from)
        }
    }

    fn list_tags(&self) -> Result<Vec<Tag>, Error> {
        let tag_array = self.repo.tag_names(None)?;

        let tag_list = tag_array
            .iter()
            .map(|tag| {
                let tag_name = tag
                    .ok_or_else(|| Error::Unknown("err".to_string()))?
                    .to_string();

                Ok(tag_name)
            })
            .collect::<Result<Vec<Tag>, Error>>();

        tag_list
    }

    fn create_tag(&mut self, tag: &Tag, commit_hash: &CommitHash) -> Result<(), Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let object = self.repo.find_object(oid, Some(ObjectType::Commit))?;
        self.repo.tag_lightweight(tag.as_str(), &object, true)?;

        Ok(())
    }

    fn locate_tag(&self, tag: &Tag) -> Result<CommitHash, Error> {
        let reference = self.repo.find_reference(&("refs/tags/".to_owned() + tag))?;
        let object = reference.peel(ObjectType::Commit)?;
        let oid = object.id();
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;
        let commit_hash = CommitHash { hash };
        Ok(commit_hash)
    }

    fn get_tag(&self, _commit_hash: &CommitHash) -> Result<Vec<Tag>, Error> {
        unimplemented!()
    }

    fn remove_tag(&mut self, tag: &Tag) -> Result<(), Error> {
        self.repo.tag_delete(tag.as_str()).map_err(Error::from)
    }

    fn create_commit(
        &mut self,
        commit_message: &str,
        _diff: Option<&str>,
    ) -> Result<CommitHash, Error> {
        let mut index = self.repo.index().unwrap();
        let id = index.write_tree().unwrap();

        let sig = self.repo.signature().unwrap();
        let tree = self.repo.find_tree(id).unwrap();

        let head = self.get_head()?;
        let parent_oid = git2::Oid::from_bytes(&head.hash)?;
        let parent_commit = self.repo.find_commit(parent_oid)?;

        let oid = self.repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            commit_message,
            &tree,
            &[&parent_commit],
        )?;

        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })

        // TODO: Change all to make commit using "diff"
    }

    fn create_semantic_commit(&mut self, _commit: SemanticCommit) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    fn read_semantic_commit(&self, _commit_hash: &CommitHash) -> Result<SemanticCommit, Error> {
        unimplemented!()
    }

    fn run_garbage_collection(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    fn checkout_clean(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    fn checkout(&mut self, branch: &Branch) -> Result<(), Error> {
        let obj = self
            .repo
            .revparse_single(&("refs/heads/".to_owned() + branch))?;
        self.repo.checkout_tree(&obj, None)?;
        self.repo.set_head(&("refs/heads/".to_owned() + branch))?;

        Ok(())
    }

    fn checkout_detach(&mut self, commit_hash: &CommitHash) -> Result<(), Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        self.repo.set_head_detached(oid)?;

        Ok(())
    }

    fn get_head(&self) -> Result<CommitHash, Error> {
        let ref_head = self.repo.head()?;
        let oid = ref_head
            .target()
            .ok_or_else(|| Error::Unknown("err".to_string()))?;
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })
    }

    fn get_initial_commit(&self) -> Result<CommitHash, Error> {
        // Check if the repository is empty
        // TODO: Replace this with repo.empty()
        let _head = self
            .repo
            .head()
            .map_err(|_| Error::InvalidRepository("repository is empty".to_string()))?;

        // TODO: A revwalk allows traversal of the commit graph defined by including one or
        //       more leaves and excluding one or more roots.
        //       --> revwalk can make error if there exists one or more roots...
        let mut revwalk = self.repo.revwalk()?;

        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME | git2::Sort::REVERSE)?;

        let oids: Vec<Oid> = revwalk
            .by_ref()
            .collect::<Result<Vec<Oid>, git2::Error>>()?;
        println!("{:?}", oids.len());
        // TODO: What if oids[0] not exist?
        let hash = <[u8; 20]>::try_from(oids[0].as_bytes())
            .map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })
    }

    fn show_commit(&self, _commit_hash: &CommitHash) -> Result<String, Error> {
        unimplemented!()
    }

    fn list_ancestors(
        &self,
        commit_hash: &CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let mut revwalk = self.repo.revwalk()?;

        // TODO: revwalk should be tested
        revwalk.push(oid)?;
        revwalk.set_sorting(git2::Sort::TIME | git2::Sort::TOPOLOGICAL)?;

        // Compare max and ancestor's size
        let oids: Vec<Oid> = revwalk
            .by_ref()
            .collect::<Result<Vec<Oid>, git2::Error>>()?;

        let oids = oids[1..oids.len()].to_vec();

        let oids_ancestor = if let Some(num_max) = max {
            for &oid in oids.iter().take(num_max) {
                // TODO: Check first one should be commit_hash
                let commit = self.repo.find_commit(oid)?;
                let num_parents = commit.parents().len();

                if num_parents > 1 {
                    return Err(Error::InvalidRepository(format!(
                        "There exists a merge commit, {}",
                        oid
                    )));
                }
                // TODO: Should check current commit's parent == oids[next]
            }
            oids[0..num_max].to_vec()
        } else {
            // If max is None
            let mut i = 0;

            loop {
                // TODO: Check first one should be commit_hash
                let commit = self.repo.find_commit(oids[i])?;
                let num_parents = commit.parents().len();

                if num_parents > 1 {
                    return Err(Error::InvalidRepository(format!(
                        "There exists a merge commit, {}",
                        oid
                    )));
                }
                // TODO: Should check current commit's parent == oids[next]
                if num_parents == 0 {
                    break;
                }
                i += 1;
            }
            oids
        };

        let ancestors = oids_ancestor
            .iter()
            .map(|&oid| {
                let hash: [u8; 20] = oid
                    .as_bytes()
                    .try_into()
                    .map_err(|_| Error::Unknown("err".to_string()))?;
                Ok(CommitHash { hash })
            })
            .collect::<Result<Vec<CommitHash>, Error>>();

        ancestors
    }

    fn list_descendants(
        &self,
        _commit_hash: &CommitHash,
        _max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error> {
        unimplemented!()
    }

    fn list_children(&self, _commit_hash: &CommitHash) -> Result<Vec<CommitHash>, Error> {
        unimplemented!()
    }

    fn find_merge_base(
        &self,
        commit_hash1: &CommitHash,
        commit_hash2: &CommitHash,
    ) -> Result<CommitHash, Error> {
        let oid1 = Oid::from_bytes(&commit_hash1.hash)?;
        let oid2 = Oid::from_bytes(&commit_hash2.hash)?;

        let oid_merge = self.repo.merge_base(oid1, oid2)?;
        let commit_hash_merge: [u8; 20] = oid_merge
            .as_bytes()
            .try_into()
            .map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash {
            hash: commit_hash_merge,
        })
    }

    fn add_remote(&mut self, remote_name: &str, remote_url: &str) -> Result<(), Error> {
        self.repo.remote(remote_name, remote_url)?;

        Ok(())
    }

    fn remove_remote(&mut self, remote_name: &str) -> Result<(), Error> {
        self.repo.remote_delete(remote_name)?;

        Ok(())
    }

    fn fetch_all(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    fn list_remotes(&self) -> Result<Vec<(String, String)>, Error> {
        let remote_array = self.repo.remotes()?;

        let remote_name_list = remote_array
            .iter()
            .map(|remote| {
                let remote_name = remote
                    .ok_or_else(|| Error::Unknown("unable to get remote".to_string()))?
                    .to_string();

                Ok(remote_name)
            })
            .collect::<Result<Vec<String>, Error>>()?;

        let res = remote_name_list
            .iter()
            .map(|name| {
                let remote = self.repo.find_remote(name.clone().as_str())?;

                let url = remote
                    .url()
                    .ok_or_else(|| Error::Unknown("unable to get valid url".to_string()))?;

                Ok((name.clone(), url.to_string()))
            })
            .collect::<Result<Vec<(String, String)>, Error>>();

        res
    }

    fn list_remote_tracking_branches(&self) -> Result<Vec<(String, String, CommitHash)>, Error> {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct RawRepositoryImpl {
    inner: tokio::sync::Mutex<Option<RawRepositoryImplInner>>,
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

    async fn list_branches(&self) -> Result<Vec<Branch>, Error> {
        let mut lock = self.inner.lock().await;
        let inner = lock.take().expect("RawRepoImpl invariant violated");
        let (result, inner) = tokio::task::spawn_blocking(move || (inner.list_branches(), inner))
            .await
            .unwrap();
        lock.replace(inner);
        result
    }

    async fn create_branch(
        &self,
        _branch_name: &Branch,
        _commit_hash: CommitHash,
    ) -> Result<(), Error> {
        unimplemented!()
    }

    async fn locate_branch(&self, _branch: &Branch) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    async fn get_branches(&self, _commit_hash: &CommitHash) -> Result<Vec<Branch>, Error> {
        unimplemented!()
    }

    async fn move_branch(
        &mut self,
        _branch: &Branch,
        _commit_hash: &CommitHash,
    ) -> Result<(), Error> {
        unimplemented!()
    }

    async fn delete_branch(&mut self, _branch: &Branch) -> Result<(), Error> {
        unimplemented!()
    }

    async fn list_tags(&self) -> Result<Vec<Tag>, Error> {
        unimplemented!()
    }

    async fn create_tag(&mut self, _tag: &Tag, _commit_hash: &CommitHash) -> Result<(), Error> {
        unimplemented!()
    }

    async fn locate_tag(&self, _tag: &Tag) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    async fn get_tag(&self, _commit_hash: &CommitHash) -> Result<Vec<Tag>, Error> {
        unimplemented!()
    }

    async fn remove_tag(&mut self, _tag: &Tag) -> Result<(), Error> {
        unimplemented!()
    }

    async fn create_commit(
        &mut self,
        _commit_message: &str,
        _diff: Option<&str>,
    ) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    async fn create_semantic_commit(
        &mut self,
        _commit: SemanticCommit,
    ) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    async fn read_semantic_commit(
        &self,
        _commit_hash: &CommitHash,
    ) -> Result<SemanticCommit, Error> {
        unimplemented!()
    }

    async fn run_garbage_collection(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    async fn checkout_clean(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    async fn checkout(&mut self, _branch: &Branch) -> Result<(), Error> {
        unimplemented!()
    }

    async fn checkout_detach(&mut self, _commit_hash: &CommitHash) -> Result<(), Error> {
        unimplemented!()
    }

    async fn get_head(&self) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    async fn get_initial_commit(&self) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    async fn show_commit(&self, _commit_hash: &CommitHash) -> Result<String, Error> {
        unimplemented!()
    }

    async fn list_ancestors(
        &self,
        _commit_hash: &CommitHash,
        _max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error> {
        unimplemented!()
    }

    async fn list_descendants(
        &self,
        _commit_hash: &CommitHash,
        _max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error> {
        unimplemented!()
    }

    async fn list_children(&self, _commit_hash: &CommitHash) -> Result<Vec<CommitHash>, Error> {
        unimplemented!()
    }

    async fn find_merge_base(
        &self,
        _commit_hash1: &CommitHash,
        _commit_hash2: &CommitHash,
    ) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    async fn add_remote(&mut self, _remote_name: &str, _remote_url: &str) -> Result<(), Error> {
        unimplemented!()
    }

    async fn remove_remote(&mut self, _remote_name: &str) -> Result<(), Error> {
        unimplemented!()
    }

    async fn fetch_all(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    async fn list_remotes(&self) -> Result<Vec<(String, String)>, Error> {
        unimplemented!()
    }

    async fn list_remote_tracking_branches(
        &self,
    ) -> Result<Vec<(String, String, CommitHash)>, Error> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use crate::raw::Error;
    use crate::raw::{RawRepository, RawRepositoryImpl};
    use std::path::Path;
    use tempfile::TempDir;

    const MAIN: &str = "main";
    const BRANCH_A: &str = "branch_a";
    const BRANCH_B: &str = "branch_b";
    const TAG_A: &str = "tag_a";
    const TAG_B: &str = "tag_b";

    /// Make a repository which includes one initial commit at "main" branch.
    /// This returns RawRepositoryImpl containing the repository.
    async fn init_repository_with_initial_commit(path: &Path) -> Result<RawRepositoryImpl, Error> {
        let repo = RawRepositoryImpl::init(path.to_str().unwrap(), "initial", &MAIN.into())
            .await
            .unwrap();

        Ok(repo)
    }

    /// Initialize repository with empty commit and empty branch.
    #[ignore]
    #[tokio::test]
    async fn init() {
        let td = TempDir::new().unwrap();
        let path = td.path();

        let repo = RawRepositoryImpl::init(path.to_str().unwrap(), "initial", &MAIN.into())
            .await
            .unwrap();
        let branch_list = repo.list_branches().await.unwrap();
        assert_eq!(branch_list, vec![MAIN.to_owned()]);

        let repo = RawRepositoryImpl::init(path.to_str().unwrap(), "initial", &MAIN.into())
            .await
            .unwrap_err();
    }

    /// Open existed repository and verifies whether it opens well.
    #[ignore]
    #[tokio::test]
    async fn open() {
        let td = TempDir::new().unwrap();
        let path = td.path();

        let init_repo = init_repository_with_initial_commit(path).await.unwrap();
        let open_repo = RawRepositoryImpl::open(path.to_str().unwrap())
            .await
            .unwrap();

        let branch_list_init = init_repo.list_branches().await.unwrap();
        let branch_list_open = open_repo.list_branches().await.unwrap();

        assert_eq!(branch_list_init, branch_list_open);
    }

    /*
       c2 (HEAD -> main)      c2 (HEAD -> main, branch_a)     c2 (HEAD -> main)
       |                -->   |                          -->  |
       c1 (branch_a)          c1                              c1
    */
    /// Create "branch_a" at c1, create c2 at "main" branch and move "branch_a" head from c1 to c2.
    /// Finally, "branch_a" is removed.
    #[ignore]
    #[tokio::test]
    async fn branch() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut repo = init_repository_with_initial_commit(path).await.unwrap();

        // There is one branch "main" at initial state
        let branch_list = repo.list_branches().await.unwrap();
        assert_eq!(branch_list, vec![MAIN.to_owned()]);

        // git branch branch_a
        let head = repo.get_head().await.unwrap();
        repo.create_branch(&BRANCH_A.into(), head).await.unwrap();

        // "branch_list" is sorted by the name of the branches in an alphabetic order
        let branch_list = repo.list_branches().await.unwrap();
        assert_eq!(branch_list, vec![BRANCH_A.to_owned(), MAIN.to_owned()]);

        let branch_a_commit_hash = repo.locate_branch(&BRANCH_A.into()).await.unwrap();
        assert_eq!(branch_a_commit_hash, head);

        // Make second commit with "main" branch
        repo.create_commit("second", Some("")).await.unwrap();

        // Move "branch_a" head to "main" head
        let main_commit_hash = repo.locate_branch(&MAIN.into()).await.unwrap();
        repo.move_branch(&BRANCH_A.into(), &main_commit_hash)
            .await
            .unwrap();
        let branch_a_commit_hash = repo.locate_branch(&BRANCH_A.into()).await.unwrap();
        assert_eq!(main_commit_hash, branch_a_commit_hash);

        // Remove "branch_a" and the remaining branch should be only "main"
        repo.delete_branch(&BRANCH_A.into()).await.unwrap();
        let branch_list = repo.list_branches().await.unwrap();
        assert_eq!(branch_list, vec![MAIN.to_owned()]);

        // This fails since current HEAD points at "main" branch
        let remove_main = repo.delete_branch(&MAIN.into()).await.unwrap_err();
    }

    /// Create a tag and remove it.
    #[ignore]
    #[tokio::test]
    async fn tag() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut repo = init_repository_with_initial_commit(path).await.unwrap();

        // There is no tags at initial state
        let tag_list = repo.list_tags().await.unwrap();
        assert!(tag_list.is_empty());

        // Create "tag_1" at first commit
        let first_commit_hash = repo.locate_branch(&MAIN.into()).await.unwrap();
        repo.create_tag(&TAG_A.into(), &first_commit_hash)
            .await
            .unwrap();
        let tag_list = repo.list_tags().await.unwrap();
        assert_eq!(tag_list, vec![TAG_A.to_owned()]);

        let tag_a_commit_hash = repo.locate_tag(&TAG_A.into()).await.unwrap();
        assert_eq!(first_commit_hash, tag_a_commit_hash);

        // Remove "tag_1"
        repo.remove_tag(&TAG_A.into()).await.unwrap();
        let tag_list = repo.list_tags().await.unwrap();
        assert!(tag_list.is_empty());
    }

    /*
        c3 (HEAD -> main)   c3 (HEAD -> main)     c3 (main)                   c3 (HEAD -> main)
        |                   |                     |                           |
        c2 (branch_b)  -->  c2 (branch_b)  -->    c2 (HEAD -> branch_b)  -->  c2 (branch_b)
        |                   |                     |                           |
        c1 (branch_a)       c1 (HEAD -> branch_a) c1 (branch_a)               c1 (branch_a)
    */
    /// Checkout to each commits with different branches.
    #[ignore]
    #[tokio::test]
    async fn checkout() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut repo = init_repository_with_initial_commit(path).await.unwrap();

        // TODO: Should change after "create_commit" is changed
        // Create branch_a at c1 and commit c2
        let first_commit_hash = repo.locate_branch(&MAIN.into()).await.unwrap();
        repo.create_branch(&BRANCH_A.into(), first_commit_hash)
            .await
            .unwrap();
        let _commit = repo.create_commit("second", Some("")).await.unwrap();
        // Create branch_b at c2 and commit c3
        let second_commit_hash = repo.locate_branch(&MAIN.into()).await.unwrap();
        repo.create_branch(&BRANCH_B.into(), second_commit_hash)
            .await
            .unwrap();
        let _commit = repo.create_commit("third", Some("")).await.unwrap();

        let first_commit_hash = repo.locate_branch(&BRANCH_A.into()).await.unwrap();
        let second_commit_hash = repo.locate_branch(&BRANCH_B.into()).await.unwrap();
        let third_commit_hash = repo.locate_branch(&MAIN.into()).await.unwrap();

        // Checkout to branch_a, branch_b, main sequentially
        // Compare the head's commit hash after checkout with each branch's commit hash
        repo.checkout(&BRANCH_A.into()).await.unwrap();
        let head_commit_hash = repo.get_head().await.unwrap();
        assert_eq!(head_commit_hash, first_commit_hash);
        repo.checkout(&BRANCH_B.into()).await.unwrap();
        let head_commit_hash = repo.get_head().await.unwrap();
        assert_eq!(head_commit_hash, second_commit_hash);
        repo.checkout(&MAIN.into()).await.unwrap();
        let head_commit_hash = repo.get_head().await.unwrap();
        assert_eq!(head_commit_hash, third_commit_hash);
    }

    /*
        c2 (HEAD -> main)       c2 (main)
         |                 -->   |
        c1                      c1 (HEAD)
    */
    /// Checkout to commit and set "HEAD" to the detached mode.
    #[ignore]
    #[tokio::test]
    async fn checkout_detach() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut repo = init_repository_with_initial_commit(path).await.unwrap();

        // There is one branch "main" at initial state
        let branch_list = repo.list_branches().await.unwrap();
        assert_eq!(branch_list, vec![MAIN.to_owned()]);

        let first_commit_hash = repo.get_head().await.unwrap();
        // Make second commit with "main" branch
        repo.create_commit("second", Some("")).await.unwrap();

        // Checkout to c1 and set HEAD detached mode
        repo.checkout_detach(&first_commit_hash).await.unwrap();

        let cur_head_commit_hash = repo.get_head().await.unwrap();
        assert_eq!(cur_head_commit_hash, first_commit_hash);

        // TODO: Create a function of getting head name(see below).
        // This means the current head is at a detached mode,
        // otherwise this should be "refs/heads/main".
        //
        // let cur_head_name = repo.head().unwrap().name().unwrap().to_string();
        // assert_eq!(cur_head_name, "HEAD");
    }

    /*
        c3 (HEAD -> main)
        |
        c2
        |
        c1
    */
    /// Get initial commit.
    /// TODO: Currently fails due to revparse
    #[ignore]
    #[tokio::test]
    async fn initial_commit() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut repo = init_repository_with_initial_commit(path).await.unwrap();

        // Create branch_a, branch_b and commits
        let first_commit_hash = repo.locate_branch(&MAIN.into()).await.unwrap();
        repo.create_commit("second", Some("")).await.unwrap();
        repo.create_commit("third", Some("")).await.unwrap();

        let initial_commit_hash = repo.get_initial_commit().await.unwrap();
        assert_eq!(initial_commit_hash, first_commit_hash);
    }

    /*
        c3 (HEAD -> main)
        |
        c2
        |
        c1
    */
    /// Get ancestors of c3 which are [c2, c1] in the linear commit above.
    /// TODO: Currently fails due to revparse
    #[ignore]
    #[tokio::test]
    async fn ancestor() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut repo = init_repository_with_initial_commit(path).await.unwrap();

        let first_commit_hash = repo.locate_branch(&MAIN.into()).await.unwrap();
        repo.create_commit("second", Some("")).await.unwrap();
        let second_commit_hash = repo.locate_branch(&MAIN.into()).await.unwrap();
        // Make second commit at "main" branch
        let third_commit_hash = repo.locate_branch(&MAIN.into()).await.unwrap();

        // Get only one ancestor(direct parent)
        let ancestors = repo
            .list_ancestors(&third_commit_hash, Some(1))
            .await
            .unwrap();
        assert_eq!(ancestors, vec![second_commit_hash]);

        // Get two ancestors with max 2
        let ancestors = repo
            .list_ancestors(&third_commit_hash, Some(2))
            .await
            .unwrap();
        assert_eq!(ancestors, vec![second_commit_hash, first_commit_hash]);

        // Get all ancestors
        let ancestors = repo.list_ancestors(&third_commit_hash, None).await.unwrap();
        assert_eq!(ancestors, vec![second_commit_hash, first_commit_hash]);

        // TODO: If max num > the number of ancestors
    }

    /*
        c3 (HEAD -> branch_b)
         |  c2 (branch_a)
         | /
        c1 (main)
    */
    /// Make three commits at different branches and the merge base of (c2,c3) would be c1.
    #[ignore]
    #[tokio::test]
    async fn merge_base() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut repo = init_repository_with_initial_commit(path).await.unwrap();

        // Create "branch_a" and "branch_b" branches at c1
        {
            let commit_hash1 = repo.locate_branch(&MAIN.into()).await.unwrap();
            repo.create_branch(&BRANCH_A.into(), commit_hash1)
                .await
                .unwrap();
            repo.create_branch(&BRANCH_B.into(), commit_hash1)
                .await
                .unwrap();
        }
        // Make a commit at "branch_a" branch
        repo.checkout(&BRANCH_A.into()).await.unwrap();
        let _commit = repo.create_commit("branch_a", Some("")).await.unwrap();
        // Make a commit at "branch_b" branch
        repo.checkout(&BRANCH_B.into()).await.unwrap();
        let _commit = repo.create_commit("branch_b", Some("")).await.unwrap();

        // Make merge base of (c2,c3)
        let commit_hash_main = repo.locate_branch(&MAIN.into()).await.unwrap();
        let commit_hash_a = repo.locate_branch(&BRANCH_A.into()).await.unwrap();
        let commit_hash_b = repo.locate_branch(&BRANCH_B.into()).await.unwrap();
        let merge_base = repo
            .find_merge_base(&commit_hash_a, &commit_hash_b)
            .await
            .unwrap();

        // The merge base of (c2,c3) should be c1
        assert_eq!(merge_base, commit_hash_main);
    }

    /// add remote repository and remove it.
    #[ignore]
    #[tokio::test]
    async fn remote() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut repo = init_repository_with_initial_commit(path).await.unwrap();

        // Add dummy remote
        repo.add_remote("origin", "/path/to/nowhere").await.unwrap();

        let remote_list = repo.list_remotes().await.unwrap();
        assert_eq!(
            remote_list,
            vec![("origin".to_owned(), "/path/to/nowhere".to_owned())]
        );

        // Remove dummy remote
        repo.remove_remote("origin").await.unwrap();
        let remote_list = repo.list_remotes().await.unwrap();
        assert!(remote_list.is_empty());
    }
}
