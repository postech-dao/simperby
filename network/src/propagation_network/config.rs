use std::{
    net::{Ipv4Addr, SocketAddrV4},
    time::Duration,
};

use libp2p::{multiaddr::Protocol, Multiaddr};

/// Configurations of [`PropagationNetwork`].
pub struct PropagationNetworkConfig {
    /// Addresses to listen on to handle incoming connection requests.
    pub(crate) listen_address: Multiaddr,
    /// Timeout parameter for listener creation.
    pub(crate) listener_creation_timeout: Duration,
    /// Timeout parameter for initial bootstrap.
    pub(crate) initial_bootstrap_timeout: Duration,
    /// Interval for the guaranteed lock aquisition for swarm.
    ///
    /// It is the maximal delay until the [`PropagationNetwork`] aquires
    /// all of the resources needed to serve a job assigned from its public interface.
    pub(crate) lock_release_interval: Duration,
    /// Interval for the regular peer discovery routine.
    pub(crate) peer_discovery_interval: Duration,
    /// Capacity for the message queue that passes messages from other nodes
    /// to its simperby node.
    pub(crate) message_queue_capacity: usize,
}

impl PropagationNetworkConfig {
    /// Returns a default.
    ///
    /// To customize configurations, call `default` and chain the functions with the name of `with_<fieldname>`.
    pub fn default() -> Self {
        Self {
            listen_address: Self::convert_socketaddr_to_multiaddr(SocketAddrV4::new(
                Ipv4Addr::new(0, 0, 0, 0),
                0,
            )),
            listener_creation_timeout: Duration::from_millis(1000),
            initial_bootstrap_timeout: Duration::from_millis(3000),
            lock_release_interval: Duration::from_millis(30),
            peer_discovery_interval: Duration::from_millis(10000),
            message_queue_capacity: 100,
        }
    }

    pub fn with_listen_address(&mut self, listen_address: SocketAddrV4) -> &mut Self {
        self.listen_address = Self::convert_socketaddr_to_multiaddr(listen_address);
        self
    }

    pub fn with_listener_creation_timeout(
        &mut self,
        listener_creation_timeout: Duration,
    ) -> &mut Self {
        self.listener_creation_timeout = listener_creation_timeout;
        self
    }

    pub fn with_initial_bootstrap_timeout(
        &mut self,
        initial_bootstrap_timeout: Duration,
    ) -> &mut Self {
        self.initial_bootstrap_timeout = initial_bootstrap_timeout;
        self
    }

    pub fn with_peer_discovery_interval(&mut self, peer_discovery_interval: Duration) -> &mut Self {
        self.peer_discovery_interval = peer_discovery_interval;
        self
    }

    pub fn with_message_queue_capacity(&mut self, message_queue_capacity: usize) -> &mut Self {
        self.message_queue_capacity = message_queue_capacity;
        self
    }

    fn convert_socketaddr_to_multiaddr(socket_addr: SocketAddrV4) -> Multiaddr {
        Multiaddr::from_iter([
            Protocol::Ip4(*socket_addr.ip()),
            Protocol::Tcp(socket_addr.port()),
        ])
    }
}
