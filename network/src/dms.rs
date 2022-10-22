use super::Storage;
use super::*;
use anyhow::anyhow;
use serde_tc::http::*;
use serde_tc::{serde_tc_full, StubCall};
use simperby_common::*;
use std::sync::Arc;
use tokio::sync::RwLock;

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
    #[allow(dead_code)]
    pub fn into_message(self) -> anyhow::Result<Message> {
        Message::new(self.data, self.signature).map_err(|e| anyhow!(e))
    }

    #[allow(dead_code)]
    pub fn from_message(message: Message) -> Self {
        RawMessage {
            data: message.data().to_owned(),
            signature: message.signature().to_owned(),
        }
    }
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
    _storage: Arc<RwLock<S>>,
    _marker: std::marker::PhantomData<N>,
}

impl<N: P2PNetwork, S: Storage> DistributedMessageSet<N, S> {
    /// Creates a new and empty storage with the given directory.
    /// If there is already a directory, it discards everything and creates a new one.
    /// You should try `open()` first!
    ///
    /// - `dms_key`: The unique key for distinguishing the DMS.
    pub async fn create(_storage: S, _height: u64, _dms_key: String) -> Result<(), Error> {
        unimplemented!()
    }

    /// Opens an existing storage with the given directory.
    pub async fn open(_storage: S) -> Result<Self, Error>
    where
        Self: Sized,
    {
        unimplemented!()
    }

    /// Fetches the unknown messages from the peers and updates the storage.
    pub async fn fetch(
        &mut self,
        _network_config: &NetworkConfig,
        _known_peers: &[Peer],
    ) -> Result<(), Error> {
        unimplemented!()
    }

    /// Adds the given message to the storage, immediately broadcasting it to the network.
    ///
    /// Note that it is guaranteed that the message will not be broadcasted unless it
    /// is successfully added to the storage. (but it is not guaranteed for the other way around)
    pub async fn add_message(
        &mut self,
        _network_config: &NetworkConfig,
        _known_peers: &[Peer],
        _message: Message,
    ) -> Result<(), Error> {
        unimplemented!()
    }

    /// Reads the messages and the height from the storage.
    pub async fn read_messages(&self) -> Result<(BlockHeight, Vec<Message>), Error> {
        unimplemented!()
    }

    /// Reads the height from the storage.
    pub async fn read_height(&self) -> Result<BlockHeight, Error> {
        unimplemented!()
    }

    /// Advances the height of the message set, discarding all the messages.
    pub async fn advance(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    /// Serves the p2p network node and the RPC server indefinitely, constantly updating the storage.
    pub async fn serve(
        self,
        _network_config: NetworkConfig,
        _rpc_port: u16,
        _peers: SharedKnownPeers,
    ) -> Result<tokio::task::JoinHandle<Result<(), Error>>, Error> {
        unimplemented!()
    }
}
