pub mod dms;
#[cfg(never)]
mod peer_discovery;
pub mod primitives;
pub mod storage;

use serde::{Deserialize, Serialize};
use simperby_common::{crypto::*, MemberName, Timestamp};
use std::collections::HashMap;
use std::net::SocketAddrV4;

pub type Error = eyre::Error;
pub type Dms<T> = dms::DistributedMessageSet<storage::StorageImpl, T>;

pub use dms::{DmsKey, DmsMessage, Message, MessageCommitmentProof};
pub use primitives::*;
pub use storage::StorageImpl;

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
    /// The unique id for distinguishing the network.
    pub network_id: String,
    /// The set of the members of the network.
    pub members: Vec<PublicKey>,
    /// The private key of this node.
    pub private_key: PrivateKey,
    /// The peer nodes to broadcast the message.
    pub peers: Vec<Peer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerNetworkConfig {
    /// The unique id for distinguishing the network.
    pub network_id: String,
    /// The set of the members of the network.
    pub members: Vec<PublicKey>,
    /// The private key of this node.
    pub private_key: PrivateKey,
    /// The map of `identifier->port` where an `identifier` represents each network service
    /// (e.g. gossip-consensus, RPC-governance, discovery, ...)
    /// The server advertises this port mappings on the peer discovery protocol,
    /// so that other peers can know on which port the server provides a specific service.
    pub ports: HashMap<String, u16>,
    // TODO: add various configurations for NAT traversal
}
