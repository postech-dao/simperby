use crate::*;
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum Error {
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("invalid proof: {0}")]
    InvalidProof(String),
    #[error("crypto error: {0}")]
    CryptoError(String, CryptoError),
}

/// Verifies whether `h2` can be the direct child of `h1`.
///
/// Note that you still need to verify
/// 1. block body (other commits)
/// 2. finalization proof
/// 3. protocol version of the node binary.
pub fn verify_header_to_header(h1: &BlockHeader, h2: &BlockHeader) -> Result<(), Error> {
    if h2.height != h1.height + 1 {
        return Err(Error::InvalidArgument(format!(
            "Invalid height: expected {}, got {}",
            h1.height + 1,
            h2.height
        )));
    }
    if h2.previous_hash != h1.to_hash256() {
        return Err(Error::InvalidArgument(format!(
            "Invalid previous hash: expected {}, got {}",
            h1.to_hash256(),
            h2.previous_hash
        )));
    }
    if !h2.validator_set.iter().any(|(pk, _)| pk == &h1.author) {
        return Err(Error::InvalidArgument(format!(
            "Invalid author: got {}",
            h2.author
        )));
    }
    if h2.timestamp <= h1.timestamp {
        return Err(Error::InvalidArgument(format!(
            "Invalid timestamp: expected larger than {}, got {}",
            h1.timestamp, h2.timestamp
        )));
    }
    for (public_key, signature) in &h2.prev_block_finalization_proof {
        signature.verify(h1, public_key).map_err(|e| {
            Error::CryptoError("Invalid prev_block_finalization_proof".to_string(), e)
        })?;
    }
    Ok(())
}

/// Verifies the finalization proof of the given block header.
pub fn verify_finalization_proof(
    header: &BlockHeader,
    block_finalization_proof: &FinalizationProof,
) -> Result<(), Error> {
    let total_voting_power: VotingPower = header.validator_set.iter().map(|(_, v)| v).sum();
    // TODO: change to `HashSet` after `PublicKey` supports `Hash`.
    let mut voted_validators = BTreeSet::new();
    for (public_key, signature) in block_finalization_proof {
        signature
            .verify(header, public_key)
            .map_err(|e| Error::CryptoError("Invalid finalization proof".to_string(), e))?;
        voted_validators.insert(public_key);
    }
    let voted_voting_power: VotingPower = header
        .validator_set
        .iter()
        .filter(|(v, _)| voted_validators.contains(v))
        .map(|(_, power)| power)
        .sum();
    if voted_voting_power * 3 <= total_voting_power * 2 {
        return Err(Error::InvalidProof(format!(
            "Invalid finalization proof - voted voting power is too low: {} / {}",
            voted_voting_power, total_voting_power
        )));
    }
    Ok(())
}

/// Verifies whether the given sequence of commits can be a subset of a finalized chain.
///
/// It may accept sequences that contain more than one `BlockHeader`.
pub fn verify_commit_sequence(
    _start_header: &BlockHeader,
    _commits: &[Commit],
) -> Result<(), Error> {
    unimplemented!()
}
