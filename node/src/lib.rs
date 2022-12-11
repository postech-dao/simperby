pub mod node;

pub use simperby_common;
pub use simperby_network;
pub use simperby_repository;

use async_trait::async_trait;
use eyre::Result;
use serde::{Deserialize, Serialize};
use simperby_common::crypto::*;
use simperby_common::*;
use simperby_governance::Governance;
use simperby_repository::raw::SemanticCommit;
use simperby_repository::CommitHash;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub chain_name: String,

    pub public_key: PublicKey,
    pub private_key: PrivateKey,

    pub broadcast_interval_ms: Option<u64>,
    pub fetch_interval_ms: Option<u64>,

    /// Public repos (usually mirrors) for the read-only accesses
    ///
    /// They're added as a remote repo, named `public_#`.
    pub public_repo_url: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConsensusStatus {
    // TODO
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkStatus {
    // TODO
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum CommitInfo {
    Block {
        semantic_commit: SemanticCommit,
        block_header: BlockHeader,
        // TODO: block-specific consensus status
    },
    Agenda {
        semantic_commit: SemanticCommit,
        agenda: Agenda,
        voters: Vec<(MemberName, Timestamp)>,
    },
    AgendaProof {
        semantic_commit: SemanticCommit,
        agenda_proof: AgendaProof,
    },
    Transaction {
        semantic_commit: SemanticCommit,
        transaction: Transaction,
    },
    PreGenesisCommit {
        title: String,
    },
    Unknown {
        semantic_commit: SemanticCommit,
        msg: String,
    }, // TODO
}

/// The API for the Simperby node.
///
/// It is for serving the **CLI**, providing low-level functions and type-specified interfaces.
#[async_trait]
pub trait SimperbyApi {
    /// Initializes a new Simperby node from the genesis state
    /// stored in the given directory (which is not yet a Git repository).
    async fn genesis(&mut self) -> Result<()>;

    /// Synchronizes the `finalized` branch to the given commit.
    async fn sync(&mut self, commmit: CommitHash) -> Result<()>;

    /// Cleans the repository, removing all the outdated commits.
    async fn clean(&mut self, hard: bool) -> Result<()>;

    /// Creates a block commit on the `finalized` branch.
    async fn create_block(&mut self) -> Result<CommitHash>;

    /// Creates a block commit on the `finalized` branch.
    async fn create_agenda(&mut self) -> Result<CommitHash>;

    /// Creates an extra-agenda transaction on the `finalized` branch.
    async fn create_extra_agenda_transaction(&mut self, tx: ExtraAgendaTransaction) -> Result<()>;

    /// Votes and propagates.
    async fn vote(&mut self, agenda_commit: CommitHash) -> Result<()>;

    /// Vetos the current round.
    async fn veto_round(&mut self) -> Result<()>;

    /// Vetos the given block.
    async fn veto_block(&mut self, block_commit: CommitHash) -> Result<()>;

    /// Shows information about the given commit.
    async fn show(&self, commit: CommitHash) -> Result<CommitInfo>;

    /// Runs indefinitely updating everything.
    async fn run(self) -> Result<()>;

    /// Makes a progress for the consensus, returning the result.
    async fn progress_for_consensus(&mut self) -> Result<String>;

    /// Gets the current status of the consensus.
    async fn get_consensus_status(&self) -> Result<ConsensusStatus>;

    /// Gets the current status of the p2p network.
    async fn get_network_status(&self) -> Result<NetworkStatus>;

    /// Serves indefinitely the p2p network.
    async fn serve(self) -> Result<Self>
    where
        Self: Sized;

    /// Fetch the data from the network and apply to the repository, the governance, and the consensus.
    async fn fetch(&mut self) -> Result<()>;

    // TODO: Add chat-related methods.
}

/// A working Simperby node.
pub type SimperbyNode = node::Node<
    simperby_network::primitives::DummyGossipNetwork,
    simperby_network::storage::StorageImpl,
    simperby_repository::raw::RawRepositoryImpl,
>;

pub async fn initialize(config: Config, path: &str) -> Result<SimperbyNode> {
    SimperbyNode::initialize(config, path).await
}
