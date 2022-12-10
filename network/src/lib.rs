pub mod dms;
#[cfg(never)]
mod peer_discovery;
pub mod primitives;
pub mod storage;

use async_trait::async_trait;
use primitives::*;
use serde::{Deserialize, Serialize};
use simperby_common::{crypto::*, MemberName, Timestamp};
use std::collections::HashMap;
use std::{net::SocketAddrV4, sync::Arc};
use tokio::sync::RwLock;

pub type Error = eyre::Error;
pub type Dms = dms::DistributedMessageSet<primitives::DummyGossipNetwork, storage::StorageImpl>;

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

/// Configuration to access the Simperby P2P network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// The unique id for distinguishing the network.
    pub network_id: String,
    /// The map of `identifier->port` where an `identifier` represent each network services
    /// (.e.g, gossip-consensus, RPC-governance, discovery, ..)
    pub ports: HashMap<String, u16>,
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
    /// It is not constantly updated once created
    pub fn new_static(peers: Vec<Peer>) -> Self {
        Self {
            lock: Arc::new(RwLock::new(peers)),
        }
    }

    pub fn new(lock: Arc<RwLock<Vec<Peer>>>) -> Self {
        Self { lock }
    }

    pub async fn read(&self) -> Vec<Peer> {
        self.lock.read().await.clone()
    }

    pub async fn add_or_replace(&self, peer: Peer) {
        let mut known_peers = self.lock.write().await;
        let index = known_peers
            .iter()
            .position(|known_peer| known_peer.public_key == peer.public_key);
        match index {
            Some(index) => known_peers[index] = peer,
            None => known_peers.push(peer),
        }
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

/// A general handle for self-serving objects.
pub struct Serve<T, E> {
    task: tokio::task::JoinHandle<Result<T, E>>,
    termination_switch: tokio::sync::oneshot::Sender<()>,
    read_only_lock: Arc<RwLock<T>>,
}

impl<T, E> Serve<T, E> {
    pub fn new(
        task: tokio::task::JoinHandle<Result<T, E>>,
        termination_switch: tokio::sync::oneshot::Sender<()>,
        copy: Arc<RwLock<T>>,
    ) -> Self {
        Self {
            task,
            termination_switch,
            read_only_lock: copy,
        }
    }

    /// Join the serve task after triggering the termination switch.
    pub async fn join(self) -> Result<Result<T, E>, tokio::task::JoinError> {
        // drop the read-only lock to make `Arc::try_unwrap()` from the serve side succeed
        drop(self.read_only_lock);
        let _ = self.termination_switch.send(());
        self.task.await
    }

    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<T> {
        self.read_only_lock.read().await
    }
}
