use super::*;
use state::*;

pub(crate) fn progress(
    state: &mut ConsensusState,
    event: ConsensusEvent,
    timestamp: Timestamp,
) -> Vec<ConsensusResponse> {
    if let Some((proposal, proof)) = state.finalized.clone() {
        return vec![ConsensusResponse::FinalizeBlock { proposal, proof }];
    }
    match event {
        ConsensusEvent::Start => start_round(state, 0, timestamp),
        ConsensusEvent::BlockProposalReceived {
            proposal,
            valid,
            valid_round,
            proposer,
            round,
            favor,
        } => {
            state.proposals.insert(
                proposal,
                Proposal {
                    proposal,
                    valid,
                    valid_round,
                    proposer,
                    round,
                    favor,
                },
            );
            let mut response = Vec::new();
            if valid_round.is_some() {
                response.extend(on_4f_non_nil_prevote_in_propose_step(
                    state, round, proposal,
                ));
            } else {
                response.extend(on_proposal(state, round, proposal));
            }
            response.extend(on_4f_non_nil_prevote_in_prevote_step(
                state, round, proposal,
            ));
            response.extend(on_4f_non_nil_precommit(state, round, proposal));
            response
        }
        ConsensusEvent::SkipRound { round } => progress(
            state,
            ConsensusEvent::BlockProposalReceived {
                proposal: 0,
                valid: false,
                valid_round: None,
                proposer: 0,
                round,
                favor: false,
            },
            timestamp,
        ),
        ConsensusEvent::BlockCandidateUpdated { proposal } => {
            state.block_candidate = proposal;
            Vec::new()
        }
        ConsensusEvent::Prevote {
            proposal,
            signer,
            round,
        } => {
            state.prevotes.insert(Vote {
                proposal,
                signer,
                round,
            });
            let mut response = Vec::new();
            if let Some(proposal) = proposal {
                response.extend(on_4f_non_nil_prevote_in_propose_step(
                    state, round, proposal,
                ));
                response.extend(on_4f_non_nil_prevote_in_prevote_step(
                    state, round, proposal,
                ));
            } else {
                response.extend(on_4f_nil_prevote(state, round));
            }
            response.extend(on_5f_prevote(state, round, proposal));
            response
        }
        ConsensusEvent::Precommit {
            proposal,
            signer,
            round,
        } => {
            state.precommits.insert(Vote {
                proposal,
                signer,
                round,
            });
            let mut response = Vec::new();
            response.extend(on_5f_precommit(state, round));
            response.extend(on_4f_nil_precommit(state, round, timestamp));
            if let Some(proposal) = proposal {
                response.extend(on_4f_non_nil_precommit(state, round, proposal));
            }
            response
        }
        ConsensusEvent::Timer => {
            let mut response = Vec::new();
            for (round, timeout) in state.propose_timeout_schedules.clone() {
                if timestamp >= timeout
                    && round == state.round
                    && state.step == ConsensusStep::Propose
                {
                    response.push(ConsensusResponse::BroadcastPrevote {
                        proposal: None,
                        round,
                    });
                    state.step = ConsensusStep::Prevote;
                }
            }
            for (round, timeout) in state.precommit_timeout_schedules.clone() {
                if timestamp >= timeout && round == state.round {
                    response.extend(start_round(state, round + 1, timestamp));
                    break;
                }
            }
            response
        }
    }
}

fn start_round(
    state: &mut ConsensusState,
    round: usize,
    timestamp: Timestamp,
) -> Vec<ConsensusResponse> {
    state.round = round;
    state.step = ConsensusStep::Propose;
    let proposer = decide_proposer(round, &state.height_info);
    if Some(proposer) == state.height_info.this_node_index {
        let proposal = if let Some(x) = state.valid_value {
            x
        } else {
            state.block_candidate
        };
        vec![ConsensusResponse::BroadcastProposal {
            proposal,
            valid_round: state.valid_round,
            round,
        }]
    } else {
        state.propose_timeout_schedules.insert((
            round,
            timestamp + decide_timeout(&state.height_info.consensus_params, round),
        ));
        Vec::new()
    }
}

fn on_proposal(
    state: &mut ConsensusState,
    target_round: Round,
    target_proposal: BlockIdentifier,
) -> Vec<ConsensusResponse> {
    if target_round != state.round {
        return Vec::new();
    }

    // take `None` as `-1` for simple comparison
    let locked_value: i64 = state.locked_value.map(|x| x as i64).unwrap_or(-1);
    let locked_round: i64 = state.locked_round.map(|x| x as i64).unwrap_or(-1);

    let valid_proposer = decide_proposer(target_round, &state.height_info);
    let proposal = if let Some(proposal) = state.proposals.get(&target_proposal) {
        proposal.clone()
    } else {
        return Vec::new();
    };
    if proposal.valid_round.is_some() {
        return Vec::new();
    }

    if proposal.proposer == valid_proposer && state.step == ConsensusStep::Propose {
        state.step = ConsensusStep::Prevote;
        if proposal.valid
            && (locked_value == target_proposal as i64 || (proposal.favor && locked_round == -1))
        {
            vec![ConsensusResponse::BroadcastPrevote {
                proposal: Some(target_proposal),
                round: target_round,
            }]
        } else {
            vec![ConsensusResponse::BroadcastPrevote {
                proposal: None,
                round: target_round,
            }]
        }
    } else {
        Vec::new()
    }
}

fn on_4f_non_nil_prevote_in_propose_step(
    state: &mut ConsensusState,
    target_round: Round,
    target_proposal: BlockIdentifier,
) -> Vec<ConsensusResponse> {
    if target_round != state.round {
        return Vec::new();
    }
    // take `None` as `-1` for simple comparison
    let locked_value: i64 = state.locked_value.map(|x| x as i64).unwrap_or(-1);
    let locked_round: i64 = state.locked_round.map(|x| x as i64).unwrap_or(-1);
    let valid_proposer = decide_proposer(target_round, &state.height_info);
    let proposal = if let Some(proposal) = state.proposals.get(&target_proposal) {
        proposal.clone()
    } else {
        return Vec::new();
    };

    let vr = if let Some(vr) = proposal.valid_round {
        vr
    } else {
        return Vec::new();
    };

    if proposal.proposer == valid_proposer
        && state.get_total_prevotes_on_proposal(vr, target_proposal) * 3
            > state.get_total_voting_power() * 2
        && state.step == ConsensusStep::Propose
        && vr < target_round
    {
        state.step = ConsensusStep::Prevote;
        if proposal.valid
            && ((proposal.favor && locked_round < vr as i64)
                || locked_value == proposal.proposal as i64)
        {
            vec![ConsensusResponse::BroadcastPrevote {
                proposal: Some(target_proposal),
                round: target_round,
            }]
        } else {
            vec![ConsensusResponse::BroadcastPrevote {
                proposal: None,
                round: target_round,
            }]
        }
    } else {
        Vec::new()
    }
}

fn on_4f_non_nil_prevote_in_prevote_step(
    state: &mut ConsensusState,
    target_round: Round,
    target_proposal: BlockIdentifier,
) -> Vec<ConsensusResponse> {
    if target_round != state.round {
        return Vec::new();
    }
    let valid_proposer = decide_proposer(target_round, &state.height_info);
    let proposal = if let Some(proposal) = state.proposals.get(&target_proposal) {
        proposal.clone()
    } else {
        return Vec::new();
    };
    if proposal.proposer == valid_proposer
        && state.get_total_prevotes_on_proposal(target_round, target_proposal) * 3
            > state.get_total_voting_power() * 2
        && proposal.valid
        && (state.step == ConsensusStep::Prevote || state.step == ConsensusStep::Precommit)
    {
        state.valid_value = Some(target_proposal);
        state.valid_round = Some(target_round);
        if let ConsensusStep::Prevote = state.step {
            state.locked_value = Some(target_proposal);
            state.locked_round = Some(target_round);
            state.step = ConsensusStep::Precommit;
            vec![ConsensusResponse::BroadcastPrecommit {
                proposal: Some(target_proposal),
                round: target_round,
            }]
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    }
}

fn on_4f_nil_prevote(state: &mut ConsensusState, target_round: Round) -> Vec<ConsensusResponse> {
    if target_round != state.round {
        return Vec::new();
    }
    if state.step == ConsensusStep::Prevote
        && state.get_total_prevotes_on_nil(target_round) * 3 > state.get_total_voting_power() * 2
    {
        state.step = ConsensusStep::Precommit;
        vec![ConsensusResponse::BroadcastPrecommit {
            proposal: None,
            round: state.round,
        }]
    } else {
        Vec::new()
    }
}

fn on_5f_prevote(
    state: &mut ConsensusState,
    target_round: Round,
    target_proposal: Option<BlockIdentifier>,
) -> Vec<ConsensusResponse> {
    if target_round != state.round {
        return Vec::new();
    }
    if state.step == ConsensusStep::Prevote
        && state.get_total_prevotes(target_round) * 6 > state.get_total_voting_power() * 5
    {
        state.step = ConsensusStep::Precommit;
        if let Some(proposal) = target_proposal {
            if state.get_total_prevotes_on_proposal(target_round, proposal) * 3
                > state.get_total_voting_power() * 2
            {
                vec![ConsensusResponse::BroadcastPrecommit {
                    proposal: target_proposal,
                    round: state.round,
                }]
            } else {
                vec![ConsensusResponse::BroadcastPrecommit {
                    proposal: None,
                    round: target_round,
                }]
            }
        } else {
            vec![ConsensusResponse::BroadcastPrecommit {
                proposal: None,
                round: target_round,
            }]
        }
    } else {
        Vec::new()
    }
}

fn on_5f_precommit(state: &mut ConsensusState, target_round: Round) -> Vec<ConsensusResponse> {
    if target_round != state.round {
        return Vec::new();
    }
    if !state.for_the_first_time_2.contains(&target_round)
        && state.get_total_precommits(target_round) * 6 > state.get_total_voting_power() * 5
    {
        state.for_the_first_time_2.insert(target_round);
        state
            .precommit_timeout_schedules
            .insert((target_round, 1000));
    }
    Vec::new()
}

fn on_4f_nil_precommit(
    state: &mut ConsensusState,
    target_round: Round,
    timestamp: Timestamp,
) -> Vec<ConsensusResponse> {
    if target_round != state.round {
        return Vec::new();
    }
    if state.get_total_precommits_on_nil(target_round) * 2 > state.get_total_voting_power() * 3 {
        start_round(state, target_round + 1, timestamp)
    } else {
        Vec::new()
    }
}

fn on_4f_non_nil_precommit(
    state: &mut ConsensusState,
    target_proposal: BlockIdentifier,
    target_round: Round,
) -> Vec<ConsensusResponse> {
    let valid_proposer = decide_proposer(target_round, &state.height_info);
    let proposal = if let Some(proposal) = state.proposals.get(&target_proposal) {
        proposal.clone()
    } else {
        return Vec::new();
    };
    if proposal.proposer == valid_proposer
        && proposal.valid
        && state.get_total_precommits_on_proposal(target_round, target_proposal) * 3
            > state.get_total_voting_power() * 2
    {
        let proof: Vec<_> = state
            .precommits
            .iter()
            .filter_map(|vote| {
                if vote.round == target_round && vote.proposal == Some(target_proposal) {
                    Some(vote.signer)
                } else {
                    None
                }
            })
            .collect();
        state.finalized = Some((target_proposal, proof.clone()));

        vec![ConsensusResponse::FinalizeBlock {
            proposal: target_proposal,
            proof,
        }]
    } else {
        Vec::new()
    }
}
