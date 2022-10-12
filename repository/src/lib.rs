pub mod raw;

use async_trait::async_trait;
use raw::RawRepository;
use serde::{Deserialize, Serialize};
use simperby_common::*;
use simperby_network::{NetworkConfig, Peer, SharedKnownPeers};
use std::fmt;
use thiserror::Error;

pub type Branch = String;
pub type Tag = String;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize, Hash)]
pub struct CommitHash {
    pub hash: [u8; 32],
}

impl fmt::Display for CommitHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?")
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("network error: {0}")]
    Network(simperby_network::Error),
    #[error("raw repository error: {0}")]
    Raw(raw::Error),
    #[error("unknown error: {0}")]
    Unknown(String),
}

/// The local Simperby blockchain data repository.
///
/// It automatically locks the repository once created.
///
/// - It **verifies** all the incoming changes and applies them to the local repository
/// only if they are valid.
#[async_trait]
pub trait DistributedRepository<T: RawRepository>: Send + Sync {
    async fn new(raw: T) -> Result<Self, Error>
    where
        Self: Sized;

    /// Initialize the genesis repository from the genesis working tree.
    async fn genesis(&mut self) -> Result<(), Error>;

    /// Returns the block header from the `main` branch.
    async fn get_last_finalized_block_header(&self) -> Result<BlockHeader, Error>;

    /// Fetches new commits from the network.
    /// It **verifies** all the incoming changes and applies them to the local repository
    /// only if they are valid.
    async fn fetch(
        &mut self,
        network_config: NetworkConfig,
        known_peers: &[Peer],
    ) -> Result<(), Error>;

    /// Notifies there was a push for the given repository.
    async fn notify_push(
        &mut self,
        network_config: NetworkConfig,
        known_peers: &[Peer],
    ) -> Result<(), Error>;

    /// Serves the distributed repository protocol indefinitely.
    /// It **verifies** all the incoming changes and applies them to the local repository
    /// only if they are valid.
    async fn serve(
        self,
        network_config: &NetworkConfig,
        peers: SharedKnownPeers,
    ) -> Result<tokio::task::JoinHandle<Result<(), Error>>, Error>;

    /// Checks the validity of the repository, starting from the given height.
    ///
    /// It checks
    /// 1. all the reserved branches and tags
    /// 2. existence of merge commits
    /// 3. the canonical history of the `main` branch.
    async fn check(&self, starting_height: BlockHeight) -> Result<bool, Error>;

    /// Synchronizes the `main` branch to the given commit.
    ///
    /// This will verify every commit along the way.
    /// If the given commit is not a descendant of the
    /// current `main` (i.e., cannot be fast-forwarded), it fails.
    ///
    /// Note that if you sync to a block `H`, then the `main` branch will move to `H-1`.
    /// To sync the last block `H`, you have to run `finalize()`.
    /// (This is because the finalization proof for a block appears in the next block.)
    async fn sync(&mut self, block_commit: &CommitHash) -> Result<(), Error>;

    /// Returns the current valid and height-acceptable agendas in the repository.
    async fn get_agendas(&self) -> Result<Vec<(CommitHash, Hash256)>, Error>;

    /// Returns the current valid and height-acceptable blocks in the repository.
    async fn get_blocks(&self) -> Result<Vec<(CommitHash, Hash256)>, Error>;

    /// Finalizes a single block and moves the `main` branch to it.
    ///
    /// It will verify the finalization proof and the commits.
    async fn finalize(
        &mut self,
        block_commit_hash: &CommitHash,
        proof: &FinalizationProof,
    ) -> Result<(), Error>;

    /// Informs that the given agenda has been approved.
    async fn approve(&mut self, agenda_commit_hash: &CommitHash) -> Result<(), Error>;

    /// Creates an agenda commit on top of the `work` branch.
    async fn create_agenda(
        &mut self,
        last_transaction_commit_hash: &CommitHash,
    ) -> Result<CommitHash, Error>;

    /// Creates a block commit on top of the `work` branch.
    async fn create_block(
        &mut self,
        last_transaction_commit_hash: &CommitHash,
    ) -> Result<CommitHash, Error>;

    /// Creates an agenda commit on top of the `work` branch.
    async fn create_extra_agenda_transaction(
        &mut self,
        transaction: &ExtraAgendaTransaction,
    ) -> Result<CommitHash, Error>;
}
