mod behaviour;
mod config;

use super::*;
use async_trait::async_trait;
use behaviour::Behaviour;
use config::PropagationNetworkConfig;
use futures::StreamExt;
use libp2p::{
    core::ConnectedPoint,
    development_transport,
    identity::{ed25519, Keypair},
    multiaddr::{Multiaddr, Protocol},
    swarm::{dial_opts::DialOpts, Swarm, SwarmEvent},
    PeerId,
};
use simperby_common::crypto::*;
use std::{collections::HashSet, net::SocketAddrV4, sync::Arc, time::Duration};
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
        known_peers: Vec<PublicKey>,
        bootstrap_points: Vec<SocketAddrV4>,
        network_id: String,
    ) -> Result<Self, String>
    where
        Self: Sized,
    {
        let default_config = PropagationNetworkConfig::default();
        Self::with_config(
            public_key,
            private_key,
            known_peers,
            bootstrap_points,
            network_id,
            default_config,
        )
        .await
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

impl PropagationNetwork {
    pub async fn with_config(
        public_key: PublicKey,
        private_key: PrivateKey,
        _known_peers: Vec<PublicKey>,
        bootstrap_points: Vec<SocketAddrV4>,
        _network_id: String,
        config: PropagationNetworkConfig,
    ) -> Result<Self, String> {
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

        // Create a listener.
        // Note: A single listener can have multiple listen addresses.
        // Todo: Pass possible error to the `PropagationNetwork`.
        swarm_inner
            .listen_on(config.listen_address)
            .expect("Failed to create a listener.");

        let swarm_event = time::timeout(
            config.listener_creation_timeout,
            swarm_inner.select_next_some(),
        )
        .await
        .expect("Failed to create listener before the timeout");

        if let SwarmEvent::NewListenAddr { .. } = swarm_event {
        } else {
            unreachable!("The first SwarmEvent must be NewListenAddr.")
        }

        // Add bootstrap nodes.
        let mut bootstrap_addresses: HashSet<Multiaddr> = bootstrap_points
            .iter()
            .map(|socket_addr_v4| {
                Multiaddr::from_iter(
                    vec![
                        Protocol::Ip4(*socket_addr_v4.ip()),
                        Protocol::Tcp(socket_addr_v4.port()),
                    ]
                    .into_iter(),
                )
            })
            .collect();

        let initial_bootstrap = async {
            // Keep dialing until we reach all given peers before the timeout.
            while !bootstrap_addresses.is_empty() {
                let mut checked_dials = 0;
                let mut outgoing_dials = 0;
                for address in &bootstrap_addresses {
                    if swarm_inner
                        .dial(DialOpts::unknown_peer_id().address(address.clone()).build())
                        .is_ok()
                    {
                        outgoing_dials += 1;
                    }
                }
                while checked_dials < outgoing_dials {
                    match swarm_inner.select_next_some().await {
                        // Successfully dialed to a peer.
                        SwarmEvent::ConnectionEstablished {
                            peer_id,
                            endpoint: ConnectedPoint::Dialer { address, .. },
                            ..
                        } => {
                            // Add every node dialed successfully to regular bootstrap targets.
                            swarm_inner
                                .behaviour_mut()
                                .kademlia
                                .add_address(&peer_id, address.clone());
                            bootstrap_addresses.remove(&address);
                            checked_dials += 1;
                        }
                        // Dialed a peer but it failed.
                        SwarmEvent::OutgoingConnectionError { .. } => {
                            checked_dials += 1;
                        }
                        _ => {}
                    }
                }
            }
        };
        // Todo: Propagate error only if `bootstrap_points.len()` != 0.
        let _ = time::timeout(config.initial_bootstrap_timeout, initial_bootstrap).await;

        // Create a message queue that a simperby node will use to receive messages from other nodes.
        let (sender, _receiver) = broadcast::channel::<Vec<u8>>(config.message_queue_capacity);

        let _task_join_handle = task::spawn(run_background_task(
            swarm.clone(),
            sender.clone(),
            config.lock_release_interval,
            config.peer_discovery_interval,
        ));

        Ok(Self {
            _task_join_handle,
            sender,
            swarm: swarm.clone(),
        })
    }
}

async fn run_background_task(
    swarm: Arc<Mutex<Swarm<Behaviour>>>,
    _sender: broadcast::Sender<Vec<u8>>,
    lock_release_interval: Duration,
    bootstrap_interval: Duration,
) {
    // This timer guarantees that the lock for swarm will be released
    // regularly and within a finite time.
    let mut lock_release_timer = time::interval(lock_release_interval);
    lock_release_timer.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

    let mut bootstrap_timer = time::interval(bootstrap_interval);

    loop {
        let mut swarm = swarm.lock().await;
        tokio::select! {
            // Get k-closest peers after every preset interval.
            // Todo: Update Floodsub publish targets to match them with the k-closest peers.
            _ = bootstrap_timer.tick() => {
                // Note: An `Err` is returned only if there is no known peer,
                //       which is not considered to be an error if this node is
                //       the first one to join the network.
                let _ =  swarm.behaviour_mut().kademlia.bootstrap();
            }
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
    use futures::future::join_all;
    use rand::{self, seq::IteratorRandom};
    use std::{collections::HashSet, iter::zip, net::Ipv4Addr};
    use tokio::sync::OnceCell;

    static CELL: OnceCell<PortDispenser> = OnceCell::const_new();

    /// A helper struct for the test.
    ///
    /// Assigns unique network ports when requested by the test functions.
    /// Note that an assigned port doesn't mean a usable port.
    /// Assigned ports can be used by dial attempts or other processes.
    struct PortDispenser {
        // 10000 ~ 65535
        available_ports: Mutex<HashSet<u16>>,
    }

    impl PortDispenser {
        async fn new() -> Self {
            Self {
                available_ports: Mutex::new(HashSet::from_iter(10000..u16::MAX)),
            }
        }

        async fn get_random_ports(&self, n: usize) -> Result<Vec<u16>, ()> {
            let mut available_ports = self.available_ports.lock().await;
            let mut rng = rand::thread_rng();
            let assigned_ports = available_ports.iter().copied().choose_multiple(&mut rng, n);
            for port in &assigned_ports {
                available_ports.remove(port);
            }
            if assigned_ports.len() == n {
                Ok(assigned_ports)
            } else {
                Err(())
            }
        }
    }

    async fn get_random_ports(n: usize) -> Vec<u16> {
        let port_dispenser = CELL.get_or_init(PortDispenser::new).await;
        port_dispenser
            .get_random_ports(n)
            .await
            .expect("Failed to get enough number of ports.")
    }

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

    /// A helper function that checks each node's routing table
    /// whether it has all the peers on the same network.
    ///
    /// Panics if the routing table does not match with expectation.
    async fn check_routing_table(nodes: &Vec<Node>) {
        for node in nodes {
            let network = node.network.as_ref().unwrap();
            let connected_peers = network
                .get_connected_peers()
                .await
                .into_iter()
                .collect::<HashSet<PeerId>>();
            for peer in nodes {
                if peer.id == node.id {
                    continue;
                }
                assert!(connected_peers.contains(&peer.id));
            }
        }
    }

    /// A helper test function with an argument.
    async fn discovery_with_n_nodes_sequential(n: usize) {
        let mut nodes: Vec<Node> = (0..n).map(|_| Node::new_random()).collect();
        let mut bootstrap_points = Vec::new();

        // Create n nodes sequentially.
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
        check_routing_table(&nodes).await
    }

    /// A helper test function with an argument.
    async fn discovery_with_n_nodes_concurrent(n: usize) {
        let mut nodes: Vec<Node> = (0..n).map(|_| Node::new_random()).collect();

        // Assign unique ports to listen to.
        let mut listen_addresses = Vec::new();
        for _ in 0..n {
            let port = *get_random_ports(1)
                .await
                .get(0)
                .expect("Failed to assign a port for a node.");
            let listen_address = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), port);
            listen_addresses.push(listen_address);
        }

        // Create n nodes asynchronously.
        let futures = zip(&mut nodes, &listen_addresses).map(|(node, listen_address)| {
            let mut config = PropagationNetworkConfig::default();
            config.with_listen_address(listen_address.clone());
            PropagationNetwork::with_config(
                node.public_key.clone(),
                node.private_key.clone(),
                Vec::new(),
                listen_addresses.clone(),
                "test".to_string(),
                config,
            )
        });

        let networks = join_all(futures)
            .await
            .into_iter()
            .map(|result| result.expect("Failed to construct PropagationNetwork."));

        for (node, network) in zip(&mut nodes, networks) {
            node.network = Some(network);
        }

        // Test if every node has filled its routing table correctly.
        check_routing_table(&nodes).await
    }

    #[tokio::test]
    /// Test if every node fills its routing table with the addresses of all the other nodes
    /// in a tiny network when they join the network sequentially.
    /// (network_size = 5 < [`libp2p::kad::K_VALUE`] = 20)
    async fn discovery_with_tiny_network_sequential() {
        discovery_with_n_nodes_sequential(5).await;
    }

    #[tokio::test]
    /// Test if every node fills its routing table with the addresses of all the other nodes
    /// in a small network when they join the network sequentially.
    /// (network_size = [`libp2p::kad::K_VALUE`] = 20)
    async fn discovery_with_small_network_sequential() {
        discovery_with_n_nodes_sequential(libp2p::kad::K_VALUE.into()).await;
    }

    #[tokio::test]
    /// Test if every node fills its routing table with the addresses of all the other nodes
    /// in a tiny network when they join the network at the same time.
    /// (network_size = 5 < [`libp2p::kad::K_VALUE`] = 20)
    async fn discovery_with_tiny_network_concurrent() {
        discovery_with_n_nodes_concurrent(5).await;
    }

    #[tokio::test]
    /// Test if every node fills its routing table with the addresses of all the other nodes
    /// in a small network when they join the network at the same time.
    /// (network_size = [`libp2p::kad::K_VALUE`] = 20)
    async fn discovery_with_small_network_concurrent() {
        discovery_with_n_nodes_concurrent(libp2p::kad::K_VALUE.into()).await;
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
