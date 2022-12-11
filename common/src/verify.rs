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
    #[error("invalid commit: applied {0} commit cannot be applied at {1} phase")]
    PhaseMismatch(String, String),
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
    if !h1
        .validator_set
        .iter()
        .any(|(public_key, _)| public_key == &h2.author)
    {
        return Err(Error::InvalidArgument(format!(
            "invalid author: {} is not in the validator set",
            h2.author
        )));
    }
    if h2.timestamp < h1.timestamp {
        return Err(Error::InvalidArgument(format!(
            "invalid timestamp: expected larger than or equal to {}, got {}",
            h1.timestamp, h2.timestamp
        )));
    }
    verify_finalization_proof(h1, &h2.prev_block_finalization_proof)?;
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
// Note that `Phase::X` is agenda phase where `Commit::X` is the last commit.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Phase {
    // The transaction phase.
    // Note that there can be agendas without transactions.
    Transaction {
        last_transaction: Transaction,
        preceding_transactions: Vec<Transaction>,
    },
    // The agenda phase.
    Agenda {
        agenda: Agenda,
    },
    // The agenda proof phase.
    AgendaProof {
        agenda_proof: AgendaProof,
    },
    // The extra phase.
    // Extra phase consists of `ExtraAgendaTransaction`s and `ChatLog`s.
    ExtraAgendaTransaction {
        last_extra_agenda_timestamp: Timestamp,
        // TODO: add `ChatLog` here.
    },
    // The block phase.
    Block,
}

/// Verifies whether the given sequence of commits can be agenda subset of agenda finalized chain.
///
/// It may accept sequences that contain more than one `BlockHeader`.
#[derive(Debug, Clone)]
pub struct CommitSequenceVerifier {
    header: BlockHeader,
    phase: Phase,
    reserved_state: ReservedState,
    next_block_commits: Vec<Commit>,
    total_commits: Vec<Commit>,
}

impl CommitSequenceVerifier {
    /// Creates agenda new `CommitSequenceVerifier` with the given block header.
    pub fn new(start_header: BlockHeader, reserved_state: ReservedState) -> Result<Self, Error> {
        Ok(Self {
            header: start_header.clone(),
            phase: Phase::Block,
            reserved_state,
            next_block_commits: vec![],
            total_commits: vec![Commit::Block(start_header)],
        })
    }

    /// Returns the commits received so far.
    pub fn get_total_commits(&self) -> &[Commit] {
        &self.total_commits
    }

    /// Returns the block headers received so far.
    ///
    /// It returns `[start_header]` if no block header has been received.
    pub fn get_block_headers(&self) -> Vec<BlockHeader> {
        self.total_commits
            .iter()
            .filter_map(|commit| match commit {
                Commit::Block(header) => Some(header.clone()),
                _ => None,
            })
            .collect()
    }

    /// Verifies finalization of the last header with the given proof.
    ///
    /// Note that due to the nature of the finalization proof (included in the next block)
    /// there is always an unverified last header (which may even not be the last commit).
    pub fn verify_last_header_finalization(&self, proof: &FinalizationProof) -> Result<(), Error> {
        verify_finalization_proof(&self.header, proof)
    }

    /// Verifies the given commit and updates the internal reserved_state of CommitSequenceVerifier.
    pub fn apply_commit(&mut self, commit: &Commit) -> Result<(), Error> {
        match (commit, &mut self.phase) {
            (Commit::Block(block_header), Phase::AgendaProof { agenda_proof: _ }) => {
                verify_header_to_header(&self.header, block_header)?;
                // Verify commit merkle root
                let commit_merkle_root =
                    BlockHeader::calculate_commit_merkle_root(&self.next_block_commits);
                if commit_merkle_root != block_header.commit_merkle_root {
                    return Err(Error::InvalidArgument(format!(
                        "invalid commit merkle root: expected {}, got {}",
                        commit_merkle_root, block_header.commit_merkle_root
                    )));
                };
                self.header = block_header.clone();
                self.phase = Phase::Block;
                self.next_block_commits = vec![];
            }
            (
                Commit::Block(block_header),
                Phase::ExtraAgendaTransaction {
                    last_extra_agenda_timestamp,
                },
            ) => {
                verify_header_to_header(&self.header, block_header)?;
                // Check if the block contains all the extra-agenda transactions.
                if block_header.timestamp < *last_extra_agenda_timestamp {
                    return Err(Error::InvalidArgument(format!(
                        "invalid block timestamp: expected larger than or equal to the last extra-agenda transaction timestamp {}, got {}",
                        last_extra_agenda_timestamp, block_header.timestamp
                    )));
                }
                // Verify commit hash
                let commit_merkle_root =
                    BlockHeader::calculate_commit_merkle_root(&self.next_block_commits);
                if commit_merkle_root != block_header.commit_merkle_root {
                    return Err(Error::InvalidArgument(format!(
                        "invalid commit merkle root: expected {}, got {}",
                        commit_merkle_root, block_header.commit_merkle_root
                    )));
                };
                self.header = block_header.clone();
                self.phase = Phase::Block;
                self.next_block_commits = vec![];
            }
            (Commit::Transaction(tx), Phase::Block) => {
                // Update reserved_state for reserved-diff transactions.
                if let Diff::Reserved(rs) = &tx.diff {
                    self.reserved_state = *rs.clone();
                }
                self.phase = Phase::Transaction {
                    last_transaction: tx.clone(),
                    preceding_transactions: vec![],
                };
            }
            (
                Commit::Transaction(tx),
                Phase::Transaction {
                    last_transaction,
                    preceding_transactions,
                },
            ) => {
                // Check if transactions are in chronological order
                if tx.timestamp < last_transaction.timestamp {
                    return Err(Error::InvalidArgument(format!(
                        "invalid transaction timestamp: expected larger than or equal to the last transaction timestamp {}, got {}",
                        last_transaction.timestamp, tx.timestamp
                    )));
                }
                // Update reserved_state for reserved-diff transactions.
                if let Diff::Reserved(rs) = &tx.diff {
                    self.reserved_state = *rs.clone();
                }
                preceding_transactions.push(last_transaction.clone());
                *last_transaction = tx.clone();
            }
            (Commit::Agenda(agenda), Phase::Block) => {
                // Check if agenda is associated with the current block sequence.
                if agenda.height != self.header.height + 1 {
                    return Err(Error::InvalidArgument(format!(
                        "invalid agenda block height: expected {}, got {}",
                        self.header.height + 1,
                        agenda.height
                    )));
                }
                // Verify agenda without transactions
                if agenda.transactions_hash != Agenda::calculate_transactions_hash(&[]) {
                    return Err(Error::InvalidArgument(format!(
                        "invalid agenda transactions_hash: expected {}, got {}",
                        Agenda::calculate_transactions_hash(&[]),
                        agenda.transactions_hash
                    )));
                }
                self.phase = Phase::Agenda {
                    agenda: agenda.clone(),
                };
            }
            (
                Commit::Agenda(agenda),
                Phase::Transaction {
                    last_transaction,
                    preceding_transactions,
                },
            ) => {
                // Check if agenda is associated with the current block sequence.
                if agenda.height != self.header.height + 1 {
                    return Err(Error::InvalidArgument(format!(
                        "invalid agenda block height: expected {}, got {}",
                        self.header.height + 1,
                        agenda.height
                    )));
                }
                // Check if agenda is in chronological order
                if agenda.timestamp < last_transaction.timestamp {
                    return Err(Error::InvalidArgument(
                        format!("invalid agenda timestamp: expected larger than or equal to the last transaction timestamp {}, got {}", last_transaction.timestamp, agenda.timestamp)
                    ));
                }
                // Verify agenda
                let transactions = [
                    preceding_transactions.clone(),
                    vec![last_transaction.clone()],
                ]
                .concat();
                if agenda.transactions_hash != Agenda::calculate_transactions_hash(&transactions) {
                    return Err(Error::InvalidArgument(format!(
                        "invalid agenda transactions_hash: expected {}, got {}",
                        Agenda::calculate_transactions_hash(&transactions),
                        agenda.transactions_hash
                    )));
                }
                self.phase = Phase::Agenda {
                    agenda: agenda.clone(),
                };
            }
            (Commit::AgendaProof(agenda_proof), Phase::Agenda { agenda }) => {
                // Check if agenda proof is associated with the current block sequence.
                if agenda_proof.height != self.header.height + 1 {
                    return Err(Error::InvalidArgument(format!(
                        "invalid agenda proof block height: expected {}, got {}",
                        self.header.height + 1,
                        agenda_proof.height
                    )));
                }
                // Check if agenda hash matches
                if agenda_proof.agenda_hash != agenda.to_hash256() {
                    return Err(Error::InvalidArgument(format!(
                        "invalid agenda proof: invalid agenda hash expected {}, got {}",
                        agenda.to_hash256(),
                        agenda_proof.agenda_hash
                    )));
                }
                // Verify the agenda proof
                for signature in agenda_proof.proof.iter() {
                    signature.verify(agenda).map_err(|e| {
                        Error::CryptoError("invalid agenda proof: invalid signature".to_string(), e)
                    })?;
                }
                self.phase = Phase::AgendaProof {
                    agenda_proof: agenda_proof.clone(),
                };
            }
            (Commit::ExtraAgendaTransaction(tx), Phase::AgendaProof { agenda_proof: _ }) => {
                match tx {
                    ExtraAgendaTransaction::Delegate(tx) => {
                        // Update reserved reserved_state by applying delegation
                        self.reserved_state.apply_delegate(tx).map_err(|e| {
                            Error::InvalidArgument(format!("invalid delegation: {}", e))
                        })?;
                        self.phase = Phase::ExtraAgendaTransaction {
                            last_extra_agenda_timestamp: tx.timestamp,
                        };
                    }
                    ExtraAgendaTransaction::Undelegate(tx) => {
                        // Update reserved reserved_state by applying undelegation
                        self.reserved_state.apply_undelegate(tx).map_err(|e| {
                            Error::InvalidArgument(format!("invalid undelegation: {}", e))
                        })?;
                        self.phase = Phase::ExtraAgendaTransaction {
                            last_extra_agenda_timestamp: tx.timestamp,
                        };
                    }
                    ExtraAgendaTransaction::Report(_tx) => unimplemented!(),
                }
            }
            (
                Commit::ExtraAgendaTransaction(tx),
                Phase::ExtraAgendaTransaction {
                    last_extra_agenda_timestamp,
                },
            ) => {
                match tx {
                    ExtraAgendaTransaction::Delegate(tx) => {
                        // Update reserved reserved_state by applying delegation
                        self.reserved_state.apply_delegate(tx).map_err(|e| {
                            Error::InvalidArgument(format!("invalid delegation: {}", e))
                        })?;
                        // Check if extra-agenda transactions are in chronological order
                        if tx.timestamp < *last_extra_agenda_timestamp {
                            return Err(Error::InvalidArgument(
                                format!("invalid extra-agenda transaction timestamp: expected larger than or equal to the last transaction timestamp {}, got {}", last_extra_agenda_timestamp, tx.timestamp)
                            ));
                        }
                        *last_extra_agenda_timestamp = tx.timestamp;
                    }
                    ExtraAgendaTransaction::Undelegate(tx) => {
                        // Update reserved reserved_state by applying undelegation
                        self.reserved_state.apply_undelegate(tx).map_err(|e| {
                            Error::InvalidArgument(format!("invalid undelegation: {}", e))
                        })?;
                        // Check if extra-agenda transactions are in chronological order
                        if tx.timestamp < *last_extra_agenda_timestamp {
                            return Err(Error::InvalidArgument(
                                format!("invalid extra-agenda transaction timestamp: expected larger than or equal to the last transaction timestamp {}, got {}", last_extra_agenda_timestamp, tx.timestamp)
                            ));
                        }
                        *last_extra_agenda_timestamp = tx.timestamp;
                    }
                    ExtraAgendaTransaction::Report(_tx) => unimplemented!(),
                }
            }
            (Commit::ChatLog(_chat_log), _) => unimplemented!(),
            (commit, phase) => {
                return Err(Error::PhaseMismatch(
                    format!("{:?}", commit),
                    format!("{:?}", phase),
                ));
            }
        }
        self.next_block_commits.push(commit.clone());
        self.total_commits.push(commit.clone());
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

    fn generate_block_header(
        validator_keypair: &[(PublicKey, PrivateKey)],
        author_index: usize,
        finalization_proof: FinalizationProof,
        previous_hash_value: Hash256,
        block_height: BlockHeight,
        time: Timestamp,
        commit_merkle_root_value: Hash256,
    ) -> BlockHeader {
        let validator_set: Vec<(PublicKey, u64)> = validator_keypair
            .iter()
            .map(|(public_key, _)| (public_key.clone(), 1))
            .collect();
        BlockHeader {
            author: validator_set[author_index].0.clone(),
            prev_block_finalization_proof: finalization_proof,
            previous_hash: previous_hash_value,
            height: block_height,
            timestamp: time,
            commit_merkle_root: commit_merkle_root_value,
            repository_merkle_root: Hash256::zero(),
            validator_set: validator_set.to_vec(),
            version: SIMPERBY_CORE_PROTOCOL_VERSION.to_string(),
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

    fn generate_reserved_state(
        validator_keypair: &[(PublicKey, PrivateKey)],
        author_index: usize,
        time: Timestamp,
    ) -> ReservedState {
        let genesis_header: BlockHeader = BlockHeader {
            author: validator_keypair[author_index].0.clone(),
            prev_block_finalization_proof: vec![],
            previous_hash: Hash256::zero(),
            height: 0,
            timestamp: time,
            commit_merkle_root: OneshotMerkleTree::create(vec![]).root(),
            repository_merkle_root: Hash256::zero(),
            validator_set: validator_keypair
                .iter()
                .map(|(public_key, _)| (public_key.clone(), 1))
                .collect(),
            version: SIMPERBY_CORE_PROTOCOL_VERSION.to_string(),
        };
        let members = get_members(&genesis_header.validator_set);
        let mut consensus_leader_order: Vec<MemberName> =
            members.iter().map(|member| member.name.clone()).collect();
        consensus_leader_order.sort();
        ReservedState {
            genesis_info: GenesisInfo {
                header: genesis_header.clone(),
                genesis_proof: generate_unanimous_finalization_proof(
                    validator_keypair,
                    &genesis_header,
                ),
                chain_name: "PDAO Chain".to_string(),
            },
            members, // TODO: fix to not use genesis header
            consensus_leader_order,
            version: SIMPERBY_CORE_PROTOCOL_VERSION.to_string(),
        }
    }

    fn generate_empty_transaction_commit(
        validator_keypair: &[(PublicKey, PrivateKey)],
        author_index: usize,
        time: Timestamp,
    ) -> Commit {
        Commit::Transaction(Transaction {
            author: validator_keypair[author_index].0.clone(),
            timestamp: time,
            head: "Test empty commit".to_string(),
            body: "This is important!".to_string(),
            diff: Diff::None,
        })
    }

    fn generate_general_diff_transaction_commit(
        validator_keypair: &[(PublicKey, PrivateKey)],
        author_index: usize,
        time: Timestamp,
    ) -> Commit {
        Commit::Transaction(Transaction {
            author: validator_keypair[author_index].0.clone(),
            timestamp: time,
            head: "Test general-diff commit".to_string(),
            body: serde_json::to_string(&json!({
                "type": "transfer-ft",
                "asset": "ETH",
                "amount": "0.1",
                "recipient": "<key:some-addr-in-ethereum>",
            }))
            .unwrap(),
            diff: Diff::NonReserved(Hash256::hash("The actual content of the diff".as_bytes())),
        })
    }

    fn generate_reserved_diff_transaction_commit(
        validator_keypair: &mut Vec<(PublicKey, PrivateKey)>,
        reserved_state: &mut ReservedState,
        time: Timestamp,
    ) -> Commit {
        // Update reserved reserved_state
        validator_keypair.push(generate_keypair([3]));
        reserved_state.members.push(Member {
            public_key: validator_keypair.last().unwrap().0.clone(),
            name: format!("member{}", validator_keypair.len()),
            governance_voting_power: 1,
            consensus_voting_power: 1,
            governance_delegations: None,
            consensus_delegations: None,
        });
        reserved_state
            .consensus_leader_order
            .push("Dave".to_string());
        reserved_state.consensus_leader_order.sort();
        Commit::Transaction(Transaction {
            author: validator_keypair[2].0.clone(),
            timestamp: time,
            head: "Test reserved-diff commit".to_string(),
            body: String::new(),
            diff: Diff::Reserved(Box::new(reserved_state.clone())),
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
            height: agenda.height,
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
        time: Timestamp,
        commit_merkle_root_value: Hash256,
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
            timestamp: time,
            commit_merkle_root: commit_merkle_root_value,
            repository_merkle_root: repository_merkle_root_value,
            validator_set: validator_keypair
                .iter()
                .map(|(public_key, _)| (public_key.clone(), 1))
                .collect(),
            version: SIMPERBY_CORE_PROTOCOL_VERSION.to_string(),
        })
    }

    fn setup_test(
        validator_set_size: u8,
    ) -> (
        Vec<(PublicKey, PrivateKey)>,
        ReservedState,
        CommitSequenceVerifier,
    ) {
        let validator_keypair: Vec<(PublicKey, PrivateKey)> =
            generate_validator_keypair(validator_set_size);
        let start_header: BlockHeader = generate_block_header(
            &validator_keypair,
            0,
            vec![],
            Hash256::zero(),
            0,
            0,
            OneshotMerkleTree::create(vec![]).root(),
        );
        let reserved_state: ReservedState = generate_reserved_state(&validator_keypair, 0, 0);
        let csv: CommitSequenceVerifier =
            CommitSequenceVerifier::new(start_header, reserved_state.clone()).unwrap();
        (validator_keypair, reserved_state, csv)
    }

    fn calculate_agenda_transactions_hash(phase: Phase) -> Hash256 {
        if let Phase::Transaction {
            ref last_transaction,
            ref preceding_transactions,
        } = phase
        {
            Agenda::calculate_transactions_hash(
                &[
                    preceding_transactions.clone(),
                    vec![last_transaction.clone()],
                ]
                .concat(),
            )
        } else {
            Agenda::calculate_transactions_hash(&[])
        }
    }

    #[test]
    /// Test the case where the commit sequence is correct.
    fn correct_commit_sequence1() {
        let (mut validator_keypair, mut reserved_state, mut csv) = setup_test(3);
        // Apply empty transaction commit
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0, 1))
            .unwrap();
        // Apply general-diff commit
        csv.apply_commit(&generate_general_diff_transaction_commit(
            &validator_keypair,
            1,
            2,
        ))
        .unwrap();
        // Apply reserved-diff commit
        csv.apply_commit(&generate_reserved_diff_transaction_commit(
            &mut validator_keypair,
            &mut reserved_state,
            3,
        ))
        .unwrap();
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 4,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda.to_hash256(),
        ))
        .unwrap();
    }

    #[test]
    /// Test the case where the commit sequence is correct but there are no transaction commits.
    fn correct_commit_sequence2() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda.to_hash256(),
        ))
        .unwrap();
    }

    #[test]
    /// Test the case where the block commit is invalid because the block height is invalid.
    fn invalid_block_commit_with_invalid_height() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda.to_hash256(),
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
            timestamp: 2,
            commit_merkle_root: BlockHeader::calculate_commit_merkle_root(&csv.next_block_commits),
            repository_merkle_root: Hash256::zero(),
            validator_set: validator_keypair
                .iter()
                .map(|(public_key, _)| (public_key.clone(), 1))
                .collect(),
            version: SIMPERBY_CORE_PROTOCOL_VERSION.to_string(),
        }))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because the previous hash is invalid.
    fn invalid_block_commit_with_invalid_previous_hash() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda.to_hash256(),
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
            timestamp: 2,
            commit_merkle_root: BlockHeader::calculate_commit_merkle_root(&csv.next_block_commits),
            repository_merkle_root: Hash256::zero(),
            validator_set: validator_keypair
                .iter()
                .map(|(public_key, _)| (public_key.clone(), 1))
                .collect(),
            version: SIMPERBY_CORE_PROTOCOL_VERSION.to_string(),
        }))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because the author is invalid.
    fn invalid_block_commit_with_invalid_author() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda.to_hash256(),
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
            timestamp: 2,
            commit_merkle_root: BlockHeader::calculate_commit_merkle_root(&csv.next_block_commits),
            repository_merkle_root: Hash256::zero(),
            validator_set: validator_keypair
                .iter()
                .map(|(public_key, _)| (public_key.clone(), 1))
                .collect(),
            version: SIMPERBY_CORE_PROTOCOL_VERSION.to_string(),
        }))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because the timestamp is invalid.
    fn invalid_block_commit_with_invalid_timestamp() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda.to_hash256(),
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
            timestamp: -1,
            commit_merkle_root: BlockHeader::calculate_commit_merkle_root(&csv.next_block_commits),
            repository_merkle_root: Hash256::zero(),
            validator_set: validator_keypair
                .iter()
                .map(|(public_key, _)| (public_key.clone(), 1))
                .collect(),
            version: SIMPERBY_CORE_PROTOCOL_VERSION.to_string(),
        }))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because the finalization proof is invalid for invalid signature.
    fn invalid_block_commit_with_invalid_finalization_proof_for_invalid_signature() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda.to_hash256(),
        ))
        .unwrap();
        // Apply block commit with invalid finalization proof for invalid signature
        csv.apply_commit(&Commit::Block(generate_block_header(
            &validator_keypair,
            0,
            generate_unanimous_finalization_proof(
                &validator_keypair,
                &generate_block_header(
                    &validator_keypair[1..],
                    0,
                    vec![],
                    csv.header.to_hash256(),
                    csv.header.height + 1,
                    2,
                    OneshotMerkleTree::create(vec![]).root(),
                ),
            ),
            csv.header.to_hash256(),
            csv.header.height + 1,
            2,
            OneshotMerkleTree::create(vec![]).root(),
        )))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because the finalization proof is invalid for low voting power.
    fn invalid_block_commit_with_invalid_finalization_proof_for_low_voting_power() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda.to_hash256(),
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
            timestamp: 2,
            commit_merkle_root: BlockHeader::calculate_commit_merkle_root(&csv.next_block_commits),
            repository_merkle_root: Hash256::zero(),
            validator_set: validator_keypair
                .iter()
                .map(|(public_key, _)| (public_key.clone(), 1))
                .collect(),
            version: SIMPERBY_CORE_PROTOCOL_VERSION.to_string(),
        }))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because the commit merkle root is invalid.
    fn invalid_block_commit_with_invalid_commit_merkle_root() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda.to_hash256(),
        ))
        .unwrap();
        // Apply block commit with invalid commit merkle root
        csv.apply_commit(&Commit::Block(BlockHeader {
            author: validator_keypair[0].0.clone(),
            prev_block_finalization_proof: generate_unanimous_finalization_proof(
                &validator_keypair,
                &csv.header,
            ),
            previous_hash: Commit::Block(csv.header.clone()).to_hash256(),
            height: csv.header.height + 1,
            timestamp: 2,
            commit_merkle_root: OneshotMerkleTree::create(vec![]).root(),
            repository_merkle_root: Hash256::zero(),
            validator_set: validator_keypair
                .iter()
                .map(|(public_key, _)| (public_key.clone(), 1))
                .collect(),
            version: SIMPERBY_CORE_PROTOCOL_VERSION.to_string(),
        }))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because block commit already exists.
    fn phase_mismatch_for_block_commit1() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply block commit at block phase
        csv.apply_commit(&generate_block_commit(
            &validator_keypair,
            0,
            csv.header.clone(),
            1,
            OneshotMerkleTree::create(vec![]).root(),
            Hash256::zero(),
        ))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because it is transaction phase.
    fn phase_mismatch_for_block_commit2() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply empty transaction commit
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0, 1))
            .unwrap();
        // Apply block commit at transaction phase
        csv.apply_commit(&generate_block_commit(
            &validator_keypair,
            0,
            csv.header.clone(),
            2,
            OneshotMerkleTree::create(vec![]).root(),
            Hash256::zero(),
        ))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the block commit is invalid because it is agenda phase.
    fn phase_mismatch_for_block_commit3() {
        let (mut validator_keypair, mut reserved_state, mut csv) = setup_test(3);
        // Apply empty transaction commit
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0, 1))
            .unwrap();
        // Apply general-diff commit
        csv.apply_commit(&generate_general_diff_transaction_commit(
            &validator_keypair,
            1,
            2,
        ))
        .unwrap();
        // Apply reserved-diff commit
        csv.apply_commit(&generate_reserved_diff_transaction_commit(
            &mut validator_keypair,
            &mut reserved_state,
            3,
        ))
        .unwrap();
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 4,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply block commit at agenda phase
        csv.apply_commit(&generate_block_commit(
            &validator_keypair,
            0,
            csv.header.clone(),
            5,
            OneshotMerkleTree::create(vec![]).root(),
            Hash256::zero(),
        ))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the transaction commit is invalid because the transaction timestamp is invalid.
    fn invalid_transaction_commit_with_invalid_timestamp() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply empty transaction commit
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0, 1))
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
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply transaction commit at agenda phase
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0, 2))
            .unwrap_err();
    }

    #[test]
    /// Test the case where the transaction commit is invalid because it is agenda proof phase.
    fn phase_mismatch_for_transaction_commit2() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda.to_hash256(),
        ))
        .unwrap();
        // Apply transaction commit at agenda proof phase
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0, 2))
            .unwrap_err();
    }

    // TODO: add test case where the transaction commit is invalid because it is extra-agenda transaction phase.

    #[test]
    /// Test the case where the agenda commit is invalid because the agenda height is invalid.
    /// The agenda height should be the next height of the last header height.
    fn invalid_agenda_commit_with_invalid_height() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit with invalid height
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: 0,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda))
            .unwrap_err();
    }

    #[test]
    /// Test the case where the agenda commit is invalid because the agenda hash is invalid.
    fn invalid_agenda_commit_with_invalid_agenda_hash1() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit with invalid agenda hash
        let agenda_transactions_hash = if let Commit::Transaction(transaction) =
            generate_empty_transaction_commit(&validator_keypair, 0, 0)
        {
            Agenda::calculate_transactions_hash(&[transaction])
        } else {
            panic!("generate_empty_transaction_commit should return Commit::Transaction type value")
        };
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda))
            .unwrap_err();
    }

    #[test]
    /// Test the case where the agenda commit is invalid because the agenda hash is invalid.
    fn invalid_agenda_commit_with_invalid_agenda_hash2() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply empty transaction commit
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0, 1))
            .unwrap();
        // Apply agenda commit with invalid agenda hash
        let agenda_transactions_hash = Agenda::calculate_transactions_hash(&[]);
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 2,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda))
            .unwrap_err();
    }

    #[test]
    /// Test the case where the agenda commit is invalid because the timestamp is invalid.
    fn invalid_agenda_commit_with_invalid_timestamp() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply empty transaction commit
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0, 1))
            .unwrap();
        // Apply agenda commit with invalid timestamp
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 0,
            transactions_hash: Agenda::calculate_transactions_hash(&[]),
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda))
            .unwrap_err();
    }

    #[test]
    /// Test the case where the agenda commit is invalid because agenda commit already exists.
    fn phase_mismatch_for_agenda_commit1() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda commit again
        csv.apply_commit(&generate_agenda_commit(&agenda))
            .unwrap_err();
    }

    #[test]
    /// Test the case where the agenda commit is invalid because it is in agenda proof phase.
    fn phase_mismatch_for_agenda_commit2() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda.to_hash256(),
        ))
        .unwrap();
        // Apply agenda commit at agenda proof phase
        csv.apply_commit(&generate_agenda_commit(&agenda))
            .unwrap_err();
    }

    // TODO: add test case where the agenda commit is invalid because it is extra-agenda transaction phase.

    #[test]
    /// Test the case where the agenda proof commit is invalid because the agenda proof height is invalid.
    /// The agenda proof height should be the next height of the last header height.
    fn invalid_agenda_proof_commit_with_invalid_height() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit with invalid height
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &Agenda {
                author: validator_keypair[1].0.clone(),
                timestamp: 1,
                transactions_hash: agenda_transactions_hash,
                height: 0,
            },
            agenda.to_hash256(),
        ))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the agenda proof commit is invalid because the agenda hash is invalid.
    fn invalid_agenda_proof_with_invalid_agenda_hash() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
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
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit with invalid signature
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &Agenda {
                author: validator_keypair[1].0.clone(),
                timestamp: 0,
                transactions_hash: Hash256::zero(),
                height: csv.header.height + 1,
            },
            agenda.to_hash256(),
        ))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the agenda proof commit is invalid because agenda proof already exists.
    fn phase_mismatch_for_agenda_proof_commit1() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda commit
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_commit(&agenda)).unwrap();
        // Apply agenda-proof commit
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda.to_hash256(),
        ))
        .unwrap();
        // Apply agenda-proof commit again
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda.to_hash256(),
        ))
        .unwrap_err();
    }

    #[test]
    /// Test the case where the agenda proof commit is invalid because it is transaction phase.
    fn phase_mismatch_for_agenda_proof_commit2() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply empty transaction commit
        csv.apply_commit(&generate_empty_transaction_commit(&validator_keypair, 0, 1))
            .unwrap();
        // Apply agenda-proof commit at transaction phase
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 2,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda.to_hash256(),
        ))
        .unwrap_err();
    }

    // TODO: add test case where the agenda proof commit is invalid because it is extra-agenda transaction phase.

    #[test]
    /// Test the case where the agenda proof commit is invalid because it is block phase.
    fn phase_mismatch_for_agenda_proof_commit3() {
        let (validator_keypair, _, mut csv) = setup_test(3);
        // Apply agenda-proof commit at block phase
        let agenda_transactions_hash = calculate_agenda_transactions_hash(csv.phase.clone());
        let agenda: Agenda = Agenda {
            author: validator_keypair[0].0.clone(),
            timestamp: 1,
            transactions_hash: agenda_transactions_hash,
            height: csv.header.height + 1,
        };
        csv.apply_commit(&generate_agenda_proof_commit(
            &validator_keypair,
            &agenda,
            agenda.to_hash256(),
        ))
        .unwrap_err();
    }

    // TODO: add test case where extra-agenda transactions are invalid.
}
