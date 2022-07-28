pub mod vetomint;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simperby_common::crypto::*;
use simperby_common::*;
use std::sync::Arc;
use tokio::sync::mpsc;

/// TODO: Provide error types.
///
/// Note: This trait is quite subject to change.
#[async_trait]
pub trait Network {
    /// Broadcasts a message to the network, after signed by the key given to this instance.
    async fn broadcast(&self, message: &[u8]) -> Result<(), String>;
    /// Creates a receiver for every message broadcasted to the network, except the one sent by this instance.
    async fn create_recv_queue(&self) -> Result<mpsc::Receiver<Vec<u8>>, ()>;
}

/// TODO: Provide error types.
///
/// Note: This trait is quite subject to change.
#[async_trait]
pub trait KVStore {
    /// Records the current state to the persistent storage.
    async fn commit_checkpoint(&mut self) -> Result<(), ()>;
    /// Reverts all the changes made since the last checkpoint.
    async fn revert_to_latest_checkpoint(&mut self) -> Result<(), ()>;
    /// Inserts a key-value pair into the store. If exists, it will be overwritten.
    async fn insert_or_update(&mut self, key: Hash256, value: &[u8]) -> Result<(), ()>;
    /// Removes a key-value pair from the store. If not exists, it will fail.
    async fn remove(&mut self, key: Hash256) -> Result<(), ()>;
    /// Retrieves the value associated with the key. If not exists, it will return `None`.
    async fn get(&self, key: Hash256) -> Result<Option<Vec<u8>>, ()>;
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Block<T> {
    pub header: BlockHeader,
    pub transactions: Vec<T>,
}

/// A state transition function.
#[async_trait]
pub trait BlockExecutor {
    type Transaction;
    async fn execute(
        &self,
        store: &mut dyn KVStore,
        transaction: Self::Transaction,
    ) -> Result<(), ()>;
}

/// A BFT consensus engine.
#[async_trait]
pub trait Consensus<E: BlockExecutor> {
    /// Peforms an one-block consensus.
    ///
    /// This method finishes when the next block is finalized.
    /// TODO: Not so complete for now.
    async fn progress(
        &mut self,
        block_to_propose: Option<Block<E::Transaction>>,
        last_finalized_header: BlockHeader,
        network: Arc<dyn Network>,
        executor: E,
        store: Box<dyn KVStore>,
    ) -> Result<Block<E::Transaction>, ()>;
}
