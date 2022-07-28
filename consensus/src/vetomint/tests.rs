use super::*;

#[ignore]
#[test]
fn success_trivial_1() {
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
        favor: true,
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
        };
        let response = progress(&height_info, &mut state, event);
        assert!(response.is_empty());
    }
    let event = ConsensusEvent::Prevote {
        proposal: 0,
        round: 0,
        signer: 5,
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
        };
        let response = progress(&height_info, &mut state, event);
        assert!(response.is_empty());
    }
    let event = ConsensusEvent::Precommit {
        proposal: 0,
        round: 0,
        signer: 5,
    };
    let response = progress(&height_info, &mut state, event);
    assert_eq!(
        response,
        vec![ConsensusResponse::FinalizeBlock { proposal: 0 }]
    );
}
