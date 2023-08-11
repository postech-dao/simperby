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

    if let Some(m) = check_invalid_proposal(state, check_proposal) {
        misbehavior.push(m);
    } else {
        println!("not found invalid proposal in this round");
    }

    if let Some(m) = check_invalid_prevote(state, check_round, check_proposal) {
        misbehavior.push(m);
    } else {
        println!("not found double precommit in this round");
    }

    if let Some(m) = check_invalid_precommit(state, check_round, check_proposal) {
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

fn check_invalid_proposal(
    state: &ConsensusState,
    check_proposal: BlockIdentifier,
) -> Option<Misbehavior> {
    if let Some(proposal) = state.proposals.get(&check_proposal) {
        if proposal.valid == false {
            return Some(Misbehavior::InvalidProposal {
                byzantine_node: proposal.proposer,
                round: proposal.round,
                proposal: proposal.proposal,
            });
        }
    }

    None
}

fn check_invalid_prevote(
    state: &ConsensusState,
    check_round: Round,
    check_proposal: BlockIdentifier,
) -> Option<Misbehavior> {
    let valid_prevotes: Vec<_> = state
        .prevotes
        .iter()
        .filter(|prevote| prevote.round == check_round)
        .collect();

    for prevote in valid_prevotes.iter() {
        if let Some(proposal) = prevote.proposal {
            if proposal == check_proposal {
                return Some(Misbehavior::InvalidPrevote {
                    byzantine_node: prevote.signer,
                    round: prevote.round,
                    proposal: proposal,
                });
            }
        }
    }

    None
}

fn check_invalid_precommit(
    state: &ConsensusState,
    check_round: Round,
    check_proposal: BlockIdentifier,
) -> Option<Misbehavior> {
    let valid_precommits: Vec<_> = state
        .precommits
        .iter()
        .filter(|prevote| prevote.round == check_round)
        .collect();

    for precommit in valid_precommits.iter() {
        if let Some(proposal) = precommit.proposal {
            if proposal == check_proposal {
                return Some(Misbehavior::InvalidPrecommit {
                    byzantine_node: precommit.signer,
                    round: precommit.round,
                    proposal: proposal,
                });
            }
        }
    }

    None
}
