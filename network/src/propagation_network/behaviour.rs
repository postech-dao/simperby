use libp2p::{
    floodsub::{Floodsub, FloodsubEvent},
    identify::{Identify, IdentifyEvent},
    kad::{store::MemoryStore, Kademlia, KademliaEvent},
    NetworkBehaviour,
};

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
