pub mod dms;

#[cfg(never)]
mod peer_discovery;
pub mod storage;

use serde::{Deserialize, Serialize};
use simperby_core::{crypto::*, MemberName, Timestamp};
use std::collections::HashMap;
use std::net::SocketAddrV4;

pub type Error = eyre::Error;
pub type Dms<T> = dms::DistributedMessageSet<storage::StorageImpl, T>;

pub use dms::{Config, DmsKey, DmsMessage, MessageCommitmentProof};
pub use storage::{Storage, StorageError, StorageImpl};

/// The information of a network peer that is discovered by the discovery protocol.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub public_key: PublicKey,
    pub name: MemberName,
    /// The address used for the discovery protocol
    pub address: SocketAddrV4,
    /// For the other network services like gossip or RPC,
    /// it provides a map of `identifier->port`.
    pub ports: HashMap<String, u16>,
    pub message: String,
    pub recently_seen_timestamp: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientNetworkConfig {
    /// The peer nodes to broadcast the message.
    pub peers: Vec<Peer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerNetworkConfig {
    pub port: u16, // TODO: add various configurations for NAT traversal
}
