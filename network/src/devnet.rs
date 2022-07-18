use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::AuthorizedNetwork;
use simperby_common::crypto::*;

/// An instance of `simperby::network::AuthorizedNetwork`
struct DevNet {}

#[async_trait]
impl AuthorizedNetwork for DevNet {
    /// Joins the network with an authorized identity.
    async fn new(
        _public_key: PublicKey,
        _private_key: PrivateKey,
        _known_ids: Vec<PublicKey>,
        _known_peers: Vec<std::net::SocketAddrV4>,
        _network_id: String,
    ) -> Result<Self, String>
    where
        Self: Sized,
    {
        unimplemented!("not implemented");
    }
    /// Broadcasts a message to the network, after signed by the key given to this instance.
    async fn broadcast(&self, _message: &[u8]) -> Result<(), String> {
        unimplemented!("not implemented");
    }
    /// Creates a receiver for every message broadcasted to the network, except the one sent by this instance.
    async fn create_recv_queue(&self) -> Result<mpsc::Receiver<Vec<u8>>, ()> {
        unimplemented!("not implemented");
    }
    /// Provides the estimated list of live nodes that are eligible and identified by their public keys.
    async fn get_live_list(&self) -> Result<Vec<PublicKey>, ()> {
        unimplemented!("not implemented");
    }
}
