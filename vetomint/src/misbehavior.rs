use super::*;
use state::*;

use std::collections::HashMap;

/// comment for check_double_proposal
pub(crate) fn check_double_proposal(
    state: &ConsensusState,
    target_round: Round,
) -> Vec<ConsensusResponse> {
    let proposals: Vec<_> = state
        .proposals
        .iter()
        .filter(|(_, proposal)| proposal.round == target_round)
        .map(|(_, proposal)| proposal)
        .collect();

    if proposals.len() > 1 {
        let byzantine_signer = proposals[0].proposer;
        let (origin_proposal, new_proposal) = (proposals[0].proposal, proposals[1].proposal);

        return vec![ConsensusResponse::ViolationReport {
            violator: 0,
            misbehavior: Misbehavior::DoubleProposal {
                byzantine_node: byzantine_signer,
                round: target_round,
                proposals: (origin_proposal, new_proposal),
            },
        }];
    }

    Vec::new()
}

/// comment for check_double_prevote
pub(crate) fn check_double_prevote(
    state: &ConsensusState,
    target_round: Round,
) -> Vec<ConsensusResponse> {
    let mut validators_map = HashMap::new();

    for vote in state.prevotes.iter() {
        let (count, origin_proposal) = validators_map
            .entry(vote.signer)
            .or_insert((0, vote.proposal));

        *count += 1;

        if *count == 2 {
            let new_proposal = vote.proposal;
            let byzantine_signer = vote.signer;

            return vec![ConsensusResponse::ViolationReport {
                violator: byzantine_signer,
                misbehavior: Misbehavior::DoublePrevote {
                    byzantine_node: byzantine_signer,
                    round: target_round,
                    proposals: (*origin_proposal, new_proposal),
                },
            }];
        }
    }

    Vec::new()
}

/// comment for check_double_precommit
pub(crate) fn check_double_precommit(
    state: &ConsensusState,
    target_round: Round,
) -> Vec<ConsensusResponse> {
    let mut validators_map = HashMap::new();

    for vote in state.precommits.iter() {
        let (count, origin_proposal) = validators_map
            .entry(vote.signer)
            .or_insert((0, vote.proposal));

        *count += 1;

        if *count == 2 {
            let byzantine_signer = vote.signer;
            let new_proposal = vote.proposal;

            return vec![ConsensusResponse::ViolationReport {
                violator: byzantine_signer,
                misbehavior: Misbehavior::DoublePrecommit {
                    byzantine_node: byzantine_signer,
                    round: target_round,
                    proposals: (*origin_proposal, new_proposal),
                },
            }];
        }
    }

    Vec::new()
}

/// comment for check_invalid_proposal
pub(crate) fn check_invalid_proposal(
    byzantine_proposer: usize,
    target_round: Round,
    target_proposal: BlockIdentifier,
) -> Vec<ConsensusResponse> {
    return vec![ConsensusResponse::ViolationReport {
        violator: byzantine_proposer,
        misbehavior: Misbehavior::InvalidProposal {
            byzantine_node: byzantine_proposer,
            round: target_round,
            proposal: target_proposal,
        },
    }];
}

/// comment for check_invalid_prevote
pub(crate) fn check_invalid_prevote(
    state: &ConsensusState,
    target_round: Round,
    target_proposal: BlockIdentifier,
) -> Vec<ConsensusResponse> {
    unimplemented!()
}

/// comment for check_invalid_precommit
pub(crate) fn check_invalid_precommit(
    state: &ConsensusState,
    check_round: Round,
    check_proposal: BlockIdentifier,
) -> Vec<ConsensusResponse> {
    unimplemented!()
}
