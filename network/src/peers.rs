use super::*;
use async_trait::async_trait;
use eyre::{eyre, Result};
use serde_tc::http::*;
use serde_tc::{serde_tc_full, StubCall};
use simperby_core::serde_spb;
use simperby_core::BlockHeader;
use simperby_core::FinalizationInfo;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;

#[derive(Debug)]
struct PeerStorage {
    path: String,
}

impl PeerStorage {
    pub async fn new(path: &str) -> Result<Self> {
        Ok(Self {
            path: path.to_owned(),
        })
    }

    pub async fn write(&mut self, peers: Vec<Peer>) -> Result<()> {
        let _ = tokio::fs::remove_file(&self.path).await;
        let mut file = File::create(&self.path).await?;
        file.write_all(serde_spb::to_string(&peers)?.as_bytes())
            .await?;
        file.flush().await?;
        Ok(())
    }

    pub async fn read(&self) -> Result<Vec<Peer>> {
        let mut file = File::open(&self.path).await?;
        let peers: Vec<Peer> = serde_spb::from_str(&{
            let mut buf = String::new();
            file.read_to_string(&mut buf).await?;
            buf
        })?;
        Ok(peers)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PingResponse {
    pub last_finalized_block_header: BlockHeader,
    pub public_key: PublicKey,
    pub timestamp: Timestamp,
    pub msg: String,
}

#[serde_tc_full]
pub(super) trait PeerRpcInterface: Send + Sync + 'static {
    /// Requests to response some packets.
    async fn ping(&self) -> Result<PingResponse, String>;
    /// Requests to response the port map of this node.
    async fn port_map(&self) -> Result<BTreeMap<String, u16>, String>;
}

pub struct PeerRpcImpl {
    peers: Arc<RwLock<Peers>>,
    port_map: BTreeMap<String, u16>,
}

/// Server-side implementation of the RPC interface.
#[async_trait]
impl PeerRpcInterface for PeerRpcImpl {
    async fn ping(&self) -> Result<PingResponse, String> {
        let peers = self.peers.read().await;
        Ok(PingResponse {
            public_key: peers.private_key.public_key(),
            timestamp: simperby_core::utils::get_timestamp(),
            msg: "hello?".to_string(),
            last_finalized_block_header: peers.lfi.header.clone(),
        })
    }

    async fn port_map(&self) -> Result<BTreeMap<String, u16>, String> {
        Ok(self.port_map.clone())
    }
}

#[derive(Debug)]
pub struct Peers {
    storage: PeerStorage,
    lfi: FinalizationInfo,
    private_key: PrivateKey,
}

impl Peers {
    pub async fn new(path: &str, lfi: FinalizationInfo, private_key: PrivateKey) -> Result<Self> {
        let storage = PeerStorage::new(path).await?;
        Ok(Self {
            storage,
            lfi,
            private_key,
        })
    }

    pub async fn update_block(&mut self, lfi: FinalizationInfo) -> Result<()> {
        let peers = self.storage.read().await?;
        self.storage.write(vec![]).await?;
        for peer in peers {
            self.add_peer(peer.name, peer.address).await?;
        }
        self.lfi = lfi;
        Ok(())
    }

    /// Adds a peer to the list of known peers. This will try to connect to the peer and ask information.
    ///
    /// - `name` - the name of the peer as it is known in the reserved state.
    /// - `addr` - the address of the peer. The port must be the one of the peer discovery RPC.
    pub async fn add_peer(&mut self, name: MemberName, addr: SocketAddrV4) -> Result<()> {
        let peer = Peer {
            public_key: self
                .lfi
                .reserved_state
                .query_public_key(&name)
                .ok_or_else(|| eyre!("peer does not exist: {}", name))?,
            name,
            address: addr,
            ports: Default::default(),
            message: "".to_owned(),
            recently_seen_timestamp: 0,
        };
        let mut peers = self.storage.read().await?;
        peers.push(peer);
        self.storage.write(peers).await?;
        Ok(())
    }

    /// Removes a peer in the list of known peers.
    pub async fn remove_peer(&mut self, name: MemberName) -> Result<()> {
        let mut peers = self.storage.read().await?;
        let index = peers
            .iter()
            .position(|peer| peer.name == name)
            .ok_or_else(|| eyre!("peer does not exist: {}", name))?;
        peers.remove(index);
        self.storage.write(peers).await?;
        Ok(())
    }

    /// Performs the actual peer update (including discovery) and applies to the storage.
    pub async fn update(&mut self) -> Result<()> {
        let peers = self.storage.read().await?;
        let mut new_peers = Vec::new();

        for peer in peers {
            let stub = PeerRpcInterfaceStub::new(Box::new(HttpClient::new(
                format!("{}:{}/peer", peer.address.ip(), peer.address.port()),
                reqwest::Client::new(),
            )));
            stub.ping()
                .await
                .map_err(|e| eyre!("failed to ping peer {}: {}", peer.name, e))?
                .map_err(|e| eyre!("failed to ping peer {}: {}", peer.name, e))?;
            let ports = stub
                .port_map()
                .await
                .map_err(|e| eyre!("failed to get port map {}: {}", peer.name, e))?
                .map_err(|e| eyre!("failed to get port map {}: {}", peer.name, e))?;

            let mut new_peer = peer.clone();
            new_peer.ports = ports;
            new_peers.push(new_peer);
        }
        self.storage.write(new_peers).await?;
        Ok(())
    }

    pub async fn list_peers(&self) -> Result<Vec<Peer>> {
        self.storage.read().await
    }

    pub async fn serve(
        this: Arc<RwLock<Peers>>,
        port_map: BTreeMap<String, u16>,
        server_network_config: ServerNetworkConfig,
    ) -> Result<(), Error> {
        run_server(
            server_network_config.port,
            [(
                "peer".to_owned(),
                create_http_object(Arc::new(PeerRpcImpl {
                    peers: Arc::clone(&this),
                    port_map,
                }) as Arc<dyn PeerRpcInterface>),
            )]
            .iter()
            .cloned()
            .collect(),
        )
        .await;
        std::future::pending::<()>().await;
        Ok(())
    }
}
