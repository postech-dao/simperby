use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
                    .entry(member.name.clone())
                    .and_modify(|v| *v += member.consensus_voting_power)
                    .or_insert(member.consensus_voting_power);
            }
        }
        // TODO handle error
        Ok(self
            .consensus_leader_order
            .iter()
            .map(|name| (self.query_public_key(name).unwrap(), validator_set[name]))
            .collect())
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
                    .entry(member.name.clone())
                    .and_modify(|v| *v += member.consensus_voting_power)
                    .or_insert(member.consensus_voting_power);
            }
        }
        Ok(governance_set
            .iter()
            .map(|(name, voting_power)| (self.query_public_key(name).unwrap(), *voting_power))
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

    pub fn query_public_key(&self, name: &MemberName) -> Option<PublicKey> {
        for member in &self.members {
            if &member.name == name {
                return Some(member.public_key.clone());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simperby_test_suite::setup_test;
    use std::collections::HashSet;

    fn create_member(keys: Vec<(PublicKey, PrivateKey)>, member_num: u8) -> Member {
        Member {
            public_key: keys[member_num as usize].0.clone(),
            name: format!("member-{:04}", member_num),
            governance_voting_power: 1,
            consensus_voting_power: 1,
            governance_delegations: None,
            consensus_delegations: None,
        }
    }

    fn create_member_with_consensus_delegation(
        keys: Vec<(PublicKey, PrivateKey)>,
        member_num: u8,
        delegatee_member_num: u8,
    ) -> Member {
        Member {
            public_key: keys[member_num as usize].0.clone(),
            name: format!("member-{:04}", member_num),
            governance_voting_power: 1,
            consensus_voting_power: 1,
            governance_delegations: None,
            consensus_delegations: Some(format!("member-{:04}", delegatee_member_num)),
        }
    }

    fn create_member_with_governance_delegation(
        keys: Vec<(PublicKey, PrivateKey)>,
        member_num: u8,
        delegatee_member_num: u8,
    ) -> Member {
        Member {
            public_key: keys[member_num as usize].0.clone(),
            name: format!("member-{:04}", member_num),
            governance_voting_power: 1,
            consensus_voting_power: 1,
            governance_delegations: Some(format!("member-{:04}", delegatee_member_num)),
            consensus_delegations: None,
        }
    }

    #[test]
    fn basic_validator_set1() {
        setup_test();
        let keys = (0..4)
            .into_iter()
            .map(|i| generate_keypair(format!("{}", i)))
            .collect::<Vec<_>>();
        let members = vec![
            create_member_with_consensus_delegation(keys.clone(), 0, 3),
            create_member_with_consensus_delegation(keys.clone(), 1, 3),
            create_member_with_consensus_delegation(keys.clone(), 2, 3),
            create_member(keys.clone(), 3),
        ];
        let genesis_header = BlockHeader {
            author: PublicKey::zero(),
            prev_block_finalization_proof: Vec::new(),
            previous_hash: Hash256::zero(),
            height: 0,
            timestamp: 0,
            commit_merkle_root: Hash256::zero(),
            repository_merkle_root: Hash256::zero(),
            validator_set: members
                .iter()
                .map(|member| (member.public_key.clone(), member.consensus_voting_power))
                .collect::<Vec<_>>(),
            version: "0.1.0".to_string(),
        };
        let genesis_info = GenesisInfo {
            header: genesis_header.clone(),
            genesis_proof: keys
                .iter()
                .map(|(_, private_key)| TypedSignature::sign(&genesis_header, private_key).unwrap())
                .collect::<Vec<_>>(),
            chain_name: "test-chain".to_string(),
        };
        let reserved_state = ReservedState {
            genesis_info,
            members,
            consensus_leader_order: vec!["member-0003".to_string()],
            version: "0.1.0".to_string(),
        };
        assert_eq!(
            reserved_state.get_validator_set().unwrap(),
            vec![(keys[3].0.clone(), 4),]
        );
    }

    #[test]
    fn basic_validator_set2() {
        setup_test();
        let keys = (0..4)
            .into_iter()
            .map(|i| generate_keypair(format!("{}", i)))
            .collect::<Vec<_>>();
        let members = vec![
            create_member_with_consensus_delegation(keys.clone(), 0, 1),
            create_member(keys.clone(), 1),
            create_member_with_consensus_delegation(keys.clone(), 2, 3),
            create_member(keys.clone(), 3),
        ];
        let genesis_header = BlockHeader {
            author: PublicKey::zero(),
            prev_block_finalization_proof: Vec::new(),
            previous_hash: Hash256::zero(),
            height: 0,
            timestamp: 0,
            commit_merkle_root: Hash256::zero(),
            repository_merkle_root: Hash256::zero(),
            validator_set: members
                .iter()
                .map(|member| (member.public_key.clone(), member.consensus_voting_power))
                .collect::<Vec<_>>(),
            version: "0.1.0".to_string(),
        };
        let genesis_info = GenesisInfo {
            header: genesis_header.clone(),
            genesis_proof: keys
                .iter()
                .map(|(_, private_key)| TypedSignature::sign(&genesis_header, private_key).unwrap())
                .collect::<Vec<_>>(),
            chain_name: "test-chain".to_string(),
        };
        let reserved_state = ReservedState {
            genesis_info,
            members,
            consensus_leader_order: vec!["member-0001".to_string(), "member-0003".to_string()],
            version: "0.1.0".to_string(),
        };
        assert_eq!(
            reserved_state.get_validator_set().unwrap(),
            vec![(keys[1].0.clone(), 2), (keys[3].0.clone(), 2),]
        );
    }

    #[test]
    fn basic_governance_set1() {
        setup_test();
        let keys = (0..4)
            .into_iter()
            .map(|i| generate_keypair(format!("{}", i)))
            .collect::<Vec<_>>();
        let members = vec![
            create_member_with_governance_delegation(keys.clone(), 0, 3),
            create_member_with_governance_delegation(keys.clone(), 1, 3),
            create_member_with_governance_delegation(keys.clone(), 2, 3),
            create_member(keys.clone(), 3),
        ];
        let genesis_header = BlockHeader {
            author: PublicKey::zero(),
            prev_block_finalization_proof: Vec::new(),
            previous_hash: Hash256::zero(),
            height: 0,
            timestamp: 0,
            commit_merkle_root: Hash256::zero(),
            repository_merkle_root: Hash256::zero(),
            validator_set: members
                .iter()
                .map(|member| (member.public_key.clone(), member.consensus_voting_power))
                .collect::<Vec<_>>(),
            version: "0.1.0".to_string(),
        };
        let genesis_info = GenesisInfo {
            header: genesis_header.clone(),
            genesis_proof: keys
                .iter()
                .map(|(_, private_key)| TypedSignature::sign(&genesis_header, private_key).unwrap())
                .collect::<Vec<_>>(),
            chain_name: "test-chain".to_string(),
        };
        let reserved_state = ReservedState {
            genesis_info,
            members,
            consensus_leader_order: (0..4)
                .into_iter()
                .map(|i| format!("member-{:04}", i))
                .collect::<Vec<_>>(),
            version: "0.1.0".to_string(),
        };
        assert_eq!(
            reserved_state.get_governance_set().unwrap(),
            vec![(keys[3].0.clone(), 4),]
        );
    }

    #[test]
    fn basic_governance_set2() {
        setup_test();
        let keys = (0..4)
            .into_iter()
            .map(|i| generate_keypair(format!("{}", i)))
            .collect::<Vec<_>>();
        let members = vec![
            create_member_with_governance_delegation(keys.clone(), 0, 1),
            create_member(keys.clone(), 1),
            create_member_with_governance_delegation(keys.clone(), 2, 3),
            create_member(keys.clone(), 3),
        ];
        let genesis_header = BlockHeader {
            author: PublicKey::zero(),
            prev_block_finalization_proof: Vec::new(),
            previous_hash: Hash256::zero(),
            height: 0,
            timestamp: 0,
            commit_merkle_root: Hash256::zero(),
            repository_merkle_root: Hash256::zero(),
            validator_set: members
                .iter()
                .map(|member| (member.public_key.clone(), member.consensus_voting_power))
                .collect::<Vec<_>>(),
            version: "0.1.0".to_string(),
        };
        let genesis_info = GenesisInfo {
            header: genesis_header.clone(),
            genesis_proof: keys
                .iter()
                .map(|(_, private_key)| TypedSignature::sign(&genesis_header, private_key).unwrap())
                .collect::<Vec<_>>(),
            chain_name: "test-chain".to_string(),
        };
        let reserved_state = ReservedState {
            genesis_info,
            members,
            consensus_leader_order: (0..4)
                .into_iter()
                .map(|i| format!("member-{:04}", i))
                .collect::<Vec<_>>(),
            version: "0.1.0".to_string(),
        };
        assert_eq!(
            reserved_state
                .get_governance_set()
                .unwrap()
                .into_iter()
                .collect::<HashSet<_>>(),
            vec![(keys[1].0.clone(), 2), (keys[3].0.clone(), 2),]
                .into_iter()
                .collect::<HashSet<_>>()
        );
    }
}
