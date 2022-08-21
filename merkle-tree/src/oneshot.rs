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
                    let is_right_node: bool = pair[0] == key;
                    merkle_proof
                        .proof
                        .push((pair[is_right_node as usize].clone(), is_right_node));
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
    /// Given a hash list [1, 2, 3], merkle tree will be built as below,
    ///
    /// ``` text
    ///     6
    ///   4   5
    ///  1 2 3 3
    /// ```
    /// which is represented as [[1, 2, 3], [4, 5], [6]].
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

#[cfg(test)]
mod test {
    use super::*;
    use simperby_common::{crypto::Hash256, MerkleProof};

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
        assert!(merkle_proof.is_some());

        let root_hash: Hash256 = OneshotMerkleTree::root(&merkle_tree);
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
        assert!(merkle_proof.is_some());

        let root_hash: Hash256 = OneshotMerkleTree::root(&merkle_tree);
        assert!(root_hash != OneshotMerkleTree::EMPTY_HASH);

        assert!(MerkleProof::verify(&merkle_proof.unwrap(), root_hash, &data).is_ok());
    }

    #[test]
    /// Test if Merkle proof generation and verification work well with a tree with an even number of leaves that is not 2^n.
    fn even_number_of_leaves_not_pow_of_two() {
        let hash_list: Vec<Hash256> = create_hash_list(10);
        let merkle_tree: OneshotMerkleTree = OneshotMerkleTree::create(hash_list);
        let key: Hash256 = Hash256::hash([2]);
        let merkle_proof: Option<MerkleProof> =
            OneshotMerkleTree::create_merkle_proof(&merkle_tree, key);
        assert!(merkle_proof.is_some());

        let root_hash: Hash256 = OneshotMerkleTree::root(&merkle_tree);
        assert!(root_hash != OneshotMerkleTree::EMPTY_HASH);

        assert!(MerkleProof::verify(&merkle_proof.unwrap(), root_hash, &[2]).is_ok());
    }

    #[test]
    /// Test if Merkle proof generation and verification work well with an odd number of leaves.
    fn odd_number_of_leaves() {
        let hash_list: Vec<Hash256> = create_hash_list(11);
        let merkle_tree: OneshotMerkleTree = OneshotMerkleTree::create(hash_list);
        let key: Hash256 = Hash256::hash([2]);
        let merkle_proof: Option<MerkleProof> =
            OneshotMerkleTree::create_merkle_proof(&merkle_tree, key);
        assert!(merkle_proof.is_some());

        let root_hash: Hash256 = OneshotMerkleTree::root(&merkle_tree);
        assert!(root_hash != OneshotMerkleTree::EMPTY_HASH);

        assert!(MerkleProof::verify(&merkle_proof.unwrap(), root_hash, &[2]).is_ok());
    }
}
