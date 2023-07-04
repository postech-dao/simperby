use super::*;
use serde::{Deserialize, Serialize};
use simperby_repository::raw::SemanticCommit;

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

/// A configuration for a node.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {}

/// Hosting a server node requires extra configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    pub peers_port: u16,
    pub governance_port: u16,
    pub consensus_port: u16,
    pub repository_port: u16,

    pub broadcast_interval_ms: Option<u64>,
    pub fetch_interval_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Auth {
    pub private_key: PrivateKey,
}
