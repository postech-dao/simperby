use super::*;
use mktemp::Temp;
use rocksdb::{checkpoint, Options, DB};
use std::path::PathBuf;

pub struct RocksDB {
    db: DB,
    origin_path: PathBuf,
    current_db_dir: Temp,
    checkpoint_db_dir: Temp,
}

impl From<rocksdb::Error> for super::Error {
    fn from(e: rocksdb::Error) -> super::Error {
        super::Error::Unknown(String::from(e))
    }
}

#[async_trait]
impl KVStorage for RocksDB {
    async fn new(path: &str) -> Result<Self, super::Error>
    where
        Self: Sized,
    {
        let origin_path = PathBuf::from(path);
        let current_db_dir = Temp::new_dir().unwrap();
        let checkpoint_db_dir = Temp::new_dir().unwrap();
        {
            let db = DB::open_default(origin_path.to_str().unwrap())?;
            let checkpoint_db = checkpoint::Checkpoint::new(&db)?;

            checkpoint_db.create_checkpoint(current_db_dir.to_path_buf().as_path().join("db"))?;
            checkpoint_db
                .create_checkpoint(checkpoint_db_dir.to_path_buf().as_path().join("db"))?;
        }

        Ok(RocksDB {
            db: DB::open_default(current_db_dir.to_path_buf().as_path().join("db"))?,
            origin_path,
            current_db_dir,
            checkpoint_db_dir,
        })
    }

    async fn open(path: &str) -> Result<Self, super::Error>
    where
        Self: Sized,
    {
        let origin_path = PathBuf::from(path);
        let current_db_dir = Temp::new_dir().unwrap();
        let checkpoint_db_dir = Temp::new_dir().unwrap();
        {
            let db = DB::open_default(origin_path.to_str().unwrap())?;
            let checkpoint_db = checkpoint::Checkpoint::new(&db)?;

            checkpoint_db.create_checkpoint(current_db_dir.to_path_buf().as_path().join("db"))?;
            checkpoint_db
                .create_checkpoint(checkpoint_db_dir.to_path_buf().as_path().join("db"))?;
        }

        Ok(RocksDB {
            db: DB::open_default(current_db_dir.to_path_buf().as_path().join("db"))?,
            origin_path,
            current_db_dir,
            checkpoint_db_dir,
        })
    }

    async fn commit_checkpoint(&mut self) -> Result<(), super::Error> {
        let new_checkpoint_db_dir = Temp::new_dir().unwrap();
        let checkpoint_db = checkpoint::Checkpoint::new(&self.db)?;

        DB::destroy(&Options::default(), self.origin_path.to_str().unwrap())?;
        checkpoint_db.create_checkpoint(self.origin_path.to_str().unwrap())?;
        checkpoint_db
            .create_checkpoint(new_checkpoint_db_dir.to_path_buf().as_path().join("db"))?;
        self.checkpoint_db_dir = new_checkpoint_db_dir;

        Ok(())
    }

    async fn revert_to_latest_checkpoint(&mut self) -> Result<(), super::Error> {
        let new_current_db_dir = Temp::new_dir().unwrap();
        let new_checkpoint_db_dir = Temp::new_dir().unwrap();
        {
            let new_db =
                DB::open_default(self.checkpoint_db_dir.to_path_buf().as_path().join("db"))?;
            let checkpoint_db = checkpoint::Checkpoint::new(&new_db)?;

            checkpoint_db
                .create_checkpoint(new_current_db_dir.to_path_buf().as_path().join("db"))?;
            checkpoint_db
                .create_checkpoint(new_checkpoint_db_dir.to_path_buf().as_path().join("db"))?;
        }

        self.db = DB::open_default(new_current_db_dir.to_path_buf().as_path().join("db"))?;
        self.current_db_dir = new_current_db_dir;
        self.checkpoint_db_dir = new_checkpoint_db_dir;

        Ok(())
    }

    async fn insert_or_update(&mut self, key: Hash256, value: &[u8]) -> Result<(), super::Error> {
        let result = self.db.put(key.as_ref(), value);
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(super::Error::from(e)),
        }
    }

    async fn remove(&mut self, key: Hash256) -> Result<(), super::Error> {
        let result = self.db.delete(key.as_ref());
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(super::Error::from(e)),
        }
    }

    async fn get(&self, key: Hash256) -> Result<Vec<u8>, super::Error> {
        let result = self.db.get(key.as_ref());
        match result {
            Ok(v) => match v {
                Some(v) => Ok(v),
                None => Err(super::Error::NotFound),
            },
            Err(e) => Err(super::Error::from(e)),
        }
    }

    async fn contain(&self, key: Hash256) -> Result<bool, super::Error> {
        let result = self.db.get(key.as_ref()).unwrap();
        match result {
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RocksDB;
    use crate::KVStorage;
    use futures::executor::block_on;
    use mktemp::Temp;
    use rocksdb::DB;
    use simperby_common::crypto::Hash256;

    async fn init_db_ver1() -> Temp {
        let tmp_directory = Temp::new_dir().unwrap();
        let db = DB::open_default(tmp_directory.to_path_buf().display().to_string()).unwrap();

        db.put(Hash256::hash("key1"), "val1").unwrap();
        db.put(Hash256::hash("key2"), "val2").unwrap();
        db.put(Hash256::hash("key3"), "val3").unwrap();
        db.put(Hash256::hash("key4"), "val4").unwrap();

        tmp_directory
    }

    async fn init_db_ver2() -> Temp {
        let tmp_directory = Temp::new_dir().unwrap();
        let mut db = RocksDB::new(&tmp_directory.to_path_buf().display().to_string())
            .await
            .unwrap();

        put_test(&mut db, "key1", "val1").await;
        put_test(&mut db, "key2", "val2").await;
        put_test(&mut db, "key3", "val3").await;
        put_test(&mut db, "key4", "val4").await;

        db.commit_checkpoint().await.unwrap();

        tmp_directory
    }

    async fn get_test(db: &RocksDB, key: &str, value: &str) -> bool {
        let has_value = db.contain(Hash256::hash(key)).await.unwrap();
        match has_value {
            false => false,
            true => {
                let val = block_on(db.get(Hash256::hash(key))).unwrap();
                let str_v = std::str::from_utf8(&val).unwrap();
                assert_eq!(str_v, value);
                true
            }
        }
    }

    async fn put_test(db: &mut RocksDB, key: &str, value: &str) {
        db.insert_or_update(Hash256::hash(key), value.as_bytes())
            .await
            .unwrap()
    }

    async fn remove_test(db: &mut RocksDB, key: &str) {
        db.remove(Hash256::hash(key)).await.unwrap()
    }

    async fn revert_test(db: &mut RocksDB) {
        db.revert_to_latest_checkpoint().await.unwrap();
    }

    #[tokio::test]
    async fn get_once_with_open() {
        let tmp_directory = init_db_ver1().await;
        let db = RocksDB::open(tmp_directory.to_path_buf().to_str().unwrap())
            .await
            .unwrap();

        assert!(get_test(&db, "key1", "val1").await);
    }

    #[tokio::test]
    async fn get_all_with_open() {
        let tmp_directory = init_db_ver1().await;
        let db = RocksDB::open(tmp_directory.to_path_buf().to_str().unwrap())
            .await
            .unwrap();

        assert!(get_test(&db, "key1", "val1").await);
        assert!(get_test(&db, "key2", "val2").await);
        assert!(get_test(&db, "key3", "val3").await);
        assert!(get_test(&db, "key4", "val4").await);
    }

    #[tokio::test]
    async fn get_once_with_new() {
        let tmp_directory = init_db_ver2().await;
        let db = RocksDB::open(tmp_directory.to_path_buf().to_str().unwrap())
            .await
            .unwrap();

        assert!(get_test(&db, "key1", "val1").await);
    }

    #[tokio::test]
    async fn get_all_with_new() {
        let tmp_directory = init_db_ver2().await;
        let db = RocksDB::open(tmp_directory.to_path_buf().to_str().unwrap())
            .await
            .unwrap();

        assert!(get_test(&db, "key1", "val1").await);
        assert!(get_test(&db, "key2", "val2").await);
        assert!(get_test(&db, "key3", "val3").await);
        assert!(get_test(&db, "key4", "val4").await);
    }

    #[tokio::test]
    async fn insert_once() {
        let tmp_directory = init_db_ver1().await;
        let mut db = RocksDB::open(tmp_directory.to_path_buf().to_str().unwrap())
            .await
            .unwrap();

        assert!(!get_test(&db, "key5", "val5").await);
        put_test(&mut db, "key5", "val5").await;
        assert!(get_test(&db, "key5", "val5").await);
    }

    #[tokio::test]
    async fn update_once() {
        let tmp_directory = init_db_ver1().await;
        let mut db = RocksDB::open(tmp_directory.to_path_buf().to_str().unwrap())
            .await
            .unwrap();

        assert!(get_test(&db, "key1", "val1").await);
        put_test(&mut db, "key1", "val5").await;
        assert!(get_test(&db, "key1", "val5").await);
    }

    #[tokio::test]
    async fn remove_once() {
        let tmp_directory = init_db_ver1().await;
        let mut db = RocksDB::open(tmp_directory.to_path_buf().to_str().unwrap())
            .await
            .unwrap();

        assert!(get_test(&db, "key1", "val1").await);
        remove_test(&mut db, "key1").await;
        assert!(!get_test(&db, "key1", "val1").await);
    }

    #[tokio::test]
    async fn revert_once() {
        let tmp_directory = init_db_ver1().await;
        let mut db = RocksDB::open(tmp_directory.to_path_buf().to_str().unwrap())
            .await
            .unwrap();

        assert!(get_test(&db, "key1", "val1").await);
        assert!(get_test(&db, "key2", "val2").await);
        assert!(get_test(&db, "key3", "val3").await);
        assert!(get_test(&db, "key4", "val4").await);
        assert!(!get_test(&db, "key5", "val5").await);

        put_test(&mut db, "key5", "val5").await;

        assert!(get_test(&db, "key1", "val1").await);
        assert!(get_test(&db, "key2", "val2").await);
        assert!(get_test(&db, "key3", "val3").await);
        assert!(get_test(&db, "key4", "val4").await);
        assert!(get_test(&db, "key5", "val5").await);

        revert_test(&mut db).await;

        assert!(get_test(&db, "key1", "val1").await);
        assert!(get_test(&db, "key2", "val2").await);
        assert!(get_test(&db, "key3", "val3").await);
        assert!(get_test(&db, "key4", "val4").await);
        assert!(!get_test(&db, "key5", "val5").await);
    }
}
