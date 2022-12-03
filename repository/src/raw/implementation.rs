use super::*;

type Error = super::Error;

impl fmt::Debug for RawRepositoryImplInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?")
    }
}

pub(crate) struct RawRepositoryImplInner {
    repo: Repository,
}

/// TODO: Error handling and its messages
impl RawRepositoryImplInner {
    pub(crate) fn init(
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

    pub(crate) fn open(directory: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let repo = Repository::open(directory)?;

        Ok(Self { repo })
    }

    pub(crate) fn list_branches(&self) -> Result<Vec<Branch>, Error> {
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

    pub(crate) fn create_branch(
        &self,
        branch_name: Branch,
        commit_hash: CommitHash,
    ) -> Result<(), Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let commit = self.repo.find_commit(oid)?;

        // TODO: Test if force true and verify new branch is created
        self.repo.branch(branch_name.as_str(), &commit, false)?;

        Ok(())
    }

    pub(crate) fn locate_branch(&self, branch: Branch) -> Result<CommitHash, Error> {
        let branch = self.repo.find_branch(&branch, BranchType::Local)?;
        let oid = branch
            .get()
            .target()
            .ok_or_else(|| Error::Unknown("err".to_string()))?;
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })
    }

    pub(crate) fn get_branches(&self, _commit_hash: CommitHash) -> Result<Vec<Branch>, Error> {
        unimplemented!()
    }

    pub(crate) fn move_branch(
        &mut self,
        branch: Branch,
        commit_hash: CommitHash,
    ) -> Result<(), Error> {
        let mut git2_branch = self.repo.find_branch(&branch, BranchType::Local)?;
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let reflog_msg = ""; // TODO: reflog_msg
        let reference = git2_branch.get_mut();
        let _set_branch = git2::Reference::set_target(reference, oid, reflog_msg)?;

        Ok(())
    }

    pub(crate) fn delete_branch(&mut self, branch: Branch) -> Result<(), Error> {
        let mut git2_branch = self.repo.find_branch(&branch, BranchType::Local)?;

        let current_branch = self
            .repo
            .head()?
            .shorthand()
            .ok_or_else(|| Error::Unknown("err".to_string()))?
            .to_string();

        if current_branch == branch {
            Err(Error::InvalidRepository(
                ("given branch is currently checkout branch").to_string(),
            ))
        } else {
            git2_branch.delete().map_err(Error::from)
        }
    }

    pub(crate) fn list_tags(&self) -> Result<Vec<Tag>, Error> {
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

    pub(crate) fn create_tag(&mut self, tag: Tag, commit_hash: CommitHash) -> Result<(), Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let object = self.repo.find_object(oid, Some(ObjectType::Commit))?;
        self.repo.tag_lightweight(tag.as_str(), &object, true)?;

        Ok(())
    }

    pub(crate) fn locate_tag(&self, tag: Tag) -> Result<CommitHash, Error> {
        let reference = self
            .repo
            .find_reference(&("refs/tags/".to_owned() + &tag))?;
        let object = reference.peel(ObjectType::Commit)?;
        let oid = object.id();
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;
        let commit_hash = CommitHash { hash };
        Ok(commit_hash)
    }

    pub(crate) fn get_tag(&self, _commit_hash: CommitHash) -> Result<Vec<Tag>, Error> {
        unimplemented!()
    }

    pub(crate) fn remove_tag(&mut self, tag: Tag) -> Result<(), Error> {
        self.repo.tag_delete(tag.as_str()).map_err(Error::from)
    }

    pub(crate) fn create_commit(
        &mut self,
        commit_message: String,
        _diff: Option<String>,
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
            commit_message.as_str(),
            &tree,
            &[&parent_commit],
        )?;

        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })

        // TODO: Change all to make commit using "diff"
    }

    pub(crate) fn create_semantic_commit(
        &mut self,
        _commit: SemanticCommit,
    ) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    pub(crate) fn read_semantic_commit(
        &self,
        _commit_hash: CommitHash,
    ) -> Result<SemanticCommit, Error> {
        unimplemented!()
    }

    pub(crate) fn run_garbage_collection(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    pub(crate) fn checkout_clean(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    pub(crate) fn checkout(&mut self, branch: Branch) -> Result<(), Error> {
        let obj = self
            .repo
            .revparse_single(&("refs/heads/".to_owned() + &branch))?;
        self.repo.checkout_tree(&obj, None)?;
        self.repo.set_head(&("refs/heads/".to_owned() + &branch))?;

        Ok(())
    }

    pub(crate) fn checkout_detach(&mut self, commit_hash: CommitHash) -> Result<(), Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        self.repo.set_head_detached(oid)?;

        Ok(())
    }

    pub(crate) fn get_head(&self) -> Result<CommitHash, Error> {
        let ref_head = self.repo.head()?;
        let oid = ref_head
            .target()
            .ok_or_else(|| Error::Unknown("err".to_string()))?;
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })
    }

    pub(crate) fn get_initial_commit(&self) -> Result<CommitHash, Error> {
        // Check if the repository is empty
        // TODO: Replace this with repo.empty()
        let _head = self
            .repo
            .head()
            .map_err(|_| Error::InvalidRepository("repository is empty".to_string()))?;

        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME)?;

        let oids: Vec<Oid> = revwalk
            .by_ref()
            .collect::<Result<Vec<Oid>, git2::Error>>()?;

        let initial_oid = if oids.len() == 1 { oids[0] } else { oids[1] };
        let hash = <[u8; 20]>::try_from(initial_oid.as_bytes())
            .map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })
    }

    pub(crate) fn show_commit(&self, _commit_hash: CommitHash) -> Result<String, Error> {
        unimplemented!()
    }

    pub(crate) fn list_ancestors(
        &self,
        commit_hash: CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push(oid)?;
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL)?;

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

    pub(crate) fn query_commit_path(
        &self,
        _ancestor: CommitHash,
        _descendant: CommitHash,
    ) -> Result<Vec<CommitHash>, Error> {
        todo!()
    }

    pub(crate) fn list_children(&self, _commit_hash: CommitHash) -> Result<Vec<CommitHash>, Error> {
        unimplemented!()
    }

    pub(crate) fn find_merge_base(
        &self,
        commit_hash1: CommitHash,
        commit_hash2: CommitHash,
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

    pub(crate) fn read_reserved_state(&self) -> Result<ReservedState, Error> {
        unimplemented!()
    }

    pub(crate) fn add_remote(
        &mut self,
        remote_name: String,
        remote_url: String,
    ) -> Result<(), Error> {
        self.repo
            .remote(remote_name.as_str(), remote_url.as_str())?;

        Ok(())
    }

    pub(crate) fn remove_remote(&mut self, remote_name: String) -> Result<(), Error> {
        self.repo.remote_delete(remote_name.as_str())?;

        Ok(())
    }

    pub(crate) fn fetch_all(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    pub(crate) fn list_remotes(&self) -> Result<Vec<(String, String)>, Error> {
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

    pub(crate) fn list_remote_tracking_branches(
        &self,
    ) -> Result<Vec<(String, String, CommitHash)>, Error> {
        unimplemented!()
    }
}
