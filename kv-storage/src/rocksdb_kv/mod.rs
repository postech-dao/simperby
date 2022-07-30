use super::*;
use rocksdb::DB;

pub struct RocksDB {
    db: DB,
}

#[async_trait]
impl KVStore for RocksDB {
    async fn new(path: &str) -> Result<Self, ()>
    where
        Self: Sized,
    {
        Ok(RocksDB { db: DB::open_default(path).unwrap() })
    }
    async fn open(path: &str) -> Result<Self, ()>
    where
        Self: Sized,
    {
        Ok(RocksDB { db: DB::open_default(path).unwrap() })
    }
    async fn commit_checkpoint(&mut self) -> Result<(), ()> {
        unimplemented!("not implemented");
    }
    async fn revert_to_latest_checkpoint(&mut self) -> Result<(), ()> {
        unimplemented!("not implemented");
    }
    async fn insert_or_update(&mut self, key: Hash256, value: &[u8]) -> Result<(), ()> {
        let result = self.db.put(key.dummy, value);
        match result {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }
    async fn remove(&mut self, key: Hash256) -> Result<(), ()> {
        let result = self.db.delete(key.dummy);
        match result {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }
    async fn get(&self, key: Hash256) -> Result<Option<Vec<u8>>, ()> {
        let result = self.db.get(key.dummy);
        match result {
            Ok(v) => Ok(v),
            Err(_) => Err(()),
        }
    }
}