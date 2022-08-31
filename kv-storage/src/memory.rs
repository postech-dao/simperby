use super::*;
use std::collections::HashMap;

type DB = HashMap<Hash256, Vec<u8>>;

#[derive(Clone)]
pub struct IMDB {
    db: DB,
    checkpoint_db: DB,
}

impl IMDB {
    pub async fn new() -> Self {
        IMDB {
            db: DB::new(),
            checkpoint_db: DB::new(),
        }
    }

    pub async fn open(&self) -> Self {
        self.clone()
    }
}

#[async_trait]
impl KVStorage for IMDB {
    async fn commit_checkpoint(&mut self) -> Result<(), Error> {
        self.checkpoint_db = self.db.clone();

        Ok(())
    }

    async fn revert_to_latest_checkpoint(&mut self) -> Result<(), Error> {
        self.db = self.checkpoint_db.clone();

        Ok(())
    }

    async fn insert_or_update(&mut self, key: Hash256, value: &[u8]) -> Result<(), Error> {
        self.db.insert(key, value.to_vec());

        Ok(())
    }

    async fn remove(&mut self, key: Hash256) -> Result<(), Error> {
        match self.db.remove(&key) {
            Some(_) => Ok(()),
            None => Err(super::Error::NotFound),
        }
    }

    async fn get(&self, key: Hash256) -> Result<Vec<u8>, Error> {
        match self.db.get(&key) {
            Some(v) => Ok(v.to_vec()),
            None => Err(super::Error::NotFound),
        }
    }

    async fn contain(&self, key: Hash256) -> Result<bool, Error> {
        match self.db.get(&key) {
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }
}
