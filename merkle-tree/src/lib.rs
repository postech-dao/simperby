use serde::{Deserialize, Serialize};
use simperby_common::crypto::Hash256;
use simperby_common::MerkleProof;
use simperby_kv_storage::KVStorage;
use std::marker::PhantomData;
use thiserror::Error;

#[derive(Error, Debug, Serialize, Deserialize, Clone)]
pub enum Error {
    /// When the given storage is not a valid Merkle-tree-indexed database.
    #[error("invalid storage {0}")]
    MalformedStorage(String),
    /// When the provided storage emitted an error during the operation.
    #[error("storage error: {0}")]
    StorageError(simperby_kv_storage::Error),
}

/// A state machine that interpret the given storage as a Merkle tree rooted by a specific key.
///
/// Note that there is no `get()` method as leaf nodes (which the user of this struct would like to read)
/// are stored by their keys directly, so one can just access directly to the underlying `KVStore` using the key.
pub struct MerkleTree<S> {
    // TODO: add the fields (for real) and remove _x (dummy)
    _x: PhantomData<S>,
}

impl<S: KVStorage> MerkleTree<S> {
    /// Initializes the merkle tree traverse machine with the given storage.
    ///
    /// This will naturally check whether the given storage has a well-formed Merkle tree starting with the given root.
    pub fn new(_storage: S, _root_node_key: Hash256) -> Result<Self, Error> {
        unimplemented!()
    }

    /// Returns the Merkle root.
    pub async fn root(&self) -> Result<Hash256, Error> {
        unimplemented!()
    }

    /// Inserts a key-value pair into the store. If exists, it will be overwritten.
    pub async fn insert_or_update(&mut self, _key: Hash256, _value: &[u8]) -> Result<(), Error> {
        unimplemented!()
    }

    /// Removes a key-value pair from the store. If not exists, it will fail.
    pub async fn remove(&mut self, _key: Hash256) -> Result<(), Error> {
        unimplemented!()
    }

    /// Returns the total size of non-leaf nodes (that are essentially storage overhead for keeping the Merkle tree indexing)
    /// in bytes.
    pub async fn get_size_overhead(&self) -> Result<usize, Error> {
        unimplemented!()
    }

    /// Creates a Merkle proof for the given entry.
    pub async fn create_merkle_proof(&self, _key: Hash256) -> Result<MerkleProof, Error> {
        unimplemented!()
    }
}
