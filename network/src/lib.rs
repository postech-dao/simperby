pub mod dms;
mod peer_discovery;
pub mod primitives;
pub mod storage;

use async_trait::async_trait;
use primitives::*;
use serde::{Deserialize, Serialize};
use simperby_common::{crypto::*, Timestamp};
use std::collections::HashMap;
use std::{net::SocketAddrV4, sync::Arc};
use tokio::sync::RwLock;

pub type Error = anyhow::Error;

/// The information of a network peer that is discovered by the discovery protocol.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub public_key: PublicKey,
    /// The address used for the discovery protocol
    pub address: SocketAddrV4,
    /// For the other network services like gossip or RPC,
    /// it provides a map of `identifier->port`.
    pub ports: HashMap<String, u16>,
    pub message: String,
    pub recently_seen_timestamp: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// The unique id for distinguishing the network.
    pub network_id: String,
    /// The port that will be used during a network operation.
    pub port: Option<u16>,
    /// The set of the members of the network.
    pub members: Vec<PublicKey>,
    /// The public key of this node.
    pub public_key: PublicKey,
    /// The private key of this node.
    pub private_key: PrivateKey,
}

/// The currently known peers that are for other modules,
/// which will be updated by `PeerDiscovery`.
#[derive(Clone, Debug)]
pub struct SharedKnownPeers {
    lock: Arc<RwLock<Vec<Peer>>>,
}

impl SharedKnownPeers {
    pub async fn read(&self) -> Vec<Peer> {
        self.lock.read().await.clone()
    }
}

/// The peer discovery protocol backed by the local file system.
///
/// For every method,
/// - If the given directory is empty, it fails (except `create()`).
/// - It locks the storage.
/// - If the given directory is locked (possibly by another instance of `DistributedMessageSet`),
/// it will `await` until the lock is released.
#[async_trait]
pub trait PeerDiscovery {
    /// Creates a new and empty storage with the given directory.
    /// Fails if there is already a directory.
    async fn create(storage_directory: &str) -> Result<(), Error>;

    /// Serves the discovery protocol indefinitely, updating the known peers on the storage.
    ///
    /// - It may discard members in the storage who are not in `NetworkConfig::members`.
    async fn serve(
        storage_directory: &str,
        network_config: &NetworkConfig,
    ) -> Result<(SharedKnownPeers, tokio::task::JoinHandle<Result<(), Error>>), Error>;

    /// Reads the known peers from the storage.
    async fn read_known_peers(storage_directory: &str) -> Result<Vec<Peer>, Error>;
}

// TODO: remove this
pub trait AuthorizedNetwork: Send + Sync {}
