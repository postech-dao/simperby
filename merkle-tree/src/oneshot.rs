use simperby_common::crypto::Hash256;
use simperby_common::MerkleProof;

/// A Merkle tree that is created once but never modified.
///
/// This is useful for per-block data such as transaction lists.
pub struct OneshotMerkleTree {
    // TODO
}

impl OneshotMerkleTree {
    pub fn create(_data: Vec<Hash256>) -> Self {
        unimplemented!()
    }

    pub fn create_merkle_proof(&self, _key: Hash256) -> MerkleProof {
        unimplemented!()
    }

    pub fn root(&self) -> Hash256 {
        unimplemented!()
    }
}
