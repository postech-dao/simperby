use libp2p::{
    identify,
    identity::PublicKey,
    kad::{store::MemoryStore, Kademlia, KademliaConfig, KademliaEvent},
    swarm::NetworkBehaviour,
};
use std::{borrow::Cow, time::Duration};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "DiscoveryEvent")]
pub struct DiscoveryBehaviour {
    /// The network behaviour that exchanges information of this node with connected peers.
    pub(crate) identify: identify::Behaviour,
    /// The network behaviour that discovers peers in the network.
    ///
    /// In [`crate`], storing and retrieving items in/from the DHT do not occur.
    /// Instead, kademlia regularly discovers some closest peers to maintain the fixed number of connections
    /// with its neighbors (the set of the closest peers is sometimes called "partial view of the network").
    pub(crate) kademlia: Kademlia<MemoryStore>,
}

impl DiscoveryBehaviour {
    pub(crate) fn new(pubkey: PublicKey, message: String) -> Self {
        let peer_id = pubkey.to_peer_id();

        let identify_config = identify::Config::new("/simperby/discovery".to_string(), pubkey)
            .with_agent_version(message)
            .with_initial_delay(Duration::ZERO);

        let mut kademlia_config = KademliaConfig::default();
        kademlia_config
            .set_protocol_names(vec![Cow::from("/simperby/discovery/kademlia".as_bytes())]);

        let store = MemoryStore::new(peer_id);

        Self {
            identify: identify::Behaviour::new(identify_config),
            kademlia: Kademlia::with_config(peer_id, store, kademlia_config),
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum DiscoveryEvent {
    Identify(identify::Event),
    Kademlia(KademliaEvent),
}

impl From<identify::Event> for DiscoveryEvent {
    fn from(e: identify::Event) -> Self {
        DiscoveryEvent::Identify(e)
    }
}

impl From<KademliaEvent> for DiscoveryEvent {
    fn from(e: KademliaEvent) -> Self {
        DiscoveryEvent::Kademlia(e)
    }
}
