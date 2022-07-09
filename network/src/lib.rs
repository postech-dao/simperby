use async_trait::async_trait;
use simperby_common::types::*;
use tokio::sync::mpsc;

/// TODO: Provide error types.
///
/// Note: This trait is quite subject to change.
#[async_trait]
pub trait AuthorizedNetwork {
    /// Joins the network with an authorized identity.
    async fn new(
        public_key: PublicKey,
        private_key: PrivateKey,
        known_ids: Vec<PublicKey>,
        known_peers: Vec<std::net::SocketAddrV4>,
        network_id: String,
    ) -> Result<Self, String>
    where
        Self: Sized;
    /// Broadcasts a message to the network, after signed by the key given to this instance.
    async fn broadcast(&self, message: &[u8]) -> Result<(), String>;
    /// Creates a receiver for every message broadcasted to the network, except the one sent by this instance.
    async fn create_recv_queue(&self) -> Result<mpsc::Receiver<Vec<u8>>, ()>;
    /// Provides the estimated list of live nodes that are eligible and identified by their public keys.
    async fn get_live_list(&self) -> Result<Vec<PublicKey>, ()>;
}

/// TODO: Provide error types.
///
/// Note: This trait is quite subject to change.
#[async_trait]
pub trait UnauthorizedNetwork {
    /// Joins the network with an authorized identity.
    async fn new(
        known_peers: Vec<std::net::SocketAddrV4>,
        network_id: String,
    ) -> Result<Self, String>
    where
        Self: Sized;
    /// Broadcasts a message to the network.
    async fn broadcast(&self, message: &[u8]) -> Result<(), String>;
    /// Creates a receiver for every message broadcasted to the network, except the one sent by this instance.
    async fn create_recv_queue(&self) -> Result<mpsc::Receiver<Vec<u8>>, ()>;
    /// Provides the estimated list of live nodes identified by their IP addresses
    async fn get_live_list(&self) -> Result<Vec<std::net::SocketAddrV4>, ()>;
}
