use super::behaviour::{DiscoveryBehaviour, DiscoveryEvent};
use super::utils::{
    convert_keypair, convert_multiaddr_into_sockv4, convert_public_key, get_peer_id,
};
use crate::{primitives::PeerDiscoveryPrimitive, *};
use anyhow::anyhow;
use async_trait::async_trait;
use chrono::Utc;
use futures::StreamExt;
use ip_rfc::global_v4;
use libp2p::{
    core::{muxing::StreamMuxerBox, transport, upgrade},
    identify, identity,
    noise::NoiseAuthenticated,
    swarm::{SwarmBuilder, SwarmEvent},
    tcp,
    yamux::YamuxConfig,
    Multiaddr, PeerId, Swarm, Transport,
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
        let mut swarm = Self::create_swarm(&network_config, message, port_map).await?;
        swarm
            .listen_on(format!("/ip4/0.0.0.0/tcp/{}", network_config.port.unwrap_or(0)).parse()?)?;
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
        let swarm = SwarmBuilder::with_executor(
            transport,
            behaviour,
            libp2p_keypair.public().to_peer_id(),
            |fut| {
                tokio::spawn(fut);
            },
        )
        .build();
        Ok(swarm)
    }

    async fn create_transport(
        libp2p_keypair: &identity::Keypair,
    ) -> Result<transport::Boxed<(PeerId, StreamMuxerBox)>, Error> {
        let transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true))
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

    #[allow(clippy::single_match)]
    /// The background task that serves peer discovery protocol.
    async fn discovery_task(
        mut swarm: Swarm<DiscoveryBehaviour>,
        shared_known_peers: SharedKnownPeers,
    ) -> Result<(), Error> {
        Self::add_known_peers_to_routing_table(&mut swarm, &shared_known_peers).await?;
        // Todo: Make the interval configurable.
        let mut discovery_timer = tokio::time::interval(tokio::time::Duration::from_secs(10));
        loop {
            tokio::select! {
                event = swarm.select_next_some() => match event {
                    SwarmEvent::Behaviour(event) => Self::update_known_peers(&mut swarm, &shared_known_peers, event).await,
                    // All incoming events should be comsumed even though we don't handle them.
                    _any_other_event => (),
                },
                _ = discovery_timer.tick() =>
                    Self::regular_discovery(&mut swarm).await,
            }
        }
    }

    async fn update_known_peers(
        swarm: &mut Swarm<DiscoveryBehaviour>,
        shared_known_peers: &SharedKnownPeers,
        event: DiscoveryEvent,
    ) {
        if let DiscoveryEvent::Identify(identify::Event::Received { info, peer_id }) = event {
            swarm
                .behaviour_mut()
                .kademlia
                .add_address(&peer_id, info.listen_addrs[0].clone());
            let _ = Self::add_new_peer(info, shared_known_peers).await;
        }
    }

    async fn regular_discovery(swarm: &mut Swarm<DiscoveryBehaviour>) {
        // If the routing table is empty, `bootstrap` returns an error, which we just ignore.
        let _ = swarm.behaviour_mut().kademlia.bootstrap();
    }

    /// Lets the kademlia know about the known peers,
    /// so that it can initiate the bootstrap process later.
    async fn add_known_peers_to_routing_table(
        swarm: &mut Swarm<DiscoveryBehaviour>,
        shared_known_peers: &SharedKnownPeers,
    ) -> Result<(), Error> {
        let known_peers = shared_known_peers.lock.read().await;
        for peer in known_peers.iter() {
            let address: Multiaddr =
                format!("/ip4/{}/tcp/{}", peer.address.ip(), peer.address.port()).parse()?;
            swarm
                .behaviour_mut()
                .kademlia
                .add_address(&get_peer_id(peer)?, address);
        }
        Ok(())
    }

    async fn add_new_peer(
        info: identify::Info,
        shared_known_peers: &SharedKnownPeers,
    ) -> Result<(), Error> {
        let public_key = convert_public_key(&info.public_key)?;
        // Todo: `is_loopback` is for testing only.
        //        We should take only public IPs. Remove it in the production.
        let public_ip_addr = info
            .listen_addrs
            .iter()
            .filter_map(|multiaddr| convert_multiaddr_into_sockv4(multiaddr.to_owned()).ok())
            .find(|address| global_v4(address.ip()) || address.ip().is_loopback())
            .ok_or_else(|| anyhow!("no public ip address found"))?;
        let (message, ports) = serde_json::from_str(&info.agent_version)?;
        let peer = Peer {
            public_key,
            address: public_ip_addr,
            ports,
            message,
            recently_seen_timestamp: Utc::now().timestamp_millis() as Timestamp,
        };
        shared_known_peers.add_or_replace(peer).await;
        Ok(())
    }
}
