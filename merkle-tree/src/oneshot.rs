use simperby_common::crypto::Hash256;
use simperby_common::MerkleProof;

/// A Merkle tree that is created once but never modified.
///
/// This is useful for per-block data such as transaction lists.
pub struct OneshotMerkleTree {
    // TODO
}

impl OneshotMerkleTree {
    pub const EMPTY_HASH: Hash256 = Hash256::zero();

    pub fn create(_data: Vec<Hash256>) -> Self {
        unimplemented!()
    }

    /// Creates a Merkle proof for a given data in the tree.
    ///
    /// Returns `None` if the data is not in the tree.
    pub fn create_merkle_proof(&self, _key: Hash256) -> Option<MerkleProof> {
        unimplemented!()
    }

    /// Returns the root of the tree.
    ///
    /// If the tree is empty, this returns a `Self::EMPTY_HASH`.
    pub fn root(&self) -> Hash256 {
        unimplemented!()
    }
}
