use super::*;

const TIMEOUT: i64 = 1;

#[ignore]
#[test]
fn success_trivial_1() {
    let height_info = HeightInfo {
        validators: vec![1, 1, 1, 1, 1, 1, 1],
        this_node_index: 6,
        timestamp: 0,
        consensus_params: ConsensusParams { timeout_ms: 1000 },
    };
    let mut state = ConsensusState::new(height_info.clone());

    // STEP 1: Proposal.
    let event = ConsensusEvent::BlockProposal {
        proposal: 0,
        proposer: 0,
        round: 0,
        time: 1,
    };
    let response = progress(&height_info, &mut state, event);
    assert!(response.is_empty());
    let event = ConsensusEvent::ProposalFavor {
        proposal: 0,
        favor: true,
        time: 2,
    };
    let response = progress(&height_info, &mut state, event);
    assert_eq!(
        response,
        vec![ConsensusResponse::BroadcastPrevote {
            proposal: 0,
            round: 0
        }]
    );

    // STEP 2: Prevote.
    for validator_index in 0..=4 {
        let event = ConsensusEvent::Prevote {
            proposal: 0,
            round: 0,
            signer: validator_index,
            time: 3,
        };
        let response = progress(&height_info, &mut state, event);
        assert!(response.is_empty());
    }
    let event = ConsensusEvent::Prevote {
        proposal: 0,
        round: 0,
        signer: 5,
        time: 3,
    };
    let response = progress(&height_info, &mut state, event);
    assert_eq!(
        response,
        vec![ConsensusResponse::BroadcastPrecommit {
            proposal: 0,
            round: 0
        }]
    );

    // STEP 3: Prevote.
    for validator_index in 0..=4 {
        let event = ConsensusEvent::Precommit {
            proposal: 0,
            round: 0,
            signer: validator_index,
            time: 4,
        };
        let response = progress(&height_info, &mut state, event);
        assert!(response.is_empty());
    }
    let event = ConsensusEvent::Precommit {
        proposal: 0,
        round: 0,
        signer: 5,
        time: 4,
    };
    let response = progress(&height_info, &mut state, event);
    assert_eq!(
        response,
        vec![ConsensusResponse::FinalizeBlock { proposal: 0 }]
    );
}

#[ignore]
#[test]
fn success_trivial_2() {
    let height_info = HeightInfo {
        validators: vec![1, 1, 1, 1, 1, 1, 1],
        this_node_index: 6,
        timestamp: 0,
    };
    let mut state = ConsensusState::new(height_info.clone());

    // STEP 1: Proposal.
    let event = ConsensusEvent::BlockProposal {
        proposal: 0,
        proposer: 0,
        round: 0,
        time: 1,
    };
    let response = progress(&height_info, &mut state, event);
    assert!(response.is_empty());
    let event = ConsensusEvent::ProposalFavor {
        proposal: 0,
        favor: true,
        time: 2,
    };
    let response = progress(&height_info, &mut state, event);
    assert_eq!(
        response,
        vec![ConsensusResponse::BroadcastPrevote {
            proposal: 0,
            round: 0
        }]
    );

    // STEP 2: Prevote, Wait for Timeout
    for validator_index in 0..=3 {
        let event = ConsensusEvent::Prevote {
            proposal: 0,
            round: 0,
            signer: validator_index,
            time: 3,
        };
        let response = progress(&height_info, &mut state, event);
        assert!(response.is_empty());
    }
    let _event = ConsensusEvent::Prevote {
        proposal: 0,
        round: 0,
        signer: 4,
        time: 3,
    };
    let event = ConsensusEvent::Timer { time: 3 + TIMEOUT };

    let response = progress(&height_info, &mut state, event);
    assert_eq!(
        response,
        vec![ConsensusResponse::BroadcastPrecommit {
            proposal: 0,
            round: 0
        }]
    );

    // STEP 3: Prevote.
    for validator_index in 0..=3 {
        let event = ConsensusEvent::Precommit {
            proposal: 0,
            round: 0,
            signer: validator_index,
            time: 5,
        };
        let response = progress(&height_info, &mut state, event);
        assert!(response.is_empty());
    }
    let event = ConsensusEvent::Precommit {
        proposal: 0,
        round: 0,
        signer: 4,
        time: 5,
    };
    let response = progress(&height_info, &mut state, event);
    assert_eq!(
        response,
        vec![ConsensusResponse::FinalizeBlock { proposal: 0 }]
    );
}

#[ignore]
#[test]
fn success_trivial_nil_1() {
    let height_info = HeightInfo {
        validators: vec![1, 1, 1, 1, 1, 1, 1],
        this_node_index: 6,
        timestamp: 0,
    };
    let mut state = ConsensusState::new(height_info.clone());

    // STEP 1: Proposal.
    let event = ConsensusEvent::BlockProposal {
        proposal: 0,
        proposer: 0,
        round: 0,
        time: 1,
    };
    let response = progress(&height_info, &mut state, event);
    assert!(response.is_empty());
    let event = ConsensusEvent::ProposalFavor {
        proposal: 0,
        favor: true,
        time: 2,
    };
    let response = progress(&height_info, &mut state, event);
    assert_eq!(
        response,
        vec![ConsensusResponse::BroadcastPrevote {
            proposal: 0,
            round: 0
        }]
    );

    // STEP 2: Prevote.
    for validator_index in 0..=4 {
        let event = ConsensusEvent::NilPrevote {
            round: 0,
            signer: validator_index,
            time: 3,
        };
        let response = progress(&height_info, &mut state, event);
        assert!(response.is_empty());
    }
    let event = ConsensusEvent::NilPrevote {
        round: 0,
        signer: 5,
        time: 3,
    };
    let response = progress(&height_info, &mut state, event);
    assert_eq!(
        response,
        vec![ConsensusResponse::BroadcastNilPrecommit { round: 0 }]
    );

    // STEP 3: Prevote.
    for validator_index in 0..=4 {
        let event = ConsensusEvent::NilPrecommit {
            round: 0,
            signer: validator_index,
            time: 4,
        };
        let response = progress(&height_info, &mut state, event);
        assert!(response.is_empty());
    }
    let event = ConsensusEvent::NilPrecommit {
        round: 0,
        signer: 5,
        time: 4,
    };

    let response = progress(&height_info, &mut state, event);
    assert_ne!(
        response,
        vec![ConsensusResponse::FinalizeBlock { proposal: 0 }]
    );
}

#[ignore]
#[test]
fn success_trivial_nil_2() {
    let height_info = HeightInfo {
        validators: vec![1, 1, 1, 1, 1, 1, 1],
        this_node_index: 6,
        timestamp: 0,
    };
    let mut state = ConsensusState::new(height_info.clone());

    // STEP 1: Proposal.
    let event = ConsensusEvent::BlockProposal {
        proposal: 0,
        proposer: 0,
        round: 0,
        time: 1,
    };
    let response = progress(&height_info, &mut state, event);
    assert!(response.is_empty());
    let event = ConsensusEvent::ProposalFavor {
        proposal: 0,
        favor: true,
        time: 2,
    };
    let response = progress(&height_info, &mut state, event);
    assert_eq!(
        response,
        vec![ConsensusResponse::BroadcastPrevote {
            proposal: 0,
            round: 0
        }]
    );

    // STEP 2: Prevote.
    for validator_index in 0..=4 {
        let event = ConsensusEvent::Prevote {
            proposal: 0,
            round: 0,
            signer: validator_index,
            time: 3,
        };
        let response = progress(&height_info, &mut state, event);
        assert!(response.is_empty());
    }
    let event = ConsensusEvent::Prevote {
        proposal: 0,
        round: 0,
        signer: 5,
        time: 3,
    };
    let response = progress(&height_info, &mut state, event);
    assert_eq!(
        response,
        vec![ConsensusResponse::BroadcastPrecommit {
            proposal: 0,
            round: 0
        }]
    );

    // STEP 3: Prevote.
    for validator_index in 0..=4 {
        let event = ConsensusEvent::NilPrecommit {
            round: 0,
            signer: validator_index,
            time: 4,
        };
        let response = progress(&height_info, &mut state, event);
        assert!(response.is_empty());
    }
    let event = ConsensusEvent::NilPrecommit {
        round: 0,
        signer: 5,
        time: 4,
    };
    let response = progress(&height_info, &mut state, event);
    assert_ne!(
        response,
        vec![ConsensusResponse::FinalizeBlock { proposal: 0 }]
    );
}

#[ignore]
#[test]
fn success_trivial_nil_3() {
    let height_info = HeightInfo {
        validators: vec![1, 1, 1, 1, 1, 1, 1],
        this_node_index: 6,
        timestamp: 0,
    };
    let mut state = ConsensusState::new(height_info.clone());

    // STEP 1: Proposal.
    let event = ConsensusEvent::BlockProposal {
        proposal: 0,
        proposer: 0,
        round: 0,
        time: 1,
    };
    let response = progress(&height_info, &mut state, event);
    assert!(response.is_empty());
    let event = ConsensusEvent::ProposalFavor {
        proposal: 0,
        favor: true,
        time: 2,
    };
    let response = progress(&height_info, &mut state, event);
    assert_eq!(
        response,
        vec![ConsensusResponse::BroadcastPrevote {
            proposal: 0,
            round: 0
        }]
    );

    // STEP 2: Prevote.
    for validator_index in 0..=3 {
        let event = ConsensusEvent::NilPrevote {
            round: 0,
            signer: validator_index,
            time: 3,
        };
        let response = progress(&height_info, &mut state, event);
        assert!(response.is_empty());
    }
    let _event = ConsensusEvent::NilPrevote {
        round: 0,
        signer: 4,
        time: 3,
    };

    let event = ConsensusEvent::Timer { time: 3 + TIMEOUT };

    let response = progress(&height_info, &mut state, event);
    assert_eq!(
        response,
        vec![ConsensusResponse::BroadcastPrecommit {
            proposal: 0,
            round: 0
        }]
    );

    // STEP 3: Prevote.
    for validator_index in 0..=4 {
        let event = ConsensusEvent::NilPrecommit {
            round: 0,
            signer: validator_index,
            time: 5,
        };
        let response = progress(&height_info, &mut state, event);
        assert!(response.is_empty());
    }
    let event = ConsensusEvent::NilPrecommit {
        round: 0,
        signer: 5,
        time: 5,
    };
    let response = progress(&height_info, &mut state, event);
    assert_ne!(
        response,
        vec![ConsensusResponse::FinalizeBlock { proposal: 0 }]
    );
}
