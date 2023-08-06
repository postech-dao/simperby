use vetomint::*;

/// A very normal, desirable, and expected scenario.
#[test]
fn normal_1() {
    let mut height_info = HeightInfo {
        validators: vec![1, 1, 1, 1],
        this_node_index: Some(0),
        timestamp: 0,
        consensus_params: ConsensusParams {
            timeout_ms: 100,
            repeat_round_for_first_leader: 1,
        },
        initial_block_candidate: 0,
    };
    let mut proposer = Vetomint::new(height_info.clone());
    let mut nodes = Vec::new();
    for i in 1..=3 {
        height_info.this_node_index = Some(i);
        nodes.push(Vetomint::new(height_info.clone()));
    }
    let response = proposer.progress(ConsensusEvent::Start, 0);
    assert_eq!(
        response,
        vec![
            ConsensusResponse::BroadcastProposal {
                proposal: 0,
                valid_round: None,
                round: 0,
            },
            ConsensusResponse::BroadcastPrevote {
                proposal: Some(0),
                round: 0
            }
        ]
    );
    for node in nodes.iter_mut() {
        let response = node.progress(ConsensusEvent::Start, 0);
        assert_eq!(response, vec![]);
    }

    for node in nodes.iter_mut() {
        let response = node.progress(
            ConsensusEvent::BlockProposalReceived {
                proposal: 0,
                valid: true,
                valid_round: None,
                proposer: 0,
                round: 0,
                favor: true,
            },
            1,
        );
        assert_eq!(
            response,
            vec![ConsensusResponse::BroadcastPrevote {
                proposal: Some(0),
                round: 0,
            }]
        );
    }

    let mut nodes = vec![vec![proposer], nodes].concat();

    for (i, node) in nodes.iter_mut().enumerate() {
        let response = node.progress(
            ConsensusEvent::Prevote {
                proposal: Some(0),
                signer: (i + 1) % 4,
                round: 0,
            },
            2,
        );
        assert_eq!(response, Vec::new());
        let response = node.progress(
            ConsensusEvent::Prevote {
                proposal: Some(0),
                signer: (i + 2) % 4,
                round: 0,
            },
            2,
        );
        assert_eq!(
            response,
            vec![ConsensusResponse::BroadcastPrecommit {
                proposal: Some(0),
                round: 0,
            }]
        );
        let response = node.progress(
            ConsensusEvent::Prevote {
                proposal: Some(0),
                signer: (i + 3) % 4,
                round: 0,
            },
            2,
        );
        assert_eq!(response, Vec::new());
    }

    for (i, node) in nodes.iter_mut().enumerate() {
        let response = node.progress(
            ConsensusEvent::Precommit {
                proposal: Some(0),
                signer: (i + 1) % 4,
                round: 0,
            },
            3,
        );
        assert_eq!(response, Vec::new());
        let response = node.progress(
            ConsensusEvent::Precommit {
                proposal: Some(0),
                signer: (i + 2) % 4,
                round: 0,
            },
            3,
        );
        assert_eq!(
            response,
            vec![ConsensusResponse::FinalizeBlock {
                proposal: 0,
                proof: (0..4).filter(|x| *x != (i + 3) % 4).collect(),
                round: 0
            }]
        );
    }
}

/// Tendermint lock happens and it helps to keep the safety by reaching the consensus in the second round.
#[test]
fn lock_1() {
    let mut height_info = HeightInfo {
        validators: vec![1, 1, 1, 1],
        this_node_index: Some(0),
        timestamp: 0,
        consensus_params: ConsensusParams {
            timeout_ms: 100,
            repeat_round_for_first_leader: 1,
        },
        initial_block_candidate: 0,
    };
    let mut proposer = Vetomint::new(height_info.clone());
    let mut nodes = Vec::new();

    for i in 1..=3 {
        height_info.this_node_index = Some(i);
        nodes.push(Vetomint::new(height_info.clone()));
    }

    let response = proposer.progress(ConsensusEvent::Start, 0);
    assert_eq!(
        response,
        vec![
            ConsensusResponse::BroadcastProposal {
                proposal: 0,
                valid_round: None,
                round: 0,
            },
            ConsensusResponse::BroadcastPrevote {
                proposal: Some(0),
                round: 0
            }
        ]
    );
    for node in nodes.iter_mut() {
        let response = node.progress(ConsensusEvent::Start, 0);
        assert_eq!(response, vec![]);
    }

    for node in nodes.iter_mut() {
        let response = node.progress(
            ConsensusEvent::BlockProposalReceived {
                proposal: 0,
                valid: true,
                valid_round: None,
                proposer: 0,
                round: 0,
                favor: true,
            },
            1,
        );
        assert_eq!(
            response,
            vec![ConsensusResponse::BroadcastPrevote {
                proposal: Some(0),
                round: 0,
            }]
        );
    }

    let mut nodes = vec![vec![proposer], nodes].concat();

    // node 0, 2, 3 reached polka while node1 didn't
    for (i, node) in nodes.iter_mut().enumerate() {
        if i == 1 {
            continue;
        }

        let response = node.progress(
            ConsensusEvent::Prevote {
                proposal: Some(0),
                signer: (i + 1) % 4,
                round: 0,
            },
            2,
        );
        assert_eq!(response, Vec::new());
        let response = node.progress(
            ConsensusEvent::Prevote {
                proposal: Some(0),
                signer: (i + 2) % 4,
                round: 0,
            },
            2,
        );
        assert_eq!(
            response,
            vec![ConsensusResponse::BroadcastPrecommit {
                proposal: Some(0),
                round: 0,
            }]
        );
        let response = node.progress(
            ConsensusEvent::Prevote {
                proposal: Some(0),
                signer: (i + 3) % 4,
                round: 0,
            },
            2,
        );
        assert_eq!(response, Vec::new());
    }

    // node1 failed to receives prevotes from others.
    let node1 = &mut nodes[1];
    node1.progress(ConsensusEvent::BlockCandidateUpdated { proposal: (1) }, 3);

    // all nodes get enough nil precommits to move round
    for (i, node) in nodes.iter_mut().enumerate() {
        let response = node.progress(
            ConsensusEvent::Precommit {
                proposal: None,
                signer: (i + 1) % 4,
                round: 0,
            },
            3,
        );
        assert_eq!(response, Vec::new());

        let response = node.progress(
            ConsensusEvent::Precommit {
                proposal: None,
                signer: (i + 2) % 4,
                round: 0,
            },
            3,
        );
        assert_eq!(response, Vec::new());

        let response = node.progress(
            ConsensusEvent::Precommit {
                proposal: None,
                signer: (i + 3) % 4,
                round: 0,
            },
            3,
        );
        if i == 1 {
            assert_eq!(
                response,
                vec![
                    ConsensusResponse::BroadcastProposal {
                        proposal: 1,
                        valid_round: None,
                        round: 1,
                    },
                    ConsensusResponse::BroadcastPrevote {
                        proposal: Some(1),
                        round: 1
                    }
                ]
            )
        } else {
            assert_eq!(response, Vec::new());
        }
    }

    // node1's suggestion got ignored by others
    for (i, node) in nodes.iter_mut().enumerate() {
        if i == 1 {
            continue;
        }
        let response = node.progress(
            ConsensusEvent::BlockProposalReceived {
                proposal: 1,
                valid: true,
                valid_round: None,
                proposer: 1,
                round: 1,
                favor: true,
            },
            4,
        );
        assert_eq!(
            response,
            vec![ConsensusResponse::BroadcastPrevote {
                proposal: None,
                round: 1
            }]
        );
    }

    for (i, node) in nodes.iter_mut().enumerate() {
        let response = node.progress(
            ConsensusEvent::Prevote {
                proposal: None,
                signer: (i + 1) % 4,
                round: 1,
            },
            5,
        );
        assert_eq!(response, Vec::new());

        let response = node.progress(
            ConsensusEvent::Prevote {
                proposal: None,
                signer: (i + 2) % 4,
                round: 1,
            },
            5,
        );

        if i == 1 {
            assert_eq!(response, Vec::new());
        } else {
            assert_eq!(
                response,
                vec![ConsensusResponse::BroadcastPrecommit {
                    proposal: None,
                    round: 1
                }]
            )
        }

        let response = node.progress(
            ConsensusEvent::Prevote {
                proposal: None,
                signer: (i + 3) % 4,
                round: 1,
            },
            5,
        );
        if i == 1 {
            assert_eq!(
                response,
                vec![ConsensusResponse::BroadcastPrecommit {
                    proposal: None,
                    round: 1
                }]
            )
        } else {
            assert_eq!(response, Vec::new());
        }
    }

    // move to the third round
    for (i, node) in nodes.iter_mut().enumerate() {
        let response = node.progress(
            ConsensusEvent::Precommit {
                proposal: None,
                signer: (i + 1) % 4,
                round: 1,
            },
            6,
        );
        assert_eq!(response, Vec::new());

        let response = node.progress(
            ConsensusEvent::Precommit {
                proposal: None,
                signer: (i + 2) % 4,
                round: 1,
            },
            6,
        );

        if i == 2 {
            assert_eq!(
                response,
                vec![
                    ConsensusResponse::BroadcastProposal {
                        proposal: 0,
                        valid_round: Some(0),
                        round: 2,
                    },
                    ConsensusResponse::BroadcastPrevote {
                        proposal: Some(0),
                        round: 2
                    }
                ]
            )
        } else {
            assert_eq!(response, Vec::new());
        }

        let response = node.progress(
            ConsensusEvent::Precommit {
                proposal: None,
                signer: (i + 3) % 4,
                round: 1,
            },
            6,
        );
        assert_eq!(response, Vec::new());
    }

    // node1 received non-nil prevotes of proposal at round 0
    let node1 = &mut nodes[1];

    let response = node1.progress(
        ConsensusEvent::Prevote {
            proposal: Some(0),
            signer: 2,
            round: 0,
        },
        2,
    );
    assert_eq!(response, Vec::new());

    let response = node1.progress(
        ConsensusEvent::Prevote {
            proposal: Some(0),
            signer: 3,
            round: 0,
        },
        2,
    );
    assert_eq!(response, Vec::new());

    for (i, node) in nodes.iter_mut().enumerate() {
        if i == 2 {
            continue;
        }
        let response = node.progress(
            ConsensusEvent::BlockProposalReceived {
                proposal: 0,
                valid: true,
                valid_round: Some(0),
                proposer: 2,
                round: 2,
                favor: true,
            },
            7,
        );
        assert_eq!(
            response,
            vec![ConsensusResponse::BroadcastPrevote {
                proposal: Some(0),
                round: 2
            }]
        )
    }

    // all nodes get non-nil prevotes
    for (i, node) in nodes.iter_mut().enumerate() {
        let response = node.progress(
            ConsensusEvent::Prevote {
                proposal: Some(0),
                signer: (i + 1) % 4,
                round: 2,
            },
            8,
        );
        assert_eq!(response, Vec::new());
        let response = node.progress(
            ConsensusEvent::Prevote {
                proposal: Some(0),
                signer: (i + 2) % 4,
                round: 2,
            },
            8,
        );
        assert_eq!(
            response,
            vec![ConsensusResponse::BroadcastPrecommit {
                proposal: Some(0),
                round: 2,
            }]
        );
        let response = node.progress(
            ConsensusEvent::Prevote {
                proposal: Some(0),
                signer: (i + 3) % 4,
                round: 2,
            },
            8,
        );
        assert_eq!(response, Vec::new());
    }

    // all nodes get non-nil precommits and reach the final state
    for (i, node) in nodes.iter_mut().enumerate() {
        let response = node.progress(
            ConsensusEvent::Precommit {
                proposal: Some(0),
                signer: (i + 1) % 4,
                round: 2,
            },
            9,
        );
        assert_eq!(response, Vec::new());
        let response = node.progress(
            ConsensusEvent::Precommit {
                proposal: Some(0),
                signer: (i + 2) % 4,
                round: 2,
            },
            9,
        );
        assert_eq!(
            response,
            vec![ConsensusResponse::FinalizeBlock {
                proposal: 0,
                proof: (0..4).filter(|x| *x != (i + 3) % 4).collect(),
                round: 2
            }]
        );
    }
}

/// A byzantine node broadcasts both nil and non-nil prevotes but fails to break the safety.
#[ignore]
#[test]
fn double_votes_1() {}

/// Timeout occurs in the prevote stage, skipping the first round but eventually reaching consensus.
#[ignore]
#[test]
fn timeout_prevote_1() {}
