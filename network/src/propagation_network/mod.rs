mod behaviour;

use super::*;
use async_trait::async_trait;
use behaviour::Behaviour;
use futures::StreamExt;
use libp2p::{
    development_transport,
    identity::{ed25519, Keypair},
    multiaddr::Protocol,
    swarm::Swarm,
    PeerId,
};
use simperby_common::crypto::*;
use std::{net::SocketAddrV4, sync::Arc, time::Duration};
use tokio::{
    sync::{broadcast, Mutex},
    task, time,
};

/// The backbone network of simperby that propagates serialized data such as blocks and votes.
///
/// This network discovers peers with Kademlia([`libp2p::kad`]),
/// and propagates data with FloodSub([`libp2p::floodsub`]).
pub struct PropagationNetwork {
    /// A join handle for background network task.
    ///
    /// The task running behind this handle is the main routine of [`PropagationNetwork`].
    _task_join_handle: task::JoinHandle<()>,

    /// A sending endpoint of the queue that collects broadcasted messages through the network
    /// and sends it to the simperby node.
    ///
    /// The receiving endpoint of the queue can be obtained using [`PropagationNetwork::create_receive_queue`].
    sender: broadcast::Sender<Vec<u8>>,

    /// A top-level network interface provided by libp2p.
    swarm: Arc<Mutex<Swarm<Behaviour>>>,
}

#[async_trait]
impl AuthorizedNetwork for PropagationNetwork {
    async fn new(
        public_key: PublicKey,
        private_key: PrivateKey,
        _known_peers: Vec<PublicKey>,
        _bootstrap_points: Vec<SocketAddrV4>,
        _network_id: String,
    ) -> Result<Self, String>
    where
        Self: Sized,
    {
        let mut keypair_bytes = private_key.as_ref().to_vec();
        keypair_bytes.extend(public_key.as_ref());
        // Todo: Handle returned error.
        let local_keypair = Keypair::Ed25519(
            ed25519::Keypair::decode(&mut keypair_bytes).expect("invalid keypair was given"),
        );
        let local_peer_id = PeerId::from(local_keypair.public());

        let behaviour = Behaviour::new(local_keypair.public());

        let transport = match development_transport(local_keypair).await {
            Ok(transport) => transport,
            // Todo: Use an error type of this crate.
            Err(_) => return Err("Failed to create a transport.".to_string()),
        };

        let swarm = Arc::new(Mutex::new(Swarm::new(transport, behaviour, local_peer_id)));
        let mut swarm_inner = swarm.lock().await;

        // Create listener(s).
        // Todo: Pass possible error to the `PropagationNetwork`.
        // Todo: Take listen address from network configurations.
        swarm_inner
            .listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
            .expect("Failed to start listening");

        // Create a message queue that a simperby node will use to receive messages from other nodes.
        // Todo: Choose a proper buffer size for the buffer size.
        let (sender, _receiver) = broadcast::channel::<Vec<u8>>(100);
        let _task_join_handle = task::spawn(run_background_task(swarm.clone(), sender.clone()));

        Ok(Self {
            _task_join_handle,
            sender,
            swarm: swarm.clone(),
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
        Ok(self.sender.subscribe())
    }
    async fn get_live_list(&self) -> Result<Vec<PublicKey>, ()> {
        unimplemented!();
    }
}

async fn run_background_task(
    swarm: Arc<Mutex<Swarm<Behaviour>>>,
    _sender: broadcast::Sender<Vec<u8>>,
) {
    // This timer guarantees that the lock for swarm will be released
    // regularly and within a finite time.
    let mut lock_release_timer = time::interval(Duration::from_millis(100));
    lock_release_timer.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

    // Todo: Bootstrap with already known addresses.
    // Todo: Create a timer for regular bootstrapping.

    loop {
        let mut swarm = swarm.lock().await;
        tokio::select! {
            // Listen on swarm events.
            _event = swarm.select_next_some() => {}
            // Release the lock so that other tasks can use swarm.
            _ = lock_release_timer.tick() => ()
        }
        // The lock for swarm is automatically released here.
    }
}

impl PropagationNetwork {
    #[allow(dead_code)]
    /// Returns the peers currently in contact.
    async fn get_connected_peers(&self) -> Vec<PeerId> {
        let swarm = self.swarm.lock().await;
        swarm.connected_peers().copied().collect()
    }

    #[allow(dead_code)]
    /// Returns the socketv4 addresses to which the listeners are bound.
    async fn get_listen_addresses(&self) -> Vec<SocketAddrV4> {
        let swarm = self.swarm.lock().await;

        // Convert `Multiaddr` into `SocketAddrV4`.
        let mut listen_addresses = Vec::new();
        for mut multiaddr in swarm.listeners().cloned() {
            let port = loop {
                if let Protocol::Tcp(port) = multiaddr.pop().expect("The node should listen on TCP")
                {
                    break port;
                }
            };
            let ipv4_addr = loop {
                if let Protocol::Ip4(ipv4_addr) =
                    multiaddr.pop().expect("The node should use IPv4 address")
                {
                    break ipv4_addr;
                }
            };
            listen_addresses.push(SocketAddrV4::new(ipv4_addr, port));
        }

        listen_addresses
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rand;
    use std::collections::HashSet;

    /// A helper struct for the tests.
    struct Node {
        public_key: PublicKey,
        private_key: PrivateKey,
        id: PeerId,
        network: Option<PropagationNetwork>,
    }

    impl Node {
        /// Generate a node with random key.
        fn new_random() -> Self {
            let seed: Vec<u8> = (0..16).map(|_| rand::random()).collect();
            let (public_key, private_key) = generate_keypair(seed);
            Self {
                id: PeerId::from(convert_public_key(&public_key, &private_key)),
                public_key,
                private_key,
                network: None,
            }
        }
    }

    /// A helper function with type conversion.
    fn convert_public_key(
        public_key: &PublicKey,
        private_key: &PrivateKey,
    ) -> libp2p::identity::PublicKey {
        let mut keypair_bytes = private_key.as_ref().to_vec();
        keypair_bytes.extend(public_key.as_ref());
        // Todo: Handle returned error.
        let keypair = Keypair::Ed25519(
            ed25519::Keypair::decode(&mut keypair_bytes).expect("invalid keypair was given"),
        );
        keypair.public()
    }

    /// A helper test function with an argument.
    async fn discovery_with_n_nodes_sequential(n: usize) {
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
            for listen_address in network.get_listen_addresses().await {
                bootstrap_points.push(listen_address);
            }
        }

        // Test if every node has filled its routing table correctly.
        for node in &nodes {
            let network = node.network.as_ref().unwrap();
            let connected_peers = network
                .get_connected_peers()
                .await
                .into_iter()
                .collect::<HashSet<PeerId>>();
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
    /// in a tiny network when they join the network sequentially.
    /// (network_size = 5 < [`libp2p::kad::K_VALUE`] = 20)
    async fn discovery_with_tiny_network_sequential() {
        discovery_with_n_nodes_sequential(5).await;
    }

    #[tokio::test]
    #[ignore]
    /// Test if every node fills its routing table with the addresses of all the other nodes
    /// in a small network when they join the network sequentially.
    /// (network_size = [`libp2p::kad::K_VALUE`] = 20)
    async fn discovery_with_small_network_sequential() {
        discovery_with_n_nodes_sequential(libp2p::kad::K_VALUE.into()).await;
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
