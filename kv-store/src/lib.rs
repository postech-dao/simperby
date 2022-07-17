use async_trait::async_trait;
use simperby_common::crypto::*;

/// TODO: Provide error types.
///
/// Note: This trait is quite subject to change.
#[async_trait]
pub trait KVStore {
    /// Creates an empty store with the path to newly create.
    async fn new(path: &str) -> Result<Self, ()>
    where
        Self: Sized;
    /// Open an existing store with the path given.
    async fn open(path: &str) -> Result<Self, ()>
    where
        Self: Sized;
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
