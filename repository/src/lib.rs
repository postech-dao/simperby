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

/// The local Simperby blockchain data repository.
///
/// It automatically locks the repository once created.
///
/// - It **verifies** all the incoming changes and applies them to the local repository
/// only if they are valid.
pub struct DistributedRepository<T> {
    raw: T,
}

fn get_timestamp() -> Timestamp {
    let now = std::time::SystemTime::now();
    let since_the_epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap();
    since_the_epoch.as_millis() as Timestamp
}

impl<T: RawRepository> DistributedRepository<T> {
    pub async fn new(raw: T) -> Result<Self, Error> {
        Ok(Self { raw })
    }

    /// Initializes the genesis repository from the genesis working tree.
    pub async fn genesis(&mut self) -> Result<(), Error> {
        unimplemented!()
    }
    /// Returns the block header from the `finalized` branch.
    pub async fn get_last_finalized_block_header(&self) -> Result<BlockHeader, Error> {
        unimplemented!()
    }

    /// Returns the reserved state from the `finalized` branch.
    pub async fn get_reserved_state(&self) -> Result<ReservedState, Error> {
        unimplemented!()
    }

    /// Fetches new commits from the network.
    /// It **verifies** all the incoming changes and applies them to the local repository
    /// only if they are valid.
    pub async fn fetch(
        &mut self,
        _network_config: &NetworkConfig,
        _known_peers: &[Peer],
    ) -> Result<(), Error> {
        unimplemented!()
    }

    /// Notifies there was a push for the given repository.
    pub async fn notify_push(
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
    /// 2. the existence of merge commits
    /// 3. the canonical history of the `finalized` branch.
    pub async fn check(&self, _starting_height: BlockHeight) -> Result<bool, Error> {
        unimplemented!()
    }

    /// Synchronizes the `finalized` branch to the given commit.
    ///
    /// This will verify every commit along the way.
    /// If the given commit is not a descendant of the
    /// current `finalized` (i.e., cannot be fast-forwarded), it fails.
    ///
    /// Note that if you sync to a block `H`, then the `finalized` branch will move to `H-1`.
    /// To sync the last block `H`, you have to run `finalize()`.
    /// (This is because the finalization proof for a block appears in the next block.)
    pub async fn sync(&mut self, _block_commit: &CommitHash) -> Result<(), Error> {
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

    /// Finalizes a single block and moves the `finalized` branch to it.
    ///
    /// It will verify the finalization proof and the commits.
    pub async fn finalize(
        &mut self,
        _block_commit_hash: &CommitHash,
        _proof: &FinalizationProof,
    ) -> Result<(), Error> {
        unimplemented!()
    }

    /// Informs that the given agenda has been approved.
    pub async fn approve(
        &mut self,
        _agenda_commit_hash: &CommitHash,
        _proof: Vec<(PublicKey, TypedSignature<Agenda>)>,
    ) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    /// Creates an agenda commit on top of the `work` branch.
    pub async fn create_agenda(&mut self, author: PublicKey) -> Result<CommitHash, Error> {
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
        let commits = commits.into_iter().collect::<Result<Vec<_>, _>>()?;
        let commits = commits
            .into_iter()
            .map(|(commit, hash)| {
                from_semantic_commit(commit, &last_header)
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

        let agenda_commit = Commit::Agenda(Agenda {
            author,
            timestamp: get_timestamp(),
            hash: Agenda::calculate_hash(last_header.height + 1, &transactions),
        });
        let semantic_commit = to_semantic_commit(&agenda_commit, &last_header);

        self.raw.checkout_clean().await?;
        self.raw.checkout(WORK_BRANCH_NAME.into()).await?;
        let result = self.raw.create_semantic_commit(semantic_commit).await?;
        Ok(result)
    }

    /// Creates a block commit on top of the `work` branch.
    pub async fn create_block(&mut self, _author: PublicKey) -> Result<CommitHash, Error> {
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
