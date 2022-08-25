use crate::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HistoryStorageError {
    #[error("invalid operation: {0}")]
    InvalidOperation(String),
    #[error("storage error: {0}")]
    StorageError(simperby_kv_storage::Error),
    /// When an essential item is missing or corrupted.
    #[error("malformed storage: {0}")]
    StorageIntegrityError(String),
}

type Error = HistoryStorageError;

impl From<simperby_kv_storage::Error> for Error {
    fn from(e: simperby_kv_storage::Error) -> Self {
        Error::StorageError(e)
    }
}

impl From<Error> for SimperbyError {
    fn from(e: Error) -> Self {
        match e {
            HistoryStorageError::InvalidOperation(e) => SimperbyError::InvalidOperation(e),
            HistoryStorageError::StorageError(e) => SimperbyError::StorageError(e),
            HistoryStorageError::StorageIntegrityError(e) => {
                SimperbyError::StorageIntegrityError(e)
            }
        }
    }
}

fn create_block_key(height: BlockHeight) -> Hash256 {
    Hash256::hash(format!("block-{}", height).as_bytes())
}

fn create_last_finalized_height_key() -> Hash256 {
    Hash256::hash("last-finalized-height".as_bytes())
}

fn create_last_finalized_block_proof_key() -> Hash256 {
    Hash256::hash("last-finalized-block-finalization-proof".as_bytes())
}

fn initialization_flag_key() -> Hash256 {
    Hash256::hash("initialization-flag".as_bytes())
}

async fn get_json_from_storage<T>(storage: &dyn KVStorage, key: Hash256) -> Result<T, Error>
where
    T: serde::de::DeserializeOwned,
{
    let value = storage.get(key).await?;
    serde_json::from_slice(&value).map_err(|e| Error::StorageIntegrityError(e.to_string()))
}

async fn set_json_to_storage<T>(
    storage: &mut dyn KVStorage,
    key: Hash256,
    value: &T,
) -> Result<(), Error>
where
    T: serde::Serialize,
{
    let value = serde_json::to_vec(value).unwrap();
    storage.insert_or_update(key, &value).await?;
    Ok(())
}

pub(crate) struct HistoryStorage {
    pub storage: Box<dyn KVStorage>,
    pub height: BlockHeight,
}

impl HistoryStorage {
    pub async fn new(
        mut storage: Box<dyn KVStorage>,
        genesis_info: GenesisInfo,
    ) -> Result<Self, Error> {
        let stored_genesis_block =
            match get_json_from_storage::<Block>(storage.as_ref(), create_block_key(0)).await {
                Ok(b) => Some(b),
                Err(Error::StorageError(simperby_kv_storage::Error::NotFound)) => None,
                Err(e) => return Err(e),
            };
        let height = if let Some(genesis_block) = stored_genesis_block {
            if genesis_block != genesis_info.create_genesis_block() {
                return Err(Error::StorageIntegrityError(format!(
                    "mismatched genesis block stored: {:?}",
                    genesis_block
                )));
            };
            get_json_from_storage(storage.as_ref(), create_last_finalized_height_key()).await?
        // empty storage; initialize
        } else {
            set_json_to_storage(
                storage.as_mut(),
                create_block_key(0),
                &genesis_info.create_genesis_block(),
            )
            .await?;
            set_json_to_storage(storage.as_mut(), create_last_finalized_height_key(), &0).await?;
            set_json_to_storage(
                storage.as_mut(),
                create_last_finalized_block_proof_key(),
                &genesis_info.genesis_signature,
            )
            .await?;
            0
        };
        Ok(HistoryStorage { storage, height })
    }

    /// Atomically inserts the block, updates the height and the finalization proof.
    pub async fn append_block(
        &mut self,
        block: &Block,
        finalization_proof: FinalizationProof,
    ) -> Result<(), Error> {
        if block.header.height != self.height + 1 {
            return Err(Error::InvalidOperation(format!(
                "block height {} does not match the expected next height {}",
                block.header.height, self.height
            )));
        }
        set_json_to_storage(
            self.storage.as_mut(),
            create_block_key(block.header.height),
            block,
        )
        .await?;
        set_json_to_storage(
            self.storage.as_mut(),
            create_last_finalized_height_key(),
            &block.header.height,
        )
        .await?;
        set_json_to_storage(
            self.storage.as_mut(),
            create_last_finalized_block_proof_key(),
            &finalization_proof,
        )
        .await?;
        self.storage.commit_checkpoint().await?;
        self.height = block.header.height;
        Ok(())
    }

    pub async fn get_last_finalized_height(&self) -> Result<u64, Error> {
        get_json_from_storage(self.storage.as_ref(), create_last_finalized_height_key()).await
    }

    pub async fn get_block(&self, height: BlockHeight) -> Result<Block, Error> {
        if height <= self.height {
            return Err(Error::InvalidOperation(format!(
                "block height {} excees the current height {}",
                height, self.height
            )));
        }
        get_json_from_storage(self.storage.as_ref(), create_block_key(height)).await
    }

    /// Since the finalization proof for a block is in the next header,
    /// we partially store the finalization proof for the last block.
    pub async fn get_last_finalized_block_proof(&self) -> Result<FinalizationProof, Error> {
        get_json_from_storage(
            self.storage.as_ref(),
            create_last_finalized_block_proof_key(),
        )
        .await
    }
}
