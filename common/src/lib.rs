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

/// All about the delegation status, which will be stored in the blockchain state.
///
/// Note that this is not a part of `EssentialState`.
#[allow(clippy::type_complexity)]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct DelegationState {
    /// The original validator set for this block.
    ///
    /// The order here is the leader selection priority order of the validators
    /// which directly affects the effective validator set.
    ///
    /// The `(usize, usize)` is `(initial delegatee, current delegatee)`.
    /// Two could differ if the initial delegatee delagtes to other validators.
    pub original_validator_set: Vec<(PublicKey, VotingPower, Option<(usize, usize)>)>,
    // TODO: add various conditions for each delegation.
    // - Unlock-Automatically-After-N-Blocks
    // - Unlock-Automatically-After-T-Seconds
    // - Unlock-If-The-Delegatee-Is-Not-Active
    // - Unlock-If-The-Validator-Set-Changes
}

impl DelegationState {
    pub fn hash(&self) -> Hash256 {
        Hash256::hash(serde_json::to_vec(self).unwrap())
    }

    pub fn calculate_effective_validator_set(
        &self,
    ) -> Result<Vec<(PublicKey, VotingPower)>, String> {
        let mut validator_set = BTreeMap::new();
        for (public_key, voting_power, delegation) in &self.original_validator_set {
            if let Some((_, current_delegatee)) = delegation {
                validator_set.insert(
                    self.original_validator_set
                        .get(*current_delegatee)
                        .ok_or(format!(
                            "current delegatee {} exceeds the validator set size",
                            current_delegatee
                        ))?
                        .0
                        .clone(),
                    validator_set
                        .get(&self.original_validator_set[*current_delegatee].0)
                        .unwrap_or(&0)
                        + *voting_power,
                );
            } else {
                validator_set.insert(
                    public_key.clone(),
                    validator_set.get(public_key).unwrap_or(&0) + *voting_power,
                );
            }
        }
        let mut result = Vec::new();
        // The result validator set is sorted by the order of the `original_validator_set`
        for (validator, _, _) in &self.original_validator_set {
            if let Some(voting_power) = validator_set.get(validator) {
                result.push((validator.clone(), *voting_power));
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
    /// The effective net validator set (delegation-applied) for the next block.
    /// The order here is the leader order.
    pub validator_set: Vec<(PublicKey, VotingPower)>,
    /// The hash of the delegation state stored in the state storage.
    ///
    /// This deserves a seperate hash (not integrated in the `state_merkle_root`) due to its special role.
    pub delegation_state_hash: Hash256,
    /// The protocol version that must be used from next block.
    ///
    /// It must be a valid semantic version (e.g., `0.2.3`).
    pub version: String,
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
        let total_voting_power: VotingPower = self.validator_set.iter().map(|(_, v)| v).sum();
        // TODO: change to `HashSet` after `PublicKey` supports `Hash`.
        let mut voted_validators = BTreeSet::new();
        for (public_key, signature) in block_finalization_proof {
            signature
                .verify(self, public_key)
                .map_err(|e| format!("Invalid finalization proof - {}", e))?;
            voted_validators.insert(public_key);
        }
        let voted_voting_power: VotingPower = self
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
