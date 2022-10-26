use super::Storage;
use super::*;
use anyhow::anyhow;
use async_trait::async_trait;
use futures::future::join_all;
use futures::prelude::*;
use futures::try_join;
use serde_tc::http::*;
use serde_tc::{serde_tc_full, StubCall};
use simperby_common::*;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

const STATE_FILE_PATH: &str = "_state.json";

#[derive(Debug, Clone, Serialize)]
pub struct Message {
    data: String,
    signature: TypedSignature<String>,
}

impl Message {
    pub fn new(data: String, signature: TypedSignature<String>) -> Result<Self, CryptoError> {
        signature.verify(&data)?;
        Ok(Self { data, signature })
    }

    pub fn data(&self) -> &str {
        &self.data
    }

    pub fn signature(&self) -> &TypedSignature<String> {
        &self.signature
    }
}

impl ToHash256 for Message {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_json::to_vec(self).unwrap())
    }
}

/// A message before verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawMessage {
    pub data: String,
    pub signature: TypedSignature<String>,
}

impl RawMessage {
    pub fn into_message(self) -> anyhow::Result<Message> {
        Message::new(self.data, self.signature).map_err(|e| anyhow!(e))
    }

    pub fn from_message(message: Message) -> Self {
        RawMessage {
            data: message.data().to_owned(),
            signature: message.signature().to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub height: BlockHeight,
    pub key: String,
}

pub struct StorageWrapper<S> {
    storage: Arc<RwLock<S>>,
}

/// The interface that will be wrapped into an HTTP RPC server for the peers.
#[serde_tc_full]
trait DistributedMessageSetRpcInterface: Send + Sync + 'static {
    /// Returns the messages except `knowns`.
    async fn get_message(
        &self,
        height: BlockHeight,
        knowns: Vec<Hash256>,
    ) -> Result<Vec<RawMessage>, String>;
}

#[async_trait]
impl<S: Storage> DistributedMessageSetRpcInterface for StorageWrapper<S> {
    async fn get_message(
        &self,
        height: BlockHeight,
        knowns: Vec<Hash256>,
    ) -> Result<Vec<RawMessage>, String> {
        let mut messages = read_messages(&(*self.storage.read().await))
            .await
            .map_err(|e| e.to_string())?;
        let height_ = read_state(&*self.storage.read().await)
            .await
            .map_err(|e| e.to_string())?
            .height;
        if height != height_ {
            return Err(format!(
                "height mismatch: requested {}, but {}",
                height, height_
            ));
        }
        let knowns: HashSet<_> = knowns.into_iter().collect();
        let messages: Vec<_> = messages
            .drain(..)
            .filter(|m| !knowns.contains(&m.to_hash256()))
            .map(RawMessage::from_message)
            .collect();
        Ok(messages)
    }
}

async fn read_messages(storage: &impl Storage) -> Result<Vec<Message>, Error> {
    let files = storage.list_files().await?;
    let tasks = files
        .into_iter()
        .map(|f| async move { storage.read_file(&f).await });
    let data = future::join_all(tasks)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let messages = data
        .into_iter()
        .map(|d| serde_json::from_str(&d))
        .collect::<Result<Vec<RawMessage>, _>>()?;
    let messages = messages
        .into_iter()
        .map(|d| d.into_message())
        .collect::<Result<Vec<_>, _>>()?;
    Ok(messages)
}

async fn add_message_but_not_broadcast(
    storage: &mut impl Storage,
    message: Message,
) -> Result<(), Error> {
    storage
        .add_or_overwrite_file(
            &format!("{}.json", message.to_hash256()),
            message.data().to_owned(),
        )
        .await?;
    Ok(())
}

async fn read_state(storage: &impl Storage) -> Result<State, Error> {
    let state: State = serde_json::from_str(&storage.read_file(STATE_FILE_PATH).await?)?;
    Ok(state)
}

async fn write_state(storage: &mut impl Storage, state: State) -> Result<(), Error> {
    storage
        .add_or_overwrite_file(STATE_FILE_PATH, serde_json::to_string(&state)?)
        .await?;
    Ok(())
}

async fn fetch<S: Storage>(
    storage: Arc<RwLock<S>>,
    _network_config: &NetworkConfig,
    known_peers: &[Peer],
) -> Result<(), Error> {
    let mut tasks = Vec::new();
    let messages = read_messages(&*storage.read().await).await?;
    let known_messages = messages
        .into_iter()
        .map(|m| m.to_hash256())
        .collect::<Vec<_>>();
    let state = read_state(&*storage.read().await).await?;
    let height = state.height;

    for peer in known_peers {
        let storage = Arc::clone(&storage);
        let port_key = state.key.clone();
        let known_messages_ = known_messages.clone();
        let task = async move {
            let stub = DistributedMessageSetRpcInterfaceStub::new(Box::new(HttpClient::new(
                format!(
                    "http://{}/{}",
                    peer.address.ip(),
                    peer.ports
                        .get(&port_key)
                        .ok_or_else(|| anyhow!("can't find port key: {}", port_key))?
                ),
                reqwest::Client::new(),
            )));
            let messages = stub
                .get_message(height, known_messages_)
                .await
                .map_err(|e| anyhow!(e))?
                .map_err(|e| anyhow!(e))?;
            let mut storage = storage.write().await;
            for message in messages {
                let message = message.into_message()?;
                add_message_but_not_broadcast(&mut *storage, message).await?;
            }
            Result::<(), Error>::Ok(())
        };
        tasks.push(task);
    }
    let results = future::join_all(tasks).await;
    for (result, peer) in results.into_iter().zip(known_peers.iter()) {
        if let Err(e) = result {
            log::warn!("failed to fetch from client {:?}: {}", peer, e);
        }
    }
    Ok(())
}

/// A **cumulative** set that is shared in the p2p network, backed by the local file system.
///
/// One of the notable characteristics of blockchain is that it is based on heights;
/// The key idea here is that we retain an instance (both in memory or on disk)
/// of `DistributedMessageSet` only for a specific height,
/// and discard if the height progresses, creating a new and empty one again.
///
/// For every method,
/// - If the given directory is empty, it fails (except `create()`).
/// - It locks the storage.
/// - If the given directory is locked (possibly by another instance of `DistributedMessageSet`),
/// it will `await` until the lock is released.
pub struct DistributedMessageSet<N, S> {
    storage: Arc<RwLock<S>>,
    config: Config,
    _marker: std::marker::PhantomData<N>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// The interval of the broadcasts.
    /// If none, it will broadcast only in `add_message()`, not in `serve()`.
    pub broadcast_interval: Option<Duration>,
    /// The interval of the direct-peer fetch. If none, it will fetch only in `fetch()`, not in `serve()`.
    pub fetch_interval: Option<Duration>,
}

impl<N: GossipNetwork, S: Storage> DistributedMessageSet<N, S> {
    /// Creates a new and empty storage with the given directory.
    /// If there is already a directory, it discards everything and creates a new one.
    /// You should try `open()` first!
    ///
    /// - `dms_key`: The unique key for distinguishing the DMS.
    pub async fn create(mut storage: S, height: u64, dms_key: String) -> Result<(), Error> {
        storage.remove_all_files().await?;
        write_state(
            &mut storage,
            State {
                height,
                key: dms_key,
            },
        )
        .await?;
        Ok(())
    }

    /// Opens an existing storage with the given directory.
    pub async fn open(storage: S, config: Config) -> Result<Self, Error>
    where
        Self: Sized,
    {
        Ok(Self {
            storage: Arc::new(RwLock::new(storage)),
            config,
            _marker: std::marker::PhantomData,
        })
    }

    /// Fetches the unknown messages from the peers and updates the storage.
    pub async fn fetch(
        &mut self,
        _network_config: &NetworkConfig,
        known_peers: &[Peer],
    ) -> Result<(), Error> {
        fetch(Arc::clone(&self.storage), _network_config, known_peers).await
    }

    /// Adds the given message to the storage, immediately broadcasting it to the network.
    ///
    /// Note that it is guaranteed that the message will not be broadcasted unless it
    /// is successfully added to the storage. (but it is not guaranteed for the other way around)
    pub async fn add_message(
        &mut self,
        network_config: &NetworkConfig,
        known_peers: &[Peer],
        message: Message,
    ) -> Result<(), Error> {
        add_message_but_not_broadcast(&mut *self.storage.write().await, message.clone()).await?;
        N::broadcast(
            network_config,
            known_peers,
            serde_json::to_vec(&message).unwrap(),
        )
        .await?;
        Ok(())
    }

    /// Reads the messages from the storage.
    pub async fn read_messages(&self) -> Result<Vec<Message>, Error> {
        let result = read_messages(&*self.storage.read().await).await?;
        Ok(result)
    }

    /// Reads the height from the storage.
    pub async fn read_height(&self) -> Result<BlockHeight, Error> {
        let state = read_state(&*self.storage.read().await).await?;
        Ok(state.height)
    }

    /// Advances the height of the message set, discarding all the messages.
    pub async fn advance(&mut self) -> Result<(), Error> {
        let state = read_state(&*self.storage.read().await).await?;
        let mut storage = self.storage.write().await;
        storage.remove_all_files().await?;
        write_state(
            &mut *storage,
            State {
                height: state.height + 1,
                key: state.key,
            },
        )
        .await?;
        Ok(())
    }

    /// Serves the p2p network node and the RPC server indefinitely, constantly updating the storage.
    pub async fn serve(
        self,
        network_config: NetworkConfig,
        rpc_port: u16,
        peers: SharedKnownPeers,
    ) -> Result<tokio::task::JoinHandle<Result<(), Error>>, Error> {
        let mut recv = N::serve(network_config.clone(), peers.clone()).await?;
        let storage_ = Arc::clone(&self.storage);
        let peers_ = peers.clone();
        let network_config_ = network_config.clone();
        let fetch_task = async move {
            let interval = if let Some(x) = self.config.fetch_interval {
                x
            } else {
                return Result::<(), Error>::Ok(());
            };
            loop {
                let peers = peers_.read().await.to_vec();
                fetch(Arc::clone(&storage_), &network_config_, &peers).await?;
                tokio::time::sleep(interval).await;
            }
        };
        let storage_ = Arc::clone(&self.storage);
        let peers_ = peers.clone();
        let broadcast_task = async move {
            let interval = if let Some(x) = self.config.broadcast_interval {
                x
            } else {
                return Result::<(), Error>::Ok(());
            };
            loop {
                let peers = peers_.read().await.to_vec();
                let messages = read_messages(&*storage_.read().await).await?;
                let tasks = messages.into_iter().map(|message| {
                    let network_config = network_config.clone();
                    let peers = peers.clone();
                    async move {
                        N::broadcast(
                            &network_config,
                            &peers,
                            serde_json::to_vec(&message).unwrap(),
                        )
                        .await?;
                        Result::<(), Error>::Ok(())
                    }
                });
                join_all(tasks).await;
                tokio::time::sleep(interval).await;
            }
        };
        let storage_ = Arc::clone(&self.storage);
        let gossip_serve_task = async move {
            while let Some(m) = recv.0.recv().await {
                match serde_json::from_slice::<RawMessage>(&m) {
                    Ok(raw_message) => {
                        let message = raw_message.into_message()?;
                        add_message_but_not_broadcast(&mut *storage_.write().await, message)
                            .await?;
                    }
                    Err(e) => {
                        log::warn!("failed to parse message from the gossip network: {}", e);
                    }
                }
            }
            Result::<(), Error>::Ok(())
        };
        let storage = Arc::clone(&self.storage);
        let rpc_task = async move {
            run_server(
                rpc_port,
                [(
                    "x".to_owned(),
                    create_http_object(Arc::new(StorageWrapper { storage })
                        as Arc<dyn DistributedMessageSetRpcInterface>),
                )]
                .iter()
                .cloned()
                .collect(),
            )
            .await;
            Ok(())
        };
        Ok(tokio::spawn(async move {
            let x = try_join!(rpc_task, gossip_serve_task, broadcast_task, fetch_task);
            match x {
                Ok(_) => Ok(()),
                Err(e) => Err(e),
            }
        }))
    }
}
