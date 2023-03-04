use super::Storage;
use super::*;
use async_trait::async_trait;
use eyre::eyre;
use futures::future::join;
use futures::prelude::*;
use serde_tc::http::*;
use serde_tc::{serde_tc_full, StubCall};
use simperby_common::*;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

const STATE_FILE_PATH: &str = "_state.json";
type DmsKey = String;

#[derive(Debug, Clone, Serialize)]
pub struct Message {
    data: String,
    dms_key: DmsKey,
    signature: TypedSignature<(String, DmsKey)>,
}

impl Message {
    pub fn new(
        data: String,
        dms_key: DmsKey,
        signature: TypedSignature<(String, DmsKey)>,
    ) -> Result<Self, CryptoError> {
        signature.verify(&(data.clone(), dms_key.clone()))?;
        Ok(Self {
            data,
            dms_key,
            signature,
        })
    }

    pub fn data(&self) -> &str {
        &self.data
    }

    pub fn signature(&self) -> &TypedSignature<(String, DmsKey)> {
        &self.signature
    }
}

impl ToHash256 for Message {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_spb::to_vec(self).unwrap())
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
    pub dms_key: DmsKey,
    pub signature: TypedSignature<(String, DmsKey)>,
}

impl RawMessage {
    pub fn try_into_message(self) -> eyre::Result<Message> {
        Message::new(self.data, self.dms_key, self.signature).map_err(|e| eyre!(e))
    }

    pub fn from_message(message: Message) -> Self {
        RawMessage {
            data: message.data().to_owned(),
            dms_key: message.dms_key.to_owned(),
            signature: message.signature().to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub dms_key: DmsKey,
}

pub struct DistributedMessageSet<S> {
    storage: Arc<RwLock<S>>,
    filter: Arc<dyn MessageFilter>,
    key: DmsKey,
}

impl<S> std::fmt::Debug for DistributedMessageSet<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "?")
    }
}

/// A **cumulative** set that is shared in the p2p network, backed by the local file system.
///
/// One of the notable characteristics of blockchain is that it is based on heights;
/// The key idea here is that we retain an instance (both in memory or on disk)
/// of `DistributedMessageSet` only for a specific height,
/// and discard it if the height progresses, creating a new and empty one again.
///
/// Note that this struct represents only the **client side**.
/// The server side is implemented in [`serve`].
///
/// For every method,
/// - It locks the storage.
/// - If the given directory is locked (possibly by another instance of `DistributedMessageSet`),
///   it will `await` until the lock is released.
///  - It takes 'Arc<RwLock<Self>>' instead of `self` if network clients are used.
impl<S: Storage> DistributedMessageSet<S> {
    /// Creates a message set instance.
    ///
    /// If the storage is empty, it creates a new one.
    /// If not, check the `dms_key` with the stored one.
    /// It loads the storage if the `dms_key` is the same.
    /// It clears all and initializes a new one if not.
    ///
    /// - `dms_key`: The unique key for distinguishing the DMS instance.
    /// Note that it will be further extended with the height.
    pub async fn new(storage: S, dms_key: String) -> Result<Self, Error> {
        let dms_key_ = dms_key.clone();
        let mut this = Self {
            storage: Arc::new(RwLock::new(storage)),
            filter: Arc::new(DummyFilter),
            key: dms_key_,
        };
        if this.storage.read().await.list_files().await?.is_empty() {
            this.write_state(State { dms_key }).await?;
        } else {
            let state: State =
                serde_spb::from_str(&this.storage.read().await.read_file(STATE_FILE_PATH).await?)?;
            if state.dms_key != dms_key {
                this.storage.write().await.remove_all_files().await?;
                this.write_state(State { dms_key }).await?;
            }
        };
        Ok(this)
    }

    pub async fn clear(&mut self, dms_key: DmsKey) -> Result<(), Error> {
        self.storage.write().await.remove_all_files().await?;
        self.write_state(State { dms_key }).await?;
        Ok(())
    }

    pub fn get_key(&self) -> String {
        self.key.clone()
    }

    pub fn set_filter(&mut self, filter: Arc<dyn MessageFilter>) {
        self.filter = filter;
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
            .map(|d| serde_spb::from_str::<RawMessage>(&d))
            .collect::<Result<Vec<RawMessage>, _>>()?;
        let messages = messages
            .into_iter()
            .map(|d| d.try_into_message())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(messages)
    }

    pub async fn add_message(&mut self, message: Message) -> Result<(), Error> {
        self.storage
            .write()
            .await
            .add_or_overwrite_file(
                &format!("{}.json", message.to_hash256()),
                serde_spb::to_string(&message).unwrap(),
            )
            .await?;
        Ok(())
    }

    pub async fn write_state(&mut self, state: State) -> Result<(), Error> {
        self.storage
            .write()
            .await
            .add_or_overwrite_file(STATE_FILE_PATH, serde_spb::to_string(&state)?)
            .await?;
        Ok(())
    }

    /// Fetches unknown messages from the peers using an RPC protocol,
    /// and adds them to the local storage.
    pub async fn fetch(
        this: Arc<RwLock<Self>>,
        network_config: &ClientNetworkConfig,
    ) -> Result<(), Error> {
        let mut tasks = Vec::new();
        let messages = this.read().await.read_messages().await?;
        let known_messages = messages
            .into_iter()
            .map(|m| m.to_hash256())
            .collect::<Vec<_>>();

        for peer in &network_config.peers {
            let known_messages_ = known_messages.clone();
            let this_ = Arc::clone(&this);
            let task = async move {
                let this_read = this_.read().await;
                let filter = Arc::clone(&this_read.filter);
                let port_key = format!("dms-{}", this_read.key);
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
                let raw_messages = stub
                    .get_messages(this_read.key.clone(), known_messages_)
                    .await
                    .map_err(|e| eyre!("{}", e))?
                    .map_err(|e| eyre!(e))?;
                // Important: drop the lock before `write()`
                drop(this_read);
                for raw_message in raw_messages {
                    let message = raw_message.try_into_message()?;
                    filter.filter(&message).map_err(|e| eyre!("{}", e))?;
                    this_.write().await.add_message(message).await?;
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
        let messages = this
            .read()
            .await
            .read_messages()
            .await?
            .into_iter()
            .map(RawMessage::from_message)
            .collect::<Vec<_>>();
        for peer in &network_config.peers {
            let key = this.read().await.key.clone();
            let port_key = format!("dms-{key}");
            let messages_ = messages.clone();
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
                stub.add_messages(key.clone(), messages_.clone())
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
}

/// The interface that will be wrapped into an HTTP RPC server for the peers.
#[serde_tc_full]
trait DistributedMessageSetRpcInterface: Send + Sync + 'static {
    /// Returns the messages except `knowns`.
    async fn get_messages(
        &self,
        dms_key: DmsKey,
        knowns: Vec<Hash256>,
    ) -> Result<Vec<RawMessage>, String>;

    /// Requests this node to accept a new message.
    async fn add_messages(&self, dms_key: DmsKey, messages: Vec<RawMessage>) -> Result<(), String>;
}

struct DmsWrapper<S: Storage> {
    #[allow(clippy::type_complexity)]
    /// This is an `Option` because we have to explicitly drop the server
    /// (it could live forever in the RPC server (`axum`) otherwise)
    dms: Arc<parking_lot::RwLock<Option<Arc<RwLock<DistributedMessageSet<S>>>>>>,
}

#[async_trait]
impl<S: Storage> DistributedMessageSetRpcInterface for DmsWrapper<S> {
    async fn get_messages(
        &self,
        dms_key: DmsKey,
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
        let dms_key_ = dms.read().await.key.clone();
        if dms_key != dms_key_ {
            return Err(format!("key mismatch: requested {dms_key}, but {dms_key_}"));
        }
        let knowns: HashSet<_> = knowns.into_iter().collect();
        let messages: Vec<_> = messages
            .drain(..)
            .filter(|m| !knowns.contains(&m.to_hash256()))
            .map(RawMessage::from_message)
            .collect();
        Ok(messages)
    }

    async fn add_messages(&self, dms_key: DmsKey, messages: Vec<RawMessage>) -> Result<(), String> {
        let dms = Arc::clone(
            self.dms
                .read()
                .as_ref()
                .ok_or_else(|| "server terminated".to_owned())?,
        );
        let dms_key_ = dms.read().await.key.clone();
        if dms_key != dms.read().await.key {
            return Err(format!("key mismatch: requested {dms_key}, but {dms_key_}"));
        }
        for message in messages {
            let message = message.try_into_message().map_err(|e| e.to_string())?;
            dms.write()
                .await
                .add_message(message)
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

/// Runs a DMS server. This function will block the current thread.
pub async fn serve<S: Storage>(
    dms: Arc<RwLock<DistributedMessageSet<S>>>,
    network_config: ServerNetworkConfig,
) -> Result<(), Error> {
    let port_key = format!("dms-{}", dms.read().await.key);
    let port = network_config
        .ports
        .get(&port_key)
        .ok_or_else(|| eyre!(format!("`ports` has no field of {port_key}")))?;

    let rpc_task = async move {
        let wrapped_dms = Arc::new(parking_lot::RwLock::new(Some(dms)));
        let wrapped_dms_ = Arc::clone(&wrapped_dms);
        struct DropHelper<T> {
            wrapped_dms: Arc<parking_lot::RwLock<Option<Arc<RwLock<T>>>>>,
        }
        impl<T> Drop for DropHelper<T> {
            fn drop(&mut self) {
                self.wrapped_dms.write().take().unwrap();
            }
        }
        let _drop_helper = DropHelper { wrapped_dms };
        run_server(
            *port,
            [(
                "dms".to_owned(),
                create_http_object(Arc::new(DmsWrapper { dms: wrapped_dms_ })
                    as Arc<dyn DistributedMessageSetRpcInterface>),
            )]
            .iter()
            .cloned()
            .collect(),
        )
        .await;
    };
    rpc_task.await;
    Ok(())
}

/// Runs a DMS client with auto-sync. This function will block the current thread.
pub async fn sync<S: Storage>(
    dms: Arc<RwLock<DistributedMessageSet<S>>>,
    fetch_interval: Option<Duration>,
    broadcast_interval: Option<Duration>,
    network_config: ClientNetworkConfig,
) -> Result<(), Error> {
    let dms_ = Arc::clone(&dms);
    let network_config_ = network_config.clone();
    let fetch_task = async move {
        if let Some(interval) = fetch_interval {
            loop {
                if let Err(e) =
                    DistributedMessageSet::<S>::fetch(Arc::clone(&dms_), &network_config_).await
                {
                    log::warn!("failed to parse message from the RPC-fetch: {}", e);
                }
                tokio::time::sleep(interval).await;
            }
        } else {
            futures::future::pending::<()>().await;
        }
    };
    let dms_ = Arc::clone(&dms);
    let broadcast_task = async move {
        if let Some(interval) = broadcast_interval {
            loop {
                if let Err(e) =
                    DistributedMessageSet::<S>::broadcast(Arc::clone(&dms_), &network_config).await
                {
                    log::warn!("failed to parse message from the RPC-broadcast: {}", e);
                }
                tokio::time::sleep(interval).await;
            }
        } else {
            futures::future::pending::<()>().await;
        }
    };
    join(fetch_task, broadcast_task).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::StorageImpl;
    use futures::future::join_all;
    use rand::prelude::*;
    use simperby_test_suite::*;

    type Dms = DistributedMessageSet<StorageImpl>;

    fn generate_random_string() -> String {
        let mut rng = rand::thread_rng();
        let s1: u128 = rng.gen();
        let s2: u128 = rng.gen();
        Hash256::hash(format!("{s1}{s2}").as_bytes()).to_string()[0..16].to_owned()
    }

    /// Returns the only-serving-node and the others, with the `Peer` info for the serving node.
    /// `size` includes the serving node.
    ///
    /// TODO: clients having themselves as a peer must be allowed.
    fn generate_node_configs(
        serving_node_port: u16,
        size: usize,
    ) -> (ServerNetworkConfig, Vec<ClientNetworkConfig>) {
        let mut client_configs = Vec::new();
        let mut keys = Vec::new();
        for _ in 0..size {
            keys.push(generate_keypair_random());
        }
        let network_id = generate_random_string();
        let server_peer = Peer {
            public_key: keys[0].0.clone(),
            name: format!("{}", keys[0].0),
            address: SocketAddrV4::new("127.0.0.1".parse().unwrap(), serving_node_port),
            ports: [(format!("dms-{network_id}"), serving_node_port)]
                .iter()
                .cloned()
                .collect(),
            message: "".to_owned(),
            recently_seen_timestamp: 0,
        };

        for i in 0..size - 1 {
            client_configs.push(ClientNetworkConfig {
                network_id: network_id.clone(),
                members: keys.iter().map(|(x, _)| x).cloned().collect(),
                public_key: keys[i + 1].0.clone(),
                private_key: keys[i + 1].1.clone(),
                peers: vec![server_peer.clone()],
            });
        }
        (
            ServerNetworkConfig {
                network_id: network_id.clone(),
                ports: [(format!("dms-{network_id}"), serving_node_port)]
                    .iter()
                    .cloned()
                    .collect(),
                members: keys.iter().map(|(x, _)| x).cloned().collect(),
                public_key: keys[0].0.clone(),
                private_key: keys[0].1.clone(),
            },
            client_configs,
        )
    }

    async fn create_dms(key: String) -> Dms {
        let path = create_temp_dir();
        StorageImpl::create(&path).await.unwrap();
        let storage = StorageImpl::open(&path).await.unwrap();
        Dms::new(storage, key).await.unwrap()
    }

    #[tokio::test]
    async fn single_1() {
        let key = generate_random_string();
        let mut dms = create_dms(key.clone()).await;
        let network_config = generate_node_configs(dispense_port(), 1).0;

        for i in 0..10 {
            let msg = format!("{i}");
            dms.add_message(Message {
                data: msg.clone(),
                dms_key: key.clone(),
                signature: TypedSignature::sign(&(msg, key.clone()), &network_config.private_key)
                    .unwrap(),
            })
            .await
            .unwrap();
        }

        let messages = dms.read_messages().await.unwrap();
        assert_eq!(
            (0..10)
                .into_iter()
                .map(|x| format!("{x}"))
                .collect::<std::collections::BTreeSet<_>>(),
            messages
                .into_iter()
                .map(|x| x.data)
                .collect::<std::collections::BTreeSet<_>>()
        );
    }

    async fn run_client_node(
        dms: Arc<RwLock<Dms>>,
        message_to_create: Vec<usize>,
        network_config: ClientNetworkConfig,
        broadcast_interval: Option<Duration>,
        fetch_interval: Option<Duration>,
        message_insertion_interval: Duration,
        final_sleep: Duration,
    ) {
        let dms_ = Arc::clone(&dms);
        let network_config_ = network_config.clone();
        let sync_task = tokio::spawn(async move {
            sync(dms_, fetch_interval, broadcast_interval, network_config_)
                .await
                .unwrap();
        });
        for i in message_to_create {
            tokio::time::sleep(message_insertion_interval).await;
            let msg = format!("{i}");
            dms.write()
                .await
                .add_message(Message {
                    data: msg.clone(),
                    dms_key: network_config.network_id.clone(),
                    signature: TypedSignature::sign(
                        &(msg, network_config.network_id.clone()),
                        &network_config.private_key,
                    )
                    .unwrap(),
                })
                .await
                .unwrap();
        }
        tokio::time::sleep(final_sleep).await;
        sync_task.abort();
    }

    #[tokio::test]
    async fn multi_1() {
        let (server_network_config, client_network_configs) =
            generate_node_configs(dispense_port(), 5);
        let key = server_network_config.network_id.clone();

        let server_dms = Arc::new(RwLock::new(create_dms(key.clone()).await));
        let mut client_dmses = Vec::new();
        let mut tasks = Vec::new();

        let range_step = 10;
        for (i, client_network_config) in client_network_configs.iter().enumerate() {
            let dms = Arc::new(RwLock::new(create_dms(key.clone()).await));
            tasks.push(run_client_node(
                Arc::clone(&dms),
                (i * range_step..(i + 1) * range_step).into_iter().collect(),
                client_network_config.clone(),
                Some(Duration::from_millis(400)),
                Some(Duration::from_millis(400)),
                Duration::from_millis(50),
                Duration::from_millis(3000),
            ));
            client_dmses.push(dms);
        }
        tokio::spawn(serve(Arc::clone(&server_dms), server_network_config));
        join_all(tasks).await;

        for dms in client_dmses {
            let messages = dms.read().await.read_messages().await.unwrap();
            assert_eq!(
                (0..(range_step * client_network_configs.len()))
                    .into_iter()
                    .map(|x| format!("{x}"))
                    .collect::<std::collections::BTreeSet<_>>(),
                messages
                    .into_iter()
                    .map(|x| x.data)
                    .collect::<std::collections::BTreeSet<_>>()
            );
        }
    }
}
