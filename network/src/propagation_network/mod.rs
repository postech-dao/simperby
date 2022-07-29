mod behaviour;
use crate::AuthorizedNetwork;
use async_trait::async_trait;
use behaviour::Behaviour;
use libp2p::identity::Keypair;
use simperby_common::crypto::*;
use std::net::SocketAddrV4;
use tokio::{
    sync::{mpsc, Mutex},
    task,
};

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

    /// A join handle for background network task.
    ///
    /// The task running behind this handle is the main routine of [`PropagationNetwork`].
    _task_join_handle: task::JoinHandle<()>,

    /// A message queue that collects broadcasted messages through the network.
    ///
    /// A simperby node can obtain this queue by calling [`PropagationNetwork::create_receive_queue`].
    receive_queue: mpsc::Receiver<Vec<u8>>,
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
        // Note: This is a dummy implementation.
        // Todo: Convert `public_key` into `libp2p::identity::PublicKey`,
        let local_keypair = Keypair::generate_ed25519();

        let behaviour = Mutex::new(Behaviour::new(local_keypair.public()));

        // Create a message queue that a simperby node will use to receive messages from other nodes.
        // Todo: Choose a proper buffer size for `mpsc::channel`.
        let (send_queue, receive_queue) = mpsc::channel::<Vec<u8>>(100);
        let _task_join_handle = task::spawn(run_background_task(send_queue));

        Ok(Self {
            _behaviour: behaviour,
            _task_join_handle,
            receive_queue,
        })
    }
    async fn broadcast(&self, _message: &[u8]) -> Result<(), String> {
        unimplemented!("not implemented");
    }
    async fn create_recv_queue(&self) -> Result<&mpsc::Receiver<Vec<u8>>, ()> {
        Ok(&self.receive_queue)
    }
    async fn get_live_list(&self) -> Result<Vec<PublicKey>, ()> {
        unimplemented!("not implemented");
    }
}

async fn run_background_task(send_queue: mpsc::Sender<Vec<u8>>) {
    // Note: This is a dummy logic.
    // Todo: Loop to listen on `libp2p::Swarm::SwarmEvent`.
    //       Simperby network logic will be based on SwarmEvents.
    loop {
        if send_queue
            .send("some value".as_bytes().to_vec())
            .await
            .is_err()
        {
            panic!("Receive end is closed");
        }
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
