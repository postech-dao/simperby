pub mod format;
pub mod raw;

use anyhow::anyhow;
use format::*;
use futures::prelude::*;
use raw::RawRepository;
use serde::{Deserialize, Serialize};
use simperby_common::reserved::ReservedState;
use simperby_common::verify::CommitSequenceVerifier;
use simperby_common::*;
use simperby_network::{NetworkConfig, Peer, SharedKnownPeers};
use std::fmt;

pub type Branch = String;
pub type Tag = String;

pub const FINALIZED_BRANCH_NAME: &str = "finalized";
pub const WORK_BRANCH_NAME: &str = "work";
pub const FP_BRANCH_NAME: &str = "fp";
pub const COMMIT_TITLE_HASH_DIGITS: usize = 8;
pub const TAG_NAME_HASH_DIGITS: usize = 8;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Serialize, Deserialize, Hash)]
pub struct CommitHash {
    pub hash: [u8; 20],
}

impl fmt::Display for CommitHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?")
    }
}

pub type Error = anyhow::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Public repos (usually mirrors) for the read-only accesses
    ///
    /// They're added as a remote repo, named `public_#`.
    /// Note that they are not part of the `known_peers`.
    pub mirrors: Vec<String>,
}

/// The local Simperby blockchain data repository.
///
/// It automatically locks the repository once created.
///
/// - It **verifies** all the incoming changes and applies them to the local repository
/// only if they are valid.
pub struct DistributedRepository<T> {
    raw: T,
    _config: Config,
}

fn get_timestamp() -> Timestamp {
    let now = std::time::SystemTime::now();
    let since_the_epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap();
    since_the_epoch.as_millis() as Timestamp
}

impl<T: RawRepository> DistributedRepository<T> {
    pub fn get_raw_mut(&mut self) -> &mut T {
        &mut self.raw
    }

    pub async fn new(raw: T, _config: Config) -> Result<Self, Error> {
        Ok(Self { raw, _config })
    }

    /// Initializes the genesis repository from the genesis commit,
    /// leaving a genesis header.
    ///
    /// The repository MUST have only two commits: `initial` and `genesis` in the `finalized` branch.
    /// The `genesis` commit MUST have set the initial reserved state in a valid format.
    ///
    /// It also
    /// - creates `fp` branch and its commit (for the genesis block).
    /// - creates `work` branch at the same place with the `finalized` branch.
    pub async fn genesis(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    /// Returns the block header from the `finalized` branch.
    pub async fn get_last_finalized_block_header(&self) -> Result<BlockHeader, Error> {
        let commit_hash = self.raw.locate_branch(FINALIZED_BRANCH_NAME.into()).await?;
        let semantic_commit = self.raw.read_semantic_commit(commit_hash).await?;
        let commit = format::from_semantic_commit(semantic_commit).map_err(|e| anyhow!(e))?;
        if let Commit::Block(block_header) = commit {
            Ok(block_header)
        } else {
            Err(anyhow!(
                "repository integrity broken; `finalized` branch is not on a block"
            ))
        }
    }

    /// Returns the reserved state from the `finalized` branch.
    pub async fn get_reserved_state(&self) -> Result<ReservedState, Error> {
        self.raw.read_reserved_state().await.map_err(|e| anyhow!(e))
    }

    /// Cleans all the outdated commits, remote repositories and branches.
    ///
    /// It will leave only
    /// - the `finalized` branch
    /// - the `work` branch
    /// - the `fp` branch.
    ///
    /// and
    /// - the `p` branch
    /// - the `a-#` branches
    /// - the `b-#` branches
    /// if only the branches are not outdated (branched from the last finalized commit).
    pub async fn clean(&mut self) -> Result<(), Error> {
        let finalized_branch_commit_hash =
            self.raw.locate_branch(FINALIZED_BRANCH_NAME.into()).await?;

        let branches = self.raw.list_branches().await?;

        // delete outdated p branch, a-# branches, b-# branches
        for branch in branches {
            if !(branch.as_str() == WORK_BRANCH_NAME
                || branch.as_str() == FINALIZED_BRANCH_NAME
                || branch.as_str() == FP_BRANCH_NAME)
            {
                let branch_commit = self.raw.locate_branch(branch.clone()).await?;

                if finalized_branch_commit_hash
                    != self
                        .raw
                        .find_merge_base(branch_commit, finalized_branch_commit_hash)
                        .await?
                {
                    self.raw.delete_branch(branch.to_string()).await?;
                }
            }
        }

        // remove remote branches
        let remote_list = self.raw.list_remotes().await?;
        for (remote_name, _) in remote_list {
            self.raw.remove_remote(remote_name).await?;
        }

        // TODO : CSV

        Ok(())
    }

    /// Fetches new commits from the network.
    ///
    /// It **verifies** all the incoming changes and applies them to the local repository
    /// only if they are valid.
    ///
    /// - It may move the `finalized` branch.
    /// - It may add some `a-#` branches.
    /// - It may add some `b-#` branches.
    /// - It may update the `fp` branch.
    ///
    /// It may leave some remote repository (representing each peer) after the operation.
    pub async fn fetch(
        &mut self,
        _network_config: &NetworkConfig,
        _known_peers: &[Peer],
    ) -> Result<(), Error> {
        unimplemented!()
    }

    /// Serves the distributed repository protocol indefinitely.
    /// It **verifies** all the incoming changes and applies them to the local repository
    /// only if they are valid.
    pub async fn serve(
        self,
        _network_config: &NetworkConfig,
        _peers: SharedKnownPeers,
    ) -> Result<tokio::task::JoinHandle<Result<(), Error>>, Error> {
        unimplemented!()
    }

    /// Checks the validity of the repository, starting from the given height.
    ///
    /// It checks
    /// 1. all the reserved branches and tags
    /// 2. the finalization proof in the `fp` branch.
    /// 3. the existence of merge commits
    /// 4. the canonical history of the `finalized` branch.
    /// 5. the reserved state in a valid format.
    pub async fn check(&self, _starting_height: BlockHeight) -> Result<bool, Error> {
        unimplemented!()
    }

    /// Synchronizes the `finalized` branch to the given commit.
    ///
    /// This will verify every commit along the way.
    /// If the given commit is not a descendant of the
    /// current `finalized` (i.e., cannot be fast-forwarded), it fails.
    ///
    /// Note that the last block will be verified by the given `last_block_proof`,
    /// and the `fp` branch will be updated as well.
    pub async fn sync(
        &mut self,
        _commit: CommitHash,
        _last_block_proof: &FinalizationProof,
    ) -> Result<(), Error> {
        unimplemented!()
    }
    /// Returns the currently valid and height-acceptable agendas in the repository.
    pub async fn get_agendas(&self) -> Result<Vec<(CommitHash, Hash256)>, Error> {
        unimplemented!()
    }

    /// Returns the currently valid and height-acceptable blocks in the repository.
    pub async fn get_blocks(&self) -> Result<Vec<(CommitHash, Hash256)>, Error> {
        unimplemented!()
    }

    /// Informs that the given agenda has been approved.
    ///
    ///
    /// After verification, it will create an agenda-proof commit,
    /// and update the corresponding `a-#` branch to it
    pub async fn approve(
        &mut self,
        _agenda_commit_hash: &CommitHash,
        _proof: Vec<(PublicKey, TypedSignature<Agenda>)>,
    ) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    /// Creates an agenda commit on top of the `work` branch.
    pub async fn create_agenda(
        &mut self,
        author: PublicKey,
    ) -> Result<(Agenda, CommitHash), Error> {
        let last_header = self.get_last_finalized_block_header().await?;
        let work_commit = self.raw.locate_branch(WORK_BRANCH_NAME.into()).await?;
        let last_header_commit = self.raw.locate_branch(FINALIZED_BRANCH_NAME.into()).await?;

        // Check if the `work` branch is rebased on top of the `finalized` branch.
        if self
            .raw
            .find_merge_base(last_header_commit, work_commit)
            .await?
            != last_header_commit
        {
            return Err(anyhow!(
                "branch {} should be rebased on {}",
                WORK_BRANCH_NAME,
                FINALIZED_BRANCH_NAME
            ));
        }

        // Fetch and convert commits
        let commits = self.raw.list_ancestors(work_commit, Some(256)).await?;
        let position = commits
            .iter()
            .position(|c| *c == last_header_commit)
            .expect("TODO: handle the case where it exceeds the limit.");

        // commits starting from the very next one to the last finalized block.
        let commits = stream::iter(commits.iter().take(position).rev().cloned().map(|c| {
            let raw = &self.raw;
            async move { raw.read_semantic_commit(c).await.map(|x| (x, c)) }
        }))
        .buffered(256)
        .collect::<Vec<_>>()
        .await;
        let mut commits = commits.into_iter().collect::<Result<Vec<_>, _>>()?;
        // Add most recent commit of the branch to the list since it is not included in the ancestor commits
        commits.push((
            self.raw.read_semantic_commit(work_commit).await?,
            work_commit,
        ));
        let commits = commits
            .into_iter()
            .map(|(commit, hash)| {
                from_semantic_commit(commit)
                    .map_err(|e| (e, hash))
                    .map(|x| (x, hash))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|(error, hash)| anyhow!("failed to convert the commit {}: {}", hash, error))?;

        // Check the validity of the commit sequence
        let reserved_state = self.get_reserved_state().await?;
        let mut verifier = CommitSequenceVerifier::new(last_header.clone(), reserved_state)
            .map_err(|e| anyhow!("verification error on commit {}: {}", last_header_commit, e))?;
        for (commit, hash) in commits.iter() {
            verifier
                .apply_commit(commit)
                .map_err(|e| anyhow!("verification error on commit {}: {}", hash, e))?;
        }

        // Check whether the commit sequence is in the transaction phase.
        let mut transactions = Vec::new();

        for (commit, _) in commits {
            if let Commit::Transaction(t) = commit {
                transactions.push(t.clone());
            } else {
                return Err(anyhow!(
                    "branch {} is not in the transaction phase",
                    WORK_BRANCH_NAME
                ));
            }
        }

        let agenda = Agenda {
            author,
            timestamp: get_timestamp(),
            transactions_hash: Agenda::calculate_hash(&transactions),
            height: last_header.height + 1,
        };
        let agenda_commit = Commit::Agenda(agenda.clone());
        let semantic_commit = to_semantic_commit(&agenda_commit);

        self.raw.checkout_clean().await?;
        self.raw.checkout(WORK_BRANCH_NAME.into()).await?;
        let result = self.raw.create_semantic_commit(semantic_commit).await?;
        self.raw
            .create_branch(
                format!(
                    "a-{:?}",
                    agenda_commit
                        .to_hash256()
                        .to_string()
                        .truncate(COMMIT_TITLE_HASH_DIGITS)
                ),
                result,
            )
            .await?;
        Ok((agenda, result))
    }

    /// Puts a 'vote' tag on the commit.
    pub async fn vote(&mut self, commit_hash: CommitHash) -> Result<(), Error> {
        let semantic_commit = self.raw.read_semantic_commit(commit_hash).await?;
        let commit = format::from_semantic_commit(semantic_commit).map_err(|e| anyhow!(e))?;
        // Check if the commit is an agenda commit.
        if let Commit::Agenda(_) = commit {
            self.raw
                .create_tag(
                    format!(
                        "vote-{:?}",
                        commit
                            .to_hash256()
                            .to_string()
                            .truncate(TAG_NAME_HASH_DIGITS)
                    ),
                    commit_hash,
                )
                .await?;
            Ok(())
        } else {
            Err(anyhow!("commit {} is not an agenda commit", commit_hash))
        }
    }

    /// Puts a 'veto' tag on the commit.
    pub async fn veto(&mut self, commit_hash: CommitHash) -> Result<(), Error> {
        let semantic_commit = self.raw.read_semantic_commit(commit_hash).await?;
        let commit = format::from_semantic_commit(semantic_commit).map_err(|e| anyhow!(e))?;
        // Check if the commit is an agenda commit.
        if let Commit::Block(_) = commit {
            self.raw
                .create_tag(
                    format!(
                        "veto-{:?}",
                        commit
                            .to_hash256()
                            .to_string()
                            .truncate(TAG_NAME_HASH_DIGITS)
                    ),
                    commit_hash,
                )
                .await?;
            Ok(())
        } else {
            Err(anyhow!("commit {} is not a block commit", commit_hash))
        }
    }

    /// Creates a block commit on top of the `work` branch.
    pub async fn create_block(
        &mut self,
        _author: PublicKey,
    ) -> Result<(BlockHeader, CommitHash), Error> {
        unimplemented!()
    }

    /// Creates an agenda commit on top of the `work` branch.
    pub async fn create_extra_agenda_transaction(
        &mut self,
        _transaction: &ExtraAgendaTransaction,
    ) -> Result<CommitHash, Error> {
        unimplemented!()
    }
}
