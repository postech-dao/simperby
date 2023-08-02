use super::*;
use eyre::eyre;
use serde::{Deserialize, Serialize};
use simperby_core::*;
use simperby_network::*;
use std::collections::{BTreeMap, BTreeSet};
use vetomint::{
    BlockIdentifier, ConsensusEvent, ConsensusParams, ConsensusResponse, HeightInfo, Vetomint,
};

pub type Error = eyre::Error;

/// Consensus messages to propagate each other.
///
/// Note that all message are signed by DMS itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusMessage {
    Proposal {
        round: ConsensusRound,
        valid_round: Option<ConsensusRound>,
        block_hash: Hash256,
    },
    NonNilPreVoted(ConsensusRound, Hash256),
    NonNilPreCommitted(ConsensusRound, Hash256),
    NilPreVoted(ConsensusRound),
    NilPreCommitted(ConsensusRound),
}

impl ToHash256 for ConsensusMessage {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_spb::to_vec(self).unwrap())
    }
}

impl DmsMessage for ConsensusMessage {
    const DMS_TAG: &'static str = "consensus";

    fn check(&self) -> Result<(), dms::Error> {
        Ok(())
    }

    fn commit(
        &self,
        dms_key: &DmsKey,
        private_key: &PrivateKey,
    ) -> Result<MessageCommitmentProof, simperby_core::CryptoError>
    where
        Self: Sized,
    {
        Ok(MessageCommitmentProof {
            signature: match self {
                ConsensusMessage::NonNilPreCommitted(round, block_hash) => Signature::sign(
                    FinalizationSignTarget {
                        block_hash: *block_hash,
                        round: *round,
                    }
                    .to_hash256(),
                    private_key,
                )?,
                _ => Signature::sign(
                    self.to_hash256().aggregate(&dms_key.to_hash256()),
                    private_key,
                )?,
            },
            committer: private_key.public_key(),
        })
    }

    fn verify_commitment(
        &self,
        proof: &MessageCommitmentProof,
        dms_key: &DmsKey,
    ) -> Result<(), simperby_core::CryptoError> {
        match self {
            ConsensusMessage::NonNilPreCommitted(round, block_hash) => proof.signature.verify(
                FinalizationSignTarget {
                    block_hash: *block_hash,
                    round: *round,
                }
                .to_hash256(),
                &proof.committer,
            ),
            _ => proof.signature.verify(
                self.to_hash256().aggregate(&dms_key.to_hash256()),
                &proof.committer,
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    /// The vetomint state machine.
    vetomint: Vetomint,
    /// The block header that this consensus is performing on.
    block_header: BlockHeader,
    /// An increasing counter for assigning block identifiers.
    block_identifier_count: BlockIdentifier,
    /// The list of the block hashes that have been verified.
    verified_block_hashes: BTreeMap<Hash256, BlockIdentifier>,
    /// The set of hashes of the block that are valid but vetoed by the user.
    vetoed_block_hashes: BTreeSet<Hash256>,
    /// The list of the events that are to be processed.
    to_be_processed_events: Vec<(ConsensusEvent, Timestamp)>,
    /// The set of messages that have been already updated to the Vetomint state machine.
    updated_events: BTreeSet<ConsensusEvent>,
    /// Messages by this node, which are to be broadcasted.
    messages_to_broadcast: Vec<ConsensusMessage>,
    /// Precommits collected so far, for each `(block, round)`.
    precommits: BTreeMap<(Hash256, ConsensusRound), Vec<TypedSignature<FinalizationSignTarget>>>,
    /// If `Some`, any operation on the consensus module will fail;
    /// the user must run `new()` with the next height info.
    finalized: Option<Finalization>,
}

impl State {
    pub fn new(
        block_header: &BlockHeader,
        consensus_parameters: ConsensusParams,
        round_zero_timestamp: Timestamp,
        this_node_key: PrivateKey,
    ) -> Result<State, Error> {
        let height_info = generate_height_info(
            block_header,
            consensus_parameters,
            round_zero_timestamp,
            this_node_key,
        )?;
        let state = State {
            vetomint: Vetomint::new(height_info),
            block_header: block_header.clone(),
            block_identifier_count: 0,
            to_be_processed_events: vec![(ConsensusEvent::Start, round_zero_timestamp)],
            updated_events: BTreeSet::new(),
            verified_block_hashes: BTreeMap::new(),
            vetoed_block_hashes: BTreeSet::new(),
            messages_to_broadcast: Vec::new(),
            precommits: BTreeMap::new(),
            finalized: None,
        };
        Ok(state)
    }

    pub fn check_finalized(&self) -> Option<Finalization> {
        self.finalized.clone()
    }

    pub fn block_header(&self) -> &BlockHeader {
        &self.block_header
    }

    pub fn register_verified_block_hash(&mut self, block_hash: Hash256) {
        self.assert_not_finalized();
        if self.verified_block_hashes.contains_key(&block_hash) {
            return;
        }
        self.verified_block_hashes
            .insert(block_hash, self.block_identifier_count);
        self.block_identifier_count += 1;
    }

    pub fn set_proposal_candidate(
        &mut self,
        block_hash: Hash256,
        timestamp: Timestamp,
    ) -> Result<(), Error> {
        self.assert_not_finalized();
        let block_index = self.get_block_index(&block_hash)?;
        let consensus_event = ConsensusEvent::BlockCandidateUpdated {
            proposal: block_index,
        };
        self.to_be_processed_events
            .push((consensus_event, timestamp));
        Ok(())
    }

    pub fn veto_block(&mut self, block_hash: Hash256) {
        self.assert_not_finalized();
        self.vetoed_block_hashes.insert(block_hash);
    }

    pub fn veto_round(&mut self, round: ConsensusRound, timestamp: Timestamp) {
        self.assert_not_finalized();
        let consensus_event = ConsensusEvent::SkipRound {
            round: round as usize,
        };
        self.to_be_processed_events
            .push((consensus_event, timestamp));
    }

    pub fn add_consensus_messages(
        &mut self,
        messages: Vec<(ConsensusMessage, PublicKey, Signature)>,
        timestamp: Timestamp,
    ) {
        self.assert_not_finalized();
        for (message, author, signature) in messages {
            if !self.is_consensus_message_acceptable(&message) {
                continue;
            }
            let event = self.convert_consensus_message_to_event(
                &message,
                self.get_validator_index(&author)
                    .expect("dms signer must be one of the validators"),
            );
            if self.updated_events.contains(&event) {
                continue;
            }
            self.to_be_processed_events.push((event, timestamp));
            if let ConsensusMessage::NonNilPreCommitted(round, block_hash) = message {
                self.precommits
                    .entry((block_hash, round))
                    .and_modify(|v| v.push(TypedSignature::new(signature.clone(), author.clone())))
                    .or_insert(vec![TypedSignature::new(signature, author)]);
            }
        }
    }

    pub fn progress(&mut self, timestamp: Timestamp) -> Vec<ProgressResult> {
        self.assert_not_finalized();
        let mut result = Vec::new();
        self.to_be_processed_events
            .push((ConsensusEvent::Timer, timestamp));
        while let Some((event, timestamp)) = self.to_be_processed_events.pop() {
            let responses = self.vetomint.progress(event.clone(), timestamp);
            self.updated_events.insert(event);
            for response in responses {
                let (x, message) =
                    self.process_consensus_response_to_progress_result(response, timestamp);
                result.push(x);
                if let Some(message) = message {
                    self.messages_to_broadcast.push(message);
                }
            }
        }
        result
    }

    pub fn drain_messages_to_broadcast(&mut self) -> Vec<ConsensusMessage> {
        self.assert_not_finalized();
        std::mem::take(&mut self.messages_to_broadcast)
    }
}

impl State {
    fn assert_not_finalized(&self) {
        if self.finalized.is_some() {
            panic!("mutable operations on finalized state");
        }
    }

    fn get_block_index(&self, block_hash: &Hash256) -> Result<usize, Error> {
        self.verified_block_hashes
            .get(block_hash)
            .ok_or_else(|| eyre!("block not verified yet"))
            .cloned()
    }

    fn get_validator_index(&self, public_key: &PublicKey) -> Result<usize, Error> {
        self.block_header
            .validator_set
            .iter()
            .position(|(x, _)| x == public_key)
            .ok_or_else(|| eyre!("validator not found"))
    }

    /// Checks if the given message is assoicated with a verified block.
    /// If not, it's not acceptable yet (though it could be turned out to be valid later).
    fn is_consensus_message_acceptable(&self, message: &ConsensusMessage) -> bool {
        match message {
            ConsensusMessage::Proposal { block_hash, .. } => {
                self.verified_block_hashes.contains_key(block_hash)
            }
            ConsensusMessage::NonNilPreVoted(_, block_hash) => {
                self.verified_block_hashes.contains_key(block_hash)
            }
            ConsensusMessage::NonNilPreCommitted(_, block_hash) => {
                self.verified_block_hashes.contains_key(block_hash)
            }
            _ => true,
        }
    }

    fn process_consensus_response_to_progress_result(
        &mut self,
        response: ConsensusResponse,
        timestamp: Timestamp,
    ) -> (ProgressResult, Option<ConsensusMessage>) {
        fn get_block_hash(state: &State, index: BlockIdentifier) -> Hash256 {
            *state
                .verified_block_hashes
                .iter()
                .find(|(_, &v)| v == index)
                .map(|(k, _)| k)
                .expect("the block is not in verified_block_hashes")
        }
        match response {
            ConsensusResponse::BroadcastProposal {
                proposal,
                valid_round,
                round,
            } => {
                let block_hash = get_block_hash(self, proposal);
                (
                    ProgressResult::Proposed(round as u64, block_hash, timestamp),
                    Some(ConsensusMessage::Proposal {
                        round: round as u64,
                        valid_round: valid_round.map(|r| r as u64),
                        block_hash,
                    }),
                )
            }
            ConsensusResponse::BroadcastPrevote { proposal, round } => {
                let (consensus_message, progress_result) = if let Some(block_index) = proposal {
                    let block_hash = get_block_hash(self, block_index);
                    (
                        ConsensusMessage::NonNilPreVoted(round as u64, block_hash),
                        ProgressResult::NonNilPreVoted(round as u64, block_hash, timestamp),
                    )
                } else {
                    let message = ConsensusMessage::NilPreVoted(round as u64);
                    let result = ProgressResult::NilPreVoted(round as u64, timestamp);
                    (message, result)
                };
                (progress_result, Some(consensus_message))
            }
            ConsensusResponse::BroadcastPrecommit { proposal, round } => {
                let (consensus_message, progress_result) = if let Some(block_index) = proposal {
                    let block_hash = get_block_hash(self, block_index);
                    (
                        ConsensusMessage::NonNilPreCommitted(round as u64, block_hash),
                        ProgressResult::NonNilPreCommitted(round as u64, block_hash, timestamp),
                    )
                } else {
                    let message = ConsensusMessage::NilPreCommitted(round as u64);
                    let result = ProgressResult::NilPreCommitted(round as u64, timestamp);
                    (message, result)
                };
                (progress_result, Some(consensus_message))
            }
            ConsensusResponse::FinalizeBlock {
                proposal, round, ..
            } => {
                let round = round as ConsensusRound;
                let block_hash = get_block_hash(self, proposal);
                let signatures = self
                    .precommits
                    .get(&(block_hash, round))
                    .cloned()
                    .expect("there must be valid precommits for the finalized block");
                let finalization = Finalization {
                    block_hash,
                    timestamp,
                    proof: FinalizationProof { round, signatures },
                };
                self.finalized = Some(finalization.clone());
                (ProgressResult::Finalized(finalization), None)
            }
            ConsensusResponse::ViolationReport {
                violator,
                misbehavior,
            } => {
                let pubkey = self
                    .block_header
                    .validator_set
                    .get(violator)
                    .expect("the violator must be in the validator set")
                    .0
                    .clone();
                (
                    // TODO: add misbehavior handling
                    ProgressResult::ViolationReported(
                        pubkey,
                        format!("{misbehavior:?}"),
                        timestamp,
                    ),
                    None,
                )
            }
        }
    }

    fn convert_consensus_message_to_event(
        &self,
        consensus_message: &ConsensusMessage,
        signer: usize,
    ) -> ConsensusEvent {
        match consensus_message {
            ConsensusMessage::Proposal {
                round,
                valid_round,
                block_hash,
            } => {
                let valid_round = valid_round.map(|r| r as usize);
                let index = self
                    .get_block_index(block_hash)
                    .expect("this must be already verified by the message filter");
                ConsensusEvent::BlockProposalReceived {
                    proposal: index,
                    // Todo, Note: For now, all proposals are regarded as valid.
                    // See issue#201 (https://github.com/postech-dao/simperby/issues/201).
                    valid: true,
                    valid_round,
                    proposer: signer,
                    round: *round as usize,
                    favor: !self.vetoed_block_hashes.contains(block_hash),
                }
            }
            ConsensusMessage::NonNilPreVoted(round, block_hash) => {
                let index = self
                    .get_block_index(block_hash)
                    .expect("this must be already verified by the message filter");
                ConsensusEvent::Prevote {
                    proposal: Some(index),
                    signer,
                    round: *round as usize,
                }
            }
            ConsensusMessage::NonNilPreCommitted(round, block_hash) => {
                let index = self
                    .get_block_index(block_hash)
                    .expect("this must be already verified by the message filter");
                ConsensusEvent::Precommit {
                    proposal: Some(index),
                    signer,
                    round: *round as usize,
                }
            }
            ConsensusMessage::NilPreVoted(round) => ConsensusEvent::Prevote {
                proposal: None,
                signer,
                round: *round as usize,
            },
            ConsensusMessage::NilPreCommitted(round) => ConsensusEvent::Precommit {
                proposal: None,
                signer,
                round: *round as usize,
            },
        }
    }
}

fn generate_height_info(
    header: &BlockHeader,
    consensus_params: ConsensusParams,
    round_zero_timestamp: Timestamp,
    this_node_key: PrivateKey,
) -> Result<HeightInfo, Error> {
    let this_node_index = header
        .validator_set
        .iter()
        .position(|(pubkey, _)| *pubkey == this_node_key.public_key());
    let info = HeightInfo {
        validators: header
            .validator_set
            .iter()
            .map(|(_, power)| *power)
            .collect(),
        this_node_index,
        timestamp: round_zero_timestamp,
        consensus_params,
        initial_block_candidate: 0 as BlockIdentifier,
    };
    Ok(info)
}
