use super::*;
use state::*;

use std::collections::HashMap;

pub(crate) fn check_misbehavior(
    state: &ConsensusState,
    check_round: Round,
    check_proposal: BlockIdentifier,
) -> Vec<Misbehavior> {
    let mut misbehavior: Vec<Misbehavior> = vec![];

    if let Some(m) = check_double_proposals(state, check_round) {
        misbehavior.push(m);
    } else {
        println!("not found double proposals in this round");
    }

    if let Some(m) = check_double_prevote(state, check_round, check_proposal) {
        misbehavior.push(m);
    } else {
        println!("not found double prevote in this round");
    }

    if let Some(m) = check_double_precommit(state, check_round, check_proposal) {
        misbehavior.push(m);
    } else {
        println!("not found double precommit in this round");
    }

    misbehavior
}

fn check_double_proposals(state: &ConsensusState, check_round: Round) -> Option<Misbehavior> {
    let proposals: Vec<_> = state
        .proposals
        .iter()
        .filter(|(_, proposal)| proposal.round == check_round)
        .map(|(_, proposal)| proposal)
        .collect();

    if proposals.len() > 1 {
        // returnSome(result[0].1.proposer)
        return Some(Misbehavior::DoubleProposal {
            byzantine_node: proposals[0].proposer,
            round: check_round,
            proposals: (proposals[0].proposal, proposals[1].proposal),
        });
    }

    None
}

fn check_double_prevote(
    state: &ConsensusState,
    check_round: Round,
    check_proposal: BlockIdentifier,
) -> Option<Misbehavior> {
    let mut validators_map = HashMap::new();

    for vote in state.prevotes.iter() {
        let count = validators_map.entry(vote.signer).or_insert(0);
        *count += 1;

        if *count == 2 {
            return Some(Misbehavior::DoublePrevote {
                byzantine_node: vote.signer,
                round: check_round,
                proposals: (Some(check_proposal), Some(check_proposal)),
            });
        }
    }

    None
}

fn check_double_precommit(
    state: &ConsensusState,
    check_round: Round,
    check_proposal: BlockIdentifier,
) -> Option<Misbehavior> {
    let mut validators_map = HashMap::new();

    for vote in state.precommits.iter() {
        let count = validators_map.entry(vote.signer).or_insert(0);
        *count += 1;

        if *count == 2 {
            return Some(Misbehavior::DoublePrecommit {
                byzantine_node: vote.signer,
                round: check_round,
                proposals: (Some(check_proposal), Some(check_proposal)),
            });
        }
    }

    None
}
