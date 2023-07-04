mod messages;
mod rpc;
pub mod server;
#[cfg(test)]
mod tests;

use super::Storage;
use super::*;
use async_trait::async_trait;
use eyre::eyre;
use futures::future::join;
use futures::prelude::*;
use messages::*;
use rpc::*;
use serde_tc::http::*;
use serde_tc::{serde_tc_full, StubCall};
use simperby_core::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

const STATE_FILE_PATH: &str = "state.json";

pub type Error = eyre::Error;

pub use messages::{DmsKey, DmsMessage, Message, MessageCommitmentProof};
pub use rpc::PeerStatus;
pub use server::*;

#[derive(thiserror::Error, Debug)]
#[error("dms integrity broken: {msg}")]
pub struct IntegrityError {
    pub msg: String,
}

impl IntegrityError {
    pub fn new(msg: String) -> Self {
        Self { msg }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub dms_key: String,
    pub members: Vec<PublicKey>,
}

pub struct DistributedMessageSet<S, M> {
    storage: Arc<RwLock<S>>,
    config: Config,
    private_key: PrivateKey,
    _marker: std::marker::PhantomData<M>,
}

impl<S, M> std::fmt::Debug for DistributedMessageSet<S, M> {
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
/// The server side is implemented in [`serve()`].
///
/// For every method,
/// - It locks the storage.
/// - If the given directory is locked (possibly by another instance of `DistributedMessageSet`),
///   it will `await` until the lock is released.
/// - It takes 'Arc<RwLock<Self>>' instead of `self` if network clients are used.
///
/// TODO: add read only type that does not require the private key.
impl<S: Storage, M: DmsMessage> DistributedMessageSet<S, M> {
    /// Creates a message set instance.
    ///
    /// If the storage is empty, it creates a new one.
    /// If not, check the `dms_key` with the stored one.
    /// It loads the storage if the `dms_key` is the same.
    /// It clears all and initializes a new one if not.
    ///
    /// - `private_key`: The private key for signing messages.
    pub async fn new(
        mut storage: S,
        config: Config,
        private_key: PrivateKey,
    ) -> Result<Self, Error> {
        if !config.members.contains(&private_key.public_key()) {
            return Err(eyre!("given private key is not in the member list"));
        }

        match storage.read_file(STATE_FILE_PATH).await {
            Ok(x) => {
                let config2: Config = serde_spb::from_str(&x)?;
                if config2 != config {
                    return Err(eyre!("config mismatch: {:?}", config2));
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    storage.remove_all_files().await?;
                    storage
                        .add_or_overwrite_file(
                            STATE_FILE_PATH,
                            serde_spb::to_string(&config).unwrap(),
                        )
                        .await?;
                } else {
                    return Err(e.into());
                }
            }
        }

        Ok(Self {
            storage: Arc::new(RwLock::new(storage)),
            config,
            private_key,
            _marker: std::marker::PhantomData,
        })
    }

    /// Returns the underlying storage.
    ///
    /// This is useful for when you want to store some additional data
    /// under the same file lock that this DMS uses.
    ///
    /// Note that you MUST NOT create or access files that start with `message-`.
    pub fn get_storage(&self) -> Arc<RwLock<S>> {
        Arc::clone(&self.storage)
    }

    pub fn get_config(&self) -> Config {
        self.config.clone()
    }

    pub async fn clear(&mut self) -> Result<(), Error> {
        self.storage.write().await.remove_all_files().await?;
        self.storage
            .write()
            .await
            .add_or_overwrite_file(STATE_FILE_PATH, serde_spb::to_string(&self.config).unwrap())
            .await?;
        Ok(())
    }

    /// Reads the messages from the storage.
    pub async fn read_messages(&self) -> Result<Vec<Message<M>>, Error> {
        let messages = self.read_raw_messages().await?;
        let messages = messages
            .into_iter()
            .map(|(message, metadata)| Message {
                message,
                committers: metadata.committers,
            })
            .collect::<Vec<_>>();
        Ok(messages)
    }

    pub async fn query_message(&self, message_hash: Hash256) -> Result<Option<Message<M>>, Error> {
        Ok(self
            .read_raw_message(message_hash)
            .await?
            .map(|(message, metadata)| Message {
                message,
                committers: metadata.committers,
            }))
    }

    /// Signs the given message and adds it to the storage.
    pub async fn commit_message(&mut self, message: &M) -> Result<(), Error> {
        message.check()?;
        let commitment = message.commit(&self.config.dms_key, &self.private_key)?;
        self.store_message(message, commitment).await?;
        Ok(())
    }

    /// Removes the message from the storage.
    /// If `permanent` is `Some` with the reason, it permanently rejects the message.
    pub async fn remove_message(
        &mut self,
        message_hash: Hash256,
        _permanent: Option<String>,
    ) -> Result<(), Error> {
        self.storage
            .write()
            .await
            .remove_file(&format!("message-{}.json", message_hash))
            .await?;
        Ok(())
    }

    async fn read_raw_message(
        &self,
        message_hash: Hash256,
    ) -> Result<Option<(M, MessageMetadata)>, Error> {
        let data = match self
            .storage
            .read()
            .await
            .read_file(&format!("message-{}.json", message_hash))
            .await
        {
            Ok(x) => x,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Ok(None);
                } else {
                    return Err(e.into());
                }
            }
        };
        let stored_message = serde_spb::from_str::<M>(&data)
            .map_err(|e| IntegrityError::new(format!("can't decode stored data: {e}")))?;
        let data = self
            .storage
            .read()
            .await
            .read_file(&format!("metadata-{}.json", message_hash))
            .await?;
        let metadata = serde_spb::from_str::<MessageMetadata>(&data)
            .map_err(|e| IntegrityError::new(format!("can't decode stored data: {e}")))?;
        Ok(Some((stored_message, metadata)))
    }

    async fn read_raw_messages(&self) -> Result<Vec<(M, MessageMetadata)>, Error> {
        let files = self.storage.read().await.list_files().await?;
        let tasks = files
            .iter()
            .filter(|x| x.starts_with("message-"))
            .map(|f| async move {
                self.storage
                    .read()
                    .await
                    .read_file(f)
                    .await
                    .map(|message| (message, f.to_owned()))
            });
        let messages = future::join_all(tasks)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        let tasks = messages.into_iter().map(|(message, file_name)| async move {
            // TODO: it must be an integrity error if not found
            self.storage
                .read()
                .await
                .read_file(&format!("metadata-{}", &file_name[8..]))
                .await
                .map(|metadata| (metadata, message))
        });
        let messages = future::join_all(tasks)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;

        let mut result = Vec::new();
        for (metadata, message) in &messages {
            let metadata = serde_spb::from_str::<MessageMetadata>(metadata)
                .map_err(|e| IntegrityError::new(format!("can't decode stored data: {e}")))?;
            let message = serde_spb::from_str::<M>(message)
                .map_err(|e| IntegrityError::new(format!("can't decode stored data: {e}")))?;
            result.push((message, metadata));
        }
        Ok(result)
    }

    fn test_membership(&self, member: &PublicKey) -> bool {
        self.config.members.contains(member)
    }

    async fn receive_packet(&mut self, packet: Packet) -> Result<(), Error> {
        let message = serde_spb::from_slice::<M>(&packet.message)?;
        message.verify_commitment(&packet.commitment, &self.config.dms_key)?;
        if !self.test_membership(&packet.commitment.committer) {
            return Err(eyre!("commitment committer is not a member"));
        }
        self.store_message(&message, packet.commitment).await?;
        Ok(())
    }

    async fn store_message(
        &mut self,
        message: &M,
        commitment: MessageCommitmentProof,
    ) -> Result<(), Error> {
        let message_hash = message.to_hash256();
        if let Some((_, mut metadata)) = self.read_raw_message(message_hash).await? {
            if metadata.committers.contains(&commitment) {
                return Ok(());
            } else {
                metadata.committers.push(commitment);
                self.storage
                    .write()
                    .await
                    .add_or_overwrite_file(
                        &format!("metadata-{message_hash}.json"),
                        serde_spb::to_string(&metadata).unwrap(),
                    )
                    .await?;
            };
        } else {
            let mut storage = self.storage.write().await;
            storage
                .add_or_overwrite_file(
                    &format!("metadata-{message_hash}.json"),
                    serde_spb::to_string(&MessageMetadata {
                        message_hash,
                        committers: vec![commitment],
                    })
                    .unwrap(),
                )
                .await?;
            storage
                .add_or_overwrite_file(
                    &format!("message-{message_hash}.json"),
                    serde_spb::to_string(&message).unwrap(),
                )
                .await?;
        };
        Ok(())
    }

    async fn retrieve_packets(&self) -> Result<Vec<Packet>, Error> {
        let messages = self.read_raw_messages().await?;
        let mut result = Vec::new();
        for (message, metadata) in messages {
            for commitment in metadata.committers {
                result.push(Packet {
                    commitment,
                    message: serde_spb::to_vec(&message).unwrap(),
                });
            }
        }
        Ok(result)
    }
}
