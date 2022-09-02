#![allow(unused_mut)]
#![allow(dead_code)]
#![allow(unused_variables)]

use crate::*;
use thiserror::Error;

#[derive(Error, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum StateStorageError {
    #[error("invalid operation: {0}")]
    InvalidOperation(String),
    #[error("storage error: {0}")]
    StorageError(simperby_kv_storage::Error),
    /// When an essential item is missing or corrupted.
    #[error("malformed storage: {0}")]
    StorageIntegrityError(String),
}

impl From<Error> for SimperbyError {
    fn from(e: Error) -> Self {
        unimplemented!()
    }
}

type Error = StateStorageError;

pub(crate) struct StateStorage {
    pub storage: Box<dyn KVStorage>,
}

impl StateStorage {
    /// Assuming an empty storage, creates the genesis state.
    pub async fn genesis(
        mut storage: Box<dyn KVStorage>,
        genesis_info: GenesisInfo,
    ) -> Result<Self, Error> {
        unimplemented!()
    }

    /// Assuming an existing state, check the basic validty of the state.
    pub async fn new(storage: Box<dyn KVStorage>) -> Result<Self, Error> {
        unimplemented!()
    }

    pub async fn delegation_state_hash(&self) -> Result<Hash256, Error> {
        unimplemented!()
    }

    pub async fn state_root(&self) -> Result<Hash256, Error> {
        unimplemented!()
    }

    /// Atomically inserts data, hash of it, and updates the state root.
    pub async fn insert_record(&mut self, key: Hash256, data: &[u8]) -> Result<(), Error> {
        unimplemented!()
    }

    pub async fn remove_record(&mut self, key: Hash256) -> Result<(), Error> {
        unimplemented!()
    }

    pub async fn get_delegation_state(&self) -> Result<DelegationState, Error> {
        unimplemented!()
    }

    pub async fn update_delegation_state(&mut self, state: DelegationState) -> Result<(), Error> {
        unimplemented!()
    }

    pub async fn get_data(&self, key: Hash256) -> Result<Vec<u8>, Error> {
        unimplemented!()
    }

    pub async fn get_data_hash(&self, key: Hash256) -> Result<Hash256, Error> {
        unimplemented!()
    }
}
