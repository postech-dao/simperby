use crate::reserved::ReservedState;
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
    #[error("invalid commit: applied commit type does not match current commit sequence phase")]
    PhaseMismatch(),
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
            "invalid height: expected {}, got {}",
            h1.height + 1,
            h2.height
        )));
    }
    if h2.previous_hash != h1.to_hash256() {
        return Err(Error::InvalidArgument(format!(
            "invalid previous hash: expected {}, got {}",
            h1.to_hash256(),
            h2.previous_hash
        )));
    }
    verify_validator(&h1.validator_set, &h2.author)?;
    if h2.timestamp <= h1.timestamp {
        return Err(Error::InvalidArgument(format!(
            "invalid timestamp: expected larger than {}, got {}",
            h1.timestamp, h2.timestamp
        )));
    }
    verify_finalization_proof(h1, &h2.prev_block_finalization_proof)?;
    Ok(())
}

/// Verifies whether the given participant is a validator.
pub fn verify_validator(
    validator_set: &[(PublicKey, VotingPower)],
    participant: &PublicKey,
) -> Result<(), Error> {
    if !validator_set.iter().any(|(pk, _)| pk == participant) {
        return Err(Error::InvalidArgument(format!(
            "invalid validator: {} is not in the validator set",
            participant
        )));
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
    for signature in block_finalization_proof {
        signature
            .verify(header)
            .map_err(|e| Error::CryptoError("invalid finalization proof".to_string(), e))?;
        voted_validators.insert(signature.signer());
    }
    let voted_voting_power: VotingPower = header
        .validator_set
        .iter()
        .filter(|(v, _)| voted_validators.contains(v))
        .map(|(_, power)| power)
        .sum();
    if voted_voting_power * 3 <= total_voting_power * 2 {
        return Err(Error::InvalidProof(format!(
            "invalid finalization proof - voted voting power is too low: {} / {}",
            voted_voting_power, total_voting_power
        )));
    }
    Ok(())
}

// Phases of the `CommitSequenceVerifier`.
//
// Note that `Phase::X` is a phase where `Commit::X` is the last commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    // The transaction phase.
    // Note that there can be agendas without transactions.
    Transaction {
        transactions: Vec<Transaction>,
    },
    // The agenda phase.
    Agenda {
        agenda: Agenda,
        transaction_merkle_root: Hash256,
    },
    // The agenda proof phase.
    AgendaProof {
        agenda_proof: AgendaProof,
        transaction_merkle_root: Hash256,
    },
    // The extra phase.
    // Extra phase consists of `ExtraAgendaTransaction`s and `ChatLog`s.
    ExtraAgendaTransaction {
        last_extra_agenda_timestamp: Timestamp,
        transaction_merkle_root: Hash256,
        // TODO: add `ChatLog` here.
    },
    // The block phase.
    Block,
}

/// Verifies whether the given sequence of commits can be a subset of a finalized chain.
///
/// It may accept sequences that contain more than one `BlockHeader`.
#[derive(Debug, Clone)]
pub struct CommitSequenceVerifier {
    header: BlockHeader,
    phase: Phase,
    state: ReservedState,
    commit_hash: Hash256,
}

impl CommitSequenceVerifier {
    /// Creates a new `CommitSequenceVerifier` with the given block header.
    pub fn new(start_header: BlockHeader, reserved_state: ReservedState) -> Result<Self, Error> {
        Ok(Self {
            header: start_header.clone(),
            phase: Phase::Block,
            state: reserved_state,
            commit_hash: Hash256::hash(format!("{}", start_header.height + 1)),
        })
    }
    /// Verifies the given commit and updates the internal state of CommitSequenceVerifier.
    pub fn apply_commit(&mut self, commit: &Commit) -> Result<(), Error> {
        match (commit, &mut self.phase) {
            (
                Commit::Block(b),
                Phase::AgendaProof {
                    agenda_proof: _,
                    transaction_merkle_root,
                },
            ) => {
                verify_header_to_header(&self.header, b)?;
                // Verify block body
                if *transaction_merkle_root != b.tx_merkle_root {
                    return Err(Error::InvalidArgument(format!(
                        "invalid transaction merkle root hash: expected {}, got {}",
                        transaction_merkle_root, b.tx_merkle_root
                    )));
                }
                // Verify commit hash
                if self.commit_hash != b.commit_hash {
                    return Err(Error::InvalidArgument(format!(
                        "invalid block commit hash: expected {}, got {}",
                        self.commit_hash, b.commit_hash
                    )));
                }
                self.header = b.clone();
                self.phase = Phase::Block;
                self.commit_hash = Hash256::hash(format!("{}", b.height + 1));
            }
            (
                Commit::Block(b),
                Phase::ExtraAgendaTransaction {
                    last_extra_agenda_timestamp,
                    transaction_merkle_root,
                },
            ) => {
                verify_header_to_header(&self.header, b)?;
                // Check if the block contains all the extra-agenda transactions.
                if b.timestamp < *last_extra_agenda_timestamp {
                    return Err(Error::InvalidArgument(format!(
                        "invalid block timestamp: expected larger than the last extra-agenda transaction timestamp {}, got {}",
                        last_extra_agenda_timestamp, b.timestamp
                    )));
                }
                // Verify block body
                if *transaction_merkle_root != b.tx_merkle_root {
                    return Err(Error::InvalidArgument(format!(
                        "invalid transaction merkle root hash: expected {}, got {}",
                        transaction_merkle_root, b.tx_merkle_root
                    )));
                }
                // Verify commit hash
                if self.commit_hash != b.commit_hash {
                    return Err(Error::InvalidArgument(format!(
                        "invalid block commit hash: expected {}, got {}",
                        self.commit_hash, b.commit_hash
                    )));
                }
                self.header = b.clone();
                self.phase = Phase::Block;
                self.commit_hash = Hash256::hash(format!("{}", b.height + 1));
            }
            (Commit::Block(_), Phase::Block) => {
                return Err(Error::InvalidArgument(
                    "invalid block commit: block commit already exists".to_string(),
                ))
            }
            (Commit::Transaction(t), Phase::Block) => {
                verify_validator(&self.header.validator_set, &t.author)?;
                // Update reserved state for reserved-diff transactions.
                match &t.diff {
                    Diff::None => {}
                    Diff::General(_) => {}
                    Diff::Reserved(rs, _) => {
                        self.state = *rs.clone();
                    }
                }
                self.phase = Phase::Transaction {
                    transactions: vec![t.clone()],
                };
            }
            (Commit::Transaction(t), Phase::Transaction { transactions }) => {
                verify_validator(&self.header.validator_set, &t.author)?;
                // Check if transactions are in chronological order
                if t.timestamp < transactions.last().unwrap().timestamp {
                    return Err(Error::InvalidArgument(format!(
                        "invalid transaction timestamp: expected larger than {}, got {}",
                        transactions.last().unwrap().timestamp,
                        t.timestamp
                    )));
                }
                // Update reserved state for reserved-diff transactions.
                match &t.diff {
                    Diff::None => {}
                    Diff::General(_) => {}
                    Diff::Reserved(rs, _) => {
                        self.state = *rs.clone();
                    }
                }
                transactions.push(t.clone());
            }
            (Commit::Agenda(a), Phase::Block) => {
                verify_validator(&self.header.validator_set, &a.author)?;
                // Verify agenda without transactions
                if a.hash != Agenda::calculate_hash(self.header.height, &[]) {
                    return Err(Error::InvalidArgument(format!(
                        "invalid agenda hash: expected {}, got {}",
                        Agenda::calculate_hash(self.header.height, &[]),
                        a.hash
                    )));
                }
                self.phase = Phase::Agenda {
                    agenda: a.clone(),
                    transaction_merkle_root: BlockHeader::calculate_tx_merkle_root(&[]),
                };
            }
            (Commit::Agenda(a), Phase::Transaction { transactions }) => {
                verify_validator(&self.header.validator_set, &a.author)?;
                // Check if agenda is in chronological order
                if !transactions.is_empty() && a.timestamp < transactions.last().unwrap().timestamp
                {
                    return Err(Error::InvalidArgument(
                        format!("invalid agenda timestamp: expected larger than the last transaction timestamp {}, got {}", transactions.last().unwrap().timestamp, a.timestamp)
                    ));
                }
                // Verify agenda
                if a.hash != Agenda::calculate_hash(self.header.height, transactions) {
                    return Err(Error::InvalidArgument(format!(
                        "invalid agenda hash: expected {}, got {}",
                        Agenda::calculate_hash(self.header.height, transactions),
                        a.hash
                    )));
                }
                self.phase = Phase::Agenda {
                    agenda: a.clone(),
                    transaction_merkle_root: BlockHeader::calculate_tx_merkle_root(transactions),
                };
            }
            (
                Commit::Agenda(_),
                Phase::Agenda {
                    agenda: _,
                    transaction_merkle_root: _,
                },
            ) => {
                return Err(Error::InvalidArgument(
                    "invalid agenda commit: agenda commit already exists".to_string(),
                ));
            }
            (
                Commit::AgendaProof(p),
                Phase::Agenda {
                    agenda,
                    transaction_merkle_root,
                },
            ) => {
                // Check if agenda hash matches
                if p.agenda_hash != agenda.hash {
                    return Err(Error::InvalidArgument(format!(
                        "invalid agenda proof: invalid agenda hash expected {}, got {}",
                        agenda.hash, p.agenda_hash
                    )));
                }
                // Verify the agenda proof
                for signature in p.proof.iter() {
                    signature.verify(agenda).map_err(|e| {
                        Error::CryptoError(
                            "invalid agenda proof: invalid signature".to_string(),
                            e,
                        )
                    })?;
                }
                self.phase = Phase::AgendaProof {
                    agenda_proof: p.clone(),
                    transaction_merkle_root: *transaction_merkle_root,
                };
            }
            (
                Commit::AgendaProof(_),
                Phase::AgendaProof {
                    agenda_proof: _,
                    transaction_merkle_root: _,
                },
            ) => {
                return Err(Error::InvalidArgument(
                    "invalid agenda proof commit : agenda proof commit already exists".to_string(),
                ));
            }
            (
                Commit::ExtraAgendaTransaction(t),
                Phase::AgendaProof {
                    agenda_proof: _,
                    transaction_merkle_root,
                },
            ) => {
                match t {
                    ExtraAgendaTransaction::Delegate(tx) => {
                        verify_validator(&self.header.validator_set, &tx.delegator)?;
                        // Update reserved state by applying delegation
                        self.state.apply_delegate(tx).map_err(|e| {
                            Error::InvalidArgument(format!("invalid delegation: {}", e))
                        })?;
                        self.phase = Phase::ExtraAgendaTransaction {
                            last_extra_agenda_timestamp: tx.timestamp,
                            transaction_merkle_root: *transaction_merkle_root,
                        };
                    }
                    ExtraAgendaTransaction::Undelegate(tx) => {
                        verify_validator(&self.header.validator_set, &tx.delegator)?;
                        // Update reserved state by applying undelegation
                        self.state.apply_undelegate(tx).map_err(|e| {
                            Error::InvalidArgument(format!("invalid undelegation: {}", e))
                        })?;
                        self.phase = Phase::ExtraAgendaTransaction {
                            last_extra_agenda_timestamp: tx.timestamp,
                            transaction_merkle_root: *transaction_merkle_root,
                        };
                    }
                    ExtraAgendaTransaction::Report(_tx) => todo!(),
                }
            }
            (
                Commit::ExtraAgendaTransaction(t),
                Phase::ExtraAgendaTransaction {
                    last_extra_agenda_timestamp,
                    transaction_merkle_root: _,
                },
            ) => {
                match t {
                    ExtraAgendaTransaction::Delegate(tx) => {
                        verify_validator(&self.header.validator_set, &tx.delegator)?;
                        // Update reserved state by applying delegation
                        self.state.apply_delegate(tx).map_err(|e| {
                            Error::InvalidArgument(format!("invalid delegation: {}", e))
                        })?;
                        // Check if extra-agenda transactions are in chronological order
                        if tx.timestamp < *last_extra_agenda_timestamp {
                            return Err(Error::InvalidArgument(
                                format!("invalid extra-agenda transaction timestamp: expected larger than the last transaction timestamp {}, got {}", last_extra_agenda_timestamp, tx.timestamp)
                            ));
                        }
                        *last_extra_agenda_timestamp = tx.timestamp;
                    }
                    ExtraAgendaTransaction::Undelegate(tx) => {
                        verify_validator(&self.header.validator_set, &tx.delegator)?;
                        // Update reserved state by applying undelegation
                        self.state.apply_undelegate(tx).map_err(|e| {
                            Error::InvalidArgument(format!("invalid undelegation: {}", e))
                        })?;
                        // Check if extra-agenda transactions are in chronological order
                        if tx.timestamp < *last_extra_agenda_timestamp {
                            return Err(Error::InvalidArgument(
                                format!("invalid extra-agenda transaction timestamp: expected larger than the last transaction timestamp {}, got {}", last_extra_agenda_timestamp, tx.timestamp)
                            ));
                        }
                        *last_extra_agenda_timestamp = tx.timestamp;
                    }
                    ExtraAgendaTransaction::Report(_tx) => todo!(),
                }
            }
            (Commit::ChatLog(_c), _) => todo!(),
            _ => {
                return Err(Error::PhaseMismatch());
            }
        }
        self.commit_hash = self.commit_hash.aggregate(&commit.to_hash256());
        Ok(())
    }
}
