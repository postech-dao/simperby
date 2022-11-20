use super::*;

pub(super) fn progress(
    state: &mut ConsensusState,
    event: ConsensusEvent,
) -> Option<Vec<ConsensusResponse>> {
    let result = if state.waiting_for_proposal_creation {
        if let ConsensusEvent::BlockCandidateUpdated { proposal, .. } = event {
            state.waiting_for_proposal_creation = false;
            state.block_candidate = proposal;
            vec![ConsensusResponse::BroadcastProposal {
                proposal,
                round: state.round,
            }]
        } else {
            // Nothing to do; this state is waiting for a `BlockProposalCreated`.
            return None;
        }
    } else {
        match event {
            ConsensusEvent::Start { time } => start_round(state, 0, time),
            ConsensusEvent::BlockCandidateUpdated { proposal, .. } => {
                state.block_candidate = proposal;
                Vec::new()
            }
            ConsensusEvent::BlockProposalReceived {
                proposal,
                valid_round,
                proposer,
                round,
                favor,
                ..
            } => {
                let mut expected_response = Vec::new();
                let current_proposer = decide_proposer(round, &state.height_info);
                if proposer == current_proposer && state.step == ConsensusStep::Propose {
                    match valid_round {
                        Some(vr) => {
                            if vr < round {
                                expected_response.append(
                                    &mut on_4f_non_nil_prevote_in_propose_step(
                                        proposal, favor, state, round, vr,
                                    ),
                                );
                            }
                        }
                        None => expected_response
                            .append(&mut on_proposal(proposal, favor, state, round)),
                    }
                };
                expected_response
            }
            // Time-trigger events are handled later
            ConsensusEvent::Timer { .. } => Vec::new(),
            ConsensusEvent::NonNilPrevote {
                proposal,
                signer,
                round,
                ..
            } => {
                if round != state.round {
                    return None;
                }
                if state.prevote_history.get(&round).is_none() {
                    state.prevote_history.insert(round, BTreeMap::new());
                }
                let vote_history = state.prevote_history[&round].get(&signer);
                match vote_history {
                    Some(past_vote) => {
                        if *past_vote != Some(proposal) {
                            vec![ConsensusResponse::ViolationReport {
                                violator: signer,
                                description: String::from("Double NonNilPrevote"),
                            }]
                        } else {
                            Vec::new()
                        }
                    }
                    None => {
                        state.prevote_history.insert(round, {
                            let mut new_prevote_history = state
                                .prevote_history
                                .get(&round)
                                .unwrap_or(&Default::default())
                                .clone();
                            new_prevote_history.insert(signer, Some(proposal));
                            new_prevote_history
                        });
                        let total_voting_power =
                            state.height_info.validators.iter().sum::<VotingPower>();
                        let voting_power = state.height_info.validators[signer as usize];
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
                        let this_proposal_prevote = state.votes[&round]
                            .prevotes_favor
                            .get(&proposal)
                            .unwrap_or(&0);
                        if state.votes[&round].prevotes_total * 6 > total_voting_power * 5
                            && state.step == ConsensusStep::Prevote
                        {
                            on_5f_prevote(state, round)
                        } else if this_proposal_prevote * 3 > total_voting_power * 2
                            && (state.step == ConsensusStep::Prevote
                                || state.step == ConsensusStep::Precommit)
                        {
                            on_4f_non_nil_prevote_in_prevote_step(state, round)
                        } else {
                            Vec::new()
                        }
                    }
                }
            }

            ConsensusEvent::NilPrevote { signer, round, .. } => {
                if round != state.round {
                    return None;
                }
                if state.prevote_history.get(&round).is_none() {
                    state.prevote_history.insert(round, BTreeMap::new());
                }
                let vote_history = state.prevote_history[&round].get(&signer);
                match vote_history {
                    Some(past_vote) => {
                        if past_vote.is_none() {
                            vec![ConsensusResponse::ViolationReport {
                                violator: signer,
                                description: String::from("Double NonNilPrevote"),
                            }]
                        } else {
                            Vec::new()
                        }
                    }
                    None => {
                        state.prevote_history.insert(round, {
                            let mut new_prevote_history = state
                                .prevote_history
                                .get(&round)
                                .unwrap_or(&Default::default())
                                .clone();
                            new_prevote_history.insert(signer, None);
                            new_prevote_history
                        });
                        let total_voting_power =
                            &state.height_info.validators.iter().sum::<VotingPower>();
                        let voting_power = &state.height_info.validators[signer as usize];
                        state.votes.insert(round, {
                            let mut votes = state
                                .votes
                                .get(&round)
                                .unwrap_or(&Default::default())
                                .clone();
                            votes.prevotes_total += voting_power;
                            votes
                        });
                        let current_prevotes = &state.votes[&round].prevotes_total;
                        let current_non_nil_prevotes = state.votes[&round]
                            .prevotes_favor
                            .values()
                            .into_iter()
                            .sum::<VotingPower>();
                        let current_nil_prevotes = current_prevotes - current_non_nil_prevotes;

                        if state.votes[&round].prevotes_total * 6 > total_voting_power * 5
                            && state.step == ConsensusStep::Prevote
                        {
                            on_5f_prevote(state, round)
                        } else if current_nil_prevotes * 3 > total_voting_power * 2
                            && state.step == ConsensusStep::Prevote
                        {
                            on_4f_nil_prevote(state, round)
                        } else {
                            Vec::new()
                        }
                    }
                }
            }

            ConsensusEvent::NonNilPrecommit {
                proposal,
                signer,
                round,
                time,
            } => {
                if round != state.round {
                    return None;
                }
                if state.precommit_history.get(&round).is_none() {
                    state.precommit_history.insert(round, BTreeMap::new());
                }
                let vote_history = state.precommit_history[&round].get(&signer);
                match vote_history {
                    Some(past_commit) => {
                        if *past_commit != Some(proposal) {
                            vec![ConsensusResponse::ViolationReport {
                                violator: signer,
                                description: String::from("Double NonNilPrecommit"),
                            }]
                        } else {
                            Vec::new()
                        }
                    }
                    None => {
                        state.precommit_history.insert(round, {
                            let mut new_precommit_history = state
                                .precommit_history
                                .get(&round)
                                .unwrap_or(&Default::default())
                                .clone();
                            new_precommit_history.insert(signer, Some(proposal));
                            new_precommit_history
                        });
                        let total_voting_power =
                            state.height_info.validators.iter().sum::<VotingPower>();
                        let voting_power = state.height_info.validators[signer as usize];
                        state.votes.insert(round, {
                            let mut votes = state
                                .votes
                                .get(&round)
                                .unwrap_or(&Default::default())
                                .clone();
                            votes.precommits_total += voting_power;
                            votes.precommits_favor.insert(
                                proposal,
                                votes.precommits_favor.get(&proposal).unwrap_or(&0) + voting_power,
                            );
                            votes
                        });
                        let this_proposal_precommit = state.votes[&round]
                            .precommits_favor
                            .get(&proposal)
                            .unwrap_or(&0);

                        if this_proposal_precommit * 3 > total_voting_power * 2
                            && ConsensusStep::Precommit == state.step
                        {
                            on_4f_non_nil_precommit(proposal)
                        } else if state.votes[&round].precommits_total * 6 > total_voting_power * 5
                            && ConsensusStep::Precommit == state.step
                            && state.timeout_precommit.is_none()
                        {
                            on_5f_precommit(state, time)
                        } else {
                            Vec::new()
                        }
                    }
                }
            }

            ConsensusEvent::NilPrecommit {
                signer,
                round,
                time,
            } => {
                if round != state.round {
                    return None;
                }
                if state.precommit_history.get(&round).is_none() {
                    state.precommit_history.insert(round, BTreeMap::new());
                }
                let vote_history = state.precommit_history[&round].get(&signer);
                match vote_history {
                    Some(past_commit) => {
                        if past_commit.is_some() {
                            vec![ConsensusResponse::ViolationReport {
                                violator: signer,
                                description: String::from("Double NonNilPrecommit"),
                            }]
                        } else {
                            Vec::new()
                        }
                    }
                    None => {
                        state.precommit_history.insert(round, {
                            let mut new_precommit_history = state
                                .precommit_history
                                .get(&round)
                                .unwrap_or(&Default::default())
                                .clone();
                            new_precommit_history.insert(signer, None);
                            new_precommit_history
                        });
                        let total_voting_power =
                            state.height_info.validators.iter().sum::<VotingPower>();
                        let voting_power = state.height_info.validators[signer as usize];
                        state.votes.insert(round, {
                            let mut votes = state
                                .votes
                                .get(&round)
                                .unwrap_or(&Default::default())
                                .clone();
                            votes.precommits_total += voting_power;
                            votes
                        });
                        if state.votes[&round].precommits_total * 6 > total_voting_power * 5
                            && ConsensusStep::Precommit == state.step
                            && state.timeout_precommit.is_none()
                        {
                            on_5f_precommit(state, time)
                        } else {
                            Vec::new()
                        }
                    }
                }
            }
        }
    };

    if !result.is_empty() {
        Some(result)
    // Handle timeout
    } else {
        let time = event.time();
        let mut responses = Vec::new();
        match state.step {
            ConsensusStep::Propose => {
                if let Some(timeout_propose) = state.timeout_propose {
                    if time >= timeout_propose {
                        responses.append(&mut on_timeout_propose(state));
                    }
                }
            }
            ConsensusStep::Precommit => {
                if let Some(timeout_precommit) = state.timeout_precommit {
                    if time >= timeout_precommit {
                        responses.append(&mut on_timeout_precommit(state, time));
                    }
                }
            }
            _ => (),
        }
        Some(responses)
    }
}

fn start_round(
    state: &mut ConsensusState,
    round: usize,
    time: Timestamp,
) -> Vec<ConsensusResponse> {
    state.round = round;
    state.step = ConsensusStep::Propose;
    state.timeout_precommit = None;
    let proposer = decide_proposer(round, &state.height_info);
    if Some(proposer) == state.height_info.this_node_index {
        let proposal = if state.valid_value.is_some() {
            state.valid_value.unwrap()
        } else {
            state.waiting_for_proposal_creation = true;
            state.block_candidate
        };
        vec![ConsensusResponse::BroadcastProposal { proposal, round }]
    } else {
        state.timeout_propose = Some(time + state.height_info.consensus_params.timeout_ms as i64);
        Vec::new()
    }
}

fn on_proposal(
    proposal: BlockIdentifier,
    favor: bool,
    state: &mut ConsensusState,
    round: Round,
) -> Vec<ConsensusResponse> {
    let this_node_voting_power = if state.height_info.this_node_index.is_none() {
        0
    } else {
        state.height_info.validators[state.height_info.this_node_index.unwrap()]
    };
    let mut response: Vec<ConsensusResponse> = Vec::new();

    state.step = ConsensusStep::Prevote;
    state.votes.insert(round, {
        let mut votes = state
            .votes
            .get(&round)
            .unwrap_or(&Default::default())
            .clone();
        votes.prevotes_total += this_node_voting_power;
        votes.precommits_total += this_node_voting_power;
        if favor {
            votes.prevotes_favor.insert(
                proposal,
                votes.prevotes_favor.get(&proposal).unwrap_or(&0) + this_node_voting_power,
            );
            votes.precommits_favor.insert(
                proposal,
                votes.precommits_favor.get(&proposal).unwrap_or(&0) + this_node_voting_power,
            );
        }
        votes
    });

    if Some(proposal) == state.locked_value || (favor && state.locked_round.is_none()) {
        response.append(&mut vec![ConsensusResponse::BroadcastNonNilPrevote {
            proposal,
            round,
        }]);
    } else {
        response.append(&mut vec![ConsensusResponse::BroadcastNilPrevote { round }]);
    }
    response
}

fn on_4f_non_nil_prevote_in_propose_step(
    proposal: BlockIdentifier,
    favor: bool,
    state: &mut ConsensusState,
    round: Round,
    valid_round: Round,
) -> Vec<ConsensusResponse> {
    let total_voting_power = state.height_info.validators.iter().sum::<VotingPower>();
    let locked_prevotes = state.votes[&valid_round]
        .prevotes_favor
        .get(&proposal)
        .unwrap_or(&0);

    let mut response = Vec::new();

    if locked_prevotes * 3 > total_voting_power * 2 {
        state.step = ConsensusStep::Prevote;
        if Some(proposal) == state.locked_value
            || (favor && state.locked_round.unwrap_or(0) < valid_round)
        {
            response.append(&mut vec![ConsensusResponse::BroadcastNonNilPrevote {
                proposal,
                round,
            }]);
        } else {
            response.append(&mut vec![ConsensusResponse::BroadcastNilPrevote { round }]);
        }
    }
    response
}

fn on_4f_non_nil_prevote_in_prevote_step(
    state: &mut ConsensusState,
    round: Round,
) -> Vec<ConsensusResponse> {
    let mut responses = Vec::new();
    let total_voting_power = state.height_info.validators.iter().sum::<VotingPower>();
    for (proposal, prevotes_favor) in &state.votes[&round].prevotes_favor {
        if prevotes_favor * 3 > total_voting_power * 2 {
            state.valid_round = Some(round);
            state.valid_value = Some(*proposal);
            if state.step == ConsensusStep::Prevote {
                state.step = ConsensusStep::Precommit;
                state.locked_round = Some(round);
                state.locked_value = Some(*proposal);
                responses.append(&mut vec![ConsensusResponse::BroadcastNonNilPrecommit {
                    proposal: *proposal,
                    round: state.round,
                }]);
            }
        }
    }
    responses
}

fn on_4f_nil_prevote(state: &mut ConsensusState, round: Round) -> Vec<ConsensusResponse> {
    state.step = ConsensusStep::Precommit;
    vec![ConsensusResponse::BroadcastNilPrecommit { round }]
}

fn on_5f_prevote(state: &mut ConsensusState, round: Round) -> Vec<ConsensusResponse> {
    let total_voting_power = state.height_info.validators.iter().sum::<VotingPower>();
    state.step = ConsensusStep::Precommit;
    for (proposal, prevotes_favor) in &state.votes[&round].prevotes_favor {
        if prevotes_favor * 3 > total_voting_power * 2 {
            state.locked_round = Some(round);
            state.locked_value = Some(*proposal);
            return vec![ConsensusResponse::BroadcastNonNilPrecommit {
                proposal: *proposal,
                round: state.round,
            }];
        }
    }
    vec![ConsensusResponse::BroadcastNilPrecommit { round: state.round }]
}

fn on_5f_precommit(state: &mut ConsensusState, time: Timestamp) -> Vec<ConsensusResponse> {
    state.timeout_precommit = Some(time + state.height_info.consensus_params.timeout_ms as i64);
    Vec::new()
}

fn on_4f_non_nil_precommit(proposal: BlockIdentifier) -> Vec<ConsensusResponse> {
    vec![ConsensusResponse::FinalizeBlock { proposal }]
}

fn on_timeout_propose(state: &mut ConsensusState) -> Vec<ConsensusResponse> {
    if state.step == ConsensusStep::Propose {
        state.step = ConsensusStep::Prevote;
        state.timeout_propose = None;
        vec![ConsensusResponse::BroadcastNilPrevote { round: state.round }]
    } else {
        Vec::new()
    }
}

fn on_timeout_precommit(state: &mut ConsensusState, time: Timestamp) -> Vec<ConsensusResponse> {
    if state.step == ConsensusStep::Precommit {
        state.step = ConsensusStep::Propose;
        state.timeout_precommit = None;
        start_round(state, state.round + 1, time)
    } else {
        Vec::new()
    }
}
