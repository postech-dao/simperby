use crate::*;
use serde::{Deserialize, Serialize};

/// The partial set of the blockchain state which is reserved and protected.
///
/// It is stored in the reserved directory of the repository.
/// Any transaction which modifies this state MUST produce a valid next one.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct ReservedState {
    /// The genesis info. This must never be changed.
    pub genesis_info: GenesisInfo,
    /// The members.
    pub members: Vec<Member>,
    /// The leader order of the consensus rounds.
    ///
    /// It MUST be sorted by the name of the members.
    pub consensus_leader_order: Vec<usize>,
    /// The semantic version of Simperby protocol for this network.
    pub version: String,
}

impl ReservedState {
    pub fn create_validator_set(&self) -> Result<Vec<(PublicKey, VotingPower)>, String> {
        let mut validator_set = Vec::new();
        for leader in &self.consensus_leader_order {
            let member = &self.members.get(*leader).ok_or(format!(
                "invalid 
            consensus_leader_order: {}",
                leader
            ))?;
            validator_set.push((member.public_key.clone(), member.consensus_voting_power));
        }
        Ok(validator_set)
    }

    pub fn apply_delegate(&mut self, _tx: &TxDelegate) -> Result<Self, String> {
        unimplemented!()
    }

    pub fn apply_undelegate(&mut self, _tx: &TxUndelegate) -> Result<Self, String> {
        unimplemented!()
    }
}
