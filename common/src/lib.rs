pub mod crypto;

use crypto::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub type VotingPower = u64;
/// A UNIX timestamp measured in milliseconds.
pub type Timestamp = u64;
/// A block height. The genesis block is at height 0.
pub type BlockHeight = u64;
pub type FinalizationProof = Vec<(PublicKey, TypedSignature<BlockHeader>)>;

/// The state that is directly recorded in the header.
///
/// This state affects the consensus, unlike the ordinary state (which is used for data recording).
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct EssentialState {
    /// The original validator set information before the delegation calculation.
    ///
    /// The order here is the leader selection priority order of the validators
    /// which directly affects the result of `calculate_net_validator_set()`.
    pub validator_set: Vec<(PublicKey, VotingPower)>,
    /// The protocol version that must be used from next block.
    ///
    /// It must be a valid semantic version (e.g., `0.2.3`).
    pub version: String,
    /// The delegation state (delegator, delegatee).
    pub delegation: Vec<(PublicKey, PublicKey)>,
}

impl EssentialState {
    /// Calculate the actual set of validators & voting power for the next block.
    ///
    /// The order here is same as the order of leaders in each round.
    /// returns `None` if the delegation is not valid.
    pub fn calculate_net_validator_set(&self) -> Result<Vec<(PublicKey, VotingPower)>, String> {
        // check delegation by delegatee (which's forbidden)
        let mut delegatees = BTreeSet::new();
        for (delegator, delegatee) in &self.delegation {
            if delegatees.contains(delegator) {
                return Err(format!("delegatee ({}) can't delegate", delegator));
            }
            delegatees.insert(delegatee.clone());
        }

        // calculate the result
        let mut validator_set: BTreeMap<_, _> = self.validator_set.iter().cloned().collect();
        for (delegator, delegatee) in &self.delegation {
            let delegator_voting_power = if let Some(x) = validator_set.get(delegator) {
                *x
            } else {
                return Err(format!("delegator not found: {}", delegator));
            };
            let delegatee_voting_power = if let Some(x) = validator_set.get(delegatee) {
                *x
            } else {
                return Err(format!("delegatee not found: {}", delegatee));
            };
            validator_set.remove(delegator).expect("already checked");
            validator_set.insert(
                delegatee.clone(),
                delegatee_voting_power + delegator_voting_power,
            );
        }

        // reorder the result by the original `validator_set`.
        let mut result = Vec::new();
        for (key, _) in &self.validator_set {
            if let Some(x) = validator_set.get(key) {
                result.push((key.clone(), *x));
            }
        }
        Ok(result)
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct BlockHeader {
    /// The author of this block.
    pub author: PublicKey,
    /// The signature of the previous block.
    pub prev_block_finalization_proof: FinalizationProof,
    /// The hash of the previous block.
    pub previous_hash: Hash256,
    /// The height of this block.
    pub height: BlockHeight,
    /// The timestamp of this block.
    pub timestamp: Timestamp,
    /// The Merkle root of transactions.
    pub tx_merkle_root: Hash256,
    /// The Merkle root of the non-essential state.
    pub state_merkle_root: Hash256,
    /// The essential state.
    pub essential_state: EssentialState,
}

impl BlockHeader {
    pub fn hash(&self) -> Hash256 {
        Hash256::hash(serde_json::to_vec(self).unwrap())
    }

    /// Verifies whether the given block header is a valid successor of this block.
    ///
    /// Note that you still need to verify
    /// 1. block body
    /// 2. finalization proof
    /// 3. protocol version
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
            .essential_state
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
                .verify(self, public_key)
                .map_err(|e| format!("Invalid prev_block_finalization_proof - {}", e))?;
        }
        Ok(())
    }

    pub fn verify_finalization_proof(
        &self,
        block_finalization_proof: &FinalizationProof,
    ) -> Result<(), String> {
        let total_voting_power: VotingPower = self
            .essential_state
            .validator_set
            .iter()
            .map(|(_, v)| v)
            .sum();
        // TODO: change to `HashSet` after `PublicKey` supports `Hash`.
        let mut voted_validators = BTreeSet::new();
        for (public_key, signature) in block_finalization_proof {
            signature
                .verify(self, public_key)
                .map_err(|e| format!("Invalid finalization proof - {}", e))?;
            voted_validators.insert(public_key);
        }
        let voted_voting_power: VotingPower = self
            .essential_state
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
    pub proof: Vec<(Hash256, u8)>,
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
