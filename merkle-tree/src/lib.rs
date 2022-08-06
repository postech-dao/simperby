use rocksdb::DB;
use simperby_common::crypto::*;

pub struct Node {
    pub key: Hash256,
    pub value: Option<Vec<u8>>,
    pub left: Option<Box<Node>>,
    pub right: Option<Box<Node>>,
}

pub struct MerkleTree {
    pub root: Box<Node>,
}

impl MerkleTree {
    // Creates a tree with an existing root node from database
    pub fn new(_root: Option<Hash256>, _db: Option<DB>) -> Self {
        unimplemented!();
    }

    // Gets the value for key stored in the tree
    pub fn get(&self, _key: Hash256) -> &[u8] {
        unimplemented!();
    }

    // Updates value with key in the tree
    pub fn update(&self, _key: Hash256, _value: &[u8]) {
        unimplemented!();
    }

    // Inserts key and value into the tree
    pub fn insert(&self, _key: Hash256, _value: &[u8]) {
        unimplemented!();
    }

    // Deletes any existing value for key from the tree
    pub fn delete(&self, _key: Hash256) {
        unimplemented!();
    }

    // Calculates the root hash of the given tree
    pub fn hash_root(&self) -> Hash256 {
        unimplemented!();
    }
}
