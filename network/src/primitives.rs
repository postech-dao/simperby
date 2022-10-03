use super::*;
use async_trait::async_trait;
use tokio::sync::mpsc;

#[async_trait]
pub trait PeerDiscoveryPrimitive {
    /// Remains online on the network indefinitely,
    /// responding to discovery requests from other nodes,
    /// updating `known_peers`.
    async fn serve(
        network_config: &NetworkConfig,
        initially_known_peers: Vec<Peer>,
    ) -> Result<(SharedKnownPeers, tokio::task::JoinHandle<Result<(), Error>>), Error>;
}

/// The p2p gossip network.
#[async_trait]
pub trait P2PNetwork {
    /// Broadcasts a message to the network.
    async fn broadcast(
        config: &NetworkConfig,
        known_peers: &[Peer],
        message: Vec<u8>,
    ) -> Result<(), Error>;

    /// Remains online on the network indefinitely,
    /// relaying (propagating) messages broadcasted over the network.
    async fn serve(
        config: &NetworkConfig,
        peers: SharedKnownPeers,
    ) -> Result<
        (
            mpsc::Receiver<Vec<u8>>,
            tokio::task::JoinHandle<Result<(), Error>>,
        ),
        Error,
    >;
}
