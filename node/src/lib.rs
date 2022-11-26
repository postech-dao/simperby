pub mod node;

pub use simperby_common;
use simperby_governance::Governance;
pub use simperby_network;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simperby_common::crypto::*;
use simperby_common::*;
use simperby_repository::CommitHash;

pub const PROTOCOL_VERSION: &str = "0.0.0";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub public_key: PublicKey,
    pub private_key: PrivateKey,
    pub chain_name: String,

    pub peer_directory: String,
    pub governance_directory: String,
    pub consensus_directory: String,
    pub repository_directory: String,

    pub broadcast_interval_ms: Option<u64>,
    pub fetch_interval_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConsensusStatus {
    // TODO
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkStatus {
    // TODO
}

/// The API for the Simperby node.
///
/// It is for serving the **CLI**, providing low-level functions and type-specified interfaces.
#[async_trait]
pub trait SimperbyApi {
    /// Initializes a new Simperby node from the genesis state
    /// stored in the given directory (which is not yet a Git repository).
    async fn genesis(&mut self) -> Result<()>;

    /// Initializes a new Simperby node from the repository.
    ///
    /// The `finalized` branch MUST be on a valid block commit.
    async fn initialize(&mut self) -> Result<()>;

    /// Synchronizes the `finalized` branch to the given commit.
    async fn sync(&mut self, commmit: CommitHash) -> Result<()>;

    /// Cleans the repository, removing all the outdated commits.
    async fn clean(&mut self, hard: bool) -> Result<()>;

    /// Creates a block commit on the `finalized` branch.
    async fn create_block(&mut self) -> Result<()>;

    /// Creates a block commit on the `finalized` branch.
    async fn create_agenda(&mut self) -> Result<()>;

    /// Creates an extra-agenda transaction on the `finalized` branch.
    async fn create_extra_agenda_transaction(&mut self, tx: ExtraAgendaTransaction) -> Result<()>;

    /// Votes and propagates.
    async fn vote(&mut self, agenda_commit: CommitHash) -> Result<()>;

    /// Vetos the current round.
    async fn veto_round(&mut self) -> Result<()>;

    /// Vetos the given block.
    async fn veto_block(&mut self, block_commit: CommitHash) -> Result<()>;

    /// Runs indefinitely updating everything.
    async fn run(self) -> Result<()>;

    /// Makes a progress for the consensus, returning the result.
    async fn progress_for_consensus(&mut self) -> Result<String>;

    /// Gets the current status of the consensus.
    async fn get_consensus_status(&self) -> Result<ConsensusStatus>;

    /// Gets the current status of the p2p network.
    async fn get_network_status(&self) -> Result<NetworkStatus>;

    /// Serves indefinitely relaying network messages.
    async fn relay(self) -> Result<()>;

    /// Fetch the data from the network and apply to the repository, the governance, and the consensus.
    async fn fetch(&mut self) -> Result<()>;

    /// Notifies that there was a git push. This is not intended to be used by the user.
    async fn notify_git_push(&mut self) -> Result<String>;

    // TODO: Add chat-related methods.
}
