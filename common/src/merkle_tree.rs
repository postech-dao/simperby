use crate::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    ///
    /// Given a tree [[1, 2, 3], [4, 5], [6]],
    /// Merkle proof for 2 is [1, 5] and Merkle proof for 3 is [OnlyChild, 4].
    ///
    /// For `LeftChild` and `RightChild`, pair hash of the sibling node is given.
    /// For `OnlyChild`, only the instruction is given.
    pub fn create_merkle_proof(&self, key: Hash256) -> Option<MerkleProof> {
        if !self.hash_list.contains(&key) {
            return None;
        }
        let mut merkle_proof: MerkleProof = MerkleProof { proof: Vec::new() };
        let mut merkle_tree: Vec<Vec<Hash256>> = Self::merkle_tree(&self.hash_list);
        let mut target_hash: Hash256 = key;
        // Pop because the root is never included in the Merkle proof
        merkle_tree.pop();
        for level in merkle_tree {
            for pair in level.chunks(2) {
                if pair.contains(&target_hash) {
                    if pair.len() == 2 {
                        let is_right_node: bool = pair[0] == target_hash;
                        if is_right_node {
                            merkle_proof
                                .proof
                                .push(MerkleProofEntry::RightChild(pair[is_right_node as usize]))
                        } else {
                            merkle_proof
                                .proof
                                .push(MerkleProofEntry::LeftChild(pair[is_right_node as usize]))
                        }
                        target_hash = Hash256::aggregate(&pair[0], &pair[1]);
                    } else {
                        merkle_proof.proof.push(MerkleProofEntry::OnlyChild);
                        target_hash = Hash256::hash(&pair[0]);
                    };
                }
            }
        }
        Some(merkle_proof)
    }

    /// Creates a merkle tree from the given hash list.
    ///
    /// Merkle tree is returned in the form of Vec of Vec of Hash256.
    ///
    /// Given a hash list [1, 2, 3], merkle tree will be built as below,
    ///
    /// ``` text
    ///     6
    ///   4   5
    ///  1 2  3
    /// ```
    ///
    /// which is represented as [[1, 2, 3], [4, 5], [6]].
    ///
    /// For nodes with siblings, their concatenated hash value is hashed up.
    /// For nodes without siblings, its hash value is hashed up.
    fn merkle_tree(hash_list: &[Hash256]) -> Vec<Vec<Hash256>> {
        let mut merkle_tree: Vec<Vec<Hash256>> = vec![hash_list.to_vec()];
        while !Self::is_fully_created(&merkle_tree) {
            let mut upper_level_hash_list: Vec<Hash256> = Vec::new();
            for pair in merkle_tree.last().unwrap().chunks(2) {
                if pair.len() == 2 {
                    upper_level_hash_list.push(Hash256::aggregate(&pair[0], &pair[1]));
                } else {
                    upper_level_hash_list.push(Hash256::hash(&pair[0]));
                }
            }
            merkle_tree.push(upper_level_hash_list);
        }
        merkle_tree
    }

    /// Checks if a given merkle tree is fully created or is in progress.
    ///
    /// Is fully created if root exists and returns `true`.
    /// Is in progress if root does not exist and returns `false`.
    ///
    /// Note that root exists in the last element of a merkle tree with length 1.
    fn is_fully_created(merkle_tree: &[Vec<Hash256>]) -> bool {
        merkle_tree.last().unwrap().len() == 1
    }

    /// Returns the root of the tree.
    ///
    /// If the tree is empty, this returns a `Self::EMPTY_HASH`.
    ///
    /// Never panics on unwrap because merkle_tree is initialized with `vec![hash_list.to_vec()]` where `hash_list` is not empty.
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct MerkleProof {
    pub proof: Vec<MerkleProofEntry>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum MerkleProofEntry {
    LeftChild(Hash256),
    RightChild(Hash256),
    OnlyChild,
}

#[derive(Error, Debug, Serialize, Deserialize, Clone)]
pub enum MerkleProofError {
    /// When the proof is malformed.
    #[error("malformed proof: {0}")]
    MalformedProof(String),
    /// When the root doesn't match
    #[error("unmatched string: expected {0} but found {1}")]
    UnmatchedRoot(String, String),
}

impl MerkleProof {
    /// Verifies whether the given data is in the block.
    pub fn verify(&self, root: Hash256, data: &[u8]) -> Result<(), MerkleProofError> {
        let mut calculated_root: Hash256 = Hash256::hash(data);
        for node in &self.proof {
            calculated_root = match node {
                MerkleProofEntry::LeftChild(pair_hash) => {
                    Hash256::aggregate(pair_hash, &calculated_root)
                }
                MerkleProofEntry::RightChild(pair_hash) => {
                    Hash256::aggregate(&calculated_root, pair_hash)
                }
                MerkleProofEntry::OnlyChild => Hash256::hash(calculated_root),
            };
        }
        if root == calculated_root {
            Ok(())
        } else {
            Err(MerkleProofError::UnmatchedRoot(
                hex::encode(root.hash),
                hex::encode(calculated_root.hash),
            ))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Returns a hash list with `number` elements
    ///
    /// Hash list consists of hashes of 8-bit unsigned integer from 0 to `number - 1`.
    fn create_hash_list(number: u8) -> Vec<Hash256> {
        let mut hash_list: Vec<Hash256> = Vec::new();
        for n in 0..number {
            hash_list.push(Hash256::hash([n]));
        }
        hash_list
    }

    #[test]
    /// Test if `None` is returned for merkle proof of data not in the tree.
    fn create_merkle_proof_for_data_not_in_the_tree() {
        let hash_list: Vec<Hash256> = create_hash_list(16);
        let merkle_tree: OneshotMerkleTree = OneshotMerkleTree::create(hash_list);
        let key: Hash256 = Hash256::hash([42]);

        assert!(OneshotMerkleTree::create_merkle_proof(&merkle_tree, key).is_none());
    }

    #[test]
    /// Test if `OneshotMerkleTree::EMPTY_HASH` is returned for root hash of empty tree
    fn root_of_empty_tree() {
        let merkle_tree: OneshotMerkleTree = OneshotMerkleTree::create(Vec::new());

        assert_eq!(
            OneshotMerkleTree::root(&merkle_tree),
            OneshotMerkleTree::EMPTY_HASH
        );
    }

    #[test]
    /// Test if verification fails for data not in the tree.
    fn verification_failure() {
        let hash_list: Vec<Hash256> = create_hash_list(16);
        let merkle_tree: OneshotMerkleTree = OneshotMerkleTree::create(hash_list);
        let key: Hash256 = Hash256::hash([2]);
        let merkle_proof: Option<MerkleProof> =
            OneshotMerkleTree::create_merkle_proof(&merkle_tree, key);
        let root_hash: Hash256 = OneshotMerkleTree::root(&merkle_tree);

        assert!(merkle_proof.is_some());
        assert!(root_hash != OneshotMerkleTree::EMPTY_HASH);
        assert!(MerkleProof::verify(&merkle_proof.unwrap(), root_hash, &[42]).is_err());
    }

    #[test]
    /// Test if Merkle proof generation and verification work well with a tree with an even number of leaves that is 2^n.
    fn even_number_of_leaves_pow_of_two() {
        let hash_list: Vec<Hash256> = create_hash_list(16);
        let merkle_tree: OneshotMerkleTree = OneshotMerkleTree::create(hash_list);
        let data: [u8; 1] = [2];
        let key: Hash256 = Hash256::hash(data);
        let merkle_proof: Option<MerkleProof> =
            OneshotMerkleTree::create_merkle_proof(&merkle_tree, key);
        let root_hash: Hash256 = OneshotMerkleTree::root(&merkle_tree);

        assert!(merkle_proof.is_some());
        assert!(root_hash != OneshotMerkleTree::EMPTY_HASH);
        assert!(MerkleProof::verify(&merkle_proof.unwrap(), root_hash, &data).is_ok());
    }

    #[test]
    /// Test if Merkle proof generation and verification work well with a tree with an even number of leaves that is not 2^n.
    fn even_number_of_leaves_not_pow_of_two() {
        let hash_list: Vec<Hash256> = create_hash_list(10);
        let merkle_tree: OneshotMerkleTree = OneshotMerkleTree::create(hash_list);
        let key: Hash256 = Hash256::hash([9]);
        let merkle_proof: Option<MerkleProof> =
            OneshotMerkleTree::create_merkle_proof(&merkle_tree, key);
        let root_hash: Hash256 = OneshotMerkleTree::root(&merkle_tree);

        assert!(merkle_proof.is_some());
        assert!(root_hash != OneshotMerkleTree::EMPTY_HASH);
        assert!(MerkleProof::verify(&merkle_proof.unwrap(), root_hash, &[9]).is_ok());
    }

    #[test]
    /// Test if Merkle proof generation and verification work well with an odd number of leaves.
    fn odd_number_of_leaves() {
        let hash_list: Vec<Hash256> = create_hash_list(11);
        let merkle_tree: OneshotMerkleTree = OneshotMerkleTree::create(hash_list);
        let key: Hash256 = Hash256::hash([10]);
        let merkle_proof: Option<MerkleProof> =
            OneshotMerkleTree::create_merkle_proof(&merkle_tree, key);
        let root_hash: Hash256 = OneshotMerkleTree::root(&merkle_tree);

        assert!(merkle_proof.is_some());
        assert!(root_hash != OneshotMerkleTree::EMPTY_HASH);
        assert!(MerkleProof::verify(&merkle_proof.unwrap(), root_hash, &[10]).is_ok());
    }
}
