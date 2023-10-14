use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) enum ConsensusStep {
    Initial,
    Propose,
    Prevote,
    Precommit,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, PartialOrd, Ord)]
pub(crate) struct Proposal {
    pub proposal: BlockIdentifier,
    pub valid: bool,
    pub valid_round: Option<Round>,
    pub round: Round,
    pub proposer: ValidatorIndex,
    pub favor: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, PartialOrd, Ord)]
pub(crate) struct Vote {
    pub proposal: Option<BlockIdentifier>,
    pub signer: ValidatorIndex,
    pub round: Round,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct ConsensusState {
    pub height_info: HeightInfo,
    pub round: Round,
    pub step: ConsensusStep,
    pub locked_value: Option<BlockIdentifier>,
    pub locked_round: Option<Round>,
    pub valid_value: Option<BlockIdentifier>,
    pub valid_round: Option<Round>,
    pub block_candidate: BlockIdentifier,
    pub proposals: BTreeMap<BlockIdentifier, Proposal>,
    pub prevotes: BTreeSet<Vote>,
    pub precommits: BTreeSet<Vote>,
    pub propose_timeout_schedules: BTreeSet<(Round, Timestamp)>,
    pub precommit_timeout_schedules: BTreeSet<(Round, Timestamp)>,
    pub for_the_first_time_1: BTreeSet<Round>,
    pub for_the_first_time_2: BTreeSet<Round>,
    pub finalized: Option<(BlockIdentifier, Vec<ValidatorIndex>, Round)>,
}

impl ConsensusState {
    pub(crate) fn new(height_info: HeightInfo) -> Self {
        ConsensusState {
            height_info,
            round: 0,
            step: ConsensusStep::Initial,
            locked_value: None,
            locked_round: None,
            valid_value: None,
            valid_round: None,
            block_candidate: BlockIdentifier::default(),
            proposals: Default::default(),
            prevotes: Default::default(),
            precommits: Default::default(),
            propose_timeout_schedules: Default::default(),
            precommit_timeout_schedules: Default::default(),
            for_the_first_time_1: Default::default(),
            for_the_first_time_2: Default::default(),
            finalized: None,
        }
    }

    pub(crate) fn get_total_voting_power(&self) -> VotingPower {
        self.height_info.validators.iter().sum()
    }

    pub(crate) fn get_total_prevotes(&self, round: Round) -> VotingPower {
        self.prevotes
            .iter()
            .filter(|vote| vote.round == round)
            .map(|vote| self.height_info.validators[vote.signer])
            .sum()
    }

    pub(crate) fn get_total_precommits(&self, round: Round) -> VotingPower {
        self.precommits
            .iter()
            .filter(|vote| vote.round == round)
            .map(|vote| self.height_info.validators[vote.signer])
            .sum()
    }

    pub(crate) fn get_total_prevotes_on_proposal(
        &self,
        round: Round,
        proposal: BlockIdentifier,
    ) -> VotingPower {
        self.prevotes
            .iter()
            .filter(|vote| vote.round == round && vote.proposal == Some(proposal))
            .map(|vote| self.height_info.validators[vote.signer])
            .sum()
    }

    pub(crate) fn get_total_precommits_on_proposal(
        &self,
        round: Round,
        proposal: BlockIdentifier,
    ) -> VotingPower {
        self.precommits
            .iter()
            .filter(|vote| vote.round == round && vote.proposal == Some(proposal))
            .map(|vote| self.height_info.validators[vote.signer])
            .sum()
    }

    pub(crate) fn get_total_prevotes_on_nil(&self, round: Round) -> VotingPower {
        self.prevotes
            .iter()
            .filter(|vote| vote.round == round && vote.proposal.is_none())
            .map(|vote| self.height_info.validators[vote.signer])
            .sum()
    }

    pub(crate) fn get_total_precommits_on_nil(&self, round: Round) -> VotingPower {
        self.precommits
            .iter()
            .filter(|vote| vote.round == round && vote.proposal.is_none())
            .map(|vote| self.height_info.validators[vote.signer])
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_default_consensus_state() -> ConsensusState {
        let height_info = HeightInfo {
            validators: vec![1, 1, 1, 1],
            this_node_index: Some(0),
            timestamp: 0,
            consensus_params: ConsensusParams {
                timeout_ms: 100,
                repeat_round_for_first_leader: 1,
            },
            initial_block_candidate: 0,
        };
        ConsensusState::new(height_info)
    }

    #[test]
    fn test_get_total_voting_power() {
        let consensus_state = create_default_consensus_state();
        assert_eq!(consensus_state.get_total_voting_power(), 4);
    }

    #[test]
    fn get_total_prevotes() {
        // TODO: modify the default consensus state to test this.
    }

    #[test]
    fn get_total_precommits() {
        // TODO: modify the default consensus state to test this.
    }

    #[test]
    fn get_total_prevotes_on_proposal() {
        // TODO: modify the default consensus state to test this.
    }

    #[test]
    fn get_total_precommits_on_proposal() {
        // TODO: modify the default consensus state to test this.
    }

    #[test]
    fn get_total_prevotes_on_nil() {
        // TODO: modify the default consensus state to test this.
    }

    #[test]
    fn get_total_precommits_on_nil() {
        // TODO: modify the default consensus state to test this.
    }
}
