pub mod crypto;

use crypto::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct BlockHeader {
    /// The author of this block.
    pub author: PublicKey,
    /// The signature of the previous block.
    pub prev_block_finalization_proof: Vec<(PublicKey, Signature)>,
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
    /// The set of validator & voteing power for the next block.
    ///
    /// The order here is same as the order of leaders in each round.
    pub validator_set: Vec<(PublicKey, u64)>,
}

impl BlockHeader {
    pub fn hash(&self) -> Hash256 {
        unimplemented!()
    }

    /// Verifies whether the given block header is a valid successor of this block.
    ///
    /// Note that you still need to verify the block body and the finalization proof.
    pub fn verify_next_block(&self, header: &BlockHeader) -> Result<(), String> {
        if header.height != self.height + 1 {
            return Err(format!(
                "Invalid height: expected {}, got {}",
                self.height + 1,
                header.height
            ));
        }
        if header.previous_hash != self.hash() {
            return Err(format!(
                "Invalid previous hash: expected {}, got {}",
                self.hash(),
                header.previous_hash
            ));
        }
        if !self
            .validator_set
            .iter()
            .any(|(pk, _)| pk == &header.author)
        {
            return Err(format!("Invalid author: got {}", header.author));
        }
        if header.timestamp <= self.timestamp {
            return Err(format!(
                "Invalid timestamp: expected larger than {}, got {}",
                self.timestamp, header.timestamp
            ));
        }
        for (public_key, signature) in &header.prev_block_finalization_proof {
            signature
                .verify(self.hash(), public_key)
                .map_err(|e| format!("Invalid prev_block_finalization_proof - {}", e))?;
        }
        Ok(())
    }

    pub fn verify_finalization_proof(
        &self,
        block_finalization_proof: &[(PublicKey, Signature)],
    ) -> Result<(), String> {
        let total_voting_power: u64 = self.validator_set.iter().map(|(_, v)| v).sum();
        // TODO: change to `HashSet` after `PublicKey` supports `Hash`.
        let mut voted_validators = BTreeSet::new();
        for (public_key, signature) in block_finalization_proof {
            signature
                .verify(self.hash(), public_key)
                .map_err(|e| format!("Invalid finalization proof - {}", e))?;
            voted_validators.insert(public_key);
        }
        let voted_voting_power: u64 = self
            .validator_set
            .iter()
            .filter(|(v, _)| voted_validators.contains(v))
            .map(|(_, power)| power)
            .sum();
        if voted_voting_power * 3 <= total_voting_power * 2 {
            return Err(format!(
                "Invalid finalization proof - voted voting power is too low: {} / {}",
                voted_voting_power, total_voting_power
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct MerkleProof {
    // TODO
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
    pub fn verify(&self, _root: Hash256, _data: &[u8]) -> Result<(), MerkleProofError> {
        unimplemented!()
    }
}
