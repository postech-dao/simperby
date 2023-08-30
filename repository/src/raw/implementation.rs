use super::*;

type Error = super::Error;

impl fmt::Debug for RawRepositoryInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?")
    }
}

pub(crate) struct RawRepositoryInner {
    repo: Repository,
}

/// TODO: Error handling and its messages
impl RawRepositoryInner {
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
            Err(_) => {
                let mut opts = RepositoryInitOptions::new();
                opts.initial_head(init_commit_branch);
                let repo = Repository::init_opts(directory, &opts)?;
                {
                    // Set base configs.
                    let mut config = repo.config()?;

                    // Set user configs if they are not set.
                    if config.get_string("user.name").is_err() {
                        config.set_str("user.name", "user")?;
                    }
                    if config.get_string("user.email").is_err() {
                        config.set_str("user.email", "user@simperby.net")?;
                    }

                    config.set_str("receive.advertisePushOptions", "true")?;
                    config.set_str("sendpack.sideband", "false")?;

                    // Create an initial empty commit.
                    let mut index = repo.index()?;
                    let id = index.write_tree()?;
                    let sig = repo.signature()?;
                    let tree = repo.find_tree(id)?;
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
        config.set_str("sendpack.sideband", "false")?;
        Ok(Self { repo })
    }

    pub(crate) fn retrieve_commit_hash(
        &self,
        revision_selection: String,
    ) -> Result<CommitHash, Error> {
        let object = self.repo.revparse_single(&revision_selection)?;
        let commit = object.peel_to_commit()?;
        let oid = commit.id();
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;
        Ok(CommitHash { hash })
    }

    pub(crate) fn list_branches(&self) -> Result<Vec<Branch>, Error> {
        let branches = self.repo.branches(Some(BranchType::Local))?;
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
        Ok(branches)
    }

    pub(crate) fn create_branch(
        &self,
        branch_name: Branch,
        commit_hash: CommitHash,
    ) -> Result<(), Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let commit = self.repo.find_commit(oid)?;
        self.repo.branch(&branch_name, &commit, false)?;
        Ok(())
    }

    pub(crate) fn locate_branch(&self, branch: Branch) -> Result<CommitHash, Error> {
        let branch = self
            .repo
            .find_branch(&branch, BranchType::Local)
            .map_err(|err| {
                if err.code() == git2::ErrorCode::NotFound {
                    Error::NotFound("branch not found".to_string())
                } else {
                    Error::Unknown("err".to_string())
                }
            })?;
        let oid = branch
            .get()
            .target()
            .ok_or_else(|| Error::Unknown("err".to_string()))?;
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;
        Ok(CommitHash { hash })
    }

    pub(crate) fn get_branches(&self, commit_hash: CommitHash) -> Result<Vec<Branch>, Error> {
        let oid_target = Oid::from_bytes(&commit_hash.hash)?;

        let branches = self.repo.branches(Some(BranchType::Local))?;
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
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let mut git2_branch = self.repo.find_branch(&branch, BranchType::Local)?;
        let reflog_msg = format!("branch: Reset to {}", &oid.to_string()[0..8]);
        git2_branch.get_mut().set_target(oid, &reflog_msg)?;
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
                "given branch is currently checkout branch".to_string(),
            ))
        } else {
            git2_branch.delete().map_err(Error::from)
        }
    }

    pub(crate) fn list_tags(&self) -> Result<Vec<Tag>, Error> {
        let tags = self.repo.tag_names(None)?;
        let tags = tags
            .iter()
            .map(|tag| {
                let tag_name = tag
                    .ok_or_else(|| Error::Unknown("err".to_string()))?
                    .to_string();

                Ok(tag_name)
            })
            .collect::<Result<Vec<Tag>, Error>>()?;
        Ok(tags)
    }

    pub(crate) fn create_tag(&mut self, tag: Tag, commit_hash: CommitHash) -> Result<(), Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let object = self.repo.find_object(oid, Some(ObjectType::Commit))?;
        self.repo.tag_lightweight(&tag, &object, false)?;
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
        Ok(CommitHash { hash })
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
        self.repo.tag_delete(&tag).map_err(Error::from)
    }

    pub(crate) fn create_commit(&mut self, commit: RawCommit) -> Result<CommitHash, Error> {
        // Check if there are tracked-modified or staged files except untracked files.
        let has_changes;
        {
            let statuses = self.repo.statuses(None)?;
            has_changes = statuses
                .iter()
                .any(|entry| entry.status() != Status::WT_NEW);
        }
        // Stash before creating a commit if those files exist.
        if has_changes {
            self.stash()?;
        }

        let result = {
            // The `time` specified is in seconds since the epoch, and the `offset` is the time zone offset in minutes.
            let time = git2::Time::new(commit.timestamp, -540);
            let signature = git2::Signature::new(&commit.author, &commit.email, &time)?;
            let mut index = self.repo.index()?;
            let head = self.get_head()?;
            let parent_oid = Oid::from_bytes(&head.hash)?;
            let parent_commit = self.repo.find_commit(parent_oid)?;

            // Only add files in patch to the index.
            let tree = if let Some(diff) = commit.diff {
                let diff = git2::Diff::from_buffer(diff.as_bytes())?;
                self.repo.apply(&diff, ApplyLocation::WorkDir, None)?;
                let paths = diff
                    .deltas()
                    .map(|delta| {
                        let path = delta
                            .new_file()
                            .path()
                            .ok_or_else(|| {
                                Error::Unknown("failed to get the path of diff file".to_string())
                            })?
                            .to_str()
                            .ok_or_else(|| {
                                Error::Unknown(
                                    "the path of diff file is not valid unicode".to_string(),
                                )
                            })?;
                        Ok(path)
                    })
                    .collect::<Result<Vec<&str>, Error>>()?;
                for path in paths {
                    index.add_path(std::path::Path::new(path))?;
                }
                index.write()?;
                let id = index.write_tree()?;
                self.repo.find_tree(id)?
            } else {
                parent_commit.tree()?
            };

            let oid = self.repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                &commit.message,
                &tree,
                &[&parent_commit],
            )?;
            let hash = <[u8; 20]>::try_from(oid.as_bytes())
                .map_err(|_| Error::Unknown("err".to_string()))?;
            Ok(CommitHash { hash })
        };
        // Pop stash after creating a commit.
        if has_changes {
            self.stash_pop(true)?;
        }
        result
    }

    pub(crate) fn create_commit_all(&mut self, commit: RawCommit) -> Result<CommitHash, Error> {
        // The `time` specified is in seconds since the epoch, and the `offset` is the time zone offset in minutes.
        let time = git2::Time::new(commit.timestamp, -540);
        let signature = git2::Signature::new(&commit.author, &commit.email, &time)?;

        // Add all files to the index.
        let mut index = self.repo.index()?;
        index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
        index.write()?;
        let id = index.write_tree()?;
        let tree = self.repo.find_tree(id)?;
        let head = self.get_head()?;
        let parent_oid = Oid::from_bytes(&head.hash)?;
        let parent_commit = self.repo.find_commit(parent_oid)?;

        let oid = self.repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            &commit.message,
            &tree,
            &[&parent_commit],
        )?;
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;
        Ok(CommitHash { hash })
    }

    pub(crate) fn read_commit(&self, commit_hash: CommitHash) -> Result<RawCommit, Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let commit = self.repo.find_commit(oid)?;

        let message = commit
            .message()
            .ok_or_else(|| Error::Unknown("message is not valid utf-8".to_string()))?
            .to_string();
        let diff = if commit.parent_count() == 0 {
            None
        } else {
            let diff = self.get_patch(commit_hash)?;
            if diff.is_empty() {
                None
            } else {
                Some(diff)
            }
        };
        let author = commit
            .author()
            .name()
            .ok_or_else(|| Error::Unknown("name is not valid utf-8".to_string()))?
            .to_string();
        let email = commit
            .author()
            .email()
            .ok_or_else(|| Error::Unknown("email is not valid utf-8".to_string()))?
            .to_string();
        let timestamp = commit.author().when().seconds();
        Ok(RawCommit {
            message,
            diff,
            author,
            email,
            timestamp,
        })
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
                let parent_oid = Oid::from_bytes(&head.hash)?;
                let parent_commit = self.repo.find_commit(parent_oid)?;

                let oid = self.repo.commit(
                    Some("HEAD"),
                    &sig,
                    &sig,
                    &commit_message,
                    &tree,
                    &[&parent_commit],
                )?;
                let hash = <[u8; 20]>::try_from(oid.as_bytes())
                    .map_err(|_| Error::Unknown("err".to_string()))?;
                Ok(CommitHash { hash })
            }
            Diff::Reserved(reserved_state) => {
                let path = self.get_working_directory_path()?;
                tokio::runtime::Handle::current()
                    .block_on(async move {
                        reserved_state::write_reserved_state(&format!("{path}/"), &reserved_state)
                            .await
                    })
                    .map_err(|e| Error::Unknown(e.to_string()))?;

                let mut index = self.repo.index()?;
                index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;

                let sig = self.repo.signature()?;
                let id = index.write_tree()?;
                let tree = self.repo.find_tree(id)?;
                let commit_message = format!("{}{}{}", commit.title, "\n\n", commit.body); // TODO: Check "\n" divides commit message's head and body.
                let head = self.get_head()?;
                let parent_oid = Oid::from_bytes(&head.hash)?;
                let parent_commit = self.repo.find_commit(parent_oid)?;

                let oid = self.repo.commit(
                    Some("HEAD"),
                    &sig,
                    &sig,
                    &commit_message,
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
        let oid = Oid::from_bytes(&commit_hash.hash)?;
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

        let semantic_commit = SemanticCommit {
            title,
            body,
            diff,
            author: commit
                .author()
                .name()
                .ok_or_else(|| Error::Unknown("failed to parse commit author".to_string()))?
                .to_owned(),
            timestamp: commit.author().when().seconds() * 1000,
        };
        Ok(semantic_commit)
    }

    pub(crate) fn run_garbage_collection(&mut self) -> Result<(), Error> {
        todo!()
    }

    pub(crate) fn checkout_clean(&mut self) -> Result<(), Error> {
        // Remove any changes at tracked files and revert to the last commit.
        let mut opts = git2::build::CheckoutBuilder::new();
        opts.force();
        self.repo.checkout_head(Some(&mut opts))?;
        let head = self.repo.head()?.peel_to_commit()?;
        self.repo.reset(head.as_object(), ResetType::Hard, None)?;

        // Remove untracked files and directories from the working tree.
        let workdir = self.get_working_directory_path()?;
        let mut status_opts = StatusOptions::new();
        status_opts.include_untracked(true);
        status_opts.show(StatusShow::IndexAndWorkdir);
        let statuses = self.repo.statuses(Some(&mut status_opts))?;
        for status in statuses.iter() {
            if status.status() == Status::WT_NEW {
                let path = status
                    .path()
                    .ok_or_else(|| Error::Unknown("path is not valid utf-8".to_string()))?;
                let path = format!("{workdir}{path}");

                if let Ok(metadata) = std::fs::metadata(&path) {
                    if metadata.is_dir() {
                        std::fs::remove_dir_all(&path).map_err(|_| {
                            Error::Unknown(format!("failed to remove directory '{}'", path))
                        })?;
                    } else {
                        std::fs::remove_file(&path).map_err(|_| {
                            Error::Unknown(format!("failed to remove file '{}'", path))
                        })?;
                    }
                }
            }
        }
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
        let commit = self.repo.find_commit(oid)?;
        let tree = commit.tree()?;
        self.repo.checkout_tree(tree.as_object(), None)?;
        self.repo.set_head_detached(oid)?;
        Ok(())
    }

    pub(crate) fn stash(&mut self) -> Result<(), Error> {
        let signature = self.repo.signature()?;
        self.repo
            .stash_save2(&signature, None, Some(StashFlags::DEFAULT))?;
        Ok(())
    }

    pub(crate) fn stash_pop(&mut self, index: bool) -> Result<(), Error> {
        let mut option = StashApplyOptions::new();
        let option = if index {
            Some(option.reinstantiate_index())
        } else {
            Some(&mut option)
        };
        self.repo.stash_pop(0, option).map_err(Error::from)
    }

    pub(crate) fn stash_apply(&mut self, index: bool) -> Result<(), Error> {
        let mut option = StashApplyOptions::new();
        let option = if index {
            Some(option.reinstantiate_index())
        } else {
            Some(&mut option)
        };
        self.repo.stash_apply(0, option).map_err(Error::from)
    }

    pub(crate) fn stash_drop(&mut self) -> Result<(), Error> {
        self.repo.stash_drop(0).map_err(Error::from)
    }

    pub(crate) fn check_clean(&self) -> Result<(), Error> {
        let mut options = StatusOptions::new();
        options.include_untracked(true);
        options.recurse_untracked_dirs(true);
        let statuses = self.repo.statuses(Some(&mut options))?;
        let mut has_changes = false;

        for status in statuses.iter() {
            if status.status() == Status::WT_NEW
                || status.status() == Status::WT_MODIFIED
                || status.status() == Status::WT_DELETED
                || status.status() == Status::INDEX_NEW
                || status.status() == Status::INDEX_MODIFIED
                || status.status() == Status::INDEX_DELETED
            {
                has_changes = true;
                break;
            }
        }
        if has_changes {
            return Err(Error::InvalidRepository(
                "Working tree is not clean".to_string(),
            ))?;
        }
        Ok(())
    }

    pub(crate) fn get_working_directory_path(&self) -> Result<String, Error> {
        let path = self
            .repo
            .workdir()
            .ok_or_else(|| Error::Unknown("this repository is bare repository".to_string()))?
            .to_str()
            .ok_or_else(|| {
                Error::Unknown("the path of repository is not valid unicode".to_string())
            })?
            .to_string();
        Ok(path)
    }

    pub(crate) fn get_head(&self) -> Result<CommitHash, Error> {
        let head = self.repo.head()?;
        let oid = head
            .target()
            .ok_or_else(|| Error::Unknown("err".to_string()))?;
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;
        Ok(CommitHash { hash })
    }

    pub(crate) fn get_currently_checkout_branch(&self) -> Result<Option<Branch>, Error> {
        let head = self.repo.head()?;
        let branch = head
            .shorthand()
            .ok_or_else(|| Error::Unknown("branch is not valid utf-8.".to_string()))?;
        if branch == "HEAD" {
            Ok(None)
        } else {
            Ok(Some(branch.to_string()))
        }
    }

    pub(crate) fn get_initial_commit(&self) -> Result<CommitHash, Error> {
        // Check if the repository is empty
        self.repo
            .head()
            .map_err(|_| Error::InvalidRepository("repository is empty".to_string()))?;

        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;
        let initial_oid = revwalk
            .next()
            .ok_or_else(|| Error::Unknown("failed to get revwalk".to_string()))??;
        let hash = <[u8; 20]>::try_from(initial_oid.as_bytes())
            .map_err(|_| Error::Unknown("err".to_string()))?;
        Ok(CommitHash { hash })
    }

    pub(crate) fn get_patch(&self, commit_hash: CommitHash) -> Result<String, Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let commit = self.repo.find_commit(oid)?;
        let tree = commit.tree()?;
        let parent = commit.parent(0)?;
        let parent_tree = parent.tree()?;
        let diff = self
            .repo
            .diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)?;

        let mut patch = String::new();
        diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
            match line.origin() {
                ' ' | '+' | '-' => patch.push_str(&line.origin().to_string()),
                _ => {}
            }
            let line_text = str::from_utf8(line.content()).unwrap();
            patch.push_str(line_text);
            true
        })?;
        Ok(patch)
    }

    pub(crate) fn show_commit(&self, commit_hash: CommitHash) -> Result<String, Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let commit = self.repo.find_commit(oid)?;

        if commit.parents().len() > 1 {
            //TODO: if parents > 1?
        }

        let mut emailopts = EmailCreateOptions::new();
        let email = Email::from_commit(&commit, &mut emailopts)?;
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
        let mut ancestors = vec![];
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let mut commit = self.repo.find_commit(oid)?;
        while commit.parent_count() == 1 {
            let parent = commit.parent(0)?;
            ancestors.push(parent.id());
            commit = parent;
            if let Some(max) = max {
                if ancestors.len() >= max {
                    break;
                }
            }
        }
        if commit.parent_count() > 1 {
            return Err(Error::InvalidRepository(format!(
                "there exists a merge commit, {}",
                commit_hash
            )));
        }
        let ancestors = ancestors
            .iter()
            .map(|&oid| {
                let hash: [u8; 20] = oid
                    .as_bytes()
                    .try_into()
                    .map_err(|_| Error::Unknown("err".to_string()))?;
                Ok(CommitHash { hash })
            })
            .collect::<Result<Vec<CommitHash>, Error>>()?;
        Ok(ancestors)
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
        revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;

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
        let path = self.get_working_directory_path()?;
        let reserved_state = tokio::runtime::Handle::current()
            .block_on(async move { reserved_state::read_reserved_state(&format!("{path}/")).await })
            .map_err(|e| Error::Unknown(e.to_string()))?;
        Ok(reserved_state)
    }

    pub(crate) fn read_reserved_state_at_commit(
        &self,
        commit_hash: CommitHash,
    ) -> Result<ReservedState, Error> {
        let oid = Oid::from_bytes(&commit_hash.hash)?;
        let commit = self.repo.find_commit(oid)?;
        let tree = commit.tree()?;

        let path = std::path::Path::new("reserved/genesis_info.json");
        let entry = tree.get_path(path)?;
        let blob = entry.to_object(&self.repo)?;
        let blob = blob
            .as_blob()
            .ok_or_else(|| Error::Unknown("failed to get a blob".to_string()))?;
        let content = std::str::from_utf8(blob.content())
            .map_err(|_| Error::Unknown("content of genesis_info.json is not UTF-8".to_string()))?;
        let genesis_info: GenesisInfo =
            serde_spb::from_str(content).map_err(|e| Error::Unknown(e.to_string()))?;

        let mut members: Vec<Member> = vec![];
        let path = std::path::Path::new("reserved/members");
        let entry = tree.get_path(path)?;
        let members_tree = entry.to_object(&self.repo)?.peel_to_tree()?;
        for entry in members_tree.iter() {
            let blob = entry.to_object(&self.repo)?;
            let blob = blob
                .as_blob()
                .ok_or_else(|| Error::Unknown("failed to get a blob".to_string()))?;
            let content = std::str::from_utf8(blob.content())
                .map_err(|_| Error::Unknown("content of member is not UTF-8".to_string()))?;
            let member: Member =
                serde_spb::from_str(content).map_err(|e| Error::Unknown(e.to_string()))?;
            members.push(member);
        }
        members.sort_by(|m1, m2| m1.name.cmp(&m2.name));

        let path = std::path::Path::new("reserved/consensus_leader_order.json");
        let entry = tree.get_path(path)?;
        let blob = entry.to_object(&self.repo)?;
        let blob = blob
            .as_blob()
            .ok_or_else(|| Error::Unknown("failed to get a blob".to_string()))?;
        let content = std::str::from_utf8(blob.content()).map_err(|_| {
            Error::Unknown("content of consensus_leader_order.json is not UTF-8".to_string())
        })?;
        let consensus_leader_order: Vec<MemberName> =
            serde_spb::from_str(content).map_err(|e| Error::Unknown(e.to_string()))?;

        let path = std::path::Path::new("reserved/version");
        let entry = tree.get_path(path)?;
        let blob = entry.to_object(&self.repo)?;
        let blob = blob
            .as_blob()
            .ok_or_else(|| Error::Unknown("failed to get a blob".to_string()))?;
        let content = std::str::from_utf8(blob.content())
            .map_err(|_| Error::Unknown("content of version is not UTF-8".to_string()))?;
        let version: String =
            serde_spb::from_str(content).map_err(|e| Error::Unknown(e.to_string()))?;

        Ok(ReservedState {
            genesis_info,
            members,
            consensus_leader_order,
            version,
        })
    }

    pub(crate) fn add_remote(
        &mut self,
        remote_name: String,
        remote_url: String,
    ) -> Result<(), Error> {
        self.repo.remote(&remote_name, &remote_url)?;
        Ok(())
    }

    pub(crate) fn remove_remote(&mut self, remote_name: String) -> Result<(), Error> {
        self.repo.remote_delete(&remote_name).map_err(Error::from)
    }

    pub(crate) fn fetch_all(&mut self, prune: bool) -> Result<(), Error> {
        let remotes = self.repo.remotes()?;
        let remotes = remotes
            .iter()
            .map(|remote| {
                let remote_name =
                    remote.ok_or_else(|| Error::Unknown("unable to get remote".to_string()))?;

                Ok(remote_name)
            })
            .collect::<Result<Vec<&str>, Error>>()?;

        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.prune(git2::FetchPrune::On);
        for name in remotes {
            let mut remote = self.repo.find_remote(name)?;
            if prune {
                remote.fetch(&[] as &[&str], Some(&mut fetch_options), None)?;
            } else {
                remote.fetch(&[] as &[&str], None, None)?;
            }
        }
        Ok(())
    }

    pub(crate) fn push_option(
        &self,
        remote_name: String,
        branch: Branch,
        option: Option<String>,
    ) -> Result<(), Error> {
        let workdir = self.get_working_directory_path()?;
        if let Some(option_string) = option {
            run_command(format!(
                "cd {workdir} && git push --quiet {remote_name} {branch} --push-option='{option_string}'"
            ))?
        } else {
            run_command(format!(
                "cd {workdir} && git push --quiet {remote_name} {branch}"
            ))?
        };
        Ok(())
    }

    pub(crate) fn ping_remote(&self, remote_name: String) -> Result<bool, Error> {
        let mut remote = self.repo.find_remote(remote_name.as_str())?;
        let is_open = remote.connect(git2::Direction::Fetch).ok();
        let is_open = if is_open.is_some() {
            remote.disconnect()?;
            true
        } else {
            false
        };
        Ok(is_open)
    }

    pub(crate) fn list_remotes(&self) -> Result<Vec<(String, String)>, Error> {
        let remotes = self.repo.remotes()?;
        let remotes = remotes
            .iter()
            .map(|remote| {
                let remote_name = remote
                    .ok_or_else(|| Error::Unknown("unable to get remote".to_string()))?
                    .to_string();

                Ok(remote_name)
            })
            .collect::<Result<Vec<String>, Error>>()?;

        let remotes = remotes
            .iter()
            .map(|name| {
                let remote = self.repo.find_remote(name.clone().as_str())?;

                let url = remote
                    .url()
                    .ok_or_else(|| Error::Unknown("unable to get valid url".to_string()))?;

                Ok((name.clone(), url.to_string()))
            })
            .collect::<Result<Vec<(String, String)>, Error>>()?;
        Ok(remotes)
    }

    pub(crate) fn list_remote_tracking_branches(
        &self,
    ) -> Result<Vec<(String, String, CommitHash)>, Error> {
        let branches = self.repo.branches(Some(BranchType::Remote))?;
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
        let branch = self.repo.find_branch(&name, BranchType::Remote)?;
        let oid = branch
            .get()
            .target()
            .ok_or_else(|| Error::Unknown("err".to_string()))?;
        let hash =
            <[u8; 20]>::try_from(oid.as_bytes()).map_err(|_| Error::Unknown("err".to_string()))?;
        Ok(CommitHash { hash })
    }
}
