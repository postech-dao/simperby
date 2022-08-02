mod behaviour;

use super::*;
use async_trait::async_trait;
use behaviour::Behaviour;
use libp2p::{development_transport, identity::Keypair, swarm::Swarm, PeerId};
use simperby_common::crypto::*;
use std::net::SocketAddrV4;
use tokio::{
    sync::{broadcast, Mutex},
    task,
};

/// The backbone network of simperby that propagates serialized data such as blocks and votes.
///
/// This network discovers peers with Kademlia([`libp2p::kad`]),
/// and propagates data with FloodSub([`libp2p::floodsub`]).
pub struct PropagationNetwork {
    /// A libp2p network status manager.
    ///
    /// Contains the state of the network and the way it should behave.
    _swarm: Mutex<Swarm<Behaviour>>,

    /// A join handle for background network task.
    ///
    /// The task running behind this handle is the main routine of [`PropagationNetwork`].
    _task_join_handle: task::JoinHandle<()>,

    /// A sending endpoint of the queue that collects broadcasted messages through the network
    /// and sends it to the simperby node.
    ///
    /// The receiving endpoint of the queue can be obtained using [`PropagationNetwork::create_receive_queue`].
    sender: broadcast::Sender<Vec<u8>>,
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
        let local_peer_id = PeerId::from(local_keypair.public());

        let behaviour = Behaviour::new(local_keypair.public());

        let transport = match development_transport(local_keypair).await {
            Ok(transport) => transport,
            // Todo: Use an error type of this crate.
            Err(_) => return Err("Failed to create a transport.".to_string()),
        };
        let swarm = Mutex::new(Swarm::new(transport, behaviour, local_peer_id));

        // Create a message queue that a simperby node will use to receive messages from other nodes.
        // Todo: Choose a proper buffer size for `mpsc::channel`.
        let (sender, _receiver) = broadcast::channel::<Vec<u8>>(100);
        let _task_join_handle = task::spawn(run_background_task(sender.clone()));

        Ok(Self {
            _swarm: swarm,
            _task_join_handle,
            sender,
        })
    }
    async fn broadcast(&self, _message: &[u8]) -> Result<BroadcastToken, String> {
        unimplemented!();
    }
    async fn stop_broadcast(&self, _token: BroadcastToken) -> Result<(), String> {
        unimplemented!();
    }
    async fn get_broadcast_status(
        &self,
        _token: BroadcastToken,
    ) -> Result<BroadcastStatus, String> {
        unimplemented!();
    }
    async fn create_recv_queue(&self) -> Result<broadcast::Receiver<Vec<u8>>, ()> {
        // pub fn subscribe(&self) -> Receiver<T>
        Ok(self.sender.subscribe())
    }
    async fn get_live_list(&self) -> Result<Vec<PublicKey>, ()> {
        unimplemented!();
    }
}

async fn run_background_task(send_queue: broadcast::Sender<Vec<u8>>) {
    // Note: This is a dummy logic.
    // Todo: Loop to listen on `libp2p::Swarm::SwarmEvent`.
    //       Simperby network logic will be based on SwarmEvents.
    loop {
        if send_queue.send("some value".as_bytes().to_vec()).is_err() {
            panic!("Receive end is closed");
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use libp2p::{multiaddr::Protocol, PeerId};
    use rand;
    use simperby_common::crypto;
    use std::collections::HashSet;
    use std::thread::sleep;
    use std::time::Duration;
    use tokio;

    /// A helper struct for the tests.
    struct Node {
        public_key: crypto::PublicKey,
        private_key: crypto::PrivateKey,
        id: PeerId,
        network: Option<PropagationNetwork>,
    }

    impl Node {
        /// Generate a node with random key.
        fn new_random() -> Self {
            let seed: Vec<u8> = (0..16).map(|_| rand::random()).collect();
            let (public_key, private_key) = simperby_common::crypto::generate_keypair(seed);
            Self {
                id: PeerId::from(convert_public_key(&public_key)),
                public_key,
                private_key,
                network: None,
            }
        }
    }

    /// A helper function with type conversion.
    fn convert_public_key(_simperby_public_key: &crypto::PublicKey) -> libp2p::identity::PublicKey {
        unimplemented!();
    }

    /// A helper test function with an argument.
    async fn discovery_with_n_nodes(n: usize, sleep_duration: Duration) {
        let mut nodes: Vec<Node> = (0..n).map(|_| Node::new_random()).collect();
        let mut bootstrap_points = Vec::new();

        // Create n nodes.
        for i in 0..n {
            let node = nodes.get_mut(i).unwrap();
            let network = PropagationNetwork::new(
                node.public_key.clone(),
                node.private_key.clone(),
                Vec::new(),
                bootstrap_points.clone(),
                "test".to_string(),
            )
            .await
            .expect("Failed to construct PropagationNetwork");
            node.network = Some(network);

            // Add newly joined node to the bootstrap points.
            let network = node.network.as_ref().unwrap();
            let swarm = network._swarm.lock().await;
            for multiaddr in swarm.listeners() {
                let mut multiaddr = multiaddr.clone();
                let port = loop {
                    if let Protocol::Tcp(port) = multiaddr.pop().expect("It should listen on TCP") {
                        break port;
                    }
                };
                let ipv4_addr = loop {
                    if let Protocol::Ip4(ipv4_addr) =
                        multiaddr.pop().expect("It should use IPv4 address")
                    {
                        break ipv4_addr;
                    }
                };
                let address = SocketAddrV4::new(ipv4_addr, port);
                bootstrap_points.push(address);
            }
        }

        // Wait for a possibly short duration for the nodes to finish bootstrapping.
        sleep(sleep_duration);

        // Test if every node has filled its routing table correctly.
        for node in &nodes {
            let network = node.network.as_ref().unwrap();
            let swarm = network._swarm.lock().await;
            let connected_peers = swarm.connected_peers().collect::<HashSet<&PeerId>>();
            for peer in &nodes {
                if peer.id == node.id {
                    continue;
                }
                assert!(connected_peers.contains(&peer.id));
            }
        }
    }

    #[tokio::test]
    #[ignore]
    /// Test if every node fills its routing table with the addresses of all the other nodes
    /// in a tiny network (network_size = 5 < [`libp2p::kad::K_VALUE`] = 20).
    async fn discovery_with_tiny_network() {
        discovery_with_n_nodes(5, Duration::from_secs(1)).await;
    }

    #[tokio::test]
    #[ignore]
    /// Test if every node fills its routing table with the addresses of all the other nodes
    /// in a small network (network_size = [`libp2p::kad::K_VALUE`] = 20).
    async fn discovery_with_small_network() {
        discovery_with_n_nodes(libp2p::kad::K_VALUE.into(), Duration::from_secs(1)).await;
    }

    #[tokio::test]
    #[ignore]
    /// Test if all nodes receive a message from a single broadcasting node.
    async fn broadcast_once() {
        unimplemented!();
    }

    #[tokio::test]
    #[ignore]
    /// Test if all nodes receive multiple messages from a single broadcasting node.
    async fn broadcast_multiple_times() {
        unimplemented!();
    }

    #[tokio::test]
    #[ignore]
    /// Test if all nodes receives multiple messages from multiple broadcasting nodes.
    async fn broadcast_from_multiple_nodes() {
        unimplemented!();
    }

    #[tokio::test]
    #[ignore]
    /// Test if all nodes receives multiple messages from multiple broadcasting nodes
    /// when several nodes are joining and leaving the network.
    async fn broadcast_from_multiple_nodes_with_flexible_network() {
        unimplemented!();
    }

    #[tokio::test]
    #[ignore]
    /// Test if all nodes correctly retrieve the list of all nodes in the network.
    async fn get_live_list_once() {
        unimplemented!();
    }

    #[tokio::test]
    #[ignore]
    /// Test if all nodes correctly retrieve the list of all nodes in the network multiple times
    /// with several time intervals.
    async fn get_live_list_multiple_times() {
        unimplemented!();
    }

    #[tokio::test]
    #[ignore]
    /// Test if all nodes correctly retrieve lists of all nodes in the network
    /// whenever several nodes join and leave the network.
    async fn get_live_list_with_flexible_network() {
        unimplemented!();
    }
}
