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
        let result = self.db.put(key.as_ref(), value);
        match result {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    async fn remove(&mut self, key: Hash256) -> Result<(), ()> {
        let result = self.db.delete(key.as_ref());
        match result {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    async fn get(&self, key: Hash256) -> Result<Option<Vec<u8>>, ()> {
        let result = self.db.get(key.as_ref());
        match result {
            Ok(v) => Ok(v),
            Err(_) => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RocksDB;
    use crate::KVStore;
    use futures::executor::block_on;
    use mktemp::Temp;
    use rocksdb::DB;
    use simperby_common::crypto::Hash256;

    fn init_db_ver1() -> Temp {
        let tmp_folder = Temp::new_dir().unwrap();
        let db = DB::open_default(tmp_folder.to_path_buf().display().to_string()).unwrap();

        db.put(Hash256::hash("key1"), "val1").unwrap();
        db.put(Hash256::hash("key2"), "val2").unwrap();
        db.put(Hash256::hash("key3"), "val3").unwrap();
        db.put(Hash256::hash("key4"), "val4").unwrap();

        tmp_folder
    }

    fn init_db_ver2() -> Temp {
        let tmp_folder = Temp::new_dir().unwrap();
        let mut db = block_on(RocksDB::new(
            &tmp_folder.to_path_buf().display().to_string(),
        ))
        .unwrap();

        put_test(&mut db, "key1", "val1");
        put_test(&mut db, "key2", "val2");
        put_test(&mut db, "key3", "val3");
        put_test(&mut db, "key4", "val4");

        tmp_folder
    }

    fn get_test(db: &RocksDB, key: &str, value: &str) -> bool {
        let result = block_on(db.get(Hash256::hash(key))).unwrap();
        match result {
            None => false,
            Some(v) => {
                let str_v = std::str::from_utf8(&v).unwrap();
                assert_eq!(str_v, value);
                true
            }
        }
    }

    fn put_test(db: &mut RocksDB, key: &str, value: &str) {
        block_on(db.insert_or_update(Hash256::hash(key), value.as_bytes())).unwrap()
    }

    fn remove_test(db: &mut RocksDB, key: &str) {
        block_on(db.remove(Hash256::hash(key))).unwrap()
    }

    #[test]
    fn get_once_with_open() {
        let tmp_folder = init_db_ver1();
        let db = block_on(RocksDB::open(
            &tmp_folder.to_path_buf().display().to_string(),
        ))
        .unwrap();

        get_test(&db, "key1", "val1");
    }

    #[test]
    fn get_all_with_open() {
        let tmp_folder = init_db_ver1();
        let db = block_on(RocksDB::open(
            &tmp_folder.to_path_buf().display().to_string(),
        ))
        .unwrap();

        assert!(get_test(&db, "key1", "val1"));
        assert!(get_test(&db, "key2", "val2"));
        assert!(get_test(&db, "key3", "val3"));
        assert!(get_test(&db, "key4", "val4"));
    }

    #[test]
    fn get_once_with_new() {
        let tmp_folder = init_db_ver2();
        let db = block_on(RocksDB::open(
            &tmp_folder.to_path_buf().display().to_string(),
        ))
        .unwrap();

        get_test(&db, "key1", "val1");
    }

    #[test]
    fn get_all_with_new() {
        let tmp_folder = init_db_ver2();
        let db = block_on(RocksDB::open(
            &tmp_folder.to_path_buf().display().to_string(),
        ))
        .unwrap();

        assert!(get_test(&db, "key1", "val1"));
        assert!(get_test(&db, "key2", "val2"));
        assert!(get_test(&db, "key3", "val3"));
        assert!(get_test(&db, "key4", "val4"));
    }

    #[test]
    fn insert_once() {
        let tmp_folder = init_db_ver1();
        let mut db = block_on(RocksDB::open(
            &tmp_folder.to_path_buf().display().to_string(),
        ))
        .unwrap();

        assert!(!get_test(&db, "key5", "val5"));
        put_test(&mut db, "key5", "val5");
        assert!(get_test(&db, "key5", "val5"));
    }

    #[test]
    fn update_once() {
        let tmp_folder = init_db_ver1();
        let mut db = block_on(RocksDB::open(
            &tmp_folder.to_path_buf().display().to_string(),
        ))
        .unwrap();

        assert!(get_test(&db, "key1", "val1"));
        put_test(&mut db, "key1", "val5");
        assert!(get_test(&db, "key1", "val5"));
    }

    #[test]
    fn remove_once() {
        let tmp_folder = init_db_ver1();
        let mut db = block_on(RocksDB::open(
            &tmp_folder.to_path_buf().display().to_string(),
        ))
        .unwrap();

        assert!(get_test(&db, "key1", "val1"));
        remove_test(&mut db, "key1");
        assert!(!get_test(&db, "key1", "val1"));
    }
}
