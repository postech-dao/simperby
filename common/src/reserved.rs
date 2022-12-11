use std::collections::HashMap;

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
    pub consensus_leader_order: Vec<MemberName>,
    /// The semantic version of Simperby protocol for this network.
    pub version: String,
}

impl ReservedState {
    pub fn get_validator_set(&self) -> Result<Vec<(PublicKey, VotingPower)>, String> {
        let mut validator_set = HashMap::new();
        for member in &self.members {
            if let Some(delegatee) = &member.consensus_delegations {
                validator_set
                    .entry(delegatee.clone())
                    .and_modify(|v| *v += member.consensus_voting_power)
                    .or_insert(member.consensus_voting_power);
            } else {
                validator_set
                    .entry(member.public_key.clone())
                    .and_modify(|v| *v += member.consensus_voting_power)
                    .or_insert(member.consensus_voting_power);
            }
        }
        let mut validator_set: Vec<(PublicKey, u64)> = validator_set
            .iter()
            .map(|(public_key, voting_power)| (public_key.clone(), *voting_power))
            .collect();
        validator_set.sort_by_key(|(public_key, _)| {
            self.consensus_leader_order
                .iter()
                .position(|member_name| *member_name == self.query_name(public_key).unwrap())
                .unwrap()
        });
        Ok(validator_set)
    }

    pub fn get_governance_set(&self) -> Result<Vec<(PublicKey, VotingPower)>, String> {
        let mut governance_set = HashMap::new();
        for member in &self.members {
            if let Some(delegatee) = &member.governance_delegations {
                governance_set
                    .entry(delegatee.clone())
                    .and_modify(|v| *v += member.consensus_voting_power)
                    .or_insert(member.consensus_voting_power);
            } else {
                governance_set
                    .entry(member.public_key.clone())
                    .and_modify(|v| *v += member.consensus_voting_power)
                    .or_insert(member.consensus_voting_power);
            }
        }
        Ok(governance_set
            .iter()
            .map(|(public_key, voting_power)| (public_key.clone(), *voting_power))
            .collect())
    }

    pub fn apply_delegate(&mut self, _tx: &TxDelegate) -> Result<Self, String> {
        unimplemented!()
    }

    pub fn apply_undelegate(&mut self, _tx: &TxUndelegate) -> Result<Self, String> {
        unimplemented!()
    }

    pub fn query_name(&self, public_key: &PublicKey) -> Option<MemberName> {
        for member in &self.members {
            if &member.public_key == public_key {
                return Some(member.name.clone());
            }
        }
        None
    }
}
