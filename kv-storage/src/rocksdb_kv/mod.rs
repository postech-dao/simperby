use super::*;

pub struct RocksDB {
    // TODO
}

#[async_trait]
impl KVStore for RocksDB {
    async fn new(_path: &str) -> Result<Self, ()>
    where
        Self: Sized,
    {
        unimplemented!("not implemented");
    }
    async fn open(_path: &str) -> Result<Self, ()>
    where
        Self: Sized,
    {
        unimplemented!("not implemented");   
    }
    async fn commit_checkpoint(&mut self) -> Result<(), ()> {
        unimplemented!("not implemented");
    }
    async fn revert_to_latest_checkpoint(&mut self) -> Result<(), ()> {
        unimplemented!("not implemented");
    }
    async fn insert_or_update(&mut self, _key: Hash256, _value: &[u8]) -> Result<(), ()> {
        unimplemented!("not implemented");
    }
    async fn remove(&mut self, _key: Hash256) -> Result<(), ()> {
        unimplemented!("not implemented");
    }
    async fn get(&self, _key: Hash256) -> Result<Option<Vec<u8>>, ()> {
        unimplemented!("not implemented");
    }
}