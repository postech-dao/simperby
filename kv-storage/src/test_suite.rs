//! Common test scenarios that accept only `&dyn KVStorage`, so that it can be used in multiple implementations.

use super::*;

#[allow(dead_code)]
async fn remove_non_existing_key(storage: &mut dyn KVStorage) {
    assert_eq!(
        storage.remove(Hash256::hash("1")).await,
        Err(Error::NotFound)
    );
}

#[allow(dead_code)]
async fn get_non_existing_key(storage: &mut dyn KVStorage) {
    assert_eq!(storage.get(Hash256::hash("1")).await, Err(Error::NotFound));
}

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

#[allow(dead_code)]
async fn single_crud_2(storage: &mut dyn KVStorage) {
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
        storage.remove(Hash256::hash("1")).await,
        Err(Error::NotFound)
    );
}

#[allow(dead_code)]
async fn multiple_crud_1(storage: &mut dyn KVStorage) {
    storage
        .insert_or_update(Hash256::hash("1"), b"1")
        .await
        .unwrap();
    storage
        .insert_or_update(Hash256::hash("2"), b"2")
        .await
        .unwrap();
    assert_eq!(storage.get(Hash256::hash("1")).await.unwrap(), b"1");
    assert_eq!(storage.get(Hash256::hash("2")).await.unwrap(), b"2");
    storage
        .insert_or_update(Hash256::hash("1"), b"2")
        .await
        .unwrap();
    storage
        .insert_or_update(Hash256::hash("2"), b"1")
        .await
        .unwrap();
    assert_eq!(storage.get(Hash256::hash("1")).await.unwrap(), b"2");
    assert_eq!(storage.get(Hash256::hash("2")).await.unwrap(), b"1");
    storage.remove(Hash256::hash("1")).await.unwrap();
    storage.remove(Hash256::hash("2")).await.unwrap();
    assert_eq!(storage.get(Hash256::hash("1")).await, Err(Error::NotFound));
    assert_eq!(storage.get(Hash256::hash("2")).await, Err(Error::NotFound));
}
