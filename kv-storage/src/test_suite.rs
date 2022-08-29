//! Common test scenarios that accepts only `&dyn KVStorage`, so that it can be used in multiple implementations.

use super::*;

#[allow(dead_code)]
async fn single_crud_1(storage: &mut dyn KVStorage) {
    storage
        .insert_or_update(Hash256::hash("1"), b"1")
        .await
        .unwrap();
    assert_eq!(storage.get(Hash256::hash("1")).await.unwrap(), b"1");
    storage
        .insert_or_update(Hash256::hash("1"), b"2")
        .await
        .unwrap();
    assert_eq!(storage.get(Hash256::hash("1")).await.unwrap(), b"2");
    storage.remove(Hash256::hash("1")).await.unwrap();
    assert_eq!(
        storage.get(Hash256::hash("1")).await,
        Err(crate::Error::NotFound)
    );
}
