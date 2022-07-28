use libp2p::{
    identify::{Identify, IdentifyEvent},
    NetworkBehaviour,
};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "Event")]
pub struct Behaviour {
    /// A network behaviour that identifies connected peers.
    /// Information of the identified peer contains its public key, listen addresses, etc.
    identify: Identify,
}

/// Network events captured from other network behaviours in [`Behaviour`].
pub enum Event {
    Identify(IdentifyEvent),
}

impl From<IdentifyEvent> for Event {
    fn from(e: IdentifyEvent) -> Self {
        Event::Identify(e)
    }
}
