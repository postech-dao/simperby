#![allow(dead_code)]
#![allow(unused_imports)]

use eyre::eyre;
use serde::{Deserialize, Serialize};
use simperby_common::{
    crypto::{Hash256, PublicKey},
    serde_spb, BlockHeader, BlockHeight, ConsensusRound, FinalizationProof, PrivateKey, Signature,
    Timestamp, ToHash256, TypedSignature, VotingPower,
};
use simperby_network::{
    dms::{DistributedMessageSet as DMS, Message, MessageFilter},
    primitives::{GossipNetwork, Storage},
};
use std::collections::BTreeSet;
use std::sync::Arc;
use vetomint2::*;

pub type ConsensusParameters = ConsensusParams;
pub type Error = eyre::Error;
const STATE_FILE_NAME: &str = "state.json";
pub type Nil = ();
const NIL_BLOCK_PROPOSAL_INDEX: BlockIdentifier = BlockIdentifier::MAX;

/// The signed `String` is constructed by `format!("{}-prevote", block_hash)`.
pub type Prevote = TypedSignature<String>;
/// This can be verified by `precommit.get_raw_signature().verify(block_hash, signer)`
/// where `block_hash` is the hash of `BlockHeader`.
pub type Precommit = TypedSignature<BlockHeader>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    /// The vetomint core instance.
    pub vetomint: VetomintWrapper,

    /// The block header that this consensus is performing on.
    pub block_header: BlockHeader,

    /// The set of messages that have been already updated to the Vetomint state machine.
    pub updated_messages: BTreeSet<Hash256>,
    /// The set of the block hashes that have been verified.
    pub verified_block_hashes: Vec<Hash256>,
    /// The set of the block hashes that are rejected by the user.
    pub vetoed_block_hashes: Vec<Hash256>,

    /// If true, any operation on this instance will fail; the user must
    /// run `new()` with the next height info.
    pub finalized: bool,
}

pub fn generate_dms_key(header: &BlockHeader) -> String {
    format!(
        "consensus-{}-{}",
        header.height,
        &header.to_hash256().to_string()[0..8]
    )
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

async fn commit_state(state_storage: &mut impl Storage, state: &State) -> Result<(), Error> {
    state_storage
        .add_or_overwrite_file(STATE_FILE_NAME, serde_spb::to_string(state).unwrap())
        .await?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VetomintWrapper {
    pub vetomint: Vetomint,
    pub is_started: bool,
}

impl VetomintWrapper {
    fn progress(
        &mut self,
        events: Vec<ConsensusEvent>,
        timestamp: Timestamp,
    ) -> Result<Vec<ConsensusResponse>, Error> {
        let mut result = Vec::new();
        let mut pre_events = Vec::new();
        if !self.is_started {
            pre_events.push(ConsensusEvent::Start);
            self.is_started = true;
        }
        pre_events.push(ConsensusEvent::Timer);
        for event in pre_events.into_iter().chain(events) {
            result.extend(self.vetomint.progress(event, timestamp));
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusMessage {
    Proposal {
        round: ConsensusRound,
        valid_round: Option<ConsensusRound>,
        block_hash: Hash256,
    },
    NonNilPreVoted(ConsensusRound, Hash256, Prevote),
    NonNilPreCommitted(ConsensusRound, Hash256, Precommit),
    NilPreVoted(ConsensusRound),
    NilPreCommitted(ConsensusRound),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgressResult {
    Proposed(ConsensusRound, Hash256, Timestamp),
    NonNilPreVoted(ConsensusRound, Hash256, Timestamp),
    NonNilPreCommitted(ConsensusRound, Hash256, Timestamp),
    NilPreVoted(ConsensusRound, Timestamp),
    NilPreCommitted(ConsensusRound, Timestamp),
    Finalized(Hash256, Timestamp, FinalizationProof),
    ViolationReported(PublicKey, String, Timestamp),
}

pub struct ConsensusMessageFilter {
    /// Note that it is even DESIRABLE to use a synchronous lock in the async context
    /// if it is guaranteed that the lock is not held for a long time.
    verified_block_hashes: Arc<parking_lot::RwLock<BTreeSet<Hash256>>>,
    validator_set: BTreeSet<PublicKey>,
}

impl MessageFilter for ConsensusMessageFilter {
    fn filter(&self, message: &Message) -> Result<(), String> {
        let signer = message.signature().signer();
        if !self.validator_set.contains(signer) {
            return Err("the signer is not in the validator set".to_string());
        }
        let consensus_message =
            serde_spb::from_str::<ConsensusMessage>(message.data()).map_err(|e| e.to_string())?;
        match consensus_message {
            ConsensusMessage::Proposal { block_hash, .. } => self.verify_block_hash(block_hash),
            ConsensusMessage::NonNilPreVoted(_, block_hash, prevote) => {
                if signer != prevote.signer() {
                    return Err("DMS message signer does not match with prevote signer".to_string());
                }
                let original_data = format!("{}-{}", block_hash, "prevote");
                prevote.verify(&original_data).map_err(|e| e.to_string())?;
                self.verify_block_hash(block_hash)
            }
            ConsensusMessage::NonNilPreCommitted(_, block_hash, precommit) => {
                if signer != precommit.signer() {
                    return Err(
                        "DMS message signer does not match with precommit signer".to_string()
                    );
                }
                precommit
                    .get_raw_signature()
                    .verify(block_hash, signer)
                    .map_err(|e| e.to_string())?;
                self.verify_block_hash(block_hash)
            }
            _ => Ok(()),
        }
    }
}

impl ConsensusMessageFilter {
    fn verify_block_hash(&self, block_hash: Hash256) -> Result<(), String> {
        if self.verified_block_hashes.read().contains(&block_hash) {
            Ok(())
        } else {
            Err(format!(
                "the block hash is not verified yet: {}",
                block_hash
            ))
        }
    }
}

pub struct Consensus<N: GossipNetwork, S: Storage> {
    /// The distributed consensus message set.
    dms: DMS<N, S>,
    /// The local storage for the consensus state.
    state_storage: S,
    /// The cache of the consensus state.
    state: State,
    /// The set of the block hashes that have been verified, shared by the message filter.
    ///
    /// Note that there is the exactly same copy in the `state`.
    verified_block_hashes: Arc<parking_lot::RwLock<BTreeSet<Hash256>>>,
    /// (If participated) the private key of this node
    this_node_key: Option<PrivateKey>,
}

impl<N: GossipNetwork, S: Storage> Consensus<N, S> {
    /// Creates a consensus instance.
    ///
    /// It clears and re-initializes the DMS and the stroage
    /// if the block header is different from the last one.
    pub async fn new(
        mut dms: DMS<N, S>,
        mut state_storage: S,
        block_header: BlockHeader,
        consensus_parameters: ConsensusParams,
        round_zero_timestamp: Timestamp,
        this_node_key: Option<PrivateKey>,
    ) -> Result<Self, Error> {
        // Prepare new state in case of storage reset.
        let new_state = Self::construct_new_state(
            &block_header,
            consensus_parameters,
            round_zero_timestamp,
            this_node_key.clone().unwrap(),
        )?;
        let state = if let Ok(raw_state) = state_storage.read_file(STATE_FILE_NAME).await {
            let state: State = serde_spb::from_str(&raw_state)?;
            if block_header != state.block_header {
                dms.clear(generate_dms_key(&block_header)).await?;
                state_storage.remove_all_files().await?;
                commit_state(&mut state_storage, &new_state).await?;
                new_state
            } else {
                state
            }
        } else {
            dms.clear(generate_dms_key(&block_header)).await?;
            state_storage.remove_all_files().await?;
            commit_state(&mut state_storage, &new_state).await?;
            new_state
        };
        let verified_block_hashes = Arc::new(parking_lot::RwLock::new(BTreeSet::from_iter(
            state.verified_block_hashes.iter().cloned(),
        )));
        dms.set_filter(Arc::new(ConsensusMessageFilter {
            verified_block_hashes: Arc::clone(&verified_block_hashes),
            validator_set: state
                .block_header
                .validator_set
                .iter()
                .map(|(pk, _)| pk.clone())
                .collect(),
        }));
        Ok(Self {
            dms,
            state_storage,
            state,
            verified_block_hashes,
            this_node_key,
        })
    }

    pub async fn register_verified_block_hash(&mut self, hash: Hash256) -> Result<(), Error> {
        self.abort_if_finalized()?;
        self.state.verified_block_hashes.push(hash);
        self.verified_block_hashes.write().insert(hash);
        self.state_storage
            .add_or_overwrite_file(STATE_FILE_NAME, serde_spb::to_string(&self.state).unwrap())
            .await?;
        Ok(())
    }

    // Todo: Read public state from the vetomint FSM.
    // pub async fn read_consensus_state(&self) -> Result<ConsensusState, Error> {
    //     Ok(self.state.vetomint.state)
    // }

    pub async fn set_proposal_candidate(
        &mut self,
        block_hash: Hash256,
        timestamp: Timestamp,
    ) -> Result<Vec<ProgressResult>, Error> {
        self.abort_if_finalized()?;
        let block_index = self.get_block_index(&block_hash)?;
        let consensus_event = ConsensusEvent::BlockCandidateUpdated {
            proposal: block_index,
        };
        let responses = self
            .state
            .vetomint
            .progress(vec![consensus_event], timestamp)?;
        let result = self
            .process_multiple_responses(responses, timestamp)
            .await?;
        Ok(result)
    }

    pub async fn veto_block(&mut self, block_hash: Hash256) -> Result<(), Error> {
        self.abort_if_finalized()?;
        self.state.vetoed_block_hashes.push(block_hash);
        Ok(())
    }

    pub async fn veto_round(
        &mut self,
        round: ConsensusRound,
        timestamp: Timestamp,
    ) -> Result<Vec<ProgressResult>, Error> {
        self.abort_if_finalized()?;
        let consensus_event = ConsensusEvent::SkipRound {
            round: round as usize,
        };
        let responses = self
            .state
            .vetomint
            .progress(vec![consensus_event], timestamp)?;
        let result = self
            .process_multiple_responses(responses, timestamp)
            .await?;
        Ok(result)
    }

    /// Makes a progress in the consensus process.
    /// It might
    ///
    /// 1. broadcast a proposal.
    /// 2. broadcast a pre-vote.
    /// 3. broadcast a pre-commit.
    /// 4. finalize a block, return its proof, and mark `self` as finalized to prevent any state transition.
    ///
    /// For the case 4, storage cleanup and increase of height will be handled by the node.
    pub async fn progress(&mut self, timestamp: Timestamp) -> Result<Vec<ProgressResult>, Error> {
        self.abort_if_finalized()?;
        let messages = self
            .dms
            .read_messages()
            .await?
            .into_iter()
            .filter(|m| !self.state.updated_messages.contains(&m.to_hash256()))
            .collect::<Vec<_>>();
        // Save a copy of the vetomint FSM to recover from possible DMS errors.
        // Changes are applied to the other copy, and then it is saved to the state when all messages are processed.
        let mut vetomint_copy = self.state.vetomint.clone();
        let events: Vec<ConsensusEvent> = messages
            .iter()
            .map(|message| {
                let signer = self
                    .state
                    .block_header
                    .validator_set
                    .iter()
                    .position(|(pubkey, _)| pubkey == message.signature().signer())
                    .expect("this must be already verified by the message filter");
                let consensus_message = serde_spb::from_str::<ConsensusMessage>(message.data())
                    .expect("this must be already verified by the message filter");
                self.consensus_message_to_event(&consensus_message, signer)
            })
            .collect();
        let progress_responses = vetomint_copy.progress(events, timestamp)?;
        let final_result = self
            .process_multiple_responses(progress_responses, timestamp)
            .await?;
        // The change is applied here as we reached here without facing an error.
        self.state
            .updated_messages
            .extend(messages.into_iter().map(|m| m.to_hash256()));
        self.state.vetomint = vetomint_copy;
        // Note, Todo: For now, storage errors are not handled.
        self.commit_state_to_storage().await?;
        Ok(final_result)
    }

    pub async fn fetch(&mut self) -> Result<(), Error> {
        self.dms.fetch().await
    }

    /// Serves the consensus protocol indefinitely.
    ///
    /// Note: currently it just returns itself after the given time.
    /// Todo: Implement the following:
    /// 1. It does `DistributedMessageSet::serve()`.
    /// 2. It does `Consensus::progress()` continuously.
    ///
    /// Note: Step 2 is likely to be changed because it does not notify the user
    ///       and automatically progresses.
    pub async fn serve(self, time_in_ms: u64) -> Result<Self, Error> {
        let dms = self.dms.serve(time_in_ms).await?;
        Ok(Self { dms, ..self })
    }

    /// Reads all consensus messages with its signer in the dms.
    pub async fn read_messages(&self) -> Result<Vec<(ConsensusMessage, PublicKey)>, Error> {
        let raw_messages = self.dms.read_messages().await?;
        let messages = raw_messages
            .into_iter()
            .map(|m| {
                (
                    serde_spb::from_str::<ConsensusMessage>(m.data()),
                    m.signature().signer().clone(),
                )
            })
            .filter_map(|(cm, pk)| cm.ok().map(|cm| (cm, pk)))
            .collect();
        Ok(messages)
    }

    pub async fn read_precommits(&self) -> Result<Vec<(ConsensusMessage, PublicKey)>, Error> {
        Ok(self
            .read_messages()
            .await?
            .into_iter()
            .filter(|(cm, _)| matches!(cm, ConsensusMessage::NonNilPreCommitted(..)))
            .collect())
    }
}

// Private methods
impl<N: GossipNetwork, S: Storage> Consensus<N, S> {
    fn construct_new_state(
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
        let vetomint = VetomintWrapper {
            vetomint: Vetomint::new(height_info),
            is_started: false,
        };
        let state = State {
            vetomint,
            block_header: block_header.clone(),
            updated_messages: BTreeSet::new(),
            verified_block_hashes: vec![],
            vetoed_block_hashes: vec![],
            finalized: false,
        };
        Ok(state)
    }

    fn get_block_index(&self, block_hash: &Hash256) -> Result<usize, Error> {
        self.state
            .verified_block_hashes
            .iter()
            .position(|h| h == block_hash)
            .ok_or_else(|| eyre!("block not verified"))
    }

    fn abort_if_finalized(&self) -> Result<(), Error> {
        if self.state.finalized {
            Err(eyre!("operation on finalized state"))
        } else {
            Ok(())
        }
    }

    async fn commit_state_to_storage(&mut self) -> Result<(), Error> {
        self.state_storage
            .add_or_overwrite_file(STATE_FILE_NAME, serde_spb::to_string(&self.state).unwrap())
            .await
            .map_err(|_| eyre!("failed to commit consensus state to the storage"))
    }

    async fn broadcast_consensus_message(
        &mut self,
        consensus_message: &ConsensusMessage,
    ) -> Result<(), Error> {
        let serialized = serde_spb::to_string(consensus_message).unwrap();
        let signature = TypedSignature::sign(&serialized, self.this_node_key.as_ref().unwrap())
            .expect("invalid(malformed) private key");
        let message = Message::new(serialized, signature).expect("signature just created");
        self.dms.add_message(message).await
    }

    async fn process_multiple_responses(
        &mut self,
        responses: Vec<ConsensusResponse>,
        timestamp: Timestamp,
    ) -> Result<Vec<ProgressResult>, Error> {
        let mut final_result = Vec::new();
        for consensus_response in responses {
            let consensus_result = self
                .process_single_response(consensus_response, timestamp)
                .await?;
            final_result.push(consensus_result);
            if self.state.finalized {
                break;
            }
        }
        Ok(final_result)
    }

    #[allow(unreachable_code)]
    fn consensus_message_to_event(
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
                    favor: !self.state.vetoed_block_hashes.contains(block_hash),
                }
            }
            ConsensusMessage::NonNilPreVoted(round, block_hash, _) => {
                let index = self
                    .get_block_index(block_hash)
                    .expect("this must be already verified by the message filter");
                ConsensusEvent::Prevote {
                    proposal: Some(index),
                    signer,
                    round: *round as usize,
                }
            }
            ConsensusMessage::NonNilPreCommitted(round, block_hash, _) => {
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

    /// Handles the consensus response from the consensus state (vetomint).
    ///
    /// It might broadcast a block or a vote as needed.
    async fn process_single_response(
        &mut self,
        consensus_response: ConsensusResponse,
        timestamp: Timestamp,
    ) -> Result<ProgressResult, Error> {
        match consensus_response {
            ConsensusResponse::BroadcastProposal {
                proposal,
                valid_round,
                round,
            } => {
                let _ = self
                    .this_node_key
                    .as_ref()
                    .ok_or_else(|| eyre!("this node is not a validator"))?;
                let valid_round = valid_round.map(|r| r as u64);
                let block_hash = *self
                    .state
                    .verified_block_hashes
                    .get(proposal)
                    .expect("the block to propose is not in verified_block_hashes");
                let consensus_message = ConsensusMessage::Proposal {
                    round: round as u64,
                    valid_round,
                    block_hash,
                };
                self.broadcast_consensus_message(&consensus_message).await?;
                Ok(ProgressResult::Proposed(
                    round as u64,
                    block_hash,
                    timestamp,
                ))
            }
            ConsensusResponse::BroadcastPrevote { proposal, round } => {
                let private_key = self
                    .this_node_key
                    .as_ref()
                    .ok_or_else(|| eyre!("this node is not a validator"))?;
                let (consensus_message, progress_result) = if let Some(block_index) = proposal {
                    let block_hash = *self
                        .state
                        .verified_block_hashes
                        .get(block_index)
                        .expect("the block to propose is not in verified_block_hashes");
                    let message = ConsensusMessage::NonNilPreVoted(
                        round as u64,
                        block_hash,
                        TypedSignature::sign(
                            &format!("{}-{}", block_hash, "prevote"),
                            private_key,
                        )?,
                    );
                    let result =
                        ProgressResult::NonNilPreVoted(round as u64, block_hash, timestamp);
                    (message, result)
                } else {
                    let message = ConsensusMessage::NilPreVoted(round as u64);
                    let result = ProgressResult::NilPreVoted(round as u64, timestamp);
                    (message, result)
                };
                self.broadcast_consensus_message(&consensus_message).await?;
                Ok(progress_result)
            }
            ConsensusResponse::BroadcastPrecommit { proposal, round } => {
                let private_key = self
                    .this_node_key
                    .as_ref()
                    .ok_or_else(|| eyre!("this node is not a validator"))?;
                let (consensus_message, progress_result) = if let Some(block_index) = proposal {
                    let block_hash = *self
                        .state
                        .verified_block_hashes
                        .get(block_index)
                        .expect("the block to propose is not in verified_block_hashes");
                    let message = ConsensusMessage::NonNilPreCommitted(
                        round as u64,
                        block_hash,
                        TypedSignature::new(
                            Signature::sign(block_hash, private_key)?,
                            private_key.public_key(),
                        ),
                    );
                    let result =
                        ProgressResult::NonNilPreCommitted(round as u64, block_hash, timestamp);
                    (message, result)
                } else {
                    let message = ConsensusMessage::NilPreCommitted(round as u64);
                    let result = ProgressResult::NilPreCommitted(round as u64, timestamp);
                    (message, result)
                };
                self.broadcast_consensus_message(&consensus_message).await?;
                Ok(progress_result)
            }
            ConsensusResponse::FinalizeBlock { proposal, proof } => {
                let block_hash = *self
                    .state
                    .verified_block_hashes
                    .get(proposal)
                    .expect("oob access to verified_block_hashes");
                if proof.iter().any(|&validator_index| {
                    validator_index >= self.state.block_header.validator_set.len()
                }) {
                    return Err(eyre!("oob access to validator_set"));
                }
                let pubkeys_for_proof: Vec<_> = proof
                    .iter()
                    .map(|&index| self.state.block_header.validator_set[index].0.clone())
                    .collect();
                let received_precommits = self.read_precommits().await?;
                let precommits_for_proof = received_precommits
                    .iter()
                    .filter(|(_, pk)| pubkeys_for_proof.contains(pk))
                    .map(|(cm, _)| cm);
                let proof = precommits_for_proof
                    .map(|cm| match cm {
                        ConsensusMessage::NonNilPreCommitted(_, _, precommit) => precommit.clone(),
                        _ => panic!(
                            "consensus::read_precommits should return only `NonNilPreCommitted`"
                        ),
                    })
                    .collect();
                self.state.finalized = true;
                Ok(ProgressResult::Finalized(block_hash, timestamp, proof))
            }
            ConsensusResponse::ViolationReport {
                violator,
                description,
            } => {
                let pubkey = self
                    .state
                    .block_header
                    .validator_set
                    .get(violator)
                    .expect("oob access to validators")
                    .0
                    .clone();
                Ok(ProgressResult::ViolationReported(
                    pubkey,
                    description,
                    timestamp,
                ))
            }
        }
    }
}
