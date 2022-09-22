#[cfg(feature = "full")]
pub mod propagation_network;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simperby_common::{crypto::*, BlockHeight};
use std::{net::SocketAddrV4, sync::Arc};
use thiserror::Error;
use tokio::sync::{mpsc, RwLock};

#[derive(Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("unknown error: {0}")]
    Unknown(String),
}

pub struct Peer {
    pub public_key: PublicKey,
    pub address: SocketAddrV4,
    /// An arbitrary string that the peer has set for itself.
    /// This is usually used for indicating ports for the other services
    /// that ths peer is running (e.g., Git, RPC, Message, ...)
    pub message: String,
}

#[async_trait]
pub trait PeerDiscovery {
    /// Remains online on the network indefinitely,
    /// responding to discovery requests from other nodes,
    /// updating `known_peers`.
    async fn serve(
        network_config: &NetworkConfig,
        known_peers: Arc<RwLock<Vec<Peer>>>,
    ) -> Result<(), Error>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// The set of the members of the network.
    pub members: Vec<PublicKey>,
    /// The public key of this node.
    pub public_key: PublicKey,
    /// The private key of this node.
    pub private_key: PrivateKey,
}

/// The p2p gossip network.
#[async_trait]
pub trait P2PNetwork {
    /// Broadcasts a message to the network.
    async fn broadcast(
        config: &NetworkConfig,
        known_peers: &[Peer],
        message: Vec<u8>,
    ) -> Result<(), Error>;

    /// Remains online on the network indefinitely,
    /// relaying (propagating) messages broadcasted over the network.
    ///
    /// * `send` - A channel to send the received messages to.
    async fn serve(
        config: &NetworkConfig,
        peers: Arc<RwLock<Vec<Peer>>>,
        send: mpsc::Sender<Vec<u8>>,
    ) -> Result<(), Error>;
}

/// A **cumulative** set that is shared in the p2p network, backed by the local file system.
///
/// One of the notable characteristics of blockchain is that it is based on heights;
/// The key idea here is that we retanin an instance (both in memory or on disk)
/// of `DistributedMessageSet` only for a specific height,
/// and discard if the height progresses, creating a new and empty one again.
///
/// For every methods,
/// - If the given directory is empty, it fails (except `create()`).
/// - It locks the storage.
/// - If the given directory is locked (possibly by another instnace of `DistributedMessageSet`),
/// it will `await` until the lock is released.
#[async_trait]
pub trait DistributedMessageSet {
    /// Creates a new and empty storage with the given directory.
    /// Fails if there is already a directory.
    async fn create(storage_directory: &str, height: u64) -> Result<(), Error>;

    /// Fetches the unknown messages from the peers and updates the storage.
    async fn fetch(
        network_config: NetworkConfig,
        known_peers: &[Peer],
        storage_directory: &str,
    ) -> Result<(), Error>;

    /// Add the given message to the storage, immediately broadcasting it to the network.
    async fn add_message(
        network_config: NetworkConfig,
        storage_directory: &str,
        message: Vec<u8>,
    ) -> Result<(), Error>;

    /// Reads the messages for the storage.
    async fn read_messages(storage_directory: &str) -> Result<(BlockHeight, Vec<Message>), Error>;

    /// Reads the height for the storage.
    async fn read_height(storage_directory: &str) -> Result<BlockHeight, Error>;

    /// Advance the height of the message set, discarding all the messages.
    async fn advance(storage_directory: &str) -> Result<(), Error>;

    /// Serves the p2p network node and the RPC server indefinitely, constantly updating the storage.
    async fn serve(
        network_config: NetworkConfig,
        peers: Arc<RwLock<Vec<Peer>>>,
        storage_directory: &str,
    ) -> Result<(), Error>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub data: Vec<u8>,
    pub author: PublicKey,
}

/// The interface that will be wrapped into an HTTP RPC server for the peers.
#[async_trait]
pub trait DistributedMessageSetRpcInterface {
    /// Returns the messages except `knowns`. If the height is different, it returns `Err(height)`.
    async fn get_message(
        &self,
        height: BlockHeight,
        knowns: Vec<Hash256>,
    ) -> Result<Vec<Message>, BlockHeight>;
}

// TODO: remove this
pub trait AuthorizedNetwork: Send + Sync {}
