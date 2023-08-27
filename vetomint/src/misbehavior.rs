use super::*;
use state::*;

use std::collections::HashMap;

/// Check whether there are double prevotes in target round
pub(crate) fn check_double_prevote(
    state: &ConsensusState,
    target_round: Round,
) -> Vec<ConsensusResponse> {
    let mut response = Vec::new();
    let mut validators_map = HashMap::new();
    let prevotes_in_target_round: Vec<_> = state
        .prevotes
        .iter()
        .filter(|vote| vote.round == target_round)
        .collect();

    for vote in prevotes_in_target_round.iter() {
        let (count, origin_proposal) = validators_map
            .entry(vote.signer)
            .or_insert((0, vote.proposal));
        *count += 1;

        if *count >= 2 {
            let byzantine_validator = vote.signer;
            let double_proposal = vote.proposal;

            response.extend(vec![ConsensusResponse::ViolationReport {
                violator: byzantine_validator,
                misbehavior: Misbehavior::DoublePrevote {
                    byzantine_node: byzantine_validator,
                    round: target_round,
                    proposals: (*origin_proposal, double_proposal),
                },
            }]);
        }
    }

    response
}

/// Check whether there are double precommits in target round
pub(crate) fn check_double_precommit(
    state: &ConsensusState,
    target_round: Round,
) -> Vec<ConsensusResponse> {
    let mut response = Vec::new();
    let mut validators_map = HashMap::new();
    let precommits_in_target_round: Vec<_> = state
        .precommits
        .iter()
        .filter(|vote| vote.round == target_round)
        .collect();

    for vote in precommits_in_target_round.iter() {
        let (count, origin_proposal) = validators_map
            .entry(vote.signer)
            .or_insert((0, vote.proposal));
        *count += 1;

        if *count >= 2 {
            let byzantine_validator = vote.signer;
            let double_proposal = vote.proposal;

            response.extend(vec![ConsensusResponse::ViolationReport {
                violator: byzantine_validator,
                misbehavior: Misbehavior::DoublePrecommit {
                    byzantine_node: byzantine_validator,
                    round: target_round,
                    proposals: (*origin_proposal, double_proposal),
                },
            }]);
        }
    }
    response
}
