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

type Error = StateStorageError;

impl From<simperby_kv_storage::Error> for Error {
    fn from(e: simperby_kv_storage::Error) -> Self {
        Error::StorageError(e)
    }
}

impl From<Error> for SimperbyError {
    fn from(e: Error) -> Self {
        match e {
            Error::InvalidOperation(e) => SimperbyError::InvalidOperation(e),
            Error::StorageError(e) => SimperbyError::StorageError(e),
            Error::StorageIntegrityError(e) => SimperbyError::StorageIntegrityError(e),
        }
    }
}

/// Reads an essential (existence-guaranteed) item from the storage.
async fn get_json_from_storage<T>(storage: &dyn KVStorage, key: Hash256) -> Result<T, Error>
where
    T: serde::de::DeserializeOwned,
{
    let value = match storage.get(key.clone()).await {
        Ok(x) => x,
        Err(simperby_kv_storage::Error::NotFound) => {
            return Err(Error::StorageIntegrityError(format!(
                "missing essential item in the state storage: {}",
                key
            )))
        }
        Err(e) => return Err(e.into()),
    };
    serde_json::from_slice(&value).map_err(|e| Error::StorageIntegrityError(e.to_string()))
}

async fn get_hash_from_storage(storage: &dyn KVStorage, key: Hash256) -> Result<Hash256, Error> {
    let value = match storage.get(key.clone()).await {
        Ok(x) => x,
        Err(simperby_kv_storage::Error::NotFound) => {
            return Err(Error::StorageIntegrityError(format!(
                "missing essential item in the state storage: {}",
                key
            )))
        }
        Err(e) => return Err(e.into()),
    };
    let msg = if value.len() > 1024 {
        format!(
            "item hash is corrupted: (too long; omitted; {})",
            value.len()
        )
    } else {
        format!("item hash is corrupted: {:?}", value)
    };
    let data: [u8; 32] = TryInto::try_into(value).map_err(|_| Error::StorageIntegrityError(msg))?;
    Ok(Hash256::from_array(data))
}

async fn insert_or_update_json_to_storage<T>(
    storage: &mut dyn KVStorage,
    key: Hash256,
    value: &T,
) -> Result<(), Error>
where
    T: serde::Serialize,
{
    let value = serde_json::to_vec(value).unwrap();
    let hash = Hash256::hash(&value);
    storage.insert_or_update(key.clone(), &value).await?;
    storage
        .insert_or_update(create_hash_key(key), hash.as_ref())
        .await?;
    Ok(())
}

async fn insert_or_update_bytes_to_storage(
    storage: &mut dyn KVStorage,
    key: Hash256,
    data: &[u8],
) -> Result<Hash256, Error> {
    let hash = Hash256::hash(data);
    storage.insert_or_update(key.clone(), data).await?;
    storage
        .insert_or_update(create_hash_key(key), hash.as_ref())
        .await?;
    Ok(hash)
}

/// The state storage keeps data hashes for every entry.
fn create_hash_key(hash: Hash256) -> Hash256 {
    hash.aggregate(&Hash256::hash("hash-key"))
}

fn create_delegation_state_key() -> Hash256 {
    Hash256::hash("delegation-state")
}

fn create_state_root() -> Hash256 {
    Hash256::hash("state-root")
}

pub(crate) struct StateStorage {
    pub storage: Box<dyn KVStorage>,
}

impl StateStorage {
    /// Assuming an empty storage, creates the genesis state.
    pub async fn genesis(
        mut storage: Box<dyn KVStorage>,
        _genesis_info: GenesisInfo,
    ) -> Result<Self, Error> {
        insert_or_update_json_to_storage(
            storage.as_mut(),
            create_delegation_state_key(),
            &DelegationState {
                original_validator_set: Default::default(),
            },
        )
        .await?;
        storage
            .insert_or_update(create_state_root(), Hash256::zero().as_ref())
            .await?;
        Ok(Self { storage })
    }

    /// Assuming an existing state, check the basic validity of the state.
    pub async fn new(storage: Box<dyn KVStorage>) -> Result<Self, Error> {
        get_json_from_storage::<DelegationState>(storage.as_ref(), create_delegation_state_key())
            .await?;
        get_hash_from_storage(storage.as_ref(), create_state_root()).await?;
        Ok(Self { storage })
    }

    pub async fn delegation_state_hash(&self) -> Result<Hash256, Error> {
        get_hash_from_storage(
            self.storage.as_ref(),
            create_hash_key(create_delegation_state_key()),
        )
        .await
    }

    pub async fn state_root(&self) -> Result<Hash256, Error> {
        get_hash_from_storage(self.storage.as_ref(), create_state_root()).await
    }

    /// Atomically inserts data, hash of it, and updates the state root.
    pub async fn insert_or_update_data(&mut self, key: Hash256, data: &[u8]) -> Result<(), Error> {
        let hash = insert_or_update_bytes_to_storage(self.storage.as_mut(), key, data).await?;
        // TODO: introduce state Merkle tree.
        //
        // Currently the state root is just an arbitrary hash of the entire operation of the state history
        // which's deterministically decided, so that it can be used for the consensus.
        let state_root = self.state_root().await?;
        let new_state_root = state_root.aggregate(&hash);
        self.storage
            .insert_or_update(create_state_root(), new_state_root.as_ref())
            .await?;
        Ok(())
    }

    pub async fn remove_data(&mut self, key: Hash256) -> Result<(), Error> {
        let state_root = self.state_root().await?;
        let new_state_root = state_root.aggregate(&Hash256::hash("remove"));
        self.storage.remove(key.clone()).await?;
        self.storage.remove(create_hash_key(key)).await?;
        self.storage
            .insert_or_update(create_state_root(), new_state_root.as_ref())
            .await?;
        Ok(())
    }

    pub async fn get_delegation_state(&self) -> Result<DelegationState, Error> {
        get_json_from_storage::<DelegationState>(
            self.storage.as_ref(),
            create_delegation_state_key(),
        )
        .await
    }

    pub async fn update_delegation_state(&mut self, state: DelegationState) -> Result<(), Error> {
        insert_or_update_json_to_storage(
            self.storage.as_mut(),
            create_delegation_state_key(),
            &state,
        )
        .await
    }

    pub async fn get_data(&self, key: Hash256) -> Result<Vec<u8>, Error> {
        self.storage.get(key).await.map_err(|e| e.into())
    }

    pub async fn get_data_hash(&self, key: Hash256) -> Result<Hash256, Error> {
        get_hash_from_storage(self.storage.as_ref(), key).await
    }
}
