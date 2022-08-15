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
            let db = DB::open_default(origin_path.to_str().unwrap()).unwrap();
            let checkpoint_db = checkpoint::Checkpoint::new(&db).unwrap();

            checkpoint_db
                .create_checkpoint(current_db_dir.to_path_buf().as_path().join("db"))
                .unwrap();
            checkpoint_db
                .create_checkpoint(checkpoint_db_dir.to_path_buf().as_path().join("db"))
                .unwrap();
        }

        Ok(RocksDB {
            db: DB::open_default(current_db_dir.to_path_buf().as_path().join("db")).unwrap(),
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
            let db = DB::open_default(origin_path.to_str().unwrap()).unwrap();
            let checkpoint_db = checkpoint::Checkpoint::new(&db).unwrap();

            checkpoint_db
                .create_checkpoint(current_db_dir.to_path_buf().as_path().join("db"))
                .unwrap();
            checkpoint_db
                .create_checkpoint(checkpoint_db_dir.to_path_buf().as_path().join("db"))
                .unwrap();
        }

        Ok(RocksDB {
            db: DB::open_default(current_db_dir.to_path_buf().as_path().join("db")).unwrap(),
            origin_path,
            current_db_dir,
            checkpoint_db_dir,
        })
    }

    async fn commit_checkpoint(&mut self) -> Result<(), super::Error> {
        let new_checkpoint_db_dir = Temp::new_dir().unwrap();
        let checkpoint_db = checkpoint::Checkpoint::new(&self.db).unwrap();

        DB::destroy(&Options::default(), self.origin_path.to_str().unwrap()).unwrap();
        checkpoint_db
            .create_checkpoint(self.origin_path.to_str().unwrap())
            .unwrap();
        checkpoint_db
            .create_checkpoint(new_checkpoint_db_dir.to_path_buf().as_path().join("db"))
            .unwrap();
        self.checkpoint_db_dir = new_checkpoint_db_dir;

        Ok(())
    }

    async fn revert_to_latest_checkpoint(&mut self) -> Result<(), super::Error> {
        let new_current_db_dir = Temp::new_dir().unwrap();
        let new_checkpoint_db_dir = Temp::new_dir().unwrap();
        {
            let new_db =
                DB::open_default(self.checkpoint_db_dir.to_path_buf().as_path().join("db"))
                    .unwrap();
            let checkpoint_db = checkpoint::Checkpoint::new(&new_db).unwrap();

            checkpoint_db
                .create_checkpoint(new_current_db_dir.to_path_buf().as_path().join("db"))
                .unwrap();
            checkpoint_db
                .create_checkpoint(new_checkpoint_db_dir.to_path_buf().as_path().join("db"))
                .unwrap();
        }

        self.db = DB::open_default(new_current_db_dir.to_path_buf().as_path().join("db")).unwrap();
        self.current_db_dir = new_current_db_dir;
        self.checkpoint_db_dir = new_checkpoint_db_dir;

        Ok(())
    }

    async fn insert_or_update(&mut self, key: Hash256, value: &[u8]) -> Result<(), super::Error> {
        let result = self.db.put(key.as_ref(), value);
        match result {
            Ok(_) => Ok(()),
            Err(_) => Err(super::Error::Unknown("unknown error".to_string())),
        }
    }

    async fn remove(&mut self, key: Hash256) -> Result<(), super::Error> {
        let result = self.db.delete(key.as_ref());
        match result {
            Ok(_) => Ok(()),
            Err(_) => Err(super::Error::NotFound),
        }
    }

    async fn get(&self, key: Hash256) -> Result<Vec<u8>, super::Error> {
        let result = self.db.get(key.as_ref());
        match result {
            Ok(v) => Ok(v.unwrap()),
            Err(_) => Err(super::Error::NotFound),
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

        block_on(db.commit_checkpoint()).unwrap();

        tmp_folder
    }

    fn get_test(db: &RocksDB, key: &str, value: &str) -> bool {
        let has_value = block_on(db.contain(Hash256::hash(key))).unwrap();
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

    fn put_test(db: &mut RocksDB, key: &str, value: &str) {
        block_on(db.insert_or_update(Hash256::hash(key), value.as_bytes())).unwrap()
    }

    fn remove_test(db: &mut RocksDB, key: &str) {
        block_on(db.remove(Hash256::hash(key))).unwrap()
    }

    fn revert_test(db: &mut RocksDB) {
        block_on(db.revert_to_latest_checkpoint()).unwrap();
    }

    #[test]
    fn get_once_with_open() {
        let tmp_folder = init_db_ver1();
        let db = block_on(RocksDB::open(
            &tmp_folder.to_path_buf().display().to_string(),
        ))
        .unwrap();

        assert!(get_test(&db, "key1", "val1"));
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

        assert!(get_test(&db, "key1", "val1"));
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

    #[test]
    fn revert_once() {
        let tmp_folder = init_db_ver1();
        let mut db = block_on(RocksDB::open(&tmp_folder.to_path_buf().to_str().unwrap())).unwrap();

        assert!(get_test(&db, "key1", "val1"));
        assert!(get_test(&db, "key2", "val2"));
        assert!(get_test(&db, "key3", "val3"));
        assert!(get_test(&db, "key4", "val4"));
        assert!(!get_test(&db, "key5", "val5"));

        put_test(&mut db, "key5", "val5");

        assert!(get_test(&db, "key1", "val1"));
        assert!(get_test(&db, "key2", "val2"));
        assert!(get_test(&db, "key3", "val3"));
        assert!(get_test(&db, "key4", "val4"));
        assert!(get_test(&db, "key5", "val5"));

        revert_test(&mut db);

        assert!(get_test(&db, "key1", "val1"));
        assert!(get_test(&db, "key2", "val2"));
        assert!(get_test(&db, "key3", "val3"));
        assert!(get_test(&db, "key4", "val4"));
        assert!(!get_test(&db, "key5", "val5"));
    }
}
