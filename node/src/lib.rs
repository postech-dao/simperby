//! A Simperby node.
//!
//! The following CLI commands are provided by `SimperbyNode` as they are based on the node state.
//!
//! - `sync`
//! - `clean`
//! - `create`
//! - `vote`
//! - `veto`
//! - `consensus`
//! - `git`
//! - `show`
//! - `network`
//! - `update`
//! - `broadcast`
//! - `chat`
//!
//! The following CLI commands are provided as global functions as they are node-stateless.
//!
//! - `genesis`
//!
//! The following CLI commands are provided as global functions as they are about the node creation.
//!
//! - `init`
//! - `clone`
//! - `serve`
//!
//! The following CLI commands are not provided here because they are simple
//! and so directly implemented in the CLI.
//!
//! - `sign`
pub mod node;

pub use simperby_common;
pub use simperby_network;
pub use simperby_repository;

use eyre::Result;
use serde::{Deserialize, Serialize};
use simperby_common::crypto::*;
use simperby_common::*;
use simperby_governance::Governance;
use simperby_network::{Peer, SharedKnownPeers};
use simperby_repository::raw::{RawRepository, RawRepositoryImpl, SemanticCommit};
use simperby_repository::CommitHash;
use simperby_repository::DistributedRepository;

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

    pub governance_port: u16,
    pub consensus_port: u16,
    pub repository_port: u16,
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

pub type SimperbyNode = node::Node<
    simperby_network::primitives::DummyGossipNetwork,
    simperby_network::storage::StorageImpl,
    simperby_repository::raw::RawRepositoryImpl,
>;

/// Creates a genesis commit.
pub async fn genesis(config: Config, path: &str) -> Result<()> {
    let peers: Vec<Peer> =
        serde_spb::from_str(&tokio::fs::read_to_string(&format!("{}/peers.json", path)).await?)?;
    let peers = SharedKnownPeers::new_static(peers.clone());
    let raw_repository = RawRepositoryImpl::open(&format!("{}/repository/repo", path)).await?;
    let mut repository = DistributedRepository::new(
        raw_repository,
        simperby_repository::Config {
            mirrors: config.public_repo_url.clone(),
            long_range_attack_distance: 3,
        },
        peers.clone(),
    )
    .await?;
    repository.genesis().await?;

    Ok(())
}

/// Initializes a node.
pub async fn initialize(config: Config, path: &str) -> Result<SimperbyNode> {
    SimperbyNode::initialize(config, path).await
}

/// Clones a remote repository and initializes a node.
pub async fn clone(_config: Config, _path: &str, _url: &str) -> Result<SimperbyNode> {
    todo!()
}

/// Runs a server node indefinitely.
pub async fn serve(_config: Config, _path: &str) -> Result<()> {
    todo!()
}
