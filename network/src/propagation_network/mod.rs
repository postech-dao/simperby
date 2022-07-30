mod behaviour;

use super::*;
use async_trait::async_trait;
use behaviour::Behaviour;
use simperby_common::crypto::*;
use std::net::SocketAddrV4;
use tokio::sync::{mpsc, Mutex};

/// The backbone network of simperby that propagates serialized data such as blocks and votes.
///
/// This network discovers peers with Kademlia([`libp2p::kad`]),
/// and propagates data with FloodSub([`libp2p::floodsub`]).
pub struct PropagationNetwork {
    /// A custom libp2p network behaviour.
    ///
    /// It collects other network behaviours to extend their functionalities,
    /// and implements [`libp2p::swarm::NetworkBehaviour`] as well.
    _behaviour: Mutex<Behaviour>,
}

#[async_trait]
impl AuthorizedNetwork for PropagationNetwork {
    async fn new(
        _public_key: PublicKey,
        _private_key: PrivateKey,
        _known_peers: Vec<PublicKey>,
        _bootstrap_points: Vec<SocketAddrV4>,
        _network_id: String,
    ) -> Result<Self, String>
    where
        Self: Sized,
    {
        unimplemented!("not implemented");
    }
    async fn broadcast(&self, _message: &[u8]) -> Result<BroadcastToken, String> {
        unimplemented!("not implemented");
    }
    async fn stop_braodcast(&self, _token: BroadcastToken) -> Result<(), String> {
        unimplemented!();
    }
    async fn get_broadcast_status(
        &self,
        _token: BroadcastToken,
    ) -> Result<BroadcastStatus, String> {
        unimplemented!();
    }
    async fn create_recv_queue(&self) -> Result<mpsc::Receiver<Vec<u8>>, ()> {
        unimplemented!("not implemented");
    }
    async fn get_live_list(&self) -> Result<Vec<PublicKey>, ()> {
        unimplemented!("not implemented");
    }
}

#[cfg(test)]
mod test {
    #[test]
    #[ignore]
    /// Test if all nodes receive a message from a single broadcasting node.
    fn broadcast_once() {
        unimplemented!("not implemented");
    }

    #[test]
    #[ignore]
    /// Test if all nodes receive multiple messages from a single broadcasting node.
    fn broadcast_multiple_times() {
        unimplemented!("not implemented");
    }

    #[test]
    #[ignore]
    /// Test if all nodes receives multiple messages from multiple broadcasting nodes.
    fn broadcast_from_multiple_nodes() {
        unimplemented!("not implemented");
    }

    #[test]
    #[ignore]
    /// Test if all nodes receives multiple messages from multiple broadcasting nodes
    /// when several nodes are joining and leaving the network.
    fn broadcast_from_multiple_nodes_with_flexible_network() {
        unimplemented!("not implemented");
    }

    #[test]
    #[ignore]
    /// Test if all nodes correctly retrieve the list of all nodes in the network.
    fn get_live_list_once() {
        unimplemented!("not implemented");
    }

    #[test]
    #[ignore]
    /// Test if all nodes correctly retrieve the list of all nodes in the network multiple times
    /// with several time intervals.
    fn get_live_list_multiple_times() {
        unimplemented!("not implemented");
    }

    #[test]
    #[ignore]
    /// Test if all nodes correctly retrieve lists of all nodes in the network
    /// whenever several nodes join and leave the network.
    fn get_live_list_with_flexible_network() {
        unimplemented!("not implemented");
    }
}
