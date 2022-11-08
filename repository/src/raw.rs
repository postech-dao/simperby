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
        branch: &Branch, //TODO: will be removed
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

pub struct CurRepository {
    repo: Repository,
}

//TODO: Error handling and its messages
impl CurRepository {
    /// Initialize the genesis repository from the genesis working tree.
    ///
    /// Fails if there is already a repository.
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
                let repo = Repository::init_opts(directory, &opts).map_err(Error::from)?;
                {
                    //create initial empty commit
                    let mut config = repo.config().map_err(Error::from)?;
                    config.set_str("user.name", "name").map_err(Error::from)?; //TODO: user.name value
                    config.set_str("user.email", "email").map_err(Error::from)?; //TODO: user.email value
                    let mut index = repo.index().map_err(Error::from)?;
                    let id = index.write_tree().map_err(Error::from)?;
                    let sig = repo.signature().map_err(Error::from)?;
                    let tree = repo.find_tree(id).map_err(Error::from)?;

                    let _oid = repo
                        .commit(Some("HEAD"), &sig, &sig, init_commit_message, &tree, &[])
                        .map_err(Error::from)?;
                }

                Ok(Self { repo })
            }
        }
    }

    /// Loads an exisitng repository.
    fn open(directory: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let repo = Repository::open(directory).map_err(Error::from)?;

        Ok(Self { repo })
    }

    // ----------------------
    // Branch-related methods
    // ----------------------

    /// Returns the list of branches.
    fn list_branches(&self) -> Result<Vec<Branch>, Error> {
        let branches = self
            .repo
            .branches(Option::Some(BranchType::Local))
            .map_err(Error::from)?;

        branches
            .map(|branch| {
                let branch_name = branch
                    .map_err(Error::from)?
                    .0
                    .name()
                    .map_err(Error::from)?
                    .map(|name| name.to_string())
                    .ok_or_else(|| Error::Unknown("err".to_string()))?;

                Ok(branch_name)
            })
            .collect::<Result<Vec<Branch>, Error>>()
    }

    /// Creates a branch on the commit.
    fn create_branch(&self, branch_name: &Branch, commit_hash: CommitHash) -> Result<(), Error> {
        let oid = Oid::from_bytes(&commit_hash.hash).map_err(Error::from)?;
        let commit = self.repo.find_commit(oid).map_err(Error::from)?;

        //if force true and branch already exists, it replaces with new one
        let _branch = self
            .repo
            .branch(branch_name.as_str(), &commit, false)
            .map_err(Error::from)?;

        Ok(())
    }

    /// Gets the commit that the branch points to.
    fn locate_branch(&self, branch: &Branch) -> Result<CommitHash, Error> {
        let branch = self
            .repo
            .find_branch(branch, BranchType::Local)
            .map_err(Error::from)?;
        let oid = branch
            .get()
            .target()
            .ok_or_else(|| Error::Unknown("err".to_string()))?;
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })
    }

    /// Gets the list of branches from the commit.
    fn get_branches(&self, _commit_hash: &CommitHash) -> Result<Vec<Branch>, Error> {
        unimplemented!()
    }

    /// Moves the branch.
    fn move_branch(&mut self, branch: &Branch, commit_hash: &CommitHash) -> Result<(), Error> {
        let mut git2_branch = self
            .repo
            .find_branch(branch, BranchType::Local)
            .map_err(Error::from)?;
        let oid = Oid::from_bytes(&commit_hash.hash).map_err(Error::from)?;
        let reflog_msg = ""; //TODO: reflog_msg
        let reference = git2_branch.get_mut();
        let _set_branch =
            git2::Reference::set_target(reference, oid, reflog_msg).map_err(Error::from)?;

        Ok(())
    }

    /// Deletes the branch.
    fn delete_branch(&mut self, branch: &Branch) -> Result<(), Error> {
        let mut git2_branch = self
            .repo
            .find_branch(branch, BranchType::Local)
            .map_err(Error::from)?;

        let current_branch = self
            .repo
            .head()
            .map_err(Error::from)?
            .shorthand()
            .ok_or_else(|| Error::Unknown("err".to_string()))?
            .to_string();

        if &current_branch == branch {
            Err(Error::InvalidRepository(
                ("Given branch is currently checkout branch").to_string(),
            ))
        } else {
            git2_branch.delete().map_err(Error::from)
        }
    }

    // -------------------
    // Tag-related methods
    // -------------------

    /// Returns the list of tags.
    fn list_tags(&self) -> Result<Vec<Tag>, Error> {
        //pattern defines what tags you want to get
        let tag_array = self.repo.tag_names(None).map_err(Error::from)?;

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

    /// Creates a tag on the given commit.
    fn create_tag(&mut self, tag: &Tag, commit_hash: &CommitHash) -> Result<(), Error> {
        let oid = Oid::from_bytes(&commit_hash.hash).map_err(Error::from)?;

        let object = self
            .repo
            .find_object(oid, Some(ObjectType::Commit))
            .map_err(Error::from)?;

        //if force true and tag already exists, it replaces with new one
        let _lightweight_tag = self
            .repo
            .tag_lightweight(tag.as_str(), &object, true)
            .map_err(Error::from)?;

        Ok(())
    }

    /// Gets the commit that the tag points to.
    fn locate_tag(&self, tag: &Tag) -> Result<CommitHash, Error> {
        let reference = self
            .repo
            .find_reference(&("refs/tags/".to_owned() + tag))
            .map_err(Error::from)?;

        let object = reference.peel(ObjectType::Commit).map_err(Error::from)?;

        let oid = object.id();
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;
        let commit_hash = CommitHash { hash };
        Ok(commit_hash)
    }

    /// Gets the tags on the given commit.
    fn get_tag(&self, _commit_hash: &CommitHash) -> Result<Vec<Tag>, Error> {
        unimplemented!()
    }

    /// Removes the tag.
    fn remove_tag(&mut self, tag: &Tag) -> Result<(), Error> {
        self.repo.tag_delete(tag.as_str()).map_err(Error::from)
    }
    // ----------------------
    // Commit-related methods
    // ----------------------

    /// Create a commit from the currently checked out branch.
    fn create_commit(
        &mut self,
        commit_message: &str,
        _diff: Option<&str>,
        branch: &Branch, //TODO: will be removed
    ) -> Result<CommitHash, Error> {
        let mut index = self.repo.index().unwrap();
        let id = index.write_tree().unwrap();

        let sig = self.repo.signature().unwrap();
        let tree = self.repo.find_tree(id).unwrap();

        let parent_commit_hash = self.locate_branch(branch).map_err(Error::from)?;
        let parent_oid = git2::Oid::from_bytes(&parent_commit_hash.hash).map_err(Error::from)?;
        let parent_commit = self.repo.find_commit(parent_oid).map_err(Error::from)?;

        let oid = self
            .repo
            .commit(
                Some("HEAD"),
                &sig,
                &sig,
                commit_message,
                &tree,
                &[&parent_commit],
            )
            .map_err(Error::from)?;

        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })

        //TODO: change all to make commit using "diff"
    }

    /// Creates a semantic commit from the currently checked out branch.
    fn create_semantic_commit(&mut self, _commit: SemanticCommit) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    /// Reads the reserved state from the current working tree.
    fn read_semantic_commit(&self, _commit_hash: &CommitHash) -> Result<SemanticCommit, Error> {
        unimplemented!()
    }

    /// Removes orphaned commits. Same as `git gc --prune=now --aggressive`
    fn run_garbage_collection(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    // ----------------------------
    // Working-tree-related methods
    // ----------------------------

    /// Checkouts and cleans the current working tree.
    /// This is same as `git checkout . && git clean -fd`.
    fn checkout_clean(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    /// Checkouts to the branch.
    fn checkout(&mut self, branch: &Branch) -> Result<(), Error> {
        let obj = self
            .repo
            .revparse_single(&("refs/heads/".to_owned() + branch))
            .map_err(Error::from)?;

        self.repo.checkout_tree(&obj, None).map_err(Error::from)?;

        self.repo
            .set_head(&("refs/heads/".to_owned() + branch))
            .map_err(Error::from)?;

        Ok(())
    }

    /// Checkouts to the commit and make `HEAD` in a detached mode.
    fn checkout_detach(&mut self, commit_hash: &CommitHash) -> Result<(), Error> {
        let oid = Oid::from_bytes(&commit_hash.hash).map_err(Error::from)?;

        self.repo.set_head_detached(oid).map_err(Error::from)?;

        Ok(())
    }

    // ---------------
    // Various queries
    // ---------------

    /// Returns the commit hash of the current HEAD.
    fn get_head(&self) -> Result<CommitHash, Error> {
        let ref_head = self.repo.head().map_err(Error::from)?;
        let oid = ref_head
            .target()
            .ok_or_else(|| Error::Unknown("err".to_string()))?;
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })
    }

    /// Returns the commit hash of the initial commit.
    ///
    /// Fails if the repository is empty.
    fn get_initial_commit(&self) -> Result<CommitHash, Error> {
        //check if the repository is empty
        let _head = self
            .repo
            .head()
            .map_err(|_| Error::InvalidRepository("Repository is empty".to_string()))?;

        //TODO: A revwalk allows traversal of the commit graph defined by including one or
        //      more leaves and excluding one or more roots.
        //      --> revwalk can make error if there exists one or more roots...
        //if not
        let mut revwalk = self.repo.revwalk()?;

        revwalk.push_head().map_err(Error::from)?;
        revwalk
            .set_sorting(git2::Sort::TIME | git2::Sort::REVERSE)
            .map_err(Error::from)?;

        let oids: Vec<Oid> = revwalk
            .by_ref()
            .collect::<Result<Vec<Oid>, git2::Error>>()
            .map_err(Error::from)?;
        println!("{:?}", oids.len());
        //TODO: what if oids[0] not exist?
        let hash = <[u8; 20]>::try_from(oids[0].as_bytes())
            .map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })
    }

    /// Returns the diff of the given commit.
    fn show_commit(&self, _commit_hash: &CommitHash) -> Result<String, Error> {
        unimplemented!()
    }

    /// Lists the ancestor commits of the given commit (The first element is the direct parent).
    ///
    /// It fails if there is a merge commit.
    /// * `max`: the maximum number of entries to be returned.
    fn list_ancestors(
        &self,
        commit_hash: &CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error> {
        let oid = Oid::from_bytes(&commit_hash.hash).map_err(Error::from)?;
        let mut revwalk = self.repo.revwalk()?;

        revwalk.push(oid).map_err(Error::from)?;
        revwalk
            .set_sorting(git2::Sort::TIME | git2::Sort::TOPOLOGICAL)
            .map_err(Error::from)?; //TODO: should be tested

        //compare max and ancestor's size
        let oids: Vec<Oid> = revwalk
            .by_ref()
            .collect::<Result<Vec<Oid>, git2::Error>>()
            .map_err(Error::from)?;

        let oids = oids[1..oids.len()].to_vec();

        let oids_ancestor = if let Some(num_max) = max {
            for &oid in oids.iter().take(num_max) {
                //TODO: Check first one should be commit_hash
                let commit = self.repo.find_commit(oid).map_err(Error::from)?;
                let num_parents = commit.parents().len();

                if num_parents > 1 {
                    return Err(Error::InvalidRepository(
                        "There exists a merge commit".to_string(),
                    ));
                }
                //TODO: should check current commit's parent == oids[next]
            }
            oids[0..num_max].to_vec()
        } else {
            //if max==None
            let mut i = 0;

            loop {
                //TODO: Check first one should be commit_hash
                let commit = self.repo.find_commit(oids[i]).map_err(Error::from)?;
                let num_parents = commit.parents().len();

                if num_parents > 1 {
                    return Err(Error::InvalidRepository(
                        "There exists a merge commit".to_string(),
                    ));
                }
                //TODO: should check current commit's parent == oids[next]
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

    /// Lists the descendant commits of the given commit (The first element is the direct child).
    ///
    /// It fails if there are diverged commits (i.e., having multiple children commit)
    /// * `max`: the maximum number of entries to be returned.
    fn list_descendants(
        &self,
        _commit_hash: &CommitHash,
        _max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error> {
        unimplemented!()
    }

    /// Returns the children commits of the given commit.
    fn list_children(&self, _commit_hash: &CommitHash) -> Result<Vec<CommitHash>, Error> {
        unimplemented!()
    }

    /// Returns the merge base of the two commits.
    fn find_merge_base(
        &self,
        commit_hash1: &CommitHash,
        commit_hash2: &CommitHash,
    ) -> Result<CommitHash, Error> {
        let oid1 = Oid::from_bytes(&commit_hash1.hash).map_err(Error::from)?;
        let oid2 = Oid::from_bytes(&commit_hash2.hash).map_err(Error::from)?;

        let oid_merge = self.repo.merge_base(oid1, oid2).map_err(Error::from)?;
        let commit_hash_merge: [u8; 20] = oid_merge
            .as_bytes()
            .try_into()
            .map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash {
            hash: commit_hash_merge,
        })
    }

    // ----------------------------
    // Remote-related methods
    // ----------------------------

    /// Adds a remote repository.
    fn add_remote(&mut self, remote_name: &str, remote_url: &str) -> Result<(), Error> {
        let _remote = self
            .repo
            .remote(remote_name, remote_url)
            .map_err(Error::from)?;

        Ok(())
    }

    /// Removes a remote repository.
    fn remove_remote(&mut self, remote_name: &str) -> Result<(), Error> {
        self.repo.remote_delete(remote_name).map_err(Error::from)?;

        Ok(())
    }

    /// Fetches the remote repository. Same as `git fetch --all -j <LARGE NUMBER>`.
    fn fetch_all(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    /// Lists all the remote repositories.
    ///
    /// Returns `(remote_name, remote_url)`.
    fn list_remotes(&self) -> Result<Vec<(String, String)>, Error> {
        let remote_array = self.repo.remotes().map_err(Error::from)?;

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
                let remote = self
                    .repo
                    .find_remote(name.clone().as_str())
                    .map_err(Error::from)?;

                let url = remote
                    .url()
                    .ok_or_else(|| Error::Unknown("unable to get valid url".to_string()))?;

                Ok((name.clone(), url.to_string()))
            })
            .collect::<Result<Vec<(String, String)>, Error>>();

        res
    }

    /// Lists all the remote tracking branches.
    ///
    /// Returns `(remote_name, remote_url, commit_hash)`
    fn list_remote_tracking_branches(&self) -> Result<Vec<(String, String, CommitHash)>, Error> {
        unimplemented!()
    }
}

pub struct RawRepositoryImpl {
    inner: tokio::sync::Mutex<Option<CurRepository>>,
}

#[async_trait]
impl RawRepository for RawRepositoryImpl {
    /// Initialize the genesis repository from the genesis working tree.
    ///
    /// Fails if there is already a repository.
    async fn init(
        directory: &str,
        init_commit_message: &str,
        init_commit_branch: &Branch,
    ) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let repo = CurRepository::init(directory, init_commit_message, init_commit_branch)
            .map_err(Error::from)?;
        let inner = tokio::sync::Mutex::new(Some(repo));

        Ok(Self { inner })
    }

    // Loads an exisitng repository.
    async fn open(directory: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let repo = CurRepository::open(directory).map_err(Error::from)?;
        let inner = tokio::sync::Mutex::new(Some(repo));

        Ok(Self { inner })
    }

    // ----------------------
    // Branch-related methods
    // ----------------------

    /// Returns the list of branches.
    async fn list_branches(&self) -> Result<Vec<Branch>, Error> {
        let mut lock = self.inner.lock().await;
        let inner = lock.take().expect("RawRepoImpl invariant violated");
        let (result, inner) = tokio::task::spawn_blocking(move || (inner.list_branches(), inner))
            .await
            .unwrap();
        lock.replace(inner);
        result
    }

    /// Creates a branch on the commit.
    async fn create_branch(
        &self,
        _branch_name: &Branch,
        _commit_hash: CommitHash,
    ) -> Result<(), Error> {
        unimplemented!()
    }

    /// Gets the commit that the branch points to.
    async fn locate_branch(&self, _branch: &Branch) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    /// Gets the list of branches from the commit.
    async fn get_branches(&self, _commit_hash: &CommitHash) -> Result<Vec<Branch>, Error> {
        unimplemented!()
    }

    /// Moves the branch.
    async fn move_branch(
        &mut self,
        _branch: &Branch,
        _commit_hash: &CommitHash,
    ) -> Result<(), Error> {
        unimplemented!()
    }

    /// Deletes the branch.
    async fn delete_branch(&mut self, _branch: &Branch) -> Result<(), Error> {
        unimplemented!()
    }

    // -------------------
    // Tag-related methods
    // -------------------

    /// Returns the list of tags.
    async fn list_tags(&self) -> Result<Vec<Tag>, Error> {
        unimplemented!()
    }

    /// Creates a tag on the given commit.
    async fn create_tag(&mut self, _tag: &Tag, _commit_hash: &CommitHash) -> Result<(), Error> {
        unimplemented!()
    }

    /// Gets the commit that the tag points to.
    async fn locate_tag(&self, _tag: &Tag) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    /// Gets the tags on the given commit.
    async fn get_tag(&self, _commit_hash: &CommitHash) -> Result<Vec<Tag>, Error> {
        unimplemented!()
    }

    /// Removes the tag.
    async fn remove_tag(&mut self, _tag: &Tag) -> Result<(), Error> {
        unimplemented!()
    }

    // ----------------------
    // Commit-related methods
    // ----------------------

    /// Creates a commit from the currently checked out branch.
    async fn create_commit(
        &mut self,
        _commit_message: &str,
        _diff: Option<&str>,
        _branch: &Branch, //TODO: will be removed
    ) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    /// Creates a semantic commit from the currently checked out branch.
    async fn create_semantic_commit(
        &mut self,
        _commit: SemanticCommit,
    ) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    /// Reads the reserved state from the current working tree.
    async fn read_semantic_commit(
        &self,
        _commit_hash: &CommitHash,
    ) -> Result<SemanticCommit, Error> {
        unimplemented!()
    }

    /// Removes orphaned commits. Same as `git gc --prune=now --aggressive`
    async fn run_garbage_collection(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    // ----------------------------
    // Working-tree-related methods
    // ----------------------------

    /// Checkouts and cleans the current working tree.
    /// This is same as `git checkout . && git clean -fd`.
    async fn checkout_clean(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    /// Checkouts to the branch.
    async fn checkout(&mut self, _branch: &Branch) -> Result<(), Error> {
        unimplemented!()
    }

    /// Checkouts to the commit and make `HEAD` in a detached mode.
    async fn checkout_detach(&mut self, _commit_hash: &CommitHash) -> Result<(), Error> {
        unimplemented!()
    }

    // ---------------
    // Various queries
    // ---------------

    /// Returns the commit hash of the current HEAD.
    async fn get_head(&self) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    /// Returns the commit hash of the initial commit.
    ///
    /// Fails if the repository is empty.
    async fn get_initial_commit(&self) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    /// Returns the diff of the given commit.
    async fn show_commit(&self, _commit_hash: &CommitHash) -> Result<String, Error> {
        unimplemented!()
    }

    /// Lists the ancestor commits of the given commit (The first element is the direct parent).
    ///
    /// It fails if there is a merge commit.
    /// * `max`: the maximum number of entries to be returned.
    async fn list_ancestors(
        &self,
        _commit_hash: &CommitHash,
        _max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error> {
        unimplemented!()
    }

    /// Lists the descendant commits of the given commit (The first element is the direct child).
    ///
    /// It fails if there are diverged commits (i.e., having multiple children commit)
    /// * `max`: the maximum number of entries to be returned.
    async fn list_descendants(
        &self,
        _commit_hash: &CommitHash,
        _max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error> {
        unimplemented!()
    }

    /// Returns the children commits of the given commit.
    async fn list_children(&self, _commit_hash: &CommitHash) -> Result<Vec<CommitHash>, Error> {
        unimplemented!()
    }

    /// Returns the merge base of the two commits.
    async fn find_merge_base(
        &self,
        _commit_hash1: &CommitHash,
        _commit_hash2: &CommitHash,
    ) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    // ----------------------------
    // Remote-related methods
    // ----------------------------

    /// Adds a remote repository.
    async fn add_remote(&mut self, _remote_name: &str, _remote_url: &str) -> Result<(), Error> {
        unimplemented!()
    }

    /// Removes a remote repository.
    async fn remove_remote(&mut self, _remote_name: &str) -> Result<(), Error> {
        unimplemented!()
    }

    /// Fetches the remote repository. Same as `git fetch --all -j <LARGE NUMBER>`.
    async fn fetch_all(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    /// Lists all the remote repositories.
    ///
    /// Returns `(remote_name, remote_url)`.
    async fn list_remotes(&self) -> Result<Vec<(String, String)>, Error> {
        unimplemented!()
    }

    /// Lists all the remote tracking branches.
    ///
    /// Returns `(remote_name, remote_url, commit_hash)`
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

    //make a repository which includes one initial commit at "main" branch
    //this returns CurRepository containing the repository
    async fn init_repository_with_initial_commit(path: &Path) -> Result<RawRepositoryImpl, Error> {
        let repo = RawRepositoryImpl::init(path.to_str().unwrap(), "initial", &("main".to_owned()))
            .await
            .unwrap();

        Ok(repo)
    }

    //initialize repository with empty commit and empty branch
    #[ignore]
    #[tokio::test]
    async fn init() {
        let td = TempDir::new().unwrap();
        let path = td.path();

        let repo = RawRepositoryImpl::init(path.to_str().unwrap(), "initial", &("main".to_owned()))
            .await
            .unwrap();
        let branch_list = repo.list_branches().await.unwrap();

        assert_eq!(branch_list.len(), 1);
        let repo =
            RawRepositoryImpl::init(path.to_str().unwrap(), "initial", &("main".to_owned())).await;
        let res = match repo {
            Ok(_) => "success".to_owned(),
            Err(_) => "init failure, already repository with same name exists".to_owned(),
        };
        assert_eq!(
            res,
            "init failure, already repository with same name exists".to_owned()
        );
    }

    //open existed repository and verifies whether it opens well
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

        assert_eq!(branch_list_init.len(), branch_list_open.len());
        assert_eq!(branch_list_init[0], branch_list_open[0]);
    }

    /*
       c2 (HEAD -> main)
        |
       c1 (branch_1)
    */
    //create "branch_1" at c1, create c2 at "main" branch, move "branch_1" head from c1 to c2
    //finally, "branch_1" is removed
    #[ignore]
    #[tokio::test]
    async fn branch() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut repo = init_repository_with_initial_commit(path).await.unwrap();

        //there is one branch "main" at initial state
        let branch_list = repo.list_branches().await.unwrap();
        assert_eq!(branch_list.len(), 1);
        assert_eq!(branch_list[0], "main".to_owned());

        //git branch branch_1
        let head = repo.get_head().await.unwrap();
        repo.create_branch(&("branch_1".to_owned()), head)
            .await
            .unwrap();

        //branch_list is sorted by branch names' alphabetic order
        let branch_list = repo.list_branches().await.unwrap();
        assert_eq!(branch_list.len(), 2);
        assert_eq!(branch_list[0], "branch_1".to_owned());
        assert_eq!(branch_list[1], "main".to_owned());

        let branch_1_commit_hash = repo.locate_branch(&("branch_1".to_owned())).await.unwrap();
        assert_eq!(branch_1_commit_hash, head);

        //make second commit with "main" branch
        let _commit = repo
            .create_commit("second", Some(""), &("main".to_owned()))
            .await
            .unwrap();

        //move "branch_1" head to "main" head
        let main_commit_hash = repo.locate_branch(&("main".to_owned())).await.unwrap();
        repo.move_branch(&("branch_1".to_owned()), &main_commit_hash)
            .await
            .unwrap();
        let branch_1_commit_hash = repo.locate_branch(&("branch_1".to_owned())).await.unwrap();
        assert_eq!(main_commit_hash, branch_1_commit_hash);

        //remove "branch_1" and the remaining branch should be only "main"
        repo.delete_branch(&("branch_1".to_owned())).await.unwrap();
        let branch_list = repo.list_branches().await.unwrap();
        assert_eq!(branch_list.len(), 1);
        assert_eq!(branch_list[0], "main".to_owned());

        //TODO: match
        let remove_main = repo.delete_branch(&("main".to_owned())).await;
        let res = match remove_main {
            Ok(_) => "success".to_owned(),
            Err(_) => "failure".to_owned(),
        };
        assert_eq!(res, "failure".to_owned());
    }

    //create a tag and remove it
    #[ignore]
    #[tokio::test]
    async fn tag() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut repo = init_repository_with_initial_commit(path).await.unwrap();

        //there is no tags at initial state
        let tag_list = repo.list_tags().await.unwrap();
        assert_eq!(tag_list.len(), 0);

        //create "tag_1" at first commit
        let first_commit_hash = repo.locate_branch(&("main".to_owned())).await.unwrap();
        repo.create_tag(&("tag_1".to_owned()), &first_commit_hash)
            .await
            .unwrap();
        let tag_list = repo.list_tags().await.unwrap();
        assert_eq!(tag_list.len(), 1);
        assert_eq!(tag_list[0], "tag_1".to_owned());

        let tag_1_commit_hash = repo.locate_tag(&("tag_1".to_owned())).await.unwrap();
        assert_eq!(first_commit_hash, tag_1_commit_hash);

        //remove "tag_1"
        repo.remove_tag(&("tag_1".to_owned())).await.unwrap();
        let tag_list = repo.list_tags().await.unwrap();
        assert_eq!(tag_list.len(), 0);
    }

    /*
        c3 (HEAD -> main)   c3 (HEAD -> main)     c3 (main)                   c3 (HEAD -> main)
        |
        c2 (branch_2)  -->  c2 (branch_2)  -->    c2 (HEAD -> branch_2)  -->  c2 (branch_2)
        |
        c1 (branch_1)       c1 (HEAD -> branch_1) c1 (branch_1)               c1 (branch_1)
    */
    //checkout to each commits with different branches
    #[ignore]
    #[tokio::test]
    async fn checkout() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut repo = init_repository_with_initial_commit(path).await.unwrap();

        //TODO: should change after "create_commit" is changed
        //create branch_1 at c1 and commit c2
        let first_commit_hash = repo.locate_branch(&("main".to_owned())).await.unwrap();
        repo.create_branch(&("branch_1".to_owned()), first_commit_hash)
            .await
            .unwrap();
        let _commit = repo
            .create_commit("second", Some(""), &("branch_1".to_owned()))
            .await
            .unwrap();
        //create branch_2 at c2 and commit c3
        let second_commit_hash = repo.locate_branch(&("main".to_owned())).await.unwrap();
        repo.create_branch(&("branch_2".to_owned()), second_commit_hash)
            .await
            .unwrap();
        let _commit = repo
            .create_commit("third", Some(""), &("branch_2".to_owned()))
            .await
            .unwrap();

        let first_commit_hash = repo.locate_branch(&("branch_1".to_owned())).await.unwrap();
        let second_commit_hash = repo.locate_branch(&("branch_2".to_owned())).await.unwrap();
        let third_commit_hash = repo.locate_branch(&("main".to_owned())).await.unwrap();

        //checkout to branch_1, branch_2, main sequentially
        //compare the head's commit hash after checkout with each branch's commit hash
        repo.checkout(&("branch_1".to_owned())).await.unwrap();
        let head_commit_hash = repo.get_head().await.unwrap();
        assert_eq!(head_commit_hash, first_commit_hash);
        repo.checkout(&("branch_2".to_owned())).await.unwrap();
        let head_commit_hash = repo.get_head().await.unwrap();
        assert_eq!(head_commit_hash, second_commit_hash);
        repo.checkout(&("main".to_owned())).await.unwrap();
        let head_commit_hash = repo.get_head().await.unwrap();
        assert_eq!(head_commit_hash, third_commit_hash);
    }

    /*
        c2 (HEAD -> main)       c2 (main)
         |                 -->   |
        c1                      c1 (HEAD)
    */
    //checkout to commit and set "HEAD" to the detached mode
    #[ignore]
    #[tokio::test]
    async fn checkout_detach() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut repo = init_repository_with_initial_commit(path).await.unwrap();

        //there is one branch "main" at initial state
        let branch_list = repo.list_branches().await.unwrap();
        assert_eq!(branch_list.len(), 1);
        assert_eq!(branch_list[0], "main".to_owned());

        let commit1 = repo.get_head().await.unwrap();
        //make second commit with "main" branch
        let _commit = repo
            .create_commit("second", Some(""), &("main".to_owned()))
            .await
            .unwrap();

        //checkout to commit1 and set HEAD detached mode
        repo.checkout_detach(&commit1).await.unwrap();

        let cur_head_commit_hash = repo.get_head().await.unwrap();
        assert_eq!(cur_head_commit_hash, commit1);

        //TODO: create a function of getting head name(see below)
        //this means the current head is at a detached mode,
        //otherwise this should be "refs/heads/main"
        //let cur_head_name = repo.head().unwrap().name().unwrap().to_string();
        //assert_eq!(cur_head_name, "HEAD");
    }

    /*
        c3 (HEAD -> main)
        |
        c2
        |
        c1
    */
    //get initial commit
    //TODO: currently fails due to revparse
    #[ignore]
    #[tokio::test]
    async fn initial_commit() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut repo = init_repository_with_initial_commit(path).await.unwrap();

        //create branch_1, branch_2 and commits
        let first_commit_hash = repo.locate_branch(&("main".to_owned())).await.unwrap();
        println!("{:?}", first_commit_hash);
        let _commit = repo
            .create_commit("second", Some(""), &("main".to_owned()))
            .await
            .unwrap();
        println!("{:?}", _commit);
        let _commit = repo
            .create_commit("third", Some(""), &("main".to_owned()))
            .await
            .unwrap();
        println!("{:?}", _commit);

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
    //get ancestors of c3 which are [c2, c1] in the linear commit above
    //TODO: currently fails due to revparse
    #[ignore]
    #[tokio::test]
    async fn ancestor() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut repo = init_repository_with_initial_commit(path).await.unwrap();

        let first_commit_hash = repo.locate_branch(&("main".to_owned())).await.unwrap();
        let _commit = repo
            .create_commit("second", Some(""), &("main".to_owned()))
            .await
            .unwrap();
        let second_commit_hash = repo.locate_branch(&("main".to_owned())).await.unwrap();
        //make second commit at "main" branch
        let third_commit_hash = repo.locate_branch(&("main".to_owned())).await.unwrap();

        //get only one ancestor(direct parent)
        let ancestors = repo
            .list_ancestors(&third_commit_hash, Some(1))
            .await
            .unwrap();
        assert_eq!(ancestors.len(), 1);
        assert_eq!(ancestors[0], second_commit_hash);

        //get two ancestors with max 2
        let ancestors = repo
            .list_ancestors(&third_commit_hash, Some(2))
            .await
            .unwrap();
        assert_eq!(ancestors.len(), 2);
        assert_eq!(ancestors[0], second_commit_hash);
        assert_eq!(ancestors[1], first_commit_hash);

        //get all ancestors
        let ancestors = repo.list_ancestors(&third_commit_hash, None).await.unwrap();
        assert_eq!(ancestors.len(), 2);
        assert_eq!(ancestors[0], second_commit_hash);
        assert_eq!(ancestors[1], first_commit_hash);

        //TODO: if max num > the number of ancestors
    }

    /*
        c3 (HEAD -> branch_b)
         |  c2 (branch_a)
         | /
        c1 (main)
    */
    //make three commits at different branches and the merge base of (c2,c3) would be c1
    #[ignore]
    #[tokio::test]
    async fn merge_base() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut repo = init_repository_with_initial_commit(path).await.unwrap();

        //create "branch_a" and "branch_b" branches at c1
        {
            let commit_hash1 = repo.locate_branch(&("main".to_owned())).await.unwrap();
            repo.create_branch(&("branch_a".to_owned()), commit_hash1)
                .await
                .unwrap();
            repo.create_branch(&("branch_b".to_owned()), commit_hash1)
                .await
                .unwrap();
        }
        //make a commit at "branch_a" branch
        repo.checkout(&("branch_a".to_owned())).await.unwrap();
        let _commit = repo
            .create_commit("branch_a", Some(""), &("branch_a".to_owned()))
            .await
            .unwrap();
        //make a commit at "branch_b" branch
        repo.checkout(&("branch_b".to_owned())).await.unwrap();
        let _commit = repo
            .create_commit("branch_b", Some(""), &("branch_b".to_owned()))
            .await
            .unwrap();

        //make merge base of (c2,c3)
        let commit_hash1 = repo.locate_branch(&("main".to_owned())).await.unwrap();
        let commit_hash_a = repo.locate_branch(&("branch_a".to_owned())).await.unwrap();
        let commit_hash_b = repo.locate_branch(&("branch_b".to_owned())).await.unwrap();
        let merge_base = repo
            .find_merge_base(&commit_hash_a, &commit_hash_b)
            .await
            .unwrap();

        //the merge base of (c2,c3) should be c1
        assert_eq!(merge_base, commit_hash1);
    }

    //add remote repository and remove it
    #[ignore]
    #[tokio::test]
    async fn remote() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut repo = init_repository_with_initial_commit(path).await.unwrap();

        //add dummy remote
        repo.add_remote("origin", "/path/to/nowhere").await.unwrap();

        let remote_list = repo.list_remotes().await.unwrap();
        assert_eq!(remote_list.len(), 1);
        assert_eq!(remote_list[0].0, "origin".to_owned());
        assert_eq!(remote_list[0].1, "/path/to/nowhere".to_owned());

        //remove dummy remote
        repo.remove_remote("origin").await.unwrap();
        let remote_list = repo.list_remotes().await.unwrap();
        assert_eq!(remote_list.len(), 0);
    }
}
