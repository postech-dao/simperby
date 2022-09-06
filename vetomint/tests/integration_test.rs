use vetomint::*;

#[ignore]
#[test]
fn success_trivial_1() {
    let height_info = HeightInfo {
        validators: vec![1, 1, 1, 1, 1, 1, 1],
        this_node_index: 6,
        timestamp: 0,
        consensus_params: ConsensusParams {
            timeout_ms: 1000,
            repeat_round_for_first_leader: 1,
        },
        initial_block_candidate: 0,
    };
    let mut state = ConsensusState::new(height_info.clone());

    // STEP 1: Proposal.
    let event = ConsensusEvent::BlockProposalReceived {
        proposal: 0,
        proposer: 0,
        round: 0,
        time: 1,
        favor: true,
    };
    let response = state.progress(event).unwrap();
    assert_eq!(
        response,
        vec![ConsensusResponse::BroadcastPrevote {
            proposal: 0,
            round: 0
        }]
    );

    // STEP 2: Prevote.
    for validator_index in 0..=2 {
        let event = ConsensusEvent::Prevote {
            proposal: 0,
            round: 0,
            signer: validator_index,
            time: 3,
        };
        let response = state.progress(event).unwrap();
        assert!(response.is_empty());
    }
    let event = ConsensusEvent::Prevote {
        proposal: 0,
        round: 0,
        signer: 3,
        time: 3,
    };
    let response = state.progress(event).unwrap();
    assert_eq!(
        response,
        vec![ConsensusResponse::BroadcastPrecommit {
            proposal: 0,
            round: 0
        }]
    );

    // STEP 3: Prevote.
    for validator_index in 0..=2 {
        let event = ConsensusEvent::Precommit {
            proposal: 0,
            round: 0,
            signer: validator_index,
            time: 4,
        };
        let response = state.progress(event).unwrap();
        assert!(response.is_empty());
    }
    let event = ConsensusEvent::Precommit {
        proposal: 0,
        round: 0,
        signer: 3,
        time: 4,
    };
    let response = state.progress(event).unwrap();
    assert_eq!(
        response,
        vec![ConsensusResponse::FinalizeBlock { proposal: 0 }]
    );
}
