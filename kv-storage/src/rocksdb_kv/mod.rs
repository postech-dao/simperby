use super::*;
use mktemp::Temp;
use rocksdb::{checkpoint, DB};

pub struct RocksDB {
    db: DB,
    checkpoint: Option<Temp>,
}

#[async_trait]
impl KVStore for RocksDB {
    async fn new(path: &str) -> Result<Self, ()>
    where
        Self: Sized,
    {
        Ok(RocksDB {
            db: DB::open_default(path).unwrap(),
            checkpoint: None,
        })
    }
    async fn open(path: &str) -> Result<Self, ()>
    where
        Self: Sized,
    {
        Ok(RocksDB {
            db: DB::open_default(path).unwrap(),
            checkpoint: None,
        })
    }
    async fn commit_checkpoint(&mut self) -> Result<(), ()> {
        let result = Temp::new_dir();
        match result {
            Ok(path) => {
                let chk_point = checkpoint::Checkpoint::new(&self.db).unwrap();
                chk_point
                    .create_checkpoint(path.to_path_buf().display().to_string())
                    .unwrap();
                self.checkpoint = Some(path);
                Ok(())
            }
            Err(_) => Err(()),
        }
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
