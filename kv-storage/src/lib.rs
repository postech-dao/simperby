use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simperby_common::crypto::*;
use thiserror::Error;

// TODO: add error types
#[derive(Error, Debug, Serialize, Deserialize, Clone)]
pub enum Error {
    /// When the given key does not exists in the storage.
    #[error("not found")]
    NotFound,
    /// An unknown error
    #[error("unknown: {0}")]
    Unknown(String),
}

/// TODO: Provide error types.
///
/// Note: This trait is quite subject to change.
#[async_trait]
pub trait KVStorage {
    /// Creates an empty storage with the path to newly create.
    async fn new(path: &str) -> Result<Self, Error>
    where
        Self: Sized;
    /// Open an existing storage with the path given.
    async fn open(path: &str) -> Result<Self, Error>
    where
        Self: Sized;
    /// Records the current state to the persistent storage.
    ///
    /// Note that it may keep only the last checkpoint.
    async fn commit_checkpoint(&mut self) -> Result<(), Error>;
    /// Reverts all the changes made since the last checkpoint.
    async fn revert_to_latest_checkpoint(&mut self) -> Result<(), Error>;
    /// Inserts a key-value pair into the storage. If exists, it will be overwritten.
    async fn insert_or_update(&mut self, key: Hash256, value: &[u8]) -> Result<(), Error>;
    /// Removes a key-value pair from the storage. If not exists, it will `Err(NotFound)`.
    async fn remove(&mut self, key: Hash256) -> Result<(), Error>;
    /// Retrieves the value associated with the key. If not exists, it will return `Err(NotFound)`.
    async fn get(&self, key: Hash256) -> Result<Vec<u8>, Error>;
    /// Checks whether the given item exists in the storage.
    async fn contain(&self, key: Hash256) -> Result<bool, Error>;
}
