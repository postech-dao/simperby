mod progress;
mod state;

use serde::{Deserialize, Serialize};

/// An index of the validator, which is for a single height. (Mapping from the actual public key to the index may differ for different heights.)
pub type ValidatorIndex = usize;
/// An identifier of the block, which is uniquely mapped to a block. Like `ValidatorIndex`, it is for a single height. (Mapping from the actual block to the index may differ for different heights.)
pub type BlockIdentifier = usize;
/// A round.
pub type Round = usize;
/// A voting power.
pub type VotingPower = u64;
/// A UNIX timestamp measured in milliseconds.
pub type Timestamp = i64;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ConsensusParams {
    pub timeout_ms: u64,
    pub repeat_round_for_first_leader: usize,
}

/// An event that (potentially) triggers a state transition of `StateMachine`.
///
/// Note that there is no cryptography-related info here, because it's
/// the lower layer's responsibility to verifiy and refine the raw messages (containing such cryptography-related info) into this abstracted data.
/// Also all the identifiers (for blocks and validators) become integer indices here, and
/// the lower layer will keep the mapping from the actual data to the indices.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, PartialOrd, Ord)]
pub enum ConsensusEvent {
    /// Signals to start the process
    Start,
    /// Informs that the node has received a block proposal.
    BlockProposalReceived {
        proposal: BlockIdentifier,
        /// Whether this proposal is valid
        valid: bool,
        valid_round: Option<Round>,
        proposer: ValidatorIndex,
        round: Round,
        /// Whether this node is in favor of the proposal.
        favor: bool,
    },
    /// Informs that the node wants to skip the specific round regardless of proposals (which may even not exist).
    SkipRound { round: Round },
    /// Updates the block candidate in which this nodes wants to propose
    BlockCandidateUpdated { proposal: BlockIdentifier },
    /// Informs that the node has received a block prevote.
    Prevote {
        proposal: Option<BlockIdentifier>,
        signer: ValidatorIndex,
        round: Round,
    },
    /// Informs that the node has received a block precommit.
    Precommit {
        proposal: Option<BlockIdentifier>,
        signer: ValidatorIndex,
        round: Round,
    },
    /// Informs that time has passed.
    Timer,
}

/// The report and trace of a misbehavior committed by a malicious node.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum Misbehavior {
    DoubleProposal {
        /// The malicious node.
        byzantine_node: ValidatorIndex,
        /// The round in which the misbehavior is committed.
        round: Round,
        /// The two conflicting proposals.
        proposals: (BlockIdentifier, BlockIdentifier),
    },
    DoublePrevote {
        /// The malicious node.
        byzantine_node: ValidatorIndex,
        /// The round in which the misbehavior is committed.
        round: Round,
        /// The two conflicting proposals that the node has prevoted.
        proposals: (Option<BlockIdentifier>, Option<BlockIdentifier>),
    },
    DoublePrecommit {
        /// The malicious node.
        byzantine_node: ValidatorIndex,
        /// The round in which the misbehavior is committed.
        round: Round,
        /// The two conflicting proposals that the node has precommitted.
        proposals: (Option<BlockIdentifier>, Option<BlockIdentifier>),
    },
    InvalidProposal {
        /// The malicious node.
        byzantine_node: ValidatorIndex,
        /// The round in which the misbehavior is committed.
        round: Round,
        /// The proposal that the node has proposed.
        proposal: BlockIdentifier,
    },
    InvalidPrevote {
        /// The malicious node.
        byzantine_node: ValidatorIndex,
        /// The round in which the misbehavior is committed.
        round: Round,
        /// The proposal that the node has prevoted.
        proposal: BlockIdentifier,
    },
    InvalidPrecommit {
        /// The malicious node.
        byzantine_node: ValidatorIndex,
        /// The round in which the misbehavior is committed.
        round: Round,
        /// The proposal that the node has precommitted.
        proposal: BlockIdentifier,
    },
}

/// A response that the consensus might emit for a given event, which must be properly handled by the lower layer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum ConsensusResponse {
    BroadcastProposal {
        proposal: BlockIdentifier,
        valid_round: Option<Round>,
        round: Round,
    },
    BroadcastPrevote {
        proposal: Option<BlockIdentifier>,
        round: Round,
    },
    BroadcastPrecommit {
        proposal: Option<BlockIdentifier>,
        round: Round,
    },
    FinalizeBlock {
        proposal: BlockIdentifier,
        round: Round,
        proof: Vec<ValidatorIndex>,
    },
    ViolationReport {
        violator: ValidatorIndex,
        misbehavior: Misbehavior,
    },
}

/// An immutable set of information that is used to perform the consensus for a single height.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct HeightInfo {
    /// The list of voting powers sorted by the leader order.
    ///
    /// Important note: `ValidatorIndex` is used to index this list.
    pub validators: Vec<VotingPower>,

    /// The index of this node
    /// validator index can be none for supporting non-validator client
    pub this_node_index: Option<ValidatorIndex>,

    /// The timestamp of the beginning of the round 0.
    pub timestamp: Timestamp,

    /// The consensus parameters
    pub consensus_params: ConsensusParams,

    /// The initial block candidate that this node wants to propose.
    pub initial_block_candidate: BlockIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Vetomint {
    state: state::ConsensusState,
}

impl Vetomint {
    pub fn new(height_info: HeightInfo) -> Self {
        Self {
            state: state::ConsensusState::new(height_info),
        }
    }

    pub fn get_height_info(&self) -> &HeightInfo {
        &self.state.height_info
    }

    pub fn progress(
        &mut self,
        event: ConsensusEvent,
        timestamp: Timestamp,
    ) -> Vec<ConsensusResponse> {
        let mut responses = progress::progress(&mut self.state, event, timestamp);
        let mut final_responses = responses.clone();
        // feedback to myself
        loop {
            let mut responses_ = Vec::new();
            let state = self.state.clone();
            for response in responses.clone() {
                match response {
                    ConsensusResponse::BroadcastProposal {
                        proposal,
                        valid_round,
                        round,
                    } => responses_.extend(progress::progress(
                        &mut self.state,
                        ConsensusEvent::BlockProposalReceived {
                            proposal,
                            valid: true,
                            valid_round,
                            proposer: state.height_info.this_node_index.unwrap(),
                            round,
                            favor: true,
                        },
                        timestamp,
                    )),
                    ConsensusResponse::BroadcastPrevote { proposal, round } => {
                        responses_.extend(progress::progress(
                            &mut self.state,
                            ConsensusEvent::Prevote {
                                proposal,
                                signer: state.height_info.this_node_index.unwrap(),
                                round,
                            },
                            timestamp,
                        ))
                    }
                    ConsensusResponse::BroadcastPrecommit { proposal, round } => {
                        responses_.extend(progress::progress(
                            &mut self.state,
                            ConsensusEvent::Precommit {
                                proposal,
                                signer: state.height_info.this_node_index.unwrap(),
                                round,
                            },
                            timestamp,
                        ))
                    }
                    _ => (),
                }
            }
            if responses_.is_empty() {
                break;
            }
            final_responses.extend(responses_.clone());
            responses = responses_;
        }
        final_responses
    }
}

pub fn decide_proposer(round: usize, height_info: &HeightInfo) -> ValidatorIndex {
    if round < height_info.consensus_params.repeat_round_for_first_leader {
        0
    } else {
        (round - height_info.consensus_params.repeat_round_for_first_leader + 1)
            % height_info.validators.len()
    }
}

pub fn decide_timeout(params: &ConsensusParams, _round: usize) -> Timestamp {
    params.timeout_ms as i64
}
