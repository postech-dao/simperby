pub mod propagation_network;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simperby_common::crypto::*;
use std::net::SocketAddrV4;
use tokio::sync::broadcast;

pub type BroadcastToken = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastStatus {
    relayed_nodes: Vec<PublicKey>,
}

/// TODO: Provide error types.
///
/// Note: This trait is quite subject to change.
#[async_trait]
pub trait AuthorizedNetwork {
    /// Joins the network with an authorized identity.
    async fn new(
        public_key: PublicKey,
        private_key: PrivateKey,
        known_peers: Vec<PublicKey>,
        bootstrap_points: Vec<SocketAddrV4>,
        network_id: String,
    ) -> Result<Self, String>
    where
        Self: Sized;
    /// Broadcasts a message to the network, after signed by the key given to this instance.
    async fn broadcast(&self, message: &[u8]) -> Result<BroadcastToken, String>;
    /// Stops a currently broadcasting message.
    async fn stop_broadcast(&self, token: BroadcastToken) -> Result<(), String>;
    /// Gets the current status of a broadcasting message.
    async fn get_broadcast_status(&self, token: BroadcastToken) -> Result<BroadcastStatus, String>;
    /// Creates a receiver for every message broadcasted to the network, except the one sent by this instance.
    async fn create_recv_queue(&self) -> Result<broadcast::Receiver<Vec<u8>>, ()>;
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
        bootstrap_points: Vec<SocketAddrV4>,
        known_peers: Vec<PublicKey>,
        network_id: String,
    ) -> Result<Self, String>
    where
        Self: Sized;
    /// Broadcasts a message to the network.
    async fn broadcast(&self, message: &[u8]) -> Result<(), String>;
    /// Creates a receiver for every message broadcasted to the network, except the one sent by this instance.
    async fn create_recv_queue(&self) -> Result<broadcast::Receiver<Vec<u8>>, ()>;
    /// Provides the estimated list of live nodes identified by their IP addresses
    async fn get_live_list(&self) -> Result<Vec<std::net::SocketAddrV4>, ()>;
}
