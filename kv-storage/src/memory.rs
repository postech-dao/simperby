use super::*;
use std::collections::HashMap;

type DB = HashMap<Hash256, Vec<u8>>;

#[derive(Clone)]
pub struct MemoryDB {
    db: DB,
    checkpoint: DB,
}

impl MemoryDB {
    pub async fn new() -> Self {
        MemoryDB {
            db: DB::new(),
            checkpoint: DB::new(),
        }
    }

    pub async fn open(&self) -> Self {
        self.clone()
    }
}

#[async_trait]
impl KVStorage for MemoryDB {
    async fn commit_checkpoint(&mut self) -> Result<(), Error> {
        self.checkpoint = self.db.clone();

        Ok(())
    }

    async fn revert_to_latest_checkpoint(&mut self) -> Result<(), Error> {
        self.db = self.checkpoint.clone();

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

#[cfg(test)]
mod test {
    use super::*;

    async fn init() -> MemoryDB {
        let mut db: MemoryDB = MemoryDB::new().await;
        insert_or_update_handler(&mut db, "1", b"1").await;
        insert_or_update_handler(&mut db, "2", b"2").await;
        insert_or_update_handler(&mut db, "3", b"3").await;
        insert_or_update_handler(&mut db, "4", b"4").await;
        db
    }

    async fn get_from_checkpoint(db: &MemoryDB, key: &str, value: &[u8]) -> bool {
        match db.checkpoint.get(&Hash256::hash(key)) {
            Some(v) => v == value,
            None => false,
        }
    }

    async fn get_from_db(db: &MemoryDB, key: &str, value: &[u8]) -> bool {
        match db.db.get(&Hash256::hash(key)) {
            Some(v) => v == value,
            None => false,
        }
    }

    async fn insert_or_update_handler(db: &mut MemoryDB, key: &str, value: &[u8]) {
        db.insert_or_update(Hash256::hash(key), value)
            .await
            .unwrap()
    }

    async fn commit_checkpoint_handler(db: &mut MemoryDB) {
        db.commit_checkpoint().await.unwrap();
    }

    async fn revert_checkpoint_handler(db: &mut MemoryDB) {
        db.revert_to_latest_checkpoint().await.unwrap();
    }

    #[tokio::test]
    async fn get_from_empty_db() {
        let db: MemoryDB = MemoryDB::new().await;
        assert_eq!(
            db.get(Hash256::hash("1")).await,
            Err(super::Error::NotFound)
        );
    }

    #[tokio::test]
    async fn get_from_empty_checkpoint() {
        let db: MemoryDB = MemoryDB::new().await;
        assert_eq!(db.checkpoint.get(&Hash256::hash("1")), None);
    }

    #[tokio::test]
    async fn get_from_init_db() {
        let db: MemoryDB = init().await;
        assert!(get_from_db(&db, "1", b"1").await);
        assert!(get_from_db(&db, "2", b"2").await);
        assert!(get_from_db(&db, "3", b"3").await);
        assert!(get_from_db(&db, "4", b"4").await);
        assert!(!get_from_db(&db, "5", b"5").await);
    }

    #[tokio::test]
    async fn commit_checkpoint_once() {
        let mut db: MemoryDB = init().await;
        commit_checkpoint_handler(&mut db).await;
        assert!(get_from_checkpoint(&db, "1", b"1").await);
        assert!(get_from_checkpoint(&db, "2", b"2").await);
        assert!(get_from_checkpoint(&db, "3", b"3").await);
        assert!(get_from_checkpoint(&db, "4", b"4").await);
        assert!(!get_from_checkpoint(&db, "5", b"5").await);
    }

    #[tokio::test]
    async fn revert_checkpoint_once() {
        let mut db: MemoryDB = init().await;
        commit_checkpoint_handler(&mut db).await;
        insert_or_update_handler(&mut db, "5", b"5").await;
        assert!(get_from_db(&db, "5", b"5").await);
        revert_checkpoint_handler(&mut db).await;
        assert!(!get_from_db(&db, "5", b"5").await);
    }
}
