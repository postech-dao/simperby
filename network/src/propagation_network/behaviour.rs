use libp2p::{
    floodsub::{Floodsub, FloodsubEvent},
    identify::{Identify, IdentifyConfig, IdentifyEvent},
    identity::PublicKey,
    kad::{store::MemoryStore, Kademlia, KademliaConfig, KademliaEvent},
    NetworkBehaviour,
};
use std::time::Duration;

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "Event")]
pub struct Behaviour {
    /// A network behaviour that identifies connected peers.
    ///
    /// Information of the identified peer contains its public key, listen addresses, etc.
    identify: Identify,
    /// A network behaviour that implement Kademlia Distributed Hash Table (Kademlia DHT).
    ///
    /// Storing and retrieving items in/from the DHT do not occur in this crate.
    /// Instead, kademlia continuously discovers k closest peers
    /// to maintain k connections with its neighbors.
    kademlia: Kademlia<MemoryStore>,
    /// A network behaviour that implements PubSub message passing protocol.
    ///
    /// It tries to propagate a message to all peers that it has connections with,
    /// thus flooding the network with messages.
    floodsub: Floodsub,
}

impl Behaviour {
    /// Constructor with default configuration.
    pub fn new(local_public_key: PublicKey) -> Self {
        let local_peer_id = local_public_key.to_peer_id();

        let identify_config =
            IdentifyConfig::new("/simperby/identify".to_string(), local_public_key);

        // Create a key-value store, which will not be used in this crate, for Kademlia DHT.
        let store = MemoryStore::new(local_peer_id);

        // Note: The default configuration for Kademlia is subject to change.
        let mut kademlia_config = KademliaConfig::default();
        kademlia_config
            .set_protocol_name("/simperby/kademlia".as_bytes())
            .set_connection_idle_timeout(Duration::from_secs(30))
            .set_query_timeout(Duration::from_secs(20));

        Self {
            identify: Identify::new(identify_config),
            kademlia: Kademlia::with_config(local_peer_id, store, kademlia_config),
            floodsub: Floodsub::new(local_peer_id),
        }
    }
    // Todo: constructor with configuration
}

/// Network events captured from other network behaviours in [`Behaviour`].
pub enum Event {
    Identify(IdentifyEvent),
    Kademlia(KademliaEvent),
    Floodsub(FloodsubEvent),
}

impl From<IdentifyEvent> for Event {
    fn from(e: IdentifyEvent) -> Self {
        Event::Identify(e)
    }
}

impl From<KademliaEvent> for Event {
    fn from(e: KademliaEvent) -> Self {
        Event::Kademlia(e)
    }
}

impl From<FloodsubEvent> for Event {
    fn from(e: FloodsubEvent) -> Self {
        Event::Floodsub(e)
    }
}
