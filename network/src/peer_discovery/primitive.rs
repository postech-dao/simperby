use super::behaviour::DiscoveryBehaviour;
use super::utils::convert_keypair;
use crate::{primitives::PeerDiscoveryPrimitive, *};
use async_trait::async_trait;
use libp2p::{
    core::{muxing::StreamMuxerBox, transport, upgrade},
    identity,
    noise::NoiseAuthenticated,
    swarm::SwarmBuilder,
    tcp::{GenTcpConfig, TokioTcpTransport},
    yamux::YamuxConfig,
    PeerId, Swarm, Transport,
};
use tokio::task::JoinHandle;

pub(crate) struct PeerDiscoveryPrimitiveImpl;

#[async_trait]
impl PeerDiscoveryPrimitive for PeerDiscoveryPrimitiveImpl {
    async fn serve(
        network_config: NetworkConfig,
        message: String,
        port_map: HashMap<String, u16>,
        initially_known_peers: Vec<Peer>,
    ) -> Result<(SharedKnownPeers, JoinHandle<Result<(), Error>>), Error> {
        let swarm = Self::create_swarm(&network_config, message, port_map).await?;
        let shared_known_peers = SharedKnownPeers {
            lock: Arc::new(RwLock::new(initially_known_peers)),
        };
        Ok((
            shared_known_peers.to_owned(),
            tokio::spawn(Self::discovery_task(swarm, shared_known_peers)),
        ))
    }
}

impl PeerDiscoveryPrimitiveImpl {
    async fn create_swarm(
        network_config: &NetworkConfig,
        message: String,
        port_map: HashMap<String, u16>,
    ) -> Result<Swarm<DiscoveryBehaviour>, Error> {
        let libp2p_keypair =
            convert_keypair(&network_config.public_key, &network_config.private_key)?;
        let transport = Self::create_transport(&libp2p_keypair).await?;
        let behaviour = Self::create_behaviour(&libp2p_keypair, message, port_map).await?;
        let builder = SwarmBuilder::new(transport, behaviour, libp2p_keypair.public().to_peer_id());
        let swarm = builder
            .executor(Box::new(|fut| {
                tokio::spawn(fut);
            }))
            .build();
        Ok(swarm)
    }

    async fn create_transport(
        libp2p_keypair: &identity::Keypair,
    ) -> Result<transport::Boxed<(PeerId, StreamMuxerBox)>, Error> {
        let transport = TokioTcpTransport::new(GenTcpConfig::default().nodelay(true))
            .upgrade(upgrade::Version::V1)
            .authenticate(NoiseAuthenticated::xx(libp2p_keypair).unwrap())
            .multiplex(YamuxConfig::default())
            .boxed();
        Ok(transport)
    }

    async fn create_behaviour(
        libp2p_keypair: &identity::Keypair,
        message: String,
        port_map: HashMap<String, u16>,
    ) -> Result<DiscoveryBehaviour, Error> {
        let message = serde_json::to_string(&(message, port_map))?;
        Ok(DiscoveryBehaviour::new(libp2p_keypair.public(), message))
    }

    /// The background task that serves peer discovery protocol.
    async fn discovery_task(
        mut _swarm: Swarm<DiscoveryBehaviour>,
        _shared_known_peers: SharedKnownPeers,
    ) -> Result<(), Error> {
        // Todo:
        // Add known peers to the routing table.
        // Listen on swarm events to update the shared known peers continuously.
        unimplemented!();
    }
}
