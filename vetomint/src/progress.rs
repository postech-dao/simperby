use super::*;

/// TODO: we have to implement the following missing logics
/// - on-proposal
/// - on-4f-favor-prevote-propose-step
/// - on-4f-favor-prevote-prevote-step
/// - on-4f-nil-prevote
/// - on-5f-precommit
/// - on-4f-favor-precommit
/// - OnTimeoutPrecommit
pub(super) fn progress(
    _state: &mut ConsensusState,
    _event: ConsensusEvent,
) -> Option<Vec<ConsensusResponse>> {
    unimplemented!()
}
#[cfg(ignore)]
pub(super) fn progress(
    state: &mut ConsensusState,
    event: ConsensusEvent,
) -> Option<Vec<ConsensusResponse>> {
    let result = if state.waiting_for_proposal_creation {
        if let ConsensusEvent::BlockProposalCreated {
            proposal, round, ..
        } = event
        {
            if state.round != round {
                return None;
            }
            state.waiting_for_proposal_creation = false;
            return Some(vec![ConsensusResponse::BroadcastProposal {
                proposal,
                round: state.round,
            }]);
        } else {
            // Nothing to do; this state is waiting for a `BlockProposalCreated`.
            return None;
        }
    } else {
        match event {
            ConsensusEvent::Start { time } => match start_round(height_info, state, 0, time) {
                StartRoundResponse::Normal(r) => r,
                StartRoundResponse::Pending { .. } => {
                    state.waiting_for_proposal_creation = true;
                    Vec::new()
                }
            },
            ConsensusEvent::BlockProposalCreated { .. } => return None,
            // Time-trigger events are handled later
            ConsensusEvent::Timer { .. } => Vec::new(),
            ConsensusEvent::Prevote {
                proposal,
                signer,
                round,
                ..
            } => {
                let total_voting_power = height_info.validators.iter().sum::<VotingPower>();
                if round != state.round {
                    return None;
                }
                let voting_power = height_info.validators[signer as usize];
                state.votes.insert(round, {
                    let mut votes = state
                        .votes
                        .get(&round)
                        .unwrap_or(&Default::default())
                        .clone();
                    votes.prevotes_total += voting_power;
                    votes.prevotes_favor.insert(
                        proposal,
                        votes.prevotes_favor.get(&proposal).unwrap_or(&0) + voting_power,
                    );
                    votes
                });
                if state.votes[&round].prevotes_total * 6 > total_voting_power * 5
                    && state.step == ConsensusStep::Prevote
                {
                    on_5f_prevote(height_info, state, round)
                } else {
                    Vec::new()
                }
            }
            _ => unimplemented!(),
        }
    };

    if !result.is_empty() {
        Some(result)
    // Handle timeout
    } else {
        let time = event.time();
        let mut responses = Vec::new();
        if let Some(timeout_propose) = state.timeout_propose {
            if time >= timeout_propose {
                responses.append(&mut on_timeout_propose(height_info, state, state.round));
            }
        }
        Some(responses)
    }
}

enum StartRoundResponse {
    Normal(Vec<ConsensusResponse>),
    /// Emits a `CreateProposal`.
    Pending,
}

fn start_round(
    height_info: &HeightInfo,
    state: &mut ConsensusState,
    round: usize,
    time: Timestamp,
) -> StartRoundResponse {
    state.round = round;
    state.step = ConsensusStep::Propose;
    let proposer = decide_proposer(round, height_info);
    if proposer == height_info.this_node_index {
        if let Some(valid_value) = state.valid_value {
            StartRoundResponse::Normal(vec![ConsensusResponse::BroadcastProposal {
                proposal: valid_value,
                round,
            }])
        } else {
            StartRoundResponse::Pending
        }
    } else {
        state.timeout_propose = Some(time + height_info.consensus_params.timeout_ms as i64);
        StartRoundResponse::Normal(Vec::new())
    }
}

fn on_5f_prevote(
    height_info: &HeightInfo,
    state: &mut ConsensusState,
    round: Round,
) -> Vec<ConsensusResponse> {
    let total_voting_power = height_info.validators.iter().sum::<VotingPower>();
    state.step = ConsensusStep::Precommit;
    for (proposal, prevotes_favor) in &state.votes[&round].prevotes_favor {
        if prevotes_favor * 3 > total_voting_power * 2 {
            return vec![ConsensusResponse::BroadcastPrecommit {
                proposal: *proposal,
                round: state.round,
            }];
        }
    }
    vec![ConsensusResponse::BroadcastNilPrecommit { round: state.round }]
}

fn on_timeout_propose(
    _height_info: &HeightInfo,
    state: &mut ConsensusState,
    round: usize,
) -> Vec<ConsensusResponse> {
    if state.round == round && state.step == ConsensusStep::Propose {
        state.step = ConsensusStep::Prevote;
        state.timeout_propose = None;
        vec![ConsensusResponse::BroadcastNilPrevote { round }]
    } else {
        Vec::new()
    }
}
