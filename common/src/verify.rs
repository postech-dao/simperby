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
                        Error::CryptoError("invalid agenda proof: invalid signature".to_string(), e)
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::merkle_tree::OneshotMerkleTree;
    use serde_json::json;

    fn generate_validator_keypair(size: u8) -> Vec<(PublicKey, PrivateKey)> {
        let mut validator_keypair: Vec<(PublicKey, PrivateKey)> = vec![];
        for i in 0..size {
            validator_keypair.push(generate_keypair([i]))
        }
        validator_keypair
    }

    fn get_timestamp() -> Timestamp {
        let now = std::time::SystemTime::now();
        let since_the_epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap();
        since_the_epoch.as_millis() as Timestamp
    }

    fn generate_block_header(validator_keypair: &[(PublicKey, PrivateKey)]) -> BlockHeader {
        let validator_set: Vec<(PublicKey, u64)> = validator_keypair
            .iter()
            .map(|(public_key, _)| (public_key.clone(), 1))
            .collect();
        BlockHeader {
            author: validator_set[0].0.clone(),
            prev_block_finalization_proof: vec![],
            previous_hash: Hash256::zero(),
            height: 0,
            timestamp: get_timestamp(),
            commit_hash: Hash256::zero(),
            tx_merkle_root: OneshotMerkleTree::create(vec![]).root(),
            chat_merkle_root: Hash256::zero(),
            repository_merkle_root: Hash256::zero(),
            validator_set: validator_set.to_vec(),
            version: "0.0.0".to_string(),
        }
    }

    fn get_members(validator_set: &[(PublicKey, VotingPower)]) -> Vec<Member> {
        let mut members = vec![];
        for (i, (public_key, voting_power)) in validator_set.iter().enumerate() {
            members.push(Member {
                public_key: public_key.clone(),
                name: format!("member{}", i).to_string(),
                governance_voting_power: *voting_power,
                consensus_voting_power: *voting_power,
                governance_delegations: None,
                consensus_delegations: None,
            });
        }
        members
    }

    fn generate_reserved_state(start_header: &BlockHeader) -> ReservedState {
        ReservedState {
            genesis_info: GenesisInfo {
                header: start_header.clone(),
                genesis_proof: vec![],
                chain_name: "PDAO Chain".to_string(),
            },
            members: get_members(&start_header.validator_set),
            consensus_leader_order: vec![0, 1, 2],
            version: "0.0.0".to_string(),
        }
    }

    fn generate_empty_transaction_commit(
        validator_keypair: &[(PublicKey, PrivateKey)],
        author_index: usize,
    ) -> Commit {
        Commit::Transaction(Transaction {
            author: validator_keypair[author_index].0.clone(),
            timestamp: get_timestamp(),
            head: "Test empty commit".to_string(),
            body: "This is important!".to_string(),
            diff: Diff::None,
        })
    }

    fn generate_general_diff_transaction_commit(
        validator_keypair: &[(PublicKey, PrivateKey)],
        author_index: usize,
    ) -> Commit {
        Commit::Transaction(Transaction {
            author: validator_keypair[author_index].0.clone(),
            timestamp: get_timestamp(),
            head: "Test general-diff commit".to_string(),
            body: serde_json::to_string(&json!({
                "type": "transfer-ft",
                "asset": "ETH",
                "amount": "0.1",
                "recipient": "<key:some-addr-in-ethereum>",
            }))
            .unwrap(),
            diff: Diff::General(Hash256::hash("The actual content of the diff".as_bytes())),
        })
    }

    fn generate_reserved_diff_transaction_commit(
        validator_keypair: &mut Vec<(PublicKey, PrivateKey)>,
        reserved_state: &mut ReservedState,
    ) -> Commit {
        // Update reserved state
        validator_keypair.push(generate_keypair([3]));
        reserved_state.members.push(Member {
            public_key: validator_keypair.last().unwrap().0.clone(),
            name: format!("member{}", validator_keypair.len()),
            governance_voting_power: 1,
            consensus_voting_power: 1,
            governance_delegations: None,
            consensus_delegations: None,
        });
        reserved_state.consensus_leader_order.push(3);
        let diff: String = serde_json::to_string(&json!({
            "public_key": validator_keypair.last().unwrap().0,
            "consensus_voting_power": 1,
            "governance_voting_power": 1,
            "delegation": null,
        }))
        .unwrap();
        Commit::Transaction(Transaction {
            author: validator_keypair[2].0.clone(),
            timestamp: get_timestamp(),
            head: "Test reserved-diff commit".to_string(),
            body: diff.clone(),
            diff: Diff::Reserved(Box::new(reserved_state.clone()), diff.to_hash256()),
        })
    }

    fn generate_agenda_commit(agenda: &Agenda) -> Commit {
        Commit::Agenda(agenda.clone())
    }

    fn generate_agenda_proof_commit(
        validator_keypair: &[(PublicKey, PrivateKey)],
        agenda: &Agenda,
        agenda_hash_value: Hash256,
    ) -> Commit {
        let mut agenda_proof: Vec<TypedSignature<Agenda>> = vec![];
        for (_, private_key) in validator_keypair {
            agenda_proof.push(TypedSignature::sign(agenda, private_key).unwrap())
        }
        Commit::AgendaProof(AgendaProof {
            agenda_hash: agenda_hash_value,
            proof: agenda_proof,
        })
    }

    fn generate_unanimous_finalization_proof(
        validator_keypair: &[(PublicKey, PrivateKey)],
        header: &BlockHeader,
    ) -> FinalizationProof {
        let mut finalization_proof: Vec<TypedSignature<BlockHeader>> = vec![];
        for (_, private_key) in validator_keypair {
            finalization_proof.push(TypedSignature::sign(header, private_key).unwrap());
        }
        finalization_proof
    }

    fn generate_block_commit(
        validator_keypair: &[(PublicKey, PrivateKey)],
        author_index: usize,
        previous_header: BlockHeader,
        commit_hash_value: Hash256,
        tx_merkle_root_value: Hash256,
        chat_merkle_root_value: Hash256,
        repository_merkle_root_value: Hash256,
    ) -> Commit {
        Commit::Block(BlockHeader {
            author: validator_keypair[author_index].0.clone(),
            prev_block_finalization_proof: generate_unanimous_finalization_proof(
                validator_keypair,
                &previous_header,
            ),
            previous_hash: Commit::Block(previous_header.clone()).to_hash256(),
            height: previous_header.height + 1,
            timestamp: get_timestamp(),
            commit_hash: commit_hash_value,
            tx_merkle_root: tx_merkle_root_value,
            chat_merkle_root: chat_merkle_root_value,
            repository_merkle_root: repository_merkle_root_value,
            validator_set: validator_keypair
                .iter()
                .map(|(public_key, _)| (public_key.clone(), 1))
                .collect(),
            version: "0.0.0".to_string(),
        })
    }

    #[test]
    /// Test the case where the commit sequence is correct.
    fn correct_commit_sequence1() {
        let mut validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let mut reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state.clone()).unwrap();
        // Apply empty transaction commit
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0))
            .unwrap();
        // Apply general-diff commit
        csv.apply_commit(&generate_general_diff_transaction_commit(
            &validator_keypair,
            1,
        ))
        .unwrap();
        // Apply reserved-diff commit
        csv.apply_commit(&generate_reserved_diff_transaction_commit(
            &mut validator_keypair,
            &mut reserved_state,
        ))
        .unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda_hash_value,
        ))
        .unwrap();
    }

    #[test]
    /// Test the case where the commit sequence is correct but there are no transaction commits.
    fn correct_commit_sequence2() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda_hash_value,
        ))
        .unwrap();
    }

    #[test]
    /// Test the case where the block commit is invalid because the block height is invalid.
    fn invalid_block_commit_with_invalid_height() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda_hash_value,
        ))
        .unwrap();
        // Apply block commit with invalid height
        csv.apply_commit(&Commit::Block(BlockHeader {
            author: validator_keypair[0].0.clone(),
            prev_block_finalization_proof: generate_unanimous_finalization_proof(
                &validator_keypair,
                &csv.header,
            ),
            previous_hash: Commit::Block(csv.header.clone()).to_hash256(),
            height: csv.header.height + 2,
            timestamp: get_timestamp(),
            commit_hash: csv.commit_hash,
            tx_merkle_root: OneshotMerkleTree::create(vec![]).root(),
            chat_merkle_root: Hash256::zero(),
            repository_merkle_root: Hash256::zero(),
            validator_set: validator_keypair
                .iter()
                .map(|(public_key, _)| (public_key.clone(), 1))
                .collect(),
            version: "0.0.0".to_string(),
        }))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because the previous hash is invalid.
    fn invalid_block_commit_with_invalid_previous_hash() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda_hash_value,
        ))
        .unwrap();
        // Apply block commit with invalid previous hash
        csv.apply_commit(&Commit::Block(BlockHeader {
            author: validator_keypair[0].0.clone(),
            prev_block_finalization_proof: generate_unanimous_finalization_proof(
                &validator_keypair,
                &csv.header,
            ),
            previous_hash: Hash256::zero(),
            height: csv.header.height + 1,
            timestamp: get_timestamp(),
            commit_hash: csv.commit_hash,
            tx_merkle_root: OneshotMerkleTree::create(vec![]).root(),
            chat_merkle_root: Hash256::zero(),
            repository_merkle_root: Hash256::zero(),
            validator_set: validator_keypair
                .iter()
                .map(|(public_key, _)| (public_key.clone(), 1))
                .collect(),
            version: "0.0.0".to_string(),
        }))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because the author is invalid.
    fn invalid_block_commit_with_invalid_author() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda_hash_value,
        ))
        .unwrap();
        // Apply block commit with invalid author
        csv.apply_commit(&Commit::Block(BlockHeader {
            author: generate_keypair([42]).0,
            prev_block_finalization_proof: generate_unanimous_finalization_proof(
                &validator_keypair,
                &csv.header,
            ),
            previous_hash: Commit::Block(csv.header.clone()).to_hash256(),
            height: csv.header.height + 1,
            timestamp: get_timestamp(),
            commit_hash: csv.commit_hash,
            tx_merkle_root: OneshotMerkleTree::create(vec![]).root(),
            chat_merkle_root: Hash256::zero(),
            repository_merkle_root: Hash256::zero(),
            validator_set: validator_keypair
                .iter()
                .map(|(public_key, _)| (public_key.clone(), 1))
                .collect(),
            version: "0.0.0".to_string(),
        }))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because the timestamp is invalid.
    fn invalid_block_commit_with_invalid_timestamp() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda_hash_value,
        ))
        .unwrap();
        // Apply block commit with invalid timestamp
        csv.apply_commit(&Commit::Block(BlockHeader {
            author: validator_keypair[0].0.clone(),
            prev_block_finalization_proof: generate_unanimous_finalization_proof(
                &validator_keypair,
                &csv.header,
            ),
            previous_hash: Commit::Block(csv.header.clone()).to_hash256(),
            height: csv.header.height + 1,
            timestamp: 0,
            commit_hash: csv.commit_hash,
            tx_merkle_root: OneshotMerkleTree::create(vec![]).root(),
            chat_merkle_root: Hash256::zero(),
            repository_merkle_root: Hash256::zero(),
            validator_set: validator_keypair
                .iter()
                .map(|(public_key, _)| (public_key.clone(), 1))
                .collect(),
            version: "0.0.0".to_string(),
        }))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because the finalization proof is invalid for invalid signature.
    fn invalid_block_commit_with_invalid_finalization_proof_for_invalid_signature() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda_hash_value,
        ))
        .unwrap();
        // Apply block commit with invalid finalization proof for invalid signature
        csv.apply_commit(&Commit::Block(BlockHeader {
            author: validator_keypair[0].0.clone(),
            prev_block_finalization_proof: generate_unanimous_finalization_proof(
                &validator_keypair,
                &generate_block_header(&validator_keypair[1..]),
            ),
            previous_hash: Commit::Block(csv.header.clone()).to_hash256(),
            height: csv.header.height + 1,
            timestamp: get_timestamp(),
            commit_hash: csv.commit_hash,
            tx_merkle_root: OneshotMerkleTree::create(vec![]).root(),
            chat_merkle_root: Hash256::zero(),
            repository_merkle_root: Hash256::zero(),
            validator_set: validator_keypair
                .iter()
                .map(|(public_key, _)| (public_key.clone(), 1))
                .collect(),
            version: "0.0.0".to_string(),
        }))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because the finalization proof is invalid for low voting power.
    fn invalid_block_commit_with_invalid_finalization_proof_for_low_voting_power() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda_hash_value,
        ))
        .unwrap();
        // Apply block commit with invalid finalization proof for low voting power
        csv.apply_commit(&Commit::Block(BlockHeader {
            author: validator_keypair[0].0.clone(),
            prev_block_finalization_proof: vec![generate_unanimous_finalization_proof(
                &validator_keypair,
                &csv.header,
            )
            .first()
            .unwrap()
            .clone()],
            previous_hash: Commit::Block(csv.header.clone()).to_hash256(),
            height: csv.header.height + 1,
            timestamp: get_timestamp(),
            commit_hash: csv.commit_hash,
            tx_merkle_root: OneshotMerkleTree::create(vec![]).root(),
            chat_merkle_root: Hash256::zero(),
            repository_merkle_root: Hash256::zero(),
            validator_set: validator_keypair
                .iter()
                .map(|(public_key, _)| (public_key.clone(), 1))
                .collect(),
            version: "0.0.0".to_string(),
        }))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because the transaction merkle root is invalid.
    fn invalid_block_commit_with_invalid_transaction_merkle_root() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda_hash_value,
        ))
        .unwrap();
        // Apply block commit with invalid transaction merkle root
        csv.apply_commit(&Commit::Block(BlockHeader {
            author: validator_keypair[0].0.clone(),
            prev_block_finalization_proof: generate_unanimous_finalization_proof(
                &validator_keypair,
                &csv.header,
            ),
            previous_hash: Commit::Block(csv.header.clone()).to_hash256(),
            height: csv.header.height + 1,
            timestamp: get_timestamp(),
            commit_hash: csv.commit_hash,
            tx_merkle_root: OneshotMerkleTree::create(vec![generate_empty_transaction_commit(
                &validator_keypair,
                0,
            )
            .to_hash256()])
            .root(),
            chat_merkle_root: Hash256::zero(),
            repository_merkle_root: Hash256::zero(),
            validator_set: validator_keypair
                .iter()
                .map(|(public_key, _)| (public_key.clone(), 1))
                .collect(),
            version: "0.0.0".to_string(),
        }))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because the commit hash is invalid.
    fn invalid_block_commit_with_invalid_commit_hash() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda_hash_value,
        ))
        .unwrap();
        // Apply block commit with invalid commit hash
        csv.apply_commit(&Commit::Block(BlockHeader {
            author: validator_keypair[0].0.clone(),
            prev_block_finalization_proof: generate_unanimous_finalization_proof(
                &validator_keypair,
                &csv.header,
            ),
            previous_hash: Commit::Block(csv.header.clone()).to_hash256(),
            height: csv.header.height + 1,
            timestamp: get_timestamp(),
            commit_hash: Hash256::zero(),
            tx_merkle_root: OneshotMerkleTree::create(vec![]).root(),
            chat_merkle_root: Hash256::zero(),
            repository_merkle_root: Hash256::zero(),
            validator_set: validator_keypair
                .iter()
                .map(|(public_key, _)| (public_key.clone(), 1))
                .collect(),
            version: "0.0.0".to_string(),
        }))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because block commit already exists.
    fn multiple_block_commits() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply block commit at block phase
        csv.apply_commit(&generate_block_commit(
            &validator_keypair,
            0,
            csv.header.clone(),
            Hash256::zero(),
            Hash256::zero(),
            Hash256::zero(),
            Hash256::zero(),
        ))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because it is transaction phase.
    fn phase_mismatch_for_block_commit1() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply empty transaction commit
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0))
            .unwrap();
        // Apply block commit at transaction phase
        csv.apply_commit(&generate_block_commit(
            &validator_keypair,
            0,
            csv.header.clone(),
            Hash256::zero(),
            Hash256::zero(),
            Hash256::zero(),
            Hash256::zero(),
        ))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because it is agenda phase.
    fn phase_mismatch_for_block_commit2() {
        let mut validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let mut reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state.clone()).unwrap();
        // Apply empty transaction commit
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0))
            .unwrap();
        // Apply general-diff commit
        csv.apply_commit(&generate_general_diff_transaction_commit(
            &validator_keypair,
            1,
        ))
        .unwrap();
        // Apply reserved-diff commit
        csv.apply_commit(&generate_reserved_diff_transaction_commit(
            &mut validator_keypair,
            &mut reserved_state,
        ))
        .unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply block commit at agenda phase
        csv.apply_commit(&generate_block_commit(
            &validator_keypair,
            0,
            csv.header.clone(),
            Hash256::zero(),
            Hash256::zero(),
            Hash256::zero(),
            Hash256::zero(),
        ))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the transaction commit is invalid because the transaction timestamp is invalid.
    fn invalid_transaction_commit_with_invalid_timestamp() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply empty transaction commit
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0))
            .unwrap();
        // Apply empty transaction commit with invalid timestamp
        csv.apply_commit(&Commit::Transaction(Transaction {
            author: validator_keypair[0].0.clone(),
            timestamp: 0,
            head: "Test empty commit".to_string(),
            body: "This is important!".to_string(),
            diff: Diff::None,
        }))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the transaction commit is invalid because it is agenda phase.
    fn phase_mismatch_for_transaction_commit1() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply transaction commit at agenda phase
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0))
            .unwrap_err();
    }

    #[test]
    /// Test the case where the transaction commit is invalid because it is agenda proof phase.
    fn phase_mismatch_for_transaction_commit2() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda_hash_value,
        ))
        .unwrap();
        // Apply transaction commit at agenda proof phase
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0))
            .unwrap_err();
    }

    // TODO: add test case where the transaction commit is invalid because it is extra-agenda transaction phase.

    #[test]
    /// Test the case where the agenda commit is invalid because the agenda hash is invalid.
    fn invalid_agenda_commit_with_invalid_agenda_hash1() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda commit with invalid agenda hash
        let agenda_hash_value = if let Commit::Transaction(transaction) =
            generate_empty_transaction_commit(&validator_keypair, 0)
        {
            Agenda::calculate_hash(csv.header.height, &[transaction])
        } else {
            panic!("generate_empty_transaction_commit should return Commit::Transaction type value")
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda))
            .unwrap_err();
    }

    #[test]
    /// Test the case where the agenda commit is invalid because the agenda hash is invalid.
    fn invalid_agenda_commit_with_invalid_agenda_hash2() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply empty transaction commit
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0))
            .unwrap();
        // Apply agenda commit with invalid agenda hash
        let agenda_hash_value = Agenda::calculate_hash(csv.header.height, &[]);
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda))
            .unwrap_err();
    }

    #[test]
    /// Test the case where the agenda commit is invalid because the timestamp is invalid.
    fn invalid_agenda_commit_with_invalid_timestamp() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply empty transaction commit
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0))
            .unwrap();
        // Apply agenda commit with invalid timestamp
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 0,
            hash: Agenda::calculate_hash(csv.header.height, &[]),
        };
        csv.apply_commit(&generate_agenda_commit(&agenda))
            .unwrap_err();
    }

    #[test]
    /// Test the case where the agenda commit is invalid because agenda commit already exists.
    fn multiple_agendas() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda commit again
        csv.apply_commit(&generate_agenda_commit(&agenda))
            .unwrap_err();
    }

    #[test]
    /// Test the case where the agenda commit is invalid because it is in agenda proof phase.
    fn phase_mismatch_for_agenda_commit() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda_hash_value,
        ))
        .unwrap();
        // Apply agenda commit at agenda proof phase
        csv.apply_commit(&generate_agenda_commit(&agenda))
            .unwrap_err();
    }

    // TODO: add test case where the agenda commit is invalid because it is extra-agenda transaction phase.

    #[test]
    /// Test the case where the agenda proof commit is invalid because the agenda hash is invalid.
    fn invalid_agenda_proof_with_invalid_agenda_hash() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit with invalid agenda hash
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            Hash256::zero(),
        ))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the agenda proof commit is invalid because the signature is invalid.
    fn invalid_agenda_proof_with_invalid_signature() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit with invalid signature
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &Agenda {
                author: validator_keypair[1].0.clone(),
                timestamp: 0,
                hash: Hash256::zero(),
            },
            agenda_hash_value,
        ))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the agenda proof commit is invalid because agenda proof already exists.
    fn multiple_agenda_proofs() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda commit
        let agenda_timestamp = get_timestamp();
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda_hash_value,
        ))
        .unwrap();
        // Apply agenda-proof commit again
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda_hash_value,
        ))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the agenda proof commit is invalid because it is transaction phase.
    fn phase_mismatch_for_agenda_proof_commit1() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply empty transaction commit
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0))
            .unwrap();
        // Apply agenda-proof commit at transaction phase
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda_hash_value,
        ))
        .unwrap_err();
    }

    // TODO: add test case where the agenda proof commit is invalid because it is extra-agenda transaction phase.

    #[test]
    /// Test the case where the agenda proof commit is invalid because it is block phase.
    fn phase_mismatch_for_agenda_proof_commit2() {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> = generate_validator_keypair(3);
        let start_header: BlockHeader = generate_block_header(&validator_keypair);
        let reserved_state: ReservedState = generate_reserved_state(&start_header);
        let mut csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state).unwrap();
        // Apply agenda-proof commit at block phase
        let agenda_hash_value = if let Phase::Transaction { ref transactions } = csv.phase {
            Agenda::calculate_hash(csv.header.height, transactions)
        } else {
            Agenda::calculate_hash(csv.header.height, &[])
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: agenda_timestamp,
            hash: agenda_hash_value,
        };
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda_hash_value,
        ))
        .unwrap_err();
    }

    // TODO: add test case where extra-agenda transactions are invalid.
}
