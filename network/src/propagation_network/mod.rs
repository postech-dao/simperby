mod behaviour;

use super::*;
use async_trait::async_trait;
use behaviour::Behaviour;
use futures::StreamExt;
use libp2p::{
    development_transport,
    identity::{ed25519, Keypair},
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
    _swarm: Arc<Mutex<Swarm<Behaviour>>>,
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
            _swarm: swarm.clone(),
        })
    }
    async fn broadcast(&self, _message: &[u8]) -> Result<BroadcastToken, String> {
        unimplemented!("not implemented");
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
        unimplemented!("not implemented");
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
