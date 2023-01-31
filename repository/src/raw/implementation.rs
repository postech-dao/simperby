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
                    // Set base configs.
                    let mut config = repo.config()?;
                    config.set_str("user.name", "name")?; // TODO: user.name value
                    config.set_str("user.email", "email")?; // TODO: user.email value
                    config.set_str("receive.advertisePushOptions", "true")?;

                    // Create an initial empty commit.
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

    pub(crate) fn clone(directory: &str, url: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let repo = Repository::clone(url, directory)?;
        let mut config = repo.config()?;
        config.set_str("receive.advertisePushOptions", "true")?;

        Ok(Self { repo })
    }

    pub(crate) fn retrieve_commit_hash(
        &self,
        revision_selection: String,
    ) -> Result<CommitHash, Error> {
        let object = self.repo.revparse_single(revision_selection.as_str())?;
        let commit = object.peel_to_commit()?;
        let oid = commit.id();
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })
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

    pub(crate) fn get_branches(&self, commit_hash: CommitHash) -> Result<Vec<Branch>, Error> {
        let oid_target = git2::Oid::from_bytes(&commit_hash.hash)?;

        let branches = self.repo.branches(Option::Some(BranchType::Local))?;
        let branches = branches.into_iter().collect::<Result<Vec<_>, _>>()?;
        let branches = branches
            .into_iter()
            .map(|(branch, _)| {
                let oid = branch.get().target();
                match oid {
                    Some(oid) => Ok((branch, oid)),
                    None => Err(Error::Unknown("err".to_string())),
                }
            })
            .collect::<Result<Vec<(git2::Branch, Oid)>, Error>>()?;

        let branches = branches
            .into_iter()
            .filter(|(_, oid)| *oid == oid_target)
            .map(|(branch, _)| {
                branch
                    .name()?
                    .map(|name| name.to_string())
                    .ok_or_else(|| Error::Unknown("err".to_string()))
            })
            .collect::<Result<Vec<Branch>, Error>>()?;

        Ok(branches)
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

    pub(crate) fn get_tag(&self, commit_hash: CommitHash) -> Result<Vec<Tag>, Error> {
        let oid_target = Oid::from_bytes(&commit_hash.hash)?;

        let references = self.repo.references_glob("refs/tags/*")?;
        let references = references.into_iter().collect::<Result<Vec<_>, _>>()?;
        let references = references
            .into_iter()
            .map(|reference| {
                let oid = reference
                    .target()
                    .ok_or_else(|| Error::Unknown("err".to_string()))?;

                Ok((reference, oid))
            })
            .collect::<Result<Vec<(git2::Reference, Oid)>, Error>>()?;

        let tags = references
            .into_iter()
            .filter(|(_, oid)| *oid == oid_target)
            .map(|(reference, _)| {
                let tag = reference
                    .shorthand()
                    .ok_or_else(|| Error::Unknown("err".to_string()))?
                    .to_string();

                Ok(tag)
            })
            .collect::<Result<Vec<Tag>, Error>>()?;

        Ok(tags)
    }

    pub(crate) fn remove_tag(&mut self, tag: Tag) -> Result<(), Error> {
        self.repo.tag_delete(tag.as_str()).map_err(Error::from)
    }

    pub(crate) fn create_commit(
        &mut self,
        commit_message: String,
        _diff: Option<String>,
    ) -> Result<CommitHash, Error> {
        let workdir = self.repo.workdir().unwrap().to_str().unwrap();
        let p = format!("{}/{}", workdir, "test");
        fs::File::create(p.as_str())
            .map_err(|_| Error::Unknown("full directory path does not exist".to_string()))?;
        fs::write(p.as_str(), "test")
            .map_err(|_| Error::Unknown("full directory path does not exist".to_string()))?;

        let sig = self.repo.signature()?;
        let mut index = self.repo.index()?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        let id = index.write_tree()?;
        let tree = self.repo.find_tree(id)?;
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

        let obj = self.repo.find_object(oid, None)?;
        self.repo.reset(&obj, git2::ResetType::Hard, None)?;

        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;

        Ok(CommitHash { hash })
    }

    pub(crate) fn create_semantic_commit(
        &mut self,
        commit: SemanticCommit,
    ) -> Result<CommitHash, Error> {
        match commit.diff {
            Diff::None => {
                let sig = self.repo.signature()?;
                let mut index = self.repo.index()?;
                let id = index.write_tree()?;
                let tree = self.repo.find_tree(id)?;
                let commit_message = format!("{}{}{}", commit.title, "\n\n", commit.body); // TODO: Check "\n" divides commit message's head and body.
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

                let hash = <[u8; 20]>::try_from(oid.as_bytes())
                    .map_err(|_| Error::Unknown("err".to_string()))?;

                Ok(CommitHash { hash })
            }
            Diff::Reserved(reserved_state) => {
                let path = self.repo.workdir().unwrap().to_str().unwrap();
                tokio::runtime::Handle::current()
                    .block_on(async move {
                        reserved_state::write_reserved_state(&format!("{path}/"), &reserved_state)
                            .await
                    })
                    .map_err(|e| Error::Unknown(e.to_string()))?;

                let mut index = self.repo.index()?;
                index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;

                let sig = self.repo.signature()?;
                let id = index.write_tree()?;
                let tree = self.repo.find_tree(id)?;
                let commit_message = format!("{}{}{}", commit.title, "\n\n", commit.body); // TODO: Check "\n" divides commit message's head and body.
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

                let hash = <[u8; 20]>::try_from(oid.as_bytes())
                    .map_err(|_| Error::Unknown("err".to_string()))?;

                Ok(CommitHash { hash })
            }
            Diff::General(_, _) => Err(Error::InvalidRepository(
                "diff is Diff::General()".to_string(),
            )),
            Diff::NonReserved(_) => Err(Error::InvalidRepository(
                "diff is Diff::NonReserved()".to_string(),
            )),
        }
    }

    pub(crate) fn read_semantic_commit(
        &self,
        commit_hash: CommitHash,
    ) -> Result<SemanticCommit, Error> {
        let oid = git2::Oid::from_bytes(&commit_hash.hash)?;
        let commit = self.repo.find_commit(oid)?;
        let tree = commit.tree()?;
        let parent_tree = commit.parent(0)?.tree()?;

        // Create diff by verifying the commit made files or not.
        let diff = self
            .repo
            .diff_tree_to_tree(Some(&tree), Some(&parent_tree), None)?;

        let diff = if diff.deltas().len() == 0 {
            Diff::None
        } else {
            let patch = self.show_commit(commit_hash)?;
            let hash = patch.to_hash256();
            Diff::NonReserved(hash)
        };
        /* TODO: If reserved state
        let reserved_state = self.read_reserved_state()?;
        Diff::Reserved(Box::new(reserved_state))*/

        let title = commit.summary();
        let title = if let Some(msg_title) = title {
            msg_title
        } else {
            ""
        }
        .to_string();
        let body = commit.body();
        let body = if let Some(msg_body) = body {
            msg_body
        } else {
            ""
        }
        .to_string();

        let semantic_commit = SemanticCommit { title, body, diff };

        Ok(semantic_commit)
    }

    pub(crate) fn run_garbage_collection(&mut self) -> Result<(), Error> {
        todo!()
    }

    pub(crate) fn checkout_clean(&mut self) -> Result<(), Error> {
        Ok(())
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

    pub(crate) fn show_commit(&self, commit_hash: CommitHash) -> Result<String, Error> {
        let oid = git2::Oid::from_bytes(&commit_hash.hash)?;
        let commit = self.repo.find_commit(oid)?;

        if commit.parents().len() > 1 {
            //TODO: if parents > 1?
        }

        let mut emailopts = git2::EmailCreateOptions::new();
        let email = git2::Email::from_commit(&commit, &mut emailopts)?;
        let email = str::from_utf8(email.as_slice())
            .map_err(|_| Error::Unknown("err".to_string()))?
            .to_string();

        Ok(email)
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
                        "There exists a merge commit, {oid}"
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
                        "There exists a merge commit, {oid}"
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
        ancestor: CommitHash,
        descendant: CommitHash,
    ) -> Result<Vec<CommitHash>, Error> {
        if ancestor == descendant {
            return Ok(vec![]);
        }

        let merge_base = self.find_merge_base(ancestor, descendant)?;
        if merge_base != ancestor {
            return Err(Error::InvalidRepository(
                "ancestor is not the merge base of two commits".to_string(),
            ));
        }

        let descendant_oid = Oid::from_bytes(&descendant.hash)?;
        let ancestor_oid = Oid::from_bytes(&ancestor.hash)?;

        let mut revwalk = self.repo.revwalk()?;
        let range = format!("{}{}{}", ancestor_oid, "..", descendant_oid);
        revwalk.push_range(range.as_str())?;
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)?;

        let oids: Vec<Oid> = revwalk
            .by_ref()
            .collect::<Result<Vec<Oid>, git2::Error>>()?;

        let commits = oids
            .iter()
            .map(|&oid| {
                let hash: [u8; 20] = oid
                    .as_bytes()
                    .try_into()
                    .map_err(|_| Error::Unknown("err".to_string()))?;
                Ok(CommitHash { hash })
            })
            .collect::<Result<Vec<CommitHash>, Error>>()?;

        Ok(commits)
    }

    pub(crate) fn list_children(&self, _commit_hash: CommitHash) -> Result<Vec<CommitHash>, Error> {
        todo!()
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
        let path = self.repo.workdir().unwrap().to_str().unwrap();
        let reserved_state = tokio::runtime::Handle::current()
            .block_on(async move { reserved_state::read_reserved_state(&format!("{path}/")).await })
            .map_err(|e| Error::Unknown(e.to_string()))?;

        Ok(reserved_state)
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
        let remote_list = self.repo.remotes()?;
        let remote_list = remote_list
            .iter()
            .map(|remote| {
                let remote_name =
                    remote.ok_or_else(|| Error::Unknown("unable to get remote".to_string()))?;

                Ok(remote_name)
            })
            .collect::<Result<Vec<&str>, Error>>()?;

        for name in remote_list {
            let mut remote = self.repo.find_remote(name)?;
            remote.fetch(&[] as &[&str], None, None)?;
        }

        Ok(())
    }

    pub(crate) fn push_option(
        &self,
        remote_name: String,
        branch: Branch,
        option: Option<String>,
    ) -> Result<(), Error> {
        let workdir = self.repo.workdir().unwrap().to_str().unwrap();

        if let Some(option_string) = option {
            run_command(format!(
                "cd {workdir} && git push {remote_name} {branch} --push-option='{option_string}'"
            ))?
        } else {
            run_command(format!("cd {workdir} && git push {remote_name} {branch}"))?
        };

        Ok(())
    }

    pub(crate) fn list_remotes(&self) -> Result<Vec<(String, String)>, Error> {
        let remote_array = self.repo.remotes()?;

        let remote_list = remote_array
            .iter()
            .map(|remote| {
                let remote_name = remote
                    .ok_or_else(|| Error::Unknown("unable to get remote".to_string()))?
                    .to_string();

                Ok(remote_name)
            })
            .collect::<Result<Vec<String>, Error>>()?;

        let remote_list = remote_list
            .iter()
            .map(|name| {
                let remote = self.repo.find_remote(name.clone().as_str())?;

                let url = remote
                    .url()
                    .ok_or_else(|| Error::Unknown("unable to get valid url".to_string()))?;

                Ok((name.clone(), url.to_string()))
            })
            .collect::<Result<Vec<(String, String)>, Error>>()?;

        Ok(remote_list)
    }

    pub(crate) fn list_remote_tracking_branches(
        &self,
    ) -> Result<Vec<(String, String, CommitHash)>, Error> {
        let branches = self.repo.branches(Some(git2::BranchType::Remote))?;
        let branches = branches
            .map(|branch| {
                let branch_name = branch?
                    .0
                    .name()?
                    .map(|name| name.to_string())
                    .ok_or_else(|| Error::Unknown("err".to_string()))?;

                Ok(branch_name)
            })
            .collect::<Result<Vec<Branch>, Error>>()?;

        let branches = branches
            .iter()
            .map(|branch| {
                let names: Vec<&str> = branch.split('/').collect();
                let remote_name = names[0];
                let branch_name = names[1];
                let branch = self.repo.find_branch(branch, BranchType::Remote)?;

                let oid = branch
                    .get()
                    .target()
                    .ok_or_else(|| Error::Unknown("err".to_string()))?;
                let hash = <[u8; 20]>::try_from(oid.as_bytes())
                    .map_err(|_| Error::Unknown("err".to_string()))?;
                let commit_hash = CommitHash { hash };

                Ok((
                    remote_name.to_string(),
                    branch_name.to_string(),
                    commit_hash,
                ))
            })
            .collect::<Result<Vec<(String, String, CommitHash)>, Error>>()?;

        Ok(branches)
    }

    pub(crate) fn locate_remote_tracking_branch(
        &self,
        remote_name: String,
        branch_name: String,
    ) -> Result<CommitHash, Error> {
        let name = format!("{remote_name}/{branch_name}");
        let branch = self.repo.find_branch(name.as_str(), BranchType::Remote)?;

        let oid = branch
            .get()
            .target()
            .ok_or_else(|| Error::Unknown("err".to_string()))?;
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;
        let commit_hash = CommitHash { hash };

        Ok(commit_hash)
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
