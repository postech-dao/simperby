mod dms;
mod primitives;

#[cfg(feature = "full")]
pub mod propagation_network;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simperby_common::{crypto::*, BlockHeight, Timestamp};
use std::collections::HashMap;
use std::{net::SocketAddrV4, sync::Arc};
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("unknown error: {0}")]
    Unknown(String),
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub public_key: PublicKey,
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
/// For every methods,
/// - If the given directory is empty, it fails (except `create()`).
/// - It locks the storage.
/// - If the given directory is locked (possibly by another instance of `DistributedMessageSet`),
/// it will `await` until the lock is released.
#[async_trait]
pub trait PeerDiscovery {
    /// Creates a new and empty storage with the given directory.
    /// Fails if there is already a directory.
    async fn create(storage_directory: &str) -> Result<(), Error>;

    /// Serve the discovery protocol indefinitely, updating the known peers on the storage.
    ///
    /// - The initial data given in `known_peers` will be ignored.
    /// - It may discard members in the storage who are not in `NetworkConfig::members`.
    async fn serve(
        storage_directory: &str,
        network_config: &NetworkConfig,
    ) -> Result<(SharedKnownPeers, tokio::task::JoinHandle<Result<(), Error>>), Error>;

    /// Reads the known peers from the storage.
    async fn read_known_peers(storage_directory: &str) -> Result<Vec<Peer>, Error>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub data: Vec<u8>,
    pub author: PublicKey,
}

/// A **cumulative** set that is shared in the p2p network, backed by the local file system.
///
/// One of the notable characteristics of blockchain is that it is based on heights;
/// The key idea here is that we retain an instance (both in memory or on disk)
/// of `DistributedMessageSet` only for a specific height,
/// and discard if the height progresses, creating a new and empty one again.
///
/// For every methods,
/// - If the given directory is empty, it fails (except `create()`).
/// - It locks the storage.
/// - If the given directory is locked (possibly by another instance of `DistributedMessageSet`),
/// it will `await` until the lock is released.
#[async_trait]
pub trait DistributedMessageSet {
    /// Creates a new and empty storage with the given directory.
    /// Fails if there is already a directory.
    async fn create(storage_directory: &str, height: u64) -> Result<(), Error>;

    /// Fetches the unknown messages from the peers and updates the storage.
    async fn fetch(
        storage_directory: &str,
        network_config: NetworkConfig,
        known_peers: &[Peer],
    ) -> Result<(), Error>;

    /// Add the given message to the storage, immediately broadcasting it to the network.
    ///
    /// Note that it is guaranteed that the message will not be broadcasted unless it
    /// is successfully added to the storage. (but it is not guaranteed for the other way around)
    async fn add_message(
        storage_directory: &str,
        network_config: NetworkConfig,
        known_peers: &[Peer],
        message: Vec<u8>,
        height_to_assert: BlockHeight,
    ) -> Result<(), Error>;

    /// Reads the messages and the height from the storage.
    async fn read_messages(storage_directory: &str) -> Result<(BlockHeight, Vec<Message>), Error>;

    /// Reads the height from the storage.
    async fn read_height(storage_directory: &str) -> Result<BlockHeight, Error>;

    /// Advance the height of the message set, discarding all the messages.
    async fn advance(storage_directory: &str, height_to_assert: BlockHeight) -> Result<(), Error>;

    /// Serves the p2p network node and the RPC server indefinitely, constantly updating the storage.
    async fn serve(
        storage_directory: &str,
        network_config: NetworkConfig,
        peers: SharedKnownPeers,
    ) -> Result<tokio::task::JoinHandle<Result<(), Error>>, Error>;
}

// TODO: remove this
pub trait AuthorizedNetwork: Send + Sync {}
