use simperby_common::crypto::Hash256;
use simperby_common::MerkleProof;

/// A Merkle tree that is created once but never modified.
///
/// This is useful for per-block data such as transaction lists.
pub struct OneshotMerkleTree {
    hash_list: Vec<Hash256>,
}

impl OneshotMerkleTree {
    pub const EMPTY_HASH: Hash256 = Hash256::zero();

    /// Creates a new OneshotMerkleTree from the given data.
    pub fn create(data: Vec<Hash256>) -> Self {
        OneshotMerkleTree { hash_list: data }
    }

    /// Creates a Merkle proof for a given data in the tree.
    ///
    /// Returns `None` if the data is not in the tree.
    pub fn create_merkle_proof(&self, mut key: Hash256) -> Option<MerkleProof> {
        if !self.hash_list.contains(&key) {
            return None;
        }

        let mut merkle_proof: MerkleProof = MerkleProof { proof: Vec::new() };
        let mut merkle_tree: Vec<Vec<Hash256>> = Self::merkle_tree(&self.hash_list);

        merkle_tree.pop();
        for level in merkle_tree {
            for pair in level.chunks(2) {
                if pair.contains(&key) {
                    let index: u8 = if pair[0] == key { 1u8 } else { 0u8 };
                    merkle_proof
                        .proof
                        .push((pair[index as usize].clone(), index));
                    key = Self::hash_pair(pair);
                }
            }
        }
        Some(merkle_proof)
    }

    /// Creates a merkle tree from the given hash list.
    ///
    /// Merkle tree is returned in the form of Vec of Vec of Hash256.
    ///
    /// Given a hash list [1, 2, 3, 4], merkle tree will be built as below,
    ///
    /// ``` text
    ///     7
    ///   5   6
    ///  1 2 3 4
    /// ```
    /// which is represented as [[1, 2, 3, 4], [5, 6], [7]].
    fn merkle_tree(hash_list: &[Hash256]) -> Vec<Vec<Hash256>> {
        let mut merkle_tree: Vec<Vec<Hash256>> = vec![hash_list.to_vec()];

        while merkle_tree.last().unwrap().len() != 1 {
            let mut upper_level_hash_list: Vec<Hash256> = Vec::new();

            for pair in merkle_tree.last().unwrap().chunks(2) {
                upper_level_hash_list.push(Self::hash_pair(pair));
            }

            merkle_tree.push(upper_level_hash_list);
        }

        merkle_tree
    }

    /// Calculates the hash of the given pair of hashes.
    ///
    /// Note that the pair consists of a single hash if the number of nodes in a certain level is odd.
    /// In this case, the hash is calculated with the given hash duplicated.
    fn hash_pair(pair: &[Hash256]) -> Hash256 {
        if pair.len() == 2 {
            Hash256::aggregate(&pair[0], &pair[1])
        } else {
            Hash256::aggregate(&pair[0], &pair[0])
        }
    }

    /// Returns the root of the tree.
    ///
    /// If the tree is empty, this returns a `Self::EMPTY_HASH`.
    pub fn root(&self) -> Hash256 {
        if self.hash_list.is_empty() {
            Self::EMPTY_HASH
        } else {
            Self::merkle_tree(&self.hash_list)
                .last()
                .unwrap()
                .last()
                .unwrap()
                .to_owned()
        }
    }
}
