use super::*;
use async_trait::async_trait;
use tokio::sync::mpsc;

pub type StroageError = std::io::Error;

/// An abstraction of the synchronized storage backed by the host file system.
#[async_trait]
pub trait Storage: Send + Sync + 'static {
    /// Creates a new and empty directory.
    /// Fails if there is already a directory.
    async fn create(storage_directory: &str) -> Result<(), StroageError>;

    /// Opens an existing directory.
    async fn open(storage_directory: &str) -> Result<Self, StroageError>
    where
        Self: Sized;

    /// Shows the list of files.
    async fn list_files(&self) -> Result<Vec<String>, StroageError>;

    /// Add the given file to the storage.
    async fn add_or_overwrite_file(
        &mut self,
        path: String,
        content: String,
    ) -> Result<(), StroageError>;

    /// Reads the given file.
    async fn read_file(&self, path: &str) -> Result<String, StroageError>;

    /// Removes the given file.
    async fn remove_file(&mut self, path: &str) -> Result<(), StroageError>;
}

#[async_trait]
pub trait PeerDiscoveryPrimitive: Send + Sync + 'static {
    /// Remains online on the network indefinitely,
    /// responding to discovery requests from other nodes,
    /// updating `known_peers`.
    async fn serve(
        network_config: NetworkConfig,
        initially_known_peers: Vec<Peer>,
    ) -> Result<(SharedKnownPeers, tokio::task::JoinHandle<Result<(), Error>>), Error>;
}

/// The p2p gossip network.
#[async_trait]
pub trait P2PNetwork: Send + Sync + 'static {
    /// Broadcasts a message to the network.
    async fn broadcast(
        config: &NetworkConfig,
        known_peers: &[Peer],
        message: Vec<u8>,
    ) -> Result<(), Error>;

    /// Remains online on the network indefinitely,
    /// relaying (propagating) messages broadcasted over the network.
    async fn serve(
        config: NetworkConfig,
        peers: SharedKnownPeers,
    ) -> Result<
        (
            mpsc::Receiver<Vec<u8>>,
            tokio::task::JoinHandle<Result<(), Error>>,
        ),
        Error,
    >;
}
