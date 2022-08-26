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
    task, time::{self, sleep},
};

/// The backbone network of simperby that propagates serialized data such as blocks and votes.
///
/// This network discovers peers with Kademlia([`libp2p::kad`]),
/// and propagates data with FloodSub([`libp2p::floodsub`]).
pub struct PropagationNetwork {
    /// The network event handling routine.
    _event_handling_task: task::JoinHandle<()>,

    /// The network bootstrapping (node discovery) routine.
    _peer_discovery_task: task::JoinHandle<()>,

    /// The sending endpoint of the queue that collects broadcasted messages through the network
    /// and sends it to the simperby node.
    ///
    /// The receiving endpoint of the queue can be obtained using [`PropagationNetwork::create_receive_queue`].
    sender: broadcast::Sender<Vec<u8>>,

    /// The top-level network interface provided by libp2p.
    _swarm: Arc<Mutex<Swarm<Behaviour>>>,
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
        // Convert a simperby keypair into a libp2p keypair.
        let keypair = Self::convert_keypair(public_key, private_key)?;

        // Create swarm and do a series of jobs with configurable timeouts.
        let mut swarm = Self::create_swarm(keypair).await?;
        Self::create_listener(&mut swarm, &config).await?;
        Self::bootstrap(&mut swarm, &config, bootstrap_points).await?;

        // Wrap swarm to share it safely.
        let swarm_mutex = Arc::new(Mutex::new(swarm));

        // Create a message queue that a simperby node will use to receive messages from other nodes.
        let (sender, _receiver) = broadcast::channel::<Vec<u8>>(config.message_queue_capacity);

        let _event_handling_task = task::spawn(run_event_handling_task(
            Arc::clone(&swarm_mutex),
            config.lock_release_interval,
        ));

        let _peer_discovery_task = task::spawn(run_peer_discovery_task(
            Arc::clone(&swarm_mutex),
            config.peer_discovery_interval,
        ));

        Ok(Self {
            _event_handling_task,
            _peer_discovery_task,
            sender,
            _swarm: swarm_mutex,
        })
    }

    /// Converts simperby pub/priv keys into a libp2p keypair.
    fn convert_keypair(public_key: PublicKey, private_key: PrivateKey) -> Result<Keypair, String> {
        let mut keypair_bytes = private_key.as_ref().to_vec();
        keypair_bytes.extend(public_key.as_ref());
        if let Ok(keypair_inner) = ed25519::Keypair::decode(&mut keypair_bytes) {
            Ok(Keypair::Ed25519(keypair_inner))
        } else {
            Err("invalid public/private keypair was given.".to_string())
        }
    }

    /// Creates a swarm with given keypair.
    async fn create_swarm(keypair: Keypair) -> Result<Swarm<Behaviour>, String> {
        let transport = match development_transport(keypair.clone()).await {
            Ok(transport) => transport,
            Err(_) => return Err("failed to create a transport.".to_string()),
        };
        let behaviour = Behaviour::new(keypair.public());
        let local_peer_id = PeerId::from(keypair.public());

        Ok(Swarm::new(transport, behaviour, local_peer_id))
    }

    /// Creates a listener for incoming connection requests.
    /// Note that a single listener can have multiple listen addresses.
    async fn create_listener(
        swarm: &mut Swarm<Behaviour>,
        config: &PropagationNetworkConfig,
    ) -> Result<(), String> {
        if swarm.listen_on(config.listen_address.to_owned()).is_err() {
            return Err("failed to create a listener.".to_string());
        }

        let swarm_event =
            match time::timeout(config.listener_creation_timeout, swarm.select_next_some()).await {
                Ok(e) => e,
                Err(_) => return Err("failed to create listener before the timeout".to_string()),
            };

        if let SwarmEvent::NewListenAddr { .. } = swarm_event {
            Ok(())
        } else {
            unreachable!("the first SwarmEvent must be NewListenAddr.")
        }
    }

    /// Carries out an initial bootstrap by dialing given peers to establish connections.
    async fn bootstrap(
        swarm: &mut Swarm<Behaviour>,
        config: &PropagationNetworkConfig,
        bootstrap_points: Vec<SocketAddrV4>,
    ) -> Result<(), String> {
        // The first node of a network cannot have a bootstrap point.
        if bootstrap_points.is_empty() {
            return Ok(());
        }

        let mut bootstrap_addresses: HashSet<Multiaddr> = bootstrap_points
            .iter()
            .map(|socket_addr_v4| {
                Multiaddr::from_iter([
                    Protocol::Ip4(*socket_addr_v4.ip()),
                    Protocol::Tcp(socket_addr_v4.port()),
                ])
            })
            .collect();

        let initial_bootstrap = async {
            // Keep dialing until we reach all given peers before the timeout.
            while !bootstrap_addresses.is_empty() {
                let mut checked_dials = 0;
                let outgoing_dials = bootstrap_addresses
                    .iter()
                    .filter_map(|address| {
                        swarm
                            .dial(DialOpts::unknown_peer_id().address(address.clone()).build())
                            .ok()
                    })
                    .count();

                while checked_dials < outgoing_dials {
                    match swarm.select_next_some().await {
                        // Successfully dialed to a peer.
                        SwarmEvent::ConnectionEstablished {
                            peer_id,
                            endpoint: ConnectedPoint::Dialer { address, .. },
                            ..
                        } => {
                            // Add every node dialed successfully to regular bootstrap targets.
                            swarm
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
                // We've handled and ignored successes and failures.
                // Wait and loop to try again for failed attempts.
                sleep(Duration::from_millis(500)).await;
            }
        };
        let _ = time::timeout(config.initial_bootstrap_timeout, initial_bootstrap).await;

        // If no node was added, return an error.
        // Ignore the timeout if we've reached at least one node.
        if bootstrap_points.len() == bootstrap_addresses.len() {
            Err("could not connect to any of the bootstrap points.".to_string())
        } else {
            Ok(())
        }
    }
}

async fn run_peer_discovery_task(
    swarm: Arc<Mutex<Swarm<Behaviour>>>,
    bootstrap_interval: Duration,
) {
    loop {
        // Note: An `Err` is returned only if there is no known peer,
        //       which is not considered to be an error if this node is
        //       the first one to join the network. Thus we discard the result.
        let _ = swarm.lock().await.behaviour_mut().kademlia.bootstrap();
        tokio::time::sleep(bootstrap_interval).await;
    }
}

async fn run_event_handling_task(
    swarm: Arc<Mutex<Swarm<Behaviour>>>,
    lock_release_interval: Duration,
) {
    // This timer guarantees that the lock for swarm will be released
    // regularly and within a finite time.
    let mut lock_release_timer = time::interval(lock_release_interval);
    lock_release_timer.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    loop {
        let mut swarm = swarm.lock().await;
        tokio::select! {
            _item = swarm.select_next_some() => {
                // do something with item
            },
            _ = lock_release_timer.tick() => (),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use futures::future::join_all;
    use port_scanner::local_ports_available;
    use rand::{self, seq::IteratorRandom};
    use std::{
        collections::{HashMap, HashSet},
        iter::zip,
        net::Ipv4Addr,
    };
    use tokio::sync::OnceCell;

    impl PropagationNetwork {
        /// Returns the peers currently in contact.
        async fn get_connected_peers(&self) -> Vec<PeerId> {
            let swarm = self._swarm.lock().await;
            swarm.connected_peers().copied().collect()
        }

        /// Returns the socketv4 addresses to which the listeners are bound.
        async fn get_listen_addresses(&self) -> Vec<SocketAddrV4> {
            let swarm = self._swarm.lock().await;

            // Convert `Multiaddr` into `SocketAddrV4`.
            let mut listen_addresses = Vec::new();
            for mut multiaddr in swarm.listeners().cloned() {
                let port = loop {
                    if let Protocol::Tcp(port) =
                        multiaddr.pop().expect("the node should listen on TCP.")
                    {
                        break port;
                    }
                };
                let ipv4_addr = loop {
                    if let Protocol::Ip4(ipv4_addr) =
                        multiaddr.pop().expect("the node should use IPv4 address.")
                    {
                        break ipv4_addr;
                    }
                };
                listen_addresses.push(SocketAddrV4::new(ipv4_addr, port));
            }
            listen_addresses
        }
    }

    /// Constructs a network configuration for the test environment.
    fn create_testnet_config() -> PropagationNetworkConfig {
        let mut config = PropagationNetworkConfig::default();
        // Ease the bootstrap timeout, considering the test environment
        // where many nodes are concurrently running and dialing to each other.
        // The default is 3 seconds in a normal condition.
        // Note: This value may increase as more tests are added.
        config.with_initial_bootstrap_timeout(Duration::from_secs(20));
        config
    }

    static CELL: OnceCell<PortDispenser> = OnceCell::const_new();

    /// A helper struct for the test.
    ///
    /// Assigns unique network ports when requested by the test functions.
    /// Note that an assigned port doesn't mean a usable port.
    /// Assigned ports can be used by dial attempts or other processes.
    struct PortDispenser {
        // 55000 ~ 57000
        available_ports: Mutex<HashSet<u16>>,
    }

    impl PortDispenser {
        async fn new() -> Self {
            Self {
                available_ports: Mutex::new(HashSet::from_iter(55000..57000)),
            }
        }

        async fn get_random_ports(&self, n: usize) -> Result<Vec<u16>, ()> {
            let mut available_ports = self.available_ports.lock().await;
            let mut rng = rand::thread_rng();
            let mut assigned_ports = Vec::new();
            while assigned_ports.len() < n {
                let random_ports = available_ports
                    .iter()
                    .copied()
                    .choose_multiple(&mut rng, n - assigned_ports.len());
                // There is no available port.
                if random_ports.is_empty() {
                    return Err(());
                }
                // Exclude ports that are and will be in use from the reserved ports.
                for port in &random_ports {
                    available_ports.remove(port);
                }
                assigned_ports.extend(local_ports_available(random_ports).iter());
            }
            Ok(assigned_ports)
        }
    }

    async fn get_random_ports(n: usize) -> Vec<u16> {
        let port_dispenser = CELL.get_or_init(PortDispenser::new).await;
        port_dispenser
            .get_random_ports(n)
            .await
            .expect("failed to get enough number of ports.")
    }

    type NodeKey = PeerId;

    /// A helper struct for the tests.
    struct Node {
        public_key: PublicKey,
        private_key: PrivateKey,
        id: PeerId,
        network: Option<PropagationNetwork>,
    }

    /// A helper struct that represents the network of its inner nodes.
    struct Network {
        nodes: HashMap<NodeKey, Node>,
    }

    impl Node {
        /// Generates a node with random key.
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

    impl Network {
        fn new() -> Self {
            Self {
                nodes: HashMap::new(),
            }
        }

        /// Creates a node, but does not make it join the network.
        fn create_node(&mut self) -> NodeKey {
            let node = Node::new_random();
            let key = node.id;
            self.nodes.insert(key, node);
            key
        }

        /// Creates n nodes.
        fn create_nodes(&mut self, n: usize) -> Vec<NodeKey> {
            (0..n).map(|_| self.create_node()).collect()
        }

        /// Returns nodes that have joined the network.
        fn get_nodes_in_network(&self) -> Vec<NodeKey> {
            self.nodes
                .iter()
                .filter(|(_, node)| node.network.is_some())
                .map(|(&key, _)| key)
                .collect()
        }

        /// Returns nodes that have joined the network.
        fn get_nodes_not_in_network(&self) -> Vec<NodeKey> {
            self.nodes
                .iter()
                .filter(|(_, node)| node.network.is_none())
                .map(|(&key, _)| key)
                .collect()
        }

        /// Adds all nodes to the network that haven't joined it yet.
        async fn add_nodes_to_network_sequential(
            &mut self,
            max_bootstrap_points: usize,
        ) -> Result<Vec<NodeKey>, String> {
            let target_nodes = self.get_nodes_not_in_network();

            for key in &target_nodes {
                // Select random nodes and add them to bootstrap points.
                let mut bootstrap_points = Vec::new();
                let mut rng = rand::thread_rng();
                let bootstrap_nodes = self
                    .get_nodes_in_network()
                    .into_iter()
                    .choose_multiple(&mut rng, max_bootstrap_points);
                for key in &bootstrap_nodes {
                    let network = self.nodes.get(key).unwrap().network.as_ref().unwrap();
                    for address in network.get_listen_addresses().await {
                        bootstrap_points.push(address);
                    }
                }

                // Create the network interface (to make the node join the network).
                let target_node = self.nodes.get_mut(key).unwrap();
                let config = create_testnet_config();
                let network = PropagationNetwork::with_config(
                    target_node.public_key.clone(),
                    target_node.private_key.clone(),
                    Vec::new(),
                    bootstrap_points.clone(),
                    "test".to_string(),
                    config,
                )
                .await?;
                target_node.network.replace(network);
            }

            Ok(target_nodes)
        }

        async fn add_nodes_to_network_concurrent(
            &mut self,
            max_bootstrap_points: usize,
        ) -> Result<Vec<NodeKey>, String> {
            let target_nodes = self.get_nodes_not_in_network();

            // Assign addresses to bind to.
            let mut listen_addresses = Vec::new();
            for _ in 0..target_nodes.len() {
                let port = *get_random_ports(1)
                    .await
                    .first()
                    .expect("failed to assign a port for a node.");
                let listen_address = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), port);
                listen_addresses.push(listen_address);
            }

            // Create the network interfaces asynchronously.
            let nodes: Vec<&Node> = target_nodes
                .iter()
                .map(|key| self.nodes.get(key).unwrap())
                .collect();
            let futures = zip(nodes, &listen_addresses).map(|(node, listen_address)| {
                let mut config = create_testnet_config();
                config.with_listen_address(listen_address.to_owned());
                let mut rng = rand::thread_rng();
                let bootstrap_points = listen_addresses
                    .iter()
                    .cloned()
                    .choose_multiple(&mut rng, max_bootstrap_points);
                PropagationNetwork::with_config(
                    node.public_key.clone(),
                    node.private_key.clone(),
                    Vec::new(),
                    bootstrap_points,
                    "test".to_string(),
                    config,
                )
            });
            let networks = join_all(futures)
                .await
                .into_iter()
                .map(|result| result.expect("failed to construct PropagationNetwork."));

            // Pass the network interfaces to the nodes.
            for (key, network) in zip(&target_nodes, networks) {
                self.nodes.get_mut(key).unwrap().network.replace(network);
            }

            Ok(target_nodes)
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
            ed25519::Keypair::decode(&mut keypair_bytes).expect("invalid keypair was given."),
        );
        keypair.public()
    }

    /// Checks each node's routing table whether it has all the peers on the same network.
    ///
    /// Panics if the routing table does not match with expectation.
    async fn check_routing_table(nodes: Vec<&Node>) {
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

    /// A helper test function with an argument.
    async fn discovery_with_n_nodes(n: usize, join_sequential: bool, max_bootstrap_points: usize) {
        let mut network = Network::new();
        network.create_nodes(n);
        if join_sequential {
            network
                .add_nodes_to_network_sequential(max_bootstrap_points)
                .await
                .expect("failed to add nodes to the network.");
        } else {
            network
                .add_nodes_to_network_concurrent(max_bootstrap_points)
                .await
                .expect("failed to add nodes to the network.");
        }
         
        // Test if every node has filled its routing table correctly.
        check_routing_table(network.nodes.values().collect()).await
    }

    #[tokio::test]
    /// Test if every node fills its routing table with the addresses of all the other nodes
    /// in a tiny network when they join the network sequentially.
    /// (network_size = 5 < max_peers_per_node = 20)
    async fn discovery_with_tiny_network_sequential() {
        discovery_with_n_nodes(5, true, 5).await;
    }

    #[tokio::test]
    /// Test if every node fills its routing table with the addresses of all the other nodes
    /// in a small network when they join the network sequentially.
    /// (network_size = max_peers_per_node = 20)
    async fn discovery_with_small_network_sequential() {
        let n = libp2p::kad::K_VALUE.into();
        discovery_with_n_nodes(n, true, n).await;
    }

    #[tokio::test]
    /// Test if every node fills its routing table with the addresses of all the other nodes
    /// in a tiny network when they join the network at the same time.
    /// (network_size = 5 < max_peers_per_node = 20)
    async fn discovery_with_tiny_network_concurrent() {
        discovery_with_n_nodes(5, false, 5).await;
    }

    #[tokio::test]
    /// Test if every node fills its routing table with the addresses of all the other nodes
    /// in a small network when they join the network at the same time.
    /// (network_size = max_peers_per_node = 20)
    async fn discovery_with_small_network_concurrent() {
        let n = libp2p::kad::K_VALUE.into();
        discovery_with_n_nodes(n, false, n).await;
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
