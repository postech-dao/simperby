use super::Storage;
use super::*;
use anyhow::anyhow;
use async_trait::async_trait;
use futures::prelude::*;
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

/// Decides whether a message should be accepted or not.
pub trait MessageFilter: Send + Sync + 'static {
    fn filter(&self, message: &Message) -> Result<(), String>;
}

/// A message before verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawMessage {
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

/// The interface that will be wrapped into an HTTP RPC server for the peers.
#[serde_tc_full]
trait DistributedMessageSetRpcInterface: Send + Sync + 'static {
    /// Returns the messages except `knowns`.
    async fn get_message(
        &self,
        height: BlockHeight,
        knowns: Vec<Hash256>,
    ) -> Result<Vec<RawMessage>, String>;

    /// Requests this node to accept a new message.
    async fn add_messages(
        &self,
        height: BlockHeight,
        messages: Vec<RawMessage>,
    ) -> Result<(), String>;
}

struct DmsWrapper<N: GossipNetwork, S: Storage> {
    #[allow(clippy::type_complexity)]
    dms: Arc<parking_lot::RwLock<Option<Arc<RwLock<DistributedMessageSet<N, S>>>>>>,
}

#[async_trait]
impl<N: GossipNetwork, S: Storage> DistributedMessageSetRpcInterface for DmsWrapper<N, S> {
    async fn get_message(
        &self,
        height: BlockHeight,
        knowns: Vec<Hash256>,
    ) -> Result<Vec<RawMessage>, String> {
        let dms = Arc::clone(
            self.dms
                .read()
                .as_ref()
                .ok_or_else(|| "server terminated".to_owned())?,
        );
        let mut messages = dms
            .read()
            .await
            .read_messages()
            .await
            .map_err(|e| e.to_string())?;
        let height_ = dms
            .read()
            .await
            .read_state()
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

    async fn add_messages(
        &self,
        height: BlockHeight,
        messages: Vec<RawMessage>,
    ) -> Result<(), String> {
        let dms = Arc::clone(
            self.dms
                .read()
                .as_ref()
                .ok_or_else(|| "server terminated".to_owned())?,
        );
        let height_ = dms
            .read()
            .await
            .read_state()
            .await
            .map_err(|e| e.to_string())?
            .height;
        if height != height_ {
            return Err(format!(
                "height mismatch: requested {}, but {}",
                height, height_
            ));
        }
        for message in messages {
            let message = message.into_message().map_err(|e| e.to_string())?;
            DistributedMessageSet::<N, S>::add_message_but_not_broadcast(
                &mut (*dms.write().await.storage.write().await),
                message,
            )
            .await
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

struct DummyFilter;

impl MessageFilter for DummyFilter {
    fn filter(&self, _message: &Message) -> Result<(), String> {
        Ok(())
    }
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
    filter: Arc<dyn MessageFilter>,
    peers: SharedKnownPeers,
    key: String,
    _marker: std::marker::PhantomData<N>,
}

impl<N, S> std::fmt::Debug for DistributedMessageSet<N, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "?")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub network_config: NetworkConfig,
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
    /// - `dms_key`: The unique key for distinguishing the DMS instance
    /// among the networks and among the types (e.g. governance, consensus, ...).
    pub async fn create(storage: &mut S, height: u64, dms_key: String) -> Result<(), Error> {
        storage.remove_all_files().await?;
        Self::write_state(
            storage,
            State {
                height,
                key: dms_key,
            },
        )
        .await?;
        Ok(())
    }

    /// Opens an existing storage with the given directory.
    pub async fn open(storage: S, config: Config, peers: SharedKnownPeers) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let state: State = serde_json::from_str(&storage.read_file(STATE_FILE_PATH).await?)?;
        Ok(Self {
            storage: Arc::new(RwLock::new(storage)),
            config,
            filter: Arc::new(DummyFilter),
            peers,
            key: state.key,
            _marker: std::marker::PhantomData,
        })
    }

    pub fn get_key(&self) -> String {
        self.key.clone()
    }

    pub fn set_filter(&mut self, filter: Arc<dyn MessageFilter>) {
        self.filter = filter;
    }

    /// Fetches the unknown messages from the peers and updates the storage.
    pub async fn fetch(&mut self) -> Result<(), Error> {
        let mut tasks = Vec::new();
        let messages = self.read_messages().await?;
        let known_messages = messages
            .into_iter()
            .map(|m| m.to_hash256())
            .collect::<Vec<_>>();
        let state = self.read_state().await?;
        let height = state.height;
        for peer in self.peers.read().await {
            let storage = Arc::clone(&self.storage);
            let filter = Arc::clone(&self.filter);
            let port_key = format!("dms-{}", state.key);
            let known_messages_ = known_messages.clone();
            let task = async move {
                let stub = DistributedMessageSetRpcInterfaceStub::new(Box::new(HttpClient::new(
                    format!(
                        "{}:{}/dms",
                        peer.address.ip(),
                        peer.ports
                            .get(&port_key)
                            .ok_or_else(|| anyhow!("can't find port key: {}", port_key))?
                    ),
                    reqwest::Client::new(),
                )));
                let raw_messages = stub
                    .get_message(height, known_messages_)
                    .await?
                    .map_err(|e| anyhow!(e))?;
                let mut storage = storage.write().await;
                for raw_message in raw_messages {
                    let message = raw_message.into_message()?;
                    filter.filter(&message).map_err(|e| anyhow!("{}", e))?;
                    Self::add_message_but_not_broadcast(&mut *storage, message).await?;
                }
                Result::<(), Error>::Ok(())
            };
            tasks.push(task);
        }
        let results = future::join_all(tasks).await;
        for (result, peer) in results.into_iter().zip(self.peers.read().await.iter()) {
            if let Err(e) = result {
                log::warn!("failed to fetch from client {:?}: {}", peer, e);
            }
        }
        Ok(())
    }

    /// Adds the given message to the storage, immediately broadcasting it to the network.
    ///
    /// Note that it is guaranteed that the message will not be broadcasted unless it
    /// is successfully added to the storage. (but it is not guaranteed for the other way around)
    pub async fn add_message(&mut self, message: Message) -> Result<(), Error> {
        Self::add_message_but_not_broadcast(&mut *(self.storage.write().await), message.clone())
            .await?;
        self.broadcast_all().await?;
        Ok(())
    }

    /// Tries to broadcast all the message that this DMS instance has.
    pub async fn broadcast_all(&self) -> Result<(), Error> {
        let mut tasks1 = Vec::new();
        let state = self.read_state().await?;
        let messages = self
            .read_messages()
            .await?
            .into_iter()
            .map(RawMessage::from_message)
            .collect::<Vec<_>>();
        let height = state.height;

        for peer in self.peers.read().await {
            let port_key = format!("dms-{}", state.key);
            let messages_ = messages.clone();
            let task = async move {
                let stub = DistributedMessageSetRpcInterfaceStub::new(Box::new(HttpClient::new(
                    format!(
                        "{}:{}/dms",
                        peer.address.ip(),
                        peer.ports
                            .get(&port_key)
                            .ok_or_else(|| anyhow!("can't find port key: {}", port_key))?
                    ),
                    reqwest::Client::new(),
                )));
                stub.add_messages(height, messages_.clone())
                    .await?
                    .map_err(|e| anyhow!(e))?;
                Result::<(), Error>::Ok(())
            };
            tasks1.push((task, format!("RPC message add to {}", peer.public_key)));
        }
        let peers_ = self.peers.read().await;
        let tasks2 = messages.into_iter().map(|message| {
            let network_config = self.config.network_config.clone();
            let peers = peers_.clone();
            let message_hash = message.data.to_hash256();
            (
                async move {
                    N::broadcast(
                        &network_config,
                        &peers,
                        serde_json::to_vec(&message).unwrap(),
                    )
                    .await?;
                    Result::<(), Error>::Ok(())
                },
                format!("broadcast message {} to all peers", message_hash),
            )
        });
        let mut tasks = Vec::new();
        let mut messages = Vec::new();
        for (task, msg) in tasks1 {
            tasks.push(task.boxed());
            messages.push(msg);
        }
        for (task, msg) in tasks2 {
            tasks.push(task.boxed());
            messages.push(msg);
        }
        let results = future::join_all(tasks).await;
        for (result, msg) in results.into_iter().zip(messages.iter()) {
            if let Err(e) = result {
                log::warn!("failure in {}: {}", msg, e);
            }
        }
        Ok(())
    }

    /// Reads the messages from the storage.
    pub async fn read_messages(&self) -> Result<Vec<Message>, Error> {
        let files = self.storage.read().await.list_files().await?;
        let tasks = files
            .into_iter()
            .filter(|x| x != STATE_FILE_PATH)
            .map(|f| async move { self.storage.read().await.read_file(&f).await });
        let data = future::join_all(tasks)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        let messages = data
            .into_iter()
            .map(|d| serde_json::from_str::<RawMessage>(&d))
            .collect::<Result<Vec<RawMessage>, _>>()?;
        let messages = messages
            .into_iter()
            .map(|d| d.into_message())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(messages)
    }

    /// Reads the height from the storage.
    pub async fn read_height(&self) -> Result<BlockHeight, Error> {
        let state = self.read_state().await?;
        Ok(state.height)
    }

    /// Advances the height of the message set, discarding all the messages.
    pub async fn advance(&mut self) -> Result<(), Error> {
        let state = self.read_state().await?;
        let mut storage = self.storage.write().await;
        storage.remove_all_files().await?;
        Self::write_state(
            &mut *self.storage.write().await,
            State {
                height: state.height + 1,
                key: state.key,
            },
        )
        .await?;
        Ok(())
    }

    async fn add_message_but_not_broadcast(
        storage: &mut impl Storage,
        message: Message,
    ) -> Result<(), Error> {
        storage
            .add_or_overwrite_file(
                &format!("{}.json", message.to_hash256()),
                serde_json::to_string(&message).unwrap(),
            )
            .await?;
        Ok(())
    }

    async fn read_state(&self) -> Result<State, Error> {
        let state: State =
            serde_json::from_str(&self.storage.read().await.read_file(STATE_FILE_PATH).await?)?;
        Ok(state)
    }

    async fn write_state(storage: &mut impl Storage, state: State) -> Result<(), Error> {
        storage
            .add_or_overwrite_file(STATE_FILE_PATH, serde_json::to_string(&state)?)
            .await?;
        Ok(())
    }

    async fn serve_rpc(this: Arc<RwLock<Self>>, rpc_port: u16) -> Result<(), Error> {
        let wrapped_this = Arc::new(parking_lot::RwLock::new(Some(this)));
        let wrapped_this_ = Arc::clone(&wrapped_this);

        struct DropHelper<T> {
            wrapped_this: Arc<parking_lot::RwLock<Option<Arc<RwLock<T>>>>>,
        }
        impl<T> Drop for DropHelper<T> {
            fn drop(&mut self) {
                self.wrapped_this.write().take().unwrap();
            }
        }
        let _drop_helper = DropHelper { wrapped_this };
        run_server(
            rpc_port,
            [(
                "dms".to_owned(),
                create_http_object(Arc::new(DmsWrapper { dms: wrapped_this_ })
                    as Arc<dyn DistributedMessageSetRpcInterface>),
            )]
            .iter()
            .cloned()
            .collect(),
        )
        .await;
        Ok(())
    }

    async fn serve_fetch(this: Arc<RwLock<Self>>) -> Result<(), Error> {
        let interval = if let Some(x) = this.read().await.config.fetch_interval {
            x
        } else {
            return Result::<(), Error>::Ok(());
        };
        loop {
            if let Err(e) = Self::fetch(&mut *this.write().await).await {
                log::warn!("failed to parse message from the gossip network: {}", e);
            }
            tokio::time::sleep(interval).await;
        }
    }

    async fn serve_broadcast(this: Arc<RwLock<Self>>) -> Result<(), Error> {
        let interval = if let Some(x) = this.read().await.config.broadcast_interval {
            x
        } else {
            return Result::<(), Error>::Ok(());
        };
        loop {
            if let Err(e) = this.read().await.broadcast_all().await {
                log::warn!("failed to broadcast to the network: {}", e);
            }
            tokio::time::sleep(interval).await;
        }
    }

    async fn serve_gossip(this: Arc<RwLock<Self>>) -> Result<(), Error> {
        let mut recv = N::serve(
            this.read().await.config.network_config.clone(),
            this.read().await.peers.clone(),
        )
        .await?;
        while let Some(m) = recv.0.recv().await {
            let result = async {
                let message: RawMessage = serde_json::from_slice(&m)?;
                let message = message.into_message()?;
                this.read()
                    .await
                    .filter
                    .filter(&message)
                    .map_err(|e| anyhow!("{}", e))?;
                Self::add_message_but_not_broadcast(
                    &mut *this.read().await.storage.write().await,
                    message,
                )
                .await?;
                Result::<(), anyhow::Error>::Ok(())
            }
            .await;
            if let Err(e) = result {
                log::warn!("failed to receive a message from the gossip network: {}", e);
            }
        }
        Ok(())
    }

    /// Serves the gossip network node and the RPC server indefinitely, constantly updating the storage.
    ///
    /// TODO: currently it just returns itself after the given time.
    pub async fn serve(self, time_in_ms: u64) -> Result<Self, Error> {
        let port_key = format!("dms-{}", self.read_state().await?.key);
        let port = *self
            .config
            .network_config
            .ports
            .get(&port_key)
            .ok_or_else(|| anyhow!(format!("`ports` has no field of {}", port_key)))?;

        let this = Arc::new(RwLock::new(self));
        let this_ = Arc::clone(&this);
        let rpc_task = async move { Self::serve_rpc(this_, port).await.map(|_| false) };
        let this_ = Arc::clone(&this);
        let fetch_task = async move { Self::serve_fetch(this_).await.map(|_| false) };
        let this_ = Arc::clone(&this);
        let broadcast_task = async move { Self::serve_broadcast(this_).await.map(|_| false) };
        let this_ = Arc::clone(&this);
        let gossip_task = async move { Self::serve_gossip(this_).await.map(|_| false) };

        let mut tasks = vec![
            rpc_task.boxed(),
            fetch_task.boxed(),
            broadcast_task.boxed(),
            gossip_task.boxed(),
            tokio::time::sleep(std::time::Duration::from_millis(time_in_ms))
                .map(|_| Ok(true))
                .boxed(),
        ];
        loop {
            let (result, _, remaining_futures) = future::select_all(tasks).await;
            if result? {
                // `remaining_futures` drops here.
                break;
            }
            tasks = remaining_futures;
        }
        Ok(Arc::try_unwrap(this).unwrap().into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::StorageImpl;
    use rand::prelude::*;
    use simperby_test_suite::*;

    use futures::future::join_all;

    // TODO: Add other DMS types that use a working gossip network.
    type Dms = DistributedMessageSet<crate::primitives::DummyGossipNetwork, StorageImpl>;

    async fn sleep(ms: u64) {
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    }

    fn generate_random_string() -> String {
        let mut rng = rand::thread_rng();
        let s1: u128 = rng.gen();
        let s2: u128 = rng.gen();
        Hash256::hash(format!("{}{}", s1, s2).as_bytes()).to_string()[0..16].to_owned()
    }

    /// Returns the only-serving-node and the others, with the `Peer` info for the serving node.
    /// `size` includes the serving node.
    fn generate_node_configs(
        serving_node_port: u16,
        size: usize,
    ) -> (NetworkConfig, Vec<NetworkConfig>, Peer) {
        let mut configs = Vec::new();
        let mut keys = Vec::new();
        for _ in 0..size {
            keys.push(generate_keypair_random());
        }
        let network_id = generate_random_string();

        for i in 0..size - 1 {
            configs.push(NetworkConfig {
                network_id: network_id.clone(),
                ports: Default::default(),
                members: keys.iter().map(|(x, _)| x).cloned().collect(),
                public_key: keys[i + 1].0.clone(),
                private_key: keys[i + 1].1.clone(),
            });
        }
        (
            NetworkConfig {
                network_id: network_id.clone(),
                ports: [(format!("dms-{}", network_id), serving_node_port)]
                    .iter()
                    .cloned()
                    .collect(),
                members: keys.iter().map(|(x, _)| x).cloned().collect(),
                public_key: keys[0].0.clone(),
                private_key: keys[0].1.clone(),
            },
            configs,
            Peer {
                public_key: keys[0].0.clone(),
                name: format!("{}", keys[0].0),
                address: SocketAddrV4::new("127.0.0.1".parse().unwrap(), serving_node_port),
                ports: [(format!("dms-{}", network_id), serving_node_port)]
                    .iter()
                    .cloned()
                    .collect(),
                message: "".to_owned(),
                recently_seen_timestamp: 0,
            },
        )
    }

    async fn setup(network_config: NetworkConfig, peers: SharedKnownPeers) -> Dms {
        let dir = create_temp_dir();
        StorageImpl::create(&dir).await.unwrap();
        let mut storage = StorageImpl::open(&dir).await.unwrap();
        Dms::create(&mut storage, 0, network_config.network_id.clone())
            .await
            .unwrap();
        let config = Config {
            network_config,
            fetch_interval: Some(Duration::from_millis(100)),
            broadcast_interval: None,
        };
        Dms::open(storage, config, peers).await.unwrap()
    }

    #[tokio::test]
    async fn single_1() {
        let mut dms = setup(
            NetworkConfig {
                network_id: "doesn't matter".to_owned(),
                ports: Default::default(),
                members: Default::default(),
                public_key: PublicKey::zero(),
                private_key: PrivateKey::zero(),
            },
            SharedKnownPeers::new(Default::default()),
        )
        .await;
        let network_config = generate_node_configs(4200, 1).0;

        for i in 0..10 {
            let msg = format!("{}", i);
            dms.add_message(Message {
                data: msg.clone(),
                signature: TypedSignature::sign(&msg, &network_config.private_key).unwrap(),
            })
            .await
            .unwrap();
        }

        let messages = dms.read_messages().await.unwrap();
        assert_eq!(
            (0..10)
                .into_iter()
                .map(|x| format!("{}", x))
                .collect::<std::collections::BTreeSet<_>>(),
            messages
                .into_iter()
                .map(|x| x.data)
                .collect::<std::collections::BTreeSet<_>>()
        );
    }

    async fn run_non_server_node_1(
        index: usize,
        mut dms: Dms,
        my_numbers: Vec<usize>,
        other_numbers: Vec<usize>,
        network_config: NetworkConfig,
    ) {
        // Add the assigned messages to the DMS
        for i in &my_numbers {
            let msg = format!("{}", i);
            dms.add_message(Message {
                data: msg.clone(),
                signature: TypedSignature::sign(&msg, &network_config.private_key).unwrap(),
            })
            .await
            .unwrap();
        }

        // Try to sync
        let mut count = 0;
        loop {
            sleep(500).await;
            dms.broadcast_all().await.unwrap();
            sleep(500).await;
            dms.fetch().await.unwrap();
            let messages = dms.read_messages().await.unwrap();
            println!(
                "NODE #{} on trial #{}: {}%",
                index,
                count,
                messages.len() as f64 / other_numbers.len() as f64 * 100.0
            );
            if messages.len() == other_numbers.len() {
                break;
            }
            count += 1;
        }

        // Read the messages and check that they are correct
        let mut messages = dms
            .read_messages()
            .await
            .unwrap()
            .into_iter()
            .map(|x| x.data.parse::<usize>().unwrap())
            .collect::<Vec<_>>();
        messages.sort();
        let mut expected = other_numbers;
        expected.sort();
        assert_eq!(expected, messages);
    }

    /// Multi-node test assuming dummy gossip network and a single server node.
    #[tokio::test]
    async fn multi_dummy_gn_single_sn_1() {
        env_logger::init();
        let rpc_port = 4201;
        let n = 5;
        let (server_network_config, network_configs, server_peer) =
            generate_node_configs(rpc_port, n + 1);
        let serving_node_dms = setup(
            server_network_config.clone(),
            SharedKnownPeers::new(Default::default()),
        )
        .await;
        let mut tasks = Vec::new();
        let k = 5;
        let all_numbers = (0..k * n).into_iter().collect::<Vec<_>>();
        for (i, network_config) in network_configs.iter().enumerate() {
            let dms = setup(
                server_network_config.clone(),
                SharedKnownPeers::new_static(vec![server_peer.clone()]),
            )
            .await;
            let numbers = ((i * k)..(i * k + k)).into_iter().collect::<Vec<_>>();
            tasks.push(run_non_server_node_1(
                i,
                dms,
                numbers,
                all_numbers.clone(),
                network_config.clone(),
            ));
        }
        let handle = tokio::spawn(async move {
            serving_node_dms.serve(15000).await.unwrap();
        });
        join_all(tasks).await;
        handle.await.unwrap();
    }

    // Same, but the server node is not online from the beginning
    #[tokio::test]
    async fn multi_dummy_gn_single_sn_2() {
        let rpc_port = 4202;
        let n = 5;
        let (server_network_config, network_configs, server_peer) =
            generate_node_configs(rpc_port, n + 1);
        let serving_node_dms = setup(
            server_network_config.clone(),
            SharedKnownPeers::new(Default::default()),
        )
        .await;
        let mut tasks = Vec::new();
        let k = 5;
        let all_numbers = (0..k * n).into_iter().collect::<Vec<_>>();
        for (i, network_config) in network_configs.iter().enumerate() {
            let dms = setup(
                server_network_config.clone(),
                SharedKnownPeers::new(Arc::new(RwLock::new(vec![server_peer.clone()]))),
            )
            .await;
            let numbers = ((i * k)..(i * k + k)).into_iter().collect::<Vec<_>>();
            tasks.push(run_non_server_node_1(
                i,
                dms,
                numbers,
                all_numbers.clone(),
                network_config.clone(),
            ));
        }
        let handle = tokio::spawn(async move {
            sleep(5000).await;
            let _server_node = serving_node_dms.serve(15000).await.unwrap();
        });
        join_all(tasks).await;
        handle.await.unwrap();
    }
}
