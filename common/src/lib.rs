pub mod crypto;

use crypto::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct BlockHeader {
    /// The author of this block.
    pub author: PublicKey,
    /// The signature of the previous block.
    pub prev_block_finalization_proof: Vec<Signature>,
    /// The hash of the previous block.
    pub previous_hash: Hash256,
    /// The height of this block.
    pub height: u64,
    /// The timestamp of this block.
    pub timestamp: u64,
    /// The Merkle root of transactions.
    pub tx_merkle_root: Hash256,
    /// The Merkle root of the state.
    pub state_merkle_root: Hash256,
    /// The hash of the set of validator & vote weight for the next block.
    pub validator_set_hash: Hash256,
}

impl BlockHeader {
    pub fn hash(&self) -> Hash256 {
        unimplemented!()
    }
}
