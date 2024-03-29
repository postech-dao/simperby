use super::*;
use crate::keys;
use simperby_core::utils::get_timestamp;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PingResponse {
    pub public_key: PublicKey,
    pub timestamp: Timestamp,
    pub msg: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PeerStatus {
    pub public_key: PublicKey,
    pub address: std::net::SocketAddr,
    pub last_ping: String,
    pub last_observed_timestamp: Timestamp,
    pub last_claimed_local_timestamp: Timestamp,
    pub last_msg: String,
}

/// The interface that will be wrapped into an HTTP RPC server for the peers.
#[serde_tc_full]
pub(super) trait DistributedMessageSetRpcInterface: Send + Sync + 'static {
    /// Requests to response some packets.
    async fn request_packets(&self) -> Result<Vec<Packet>, String>;

    /// Sends packets to the peer.
    async fn send_packets(&self, packets: Vec<Packet>) -> Result<(), String>;

    async fn ping(&self) -> Result<PingResponse, String>;
}

pub(super) struct DmsWrapper<S: Storage, M: DmsMessage> {
    #[allow(clippy::type_complexity)]
    /// This is an `Option` because we have to explicitly drop the server
    /// (it could live forever in the RPC server (`axum`) otherwise)
    pub(super) dms: Arc<parking_lot::RwLock<Option<Arc<RwLock<DistributedMessageSet<S, M>>>>>>,
}

/// Server-side implementation of the RPC interface.
#[async_trait]
impl<S: Storage, M: DmsMessage> DistributedMessageSetRpcInterface for DmsWrapper<S, M> {
    async fn request_packets(&self) -> Result<Vec<Packet>, String> {
        let dms = Arc::clone(
            self.dms
                .read()
                .as_ref()
                .ok_or_else(|| "server terminated".to_owned())?,
        );
        let packets = dms
            .read()
            .await
            .retrieve_packets()
            .await
            .map_err(|e| e.to_string())?;
        Ok(packets)
    }

    async fn send_packets(&self, packets: Vec<Packet>) -> Result<(), String> {
        let dms = Arc::clone(
            self.dms
                .read()
                .as_ref()
                .ok_or_else(|| "server terminated".to_owned())?,
        );
        for packet in packets {
            dms.write()
                .await
                .receive_packet(packet)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    async fn ping(&self) -> Result<PingResponse, String> {
        let dms = Arc::clone(
            self.dms
                .read()
                .as_ref()
                .ok_or_else(|| "server terminated".to_owned())?,
        );
        let public_key = dms.read().await.private_key.public_key();
        Ok(PingResponse {
            public_key,
            timestamp: get_timestamp(),
            msg: "hello?".to_string(),
        })
    }
}

impl<S: Storage, M: DmsMessage> DistributedMessageSet<S, M> {
    /// Fetches unknown messages from the peers using an RPC protocol,
    /// and adds them to the local storage.
    pub async fn fetch(
        this: Arc<RwLock<Self>>,
        network_config: &ClientNetworkConfig,
    ) -> Result<(), Error> {
        let mut tasks = Vec::new();
        for peer in &network_config.peers {
            let this_ = Arc::clone(&this);
            let task = async move {
                let this_read = this_.read().await;
                let port_key = keys::port_key_dms::<M>();
                let stub = DistributedMessageSetRpcInterfaceStub::new(Box::new(HttpClient::new(
                    format!(
                        "{}:{}/dms",
                        peer.address.ip(),
                        peer.ports
                            .get(&port_key)
                            .ok_or_else(|| eyre!("can't find port key: {}", port_key))?
                    ),
                    reqwest::Client::new(),
                )));
                let packets = stub
                    .request_packets()
                    .await
                    .map_err(|e| eyre!("{}", e))?
                    .map_err(|e| eyre!(e))?;
                // Important: drop the lock before `write()`
                drop(this_read);
                for packet in packets {
                    this_.write().await.receive_packet(packet).await?;
                }
                Result::<(), Error>::Ok(())
            };
            tasks.push(task);
        }
        let results = future::join_all(tasks).await;
        for (result, peer) in results.into_iter().zip(network_config.peers.iter()) {
            if let Err(e) = result {
                log::warn!("failed to fetch from client {:?}: {}", peer, e);
            }
        }
        Ok(())
    }

    /// Tries to broadcast all the message that this DMS instance has.
    ///
    /// Note: this function may take just `&self` due to its simple implementation,
    /// but keeps `Arc<RwLock<Self>>` to make sure the interface to indicate
    /// that this is a network-involved method (unlike others)
    pub async fn broadcast(
        this: Arc<RwLock<Self>>,
        network_config: &ClientNetworkConfig,
    ) -> Result<(), Error> {
        let mut tasks_and_messages = Vec::new();

        let packets = this.read().await.retrieve_packets().await?;
        if packets.is_empty() {
            return Ok(());
        }
        for peer in &network_config.peers {
            let port_key = keys::port_key_dms::<M>();
            let packets_ = packets.clone();
            let task = async move {
                let stub = DistributedMessageSetRpcInterfaceStub::new(Box::new(HttpClient::new(
                    format!(
                        "{}:{}/dms",
                        peer.address.ip(),
                        peer.ports
                            .get(&port_key)
                            .ok_or_else(|| eyre!("can't find port key: {}", port_key))?
                    ),
                    reqwest::Client::new(),
                )));
                stub.send_packets(packets_.clone())
                    .await
                    .map_err(|e| eyre!(e))?
                    .map_err(|e| eyre!(e))?;
                Result::<(), Error>::Ok(())
            };
            tasks_and_messages.push((task, format!("RPC message add to {}", peer.public_key)));
        }
        let (tasks, messages) = tasks_and_messages
            .into_iter()
            .unzip::<_, _, Vec<_>, Vec<_>>();

        let results = future::join_all(tasks).await;
        for (result, msg) in results.into_iter().zip(messages.iter()) {
            if let Err(e) = result {
                log::warn!("failure in {}: {}", msg, e);
            }
        }
        Ok(())
    }

    pub async fn get_peer_status(
        this: Arc<RwLock<Self>>,
        network_config: &ClientNetworkConfig,
    ) -> Result<Vec<PeerStatus>, Error> {
        let mut tasks = Vec::new();
        for peer in &network_config.peers {
            let this_ = Arc::clone(&this);
            let task = async move {
                let this_read = this_.read().await;
                let port_key = keys::port_key_dms::<M>();
                let stub = DistributedMessageSetRpcInterfaceStub::new(Box::new(HttpClient::new(
                    format!(
                        "{}:{}/dms",
                        peer.address.ip(),
                        peer.ports
                            .get(&port_key)
                            .ok_or_else(|| eyre!("can't find port key: {}", port_key))?
                    ),
                    reqwest::Client::new(),
                )));
                let ping_response = stub
                    .ping()
                    .await
                    .map_err(|e| eyre!("{}", e))?
                    .map_err(|e| eyre!(e))?;
                // Important: drop the lock before `write()`
                drop(this_read);

                if peer.public_key != ping_response.public_key {
                    return Err(eyre!(
                        "peer public key mismatch: expected {}, got {}",
                        peer.public_key,
                        ping_response.public_key
                    ));
                }
                Result::<(), Error>::Ok(())
            };
            tasks.push(task);
        }
        let results = future::join_all(tasks).await;
        let mut final_results = Vec::new();
        let port_key = keys::port_key_dms::<M>();

        for (result, peer) in results.into_iter().zip(network_config.peers.iter()) {
            let ping = if let Err(e) = result {
                log::warn!("failed to ping from client {:?}: {}", peer, e);
                format!("failed: {}", e)
            } else {
                "success".to_owned()
            };

            let port = peer
                .ports
                .get(&port_key)
                .ok_or_else(|| eyre!("can't find port key: {}", port_key))?;

            final_results.push(PeerStatus {
                public_key: peer.public_key.clone(),
                address: format!("{}:{}", peer.address.ip(), port)
                    .parse()
                    .expect("valid address"),
                last_ping: ping,
                last_observed_timestamp: 0,      // TODO
                last_claimed_local_timestamp: 0, // TODO
                last_msg: "todo".to_owned(),
            });
        }
        Ok(final_results)
    }
}
