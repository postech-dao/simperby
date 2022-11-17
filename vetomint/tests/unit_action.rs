use vetomint::*;

//TODO
//fix functions without initialize and apply unit action to integration_test

pub fn initialize(
    validators: Vec<u64>,
    this_node_index: Option<ValidatorIndex>,
    timestamp: Timestamp,
    consensus_params: ConsensusParams,
    initial_block_candidate: BlockIdentifier,
) -> (HeightInfo, ConsensusState) {
    let height_info = HeightInfo {
        validators,
        this_node_index,
        timestamp,
        consensus_params,
        initial_block_candidate,
    };
    let state = ConsensusState::new(height_info.clone());
    (height_info, state)
}

#[allow(dead_code)]
pub fn prevotes(
    height_info: &HeightInfo,
    state: &mut ConsensusState,
    favor_of_this_node: (bool, u64),
    //(validator index, isFavor, voting power, timestamp)
    votes: Vec<(ValidatorIndex, bool, u64, Timestamp)>,
    round: usize,
    proposal: usize,
) -> Vec<Option<Vec<ConsensusResponse>>> {
    let mut votes_time_sorted = votes;
    let total_voting_power: u64 = height_info.validators.iter().sum();
    votes_time_sorted.sort_by_key(|k| k.3);

    let mut current_non_nil_prevoted = if favor_of_this_node.0 {
        favor_of_this_node.1
    } else {
        0
    };
    let mut current_nil_prevoted = if !favor_of_this_node.0 {
        favor_of_this_node.1
    } else {
        0
    };
    let mut current_prevoted;
    let mut return_responses = Vec::<Option<Vec<ConsensusResponse>>>::new();

    for (signer, favor, power, time) in votes_time_sorted {
        current_prevoted = current_non_nil_prevoted + current_nilvoted;
        if 3 * current_non_nil_prevoted > 2 * total_voting_power
            || 3 * current_nil_prevoted > 2 * total_voting_power
            || 6 * current_prevoted > total_voting_power
        {
            assertion_check(&return_responses);
            return return_responses;
        }

        if favor {
            let event = ConsensusEvent::NonNilPrevote {
                proposal,
                signer,
                round,
                time,
            };
            let response = state.progress(event);
            return_responses.push(response);
            current_non_nil_prevoted += power;
        } else {
            let event = ConsensusEvent::NilPrevote {
                signer,
                round,
                time,
            };
            let response = state.progress(event);
            return_responses.push(response);
            current_nil_prevoted += power;
        }
    }
    assertion_check(&return_responses);
    return_responses
}

#[allow(dead_code)]
pub fn precommits(
    height_info: &HeightInfo,
    state: &mut ConsensusState,
    favor_of_this_node: (bool, u64),
    commits: Vec<(ValidatorIndex, bool, u64, Timestamp)>,
    round: usize,
    proposal: usize,
) -> Vec<Option<Vec<ConsensusResponse>>> {
    let mut commits_time_sorted = commits;
    let total_voting_power: u64 = height_info.validators.iter().sum();
    commits_time_sorted.sort_by_key(|k| k.3);

    let mut current_non_nil_precommitted = if favor_of_this_node.0 {
        favor_of_this_node.1
    } else {
        0
    };
    let mut current_nil_precommitted = if !favor_of_this_node.0 {
        favor_of_this_node.1
    } else {
        0
    };
    let mut return_responses = Vec::<Option<Vec<ConsensusResponse>>>::new();

    for (signer, favor, power, time) in commits_time_sorted {
        if 3 * current_non_nil_precommitted > 2 * total_voting_power
            || 3 * current_nil_precommitted > 2 * total_voting_power
        {
            assertion_check(&return_responses);
            return return_responses;
        }
        if favor {
            let event = ConsensusEvent::NonNilPrecommit {
                proposal,
                signer,
                round,
                time,
            };
            let response = state.progress(event);
            return_responses.push(response);
            current_non_nil_precommitted += power;
        } else {
            let event = ConsensusEvent::NilPrecommit {
                signer,
                round,
                time,
            };
            let response = state.progress(event);
            return_responses.push(response);
            current_nil_precommitted += power;
        }
    }
    assertion_check(&return_responses);
    return_responses
}

//Check formal n-1 responses are empty, on the other hand last one is not.
//By adding expected response as a parameter, can compare actual last response with expected response.
#[allow(dead_code)]
pub fn assertion_check(responses: &Vec<Option<Vec<ConsensusResponse>>>) {
    let length = responses.len();
    if !responses.is_empty() {
        let last_response = &responses.last();
        for response in &responses[0..length - 1] {
            assert_eq!(response, &Some(Vec::new()));
        }
        assert_ne!(last_response.unwrap(), &Some(Vec::new()));
    }
}

#[allow(dead_code)]
pub fn bulk_prevote(
    state: &mut ConsensusState,
    proposal: BlockIdentifier,
    signers: Vec<ValidatorIndex>,
    round: usize,
    timestamps: Vec<Timestamp>,
) -> Option<Vec<ConsensusResponse>> {
    if signers.len() != timestamps.len() {
        panic!("Invalid lengths with signers and timestamps");
    }

    let idx = signers.len();
    let mut last_response: Option<Vec<ConsensusResponse>> = Some(Vec::new());

    for i in 0..idx {
        let signer = signers[i];
        let time = timestamps[i];
        let event = ConsensusEvent::NonNilPrevote {
            proposal,
            signer,
            round,
            time,
        };
        last_response = state.progress(event);
    }
    last_response
}

#[allow(dead_code)]
pub fn bulk_nilvote(
    state: &mut ConsensusState,
    signers: Vec<ValidatorIndex>,
    round: usize,
    timestamps: Vec<Timestamp>,
) -> Option<Vec<ConsensusResponse>> {
    if signers.len() != timestamps.len() {
        panic!("Invalid lengths with signers and timestamps");
    }

    let idx = signers.len();
    let mut last_response: Option<Vec<ConsensusResponse>> = Some(vec![]);

    for i in 0..idx {
        let signer = signers[i];
        let time = timestamps[i];
        let event = ConsensusEvent::NilPrevote {
            signer,
            round,
            time,
        };
        last_response = state.progress(event);
    }
    last_response
}

#[allow(dead_code)]
pub fn bulk_precommit(
    state: &mut ConsensusState,
    proposal: BlockIdentifier,
    signers: Vec<ValidatorIndex>,
    round: usize,
    timestamps: Vec<Timestamp>,
) -> Option<Vec<ConsensusResponse>> {
    if signers.len() != timestamps.len() {
        panic!("Invalid lengths with signers and timestamps");
    }

    let idx = signers.len();
    let mut last_response: Option<Vec<ConsensusResponse>> = Some(vec![]);

    for i in 0..idx {
        let signer = signers[i];
        let time = timestamps[i];
        let event = ConsensusEvent::NonNilPrecommit {
            proposal,
            signer,
            round,
            time,
        };
        last_response = state.progress(event);
    }
    last_response
}

#[allow(dead_code)]
pub fn bulk_nilcommit(
    state: &mut ConsensusState,
    signers: Vec<ValidatorIndex>,
    round: usize,
    timestamps: Vec<Timestamp>,
) -> Option<Vec<ConsensusResponse>> {
    if signers.len() != timestamps.len() {
        panic!("Invalid lengths with signers and timestamps");
    }

    let idx = signers.len();
    let mut last_response: Option<Vec<ConsensusResponse>> = Some(vec![]);

    for i in 0..idx {
        let signer = signers[i];
        let time = timestamps[i];
        let event = ConsensusEvent::NilPrecommit {
            signer,
            round,
            time,
        };
        last_response = state.progress(event);
    }
    last_response
}
