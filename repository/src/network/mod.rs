use super::*;
use raw::RawCommit;
use simperby_network::Error;
use simperby_network::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TipCommit {
    Block(BlockHeader),
    Agenda(Agenda),
    AgendaProof(AgendaProof),
}

impl TipCommit {
    pub fn into_commit(self) -> Commit {
        match self {
            TipCommit::Block(x) => Commit::Block(x),
            TipCommit::Agenda(x) => Commit::Agenda(x),
            TipCommit::AgendaProof(x) => Commit::AgendaProof(x),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadBranch {
    /// Starts from the very next commit from the last finalized block commit,
    /// ends very before the tip commit.
    pub commits: Vec<RawCommit>,
    /// The tip commit must not be a transaction, chat log, or extra agenda transaction.
    /// In other words, the branch must be complete.
    ///
    /// That's why we can use the `Commit` type which doesn't preserve
    /// the physical git diff. (Remeber that block, agenda, and agenda-proof are
    /// empty commits)
    pub tip_commit: TipCommit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadFinalizationProof {
    pub proof: FinalizationProof,
    pub block_hash: Hash256,
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepositoryMessage {
    Branch(PayloadBranch),
    FinalizationProof(PayloadFinalizationProof),
}

impl ToHash256 for RepositoryMessage {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_spb::to_vec(self).unwrap())
    }
}

impl DmsMessage for RepositoryMessage {
    const DMS_TAG: &'static str = "repository";

    fn check(&self) -> Result<(), Error> {
        Ok(())
    }
}

fn update_single_branch(branch: &PayloadBranch, lfi: FinalizationInfo) -> Result<(), String> {
    let mut commits = Vec::new();
    for raw_commit in &branch.commits {
        let commit =
            format::from_semantic_commit(format::raw_commit_to_semantic_commit(raw_commit.clone()))
                .map_err(|error| error.to_string())?;
        commits.push(commit);
    }
    let mut csv = CommitSequenceVerifier::new(lfi.header, lfi.reserved_state)
        .map_err(|err| format!("failed to create CSV: {}", err))?;
    for commit in commits {
        csv.apply_commit(&commit)
            .map_err(|err| format!("failed to apply commit: {}", err))?;
    }
    let tip_commit = match branch.tip_commit.clone() {
        TipCommit::Block(block) => Commit::Block(block),
        TipCommit::Agenda(ag) => Commit::Agenda(ag),
        TipCommit::AgendaProof(ap) => Commit::AgendaProof(ap),
    };
    csv.apply_commit(&tip_commit)
        .map_err(|err| format!("failed to apply commit: {}", err))?;
    Ok(())
}

impl DistributedRepository {
    async fn write_branch(
        &self,
        branch: &PayloadBranch,
        lfi: &FinalizationInfo,
    ) -> Result<(), Error> {
        let branch_prefix = match branch.tip_commit {
            TipCommit::Block(_) => "b",
            TipCommit::Agenda(_) => "a",
            TipCommit::AgendaProof(_) => "a",
        };
        let tip_commit = branch.tip_commit.clone().into_commit();
        let mut branch_name = tip_commit.to_hash256().to_string();
        branch_name.truncate(BRANCH_NAME_HASH_DIGITS);
        let branch_name = format!("{branch_prefix}-{branch_name}");
        let result = self
            .raw
            .read()
            .await
            .locate_branch(branch_name.clone())
            .await;
        match result {
            Err(raw::Error::NotFound(_)) => {
                let mut raw_repo = self.raw.write().await;
                raw_repo.checkout_clean().await?;
                let lfb_commit_hash = raw_repo
                    .locate_branch(FINALIZED_BRANCH_NAME.to_owned())
                    .await?;
                raw_repo.checkout_detach(lfb_commit_hash).await?;
                for commit in branch.commits.clone() {
                    raw_repo.create_commit(commit).await?;
                }
                let semantic_commit =
                    format::to_semantic_commit(&tip_commit, lfi.reserved_state.clone())?;
                let raw_commit = RawCommit {
                    message: format!("{}\n\n{}", semantic_commit.title, semantic_commit.body),
                    diff: None,
                    author: "Simperby".to_string(),
                    email: "hi@simperby.net".to_string(),
                    timestamp: semantic_commit.timestamp / 1000,
                };
                raw_repo.create_commit(raw_commit).await?;
                let head = raw_repo.get_head().await?;
                raw_repo.create_branch(branch_name, head).await?;
                Ok(())
            }
            Err(err) => Err(err.into()),
            Ok(_) => {
                // Branch already exists; skip
                Ok(())
            }
        }
    }

    pub async fn update_(&self) -> Result<(), Error> {
        let dms_ = self
            .dms
            .as_ref()
            .ok_or_else(|| eyre::eyre!("dms is not initialized yet"))?
            .clone();
        let mut dms = dms_.write().await;
        let messages = dms.read_messages().await?;
        let lfi = self.read_last_finalization_info().await?;

        // Update branches
        for message in &messages {
            match message.message.clone() {
                RepositoryMessage::Branch(branch) => {
                    let result = update_single_branch(&branch, lfi.clone());
                    match result {
                        Ok(()) => {
                            self.write_branch(&branch, &lfi).await?;
                        }
                        Err(err) => {
                            dms.remove_message(message.message.to_hash256(), Some(err))
                                .await?;
                        }
                    }
                }
                _ => continue,
            }
        }

        // Update finalization proof
        for message in &messages {
            match message.message.clone() {
                RepositoryMessage::FinalizationProof(PayloadFinalizationProof {
                    proof,
                    block_hash,
                }) => {
                    let blocks = self.read_blocks().await?;
                    let (block_commit_hash, _) =
                        if let Some(x) = blocks.into_iter().find(|(_, hash)| *hash == block_hash) {
                            x
                        } else {
                            continue;
                        };
                    let block_commit =
                        if let Commit::Block(x) = self.read_commit(block_commit_hash).await? {
                            x
                        } else {
                            return Err(eyre::Error::from(IntegrityError::new(format!(
                                "block commit is not a block commit: {block_commit_hash}"
                            ))));
                        };

                    match verify::verify_finalization_proof(&block_commit, &proof) {
                        Ok(_) => {
                            crate::works::advance_finalized_branch(
                                &mut *self.raw.write().await,
                                block_commit_hash,
                                LastFinalizationProof {
                                    height: block_commit.height,
                                    proof,
                                },
                            )
                            .await?;
                        }
                        Err(err) => {
                            dms.remove_message(message.message.to_hash256(), Some(err.to_string()))
                                .await?;
                        }
                    }
                }
                _ => continue,
            }
        }
        Ok(())
    }

    pub async fn flush_(&self) -> Result<(), Error> {
        let lfi = self.read_last_finalization_info().await?;

        let blocks = self.read_blocks().await?;
        let agendas = self.read_agendas().await?;
        let agenda_proofs = self.read_governance_approved_agendas().await?;

        /// A behaivor of `create_branch` abstracted over the types of branches.
        trait BranchType {
            fn commit(commit: Commit) -> Result<Self, Error>
            where
                Self: Sized;
            fn tip_commit(self) -> TipCommit;
            fn name() -> &'static str;
        }

        impl BranchType for BlockHeader {
            fn commit(commit: Commit) -> Result<Self, Error> {
                if let Commit::Block(x) = commit {
                    Ok(x)
                } else {
                    Err(eyre::Error::from(IntegrityError::new(format!(
                        "commit is not a block commit: {commit:?}"
                    ))))
                }
            }
            fn tip_commit(self) -> TipCommit {
                TipCommit::Block(self)
            }
            fn name() -> &'static str {
                "block"
            }
        }

        impl BranchType for Agenda {
            fn commit(commit: Commit) -> Result<Self, Error> {
                if let Commit::Agenda(x) = commit {
                    Ok(x)
                } else {
                    Err(eyre::Error::from(IntegrityError::new(format!(
                        "commit is not an agenda commit: {commit:?}"
                    ))))
                }
            }
            fn tip_commit(self) -> TipCommit {
                TipCommit::Agenda(self)
            }
            fn name() -> &'static str {
                "agenda"
            }
        }

        impl BranchType for AgendaProof {
            fn commit(commit: Commit) -> Result<Self, Error> {
                if let Commit::AgendaProof(x) = commit {
                    Ok(x)
                } else {
                    Err(eyre::Error::from(IntegrityError::new(format!(
                        "commit is not an agenda-proof commit: {commit:?}"
                    ))))
                }
            }
            fn tip_commit(self) -> TipCommit {
                TipCommit::AgendaProof(self)
            }
            fn name() -> &'static str {
                "agenda-proof"
            }
        }

        async fn create_branch<T: BranchType>(
            this: &DistributedRepository,
            tip_commits: Vec<(CommitHash, Hash256)>,
            lfi: &FinalizationInfo,
        ) -> Result<Vec<PayloadBranch>, Error> {
            let raw = this.raw.read().await;
            let mut branches = Vec::new();
            for (commit_hash, _) in tip_commits {
                let commits = read::read_raw_commits(&raw, lfi.commit_hash, commit_hash).await?;
                let commit = T::commit(this.read_commit(commit_hash).await?)?;
                let len = commits.len();
                if len == 0 {
                    return Err(eyre::Error::from(IntegrityError::new(format!(
                        "{} branch is same as last finalized block: {commit_hash}",
                        T::name()
                    ))));
                }
                branches.push(PayloadBranch {
                    commits: commits.into_iter().take(len - 1).map(|(x, _)| x).collect(),
                    tip_commit: T::tip_commit(commit),
                });
            }
            Ok(branches)
        }

        let mut branches = Vec::new();
        branches.append(&mut create_branch::<BlockHeader>(self, blocks, &lfi).await?);
        branches.append(&mut create_branch::<Agenda>(self, agendas, &lfi).await?);
        branches.append(&mut create_branch::<AgendaProof>(self, agenda_proofs, &lfi).await?);

        let mut fps = Vec::new();
        let range = (lfi.header.height - 5.min(lfi.header.height))..=lfi.header.height;
        for height in range {
            let fi = self.read_finalization_info(height).await?;
            fps.push(PayloadFinalizationProof {
                proof: fi.proof,
                block_hash: fi.header.to_hash256(),
            });
        }

        let dms_ = self
            .dms
            .as_ref()
            .ok_or_else(|| eyre::eyre!("dms is not initialized yet"))?
            .clone();
        let mut dms = dms_.write().await;

        for branch in branches {
            dms.commit_message(&RepositoryMessage::Branch(branch))
                .await?;
        }
        for fp in fps {
            dms.commit_message(&RepositoryMessage::FinalizationProof(fp))
                .await?;
        }
        Ok(())
    }
}
