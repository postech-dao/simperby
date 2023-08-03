use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
        let validator_set = self
            .members
            .iter()
            .map(|member| {
                let name = member
                    .consensus_delegatee
                    .as_ref()
                    .map_or_else(|| member.name.as_str(), |name| name.as_str());
                let public_key = self.query_public_key(&name.to_string()).ok_or_else(|| {
                    format!("the public key of {name} is not found in the reserved state.")
                })?;
                Ok((public_key, member.consensus_voting_power))
            })
            .try_fold(
                BTreeMap::new(),
                |mut map, result: Result<(crypto::PublicKey, u64), String>| {
                    let (public_key, voting_power) = result?;
                    map.entry(public_key)
                        .and_modify(|v| *v += voting_power)
                        .or_insert(voting_power);
                    Ok::<BTreeMap<crypto::PublicKey, u64>, String>(map)
                },
            )?
            .into_iter()
            .collect::<Vec<_>>();

        Ok(validator_set)
    }

    pub fn get_governance_set(&self) -> Result<Vec<(PublicKey, VotingPower)>, String> {
        let governance_set = self
            .members
            .iter()
            .filter(|member| !member.expelled)
            .map(|member| {
                let name = member
                    .governance_delegatee
                    .as_ref()
                    .map_or_else(|| member.name.as_str(), |name| name.as_str());
                let public_key = self.query_public_key(&name.to_string()).ok_or_else(|| {
                    format!("the public key of {name} is not found in the reserved state.")
                })?;
                Ok((public_key, member.consensus_voting_power))
            })
            .try_fold(
                BTreeMap::new(),
                |mut map, result: Result<(crypto::PublicKey, u64), String>| {
                    let (public_key, voting_power) = result?;
                    map.entry(public_key)
                        .and_modify(|v| *v += voting_power)
                        .or_insert(voting_power);
                    Ok::<BTreeMap<crypto::PublicKey, u64>, String>(map)
                },
            )?
            .into_iter()
            .collect::<Vec<_>>();

        Ok(governance_set)
    }

    pub fn apply_delegate(&mut self, tx: &TxDelegate) -> Result<Self, String> {
        if tx.data.delegator == tx.data.delegatee {
            return Err(format!(
                "delegator and delegatee are the same: {}",
                tx.data.delegator
            ));
        }
        if let Some(key) = self.query_public_key(&tx.data.delegator) {
            if &key != tx.proof.signer() {
                return Err(
                    "the key used for the proof does not match the key in the reserved state"
                        .to_string(),
                );
            }
        }
        if !self.is_member(&tx.data.delegatee) {
            return Err("delegatee not found by name".to_string());
        }
        if tx.proof.verify(&tx.data).is_err() {
            return Err("delegation proof verification failed".to_string());
        }
        for delegator in &mut self.members {
            if delegator.name == tx.data.delegator {
                if tx.data.governance {
                    delegator.governance_delegatee = Some(tx.data.delegatee.clone());
                    delegator.consensus_delegatee = Some(tx.data.delegatee.clone());
                } else {
                    delegator.consensus_delegatee = Some(tx.data.delegatee.clone());
                }
                break;
            }
        }
        Ok(self.clone())
    }

    pub fn apply_undelegate(&mut self, tx: &TxUndelegate) -> Result<Self, String> {
        if let Some(key) = self.query_public_key(&tx.data.delegator) {
            if &key != tx.proof.signer() {
                return Err(
                    "the key used for the proof does not match the key in the reserved state"
                        .to_string(),
                );
            }
        }
        if tx.proof.verify(&tx.data).is_err() {
            return Err("delegation proof verification failed".to_string());
        }
        for delegator in &mut self.members {
            if delegator.name == tx.data.delegator {
                if delegator.consensus_delegatee.is_some() {
                    delegator.consensus_delegatee = None;
                    delegator.governance_delegatee = None;
                    break;
                } else {
                    return Err("consensus delegatee is not set".to_string());
                }
            }
        }
        Ok(self.clone())
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

    pub fn is_member(&self, name: &MemberName) -> bool {
        for member in &self.members {
            if &member.name == name {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;
    use simperby_test_suite::setup_test;
    use std::collections::HashSet;

    fn create_member(keys: Vec<(PublicKey, PrivateKey)>, member_num: u8) -> Member {
        Member {
            public_key: keys[member_num as usize].0.clone(),
            name: format!("member-{member_num:04}"),
            governance_voting_power: 1,
            consensus_voting_power: 1,
            governance_delegatee: None,
            consensus_delegatee: None,
            expelled: false,
        }
    }

    fn create_expelled_member(keys: Vec<(PublicKey, PrivateKey)>, member_num: u8) -> Member {
        Member {
            public_key: keys[member_num as usize].0.clone(),
            name: format!("member-{member_num:04}"),
            governance_voting_power: 1,
            consensus_voting_power: 1,
            governance_delegatee: None,
            consensus_delegatee: None,
            expelled: true,
        }
    }

    fn create_member_with_consensus_delegation(
        keys: Vec<(PublicKey, PrivateKey)>,
        member_num: u8,
        delegatee_member_num: u8,
    ) -> Member {
        Member {
            public_key: keys[member_num as usize].0.clone(),
            name: format!("member-{member_num:04}"),
            governance_voting_power: 1,
            consensus_voting_power: 1,
            governance_delegatee: None,
            consensus_delegatee: Some(format!("member-{delegatee_member_num:04}")),
            expelled: false,
        }
    }

    fn create_member_with_governance_delegation(
        keys: Vec<(PublicKey, PrivateKey)>,
        member_num: u8,
        delegatee_member_num: u8,
    ) -> Member {
        Member {
            public_key: keys[member_num as usize].0.clone(),
            name: format!("member-{member_num:04}"),
            governance_voting_power: 1,
            consensus_voting_power: 1,
            governance_delegatee: Some(format!("member-{delegatee_member_num:04}")),
            consensus_delegatee: None,
            expelled: false,
        }
    }

    fn create_expelled_member_with_governance_delegation(
        keys: Vec<(PublicKey, PrivateKey)>,
        member_num: u8,
        delegatee_member_num: u8,
    ) -> Member {
        Member {
            public_key: keys[member_num as usize].0.clone(),
            name: format!("member-{member_num:04}"),
            governance_voting_power: 1,
            consensus_voting_power: 1,
            governance_delegatee: Some(format!("member-{delegatee_member_num:04}")),
            consensus_delegatee: None,
            expelled: true,
        }
    }

    #[test]
    fn basic_validator_set1() {
        setup_test();
        let keys = (0..4)
            .map(|i| generate_keypair(format!("{i}")))
            .collect::<Vec<_>>();
        let members = vec![
            create_member_with_consensus_delegation(keys.clone(), 0, 3),
            create_member_with_consensus_delegation(keys.clone(), 1, 3),
            create_member_with_consensus_delegation(keys.clone(), 2, 3),
            create_member(keys.clone(), 3),
        ];
        let genesis_header = BlockHeader {
            author: PublicKey::zero(),
            prev_block_finalization_proof: FinalizationProof::genesis(),
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
            genesis_proof: FinalizationProof {
                round: 0,
                signatures: keys
                    .iter()
                    .map(|(_, private_key)| {
                        TypedSignature::sign(
                            &FinalizationSignTarget {
                                block_hash: genesis_header.to_hash256(),
                                round: 0,
                            },
                            private_key,
                        )
                        .unwrap()
                    })
                    .collect::<Vec<_>>(),
            },
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

    #[ignore]
    #[test]
    fn basic_validator_set2() {
        setup_test();
        let keys = (0..4)
            .map(|i| generate_keypair(format!("{i}")))
            .collect::<Vec<_>>();
        let members = vec![
            create_member_with_consensus_delegation(keys.clone(), 0, 1),
            create_member(keys.clone(), 1),
            create_member_with_consensus_delegation(keys.clone(), 2, 3),
            create_member(keys.clone(), 3),
        ];
        let genesis_header = BlockHeader {
            author: PublicKey::zero(),
            prev_block_finalization_proof: FinalizationProof::genesis(),
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
            genesis_proof: FinalizationProof {
                round: 0,
                signatures: keys
                    .iter()
                    .map(|(_, private_key)| {
                        TypedSignature::sign(
                            &FinalizationSignTarget {
                                block_hash: genesis_header.to_hash256(),
                                round: 0,
                            },
                            private_key,
                        )
                        .unwrap()
                    })
                    .collect::<Vec<_>>(),
            },
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
            .map(|i| generate_keypair(format!("{i}")))
            .collect::<Vec<_>>();
        let members = vec![
            create_member_with_governance_delegation(keys.clone(), 0, 3),
            create_member_with_governance_delegation(keys.clone(), 1, 3),
            create_member_with_governance_delegation(keys.clone(), 2, 3),
            create_member(keys.clone(), 3),
        ];
        let genesis_header = BlockHeader {
            author: PublicKey::zero(),
            prev_block_finalization_proof: FinalizationProof::genesis(),
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
            genesis_proof: FinalizationProof {
                round: 0,
                signatures: keys
                    .iter()
                    .map(|(_, private_key)| {
                        TypedSignature::sign(
                            &FinalizationSignTarget {
                                block_hash: genesis_header.to_hash256(),
                                round: 0,
                            },
                            private_key,
                        )
                        .unwrap()
                    })
                    .collect::<Vec<_>>(),
            },
            chain_name: "test-chain".to_string(),
        };
        let reserved_state = ReservedState {
            genesis_info,
            members,
            consensus_leader_order: (0..4).map(|i| format!("member-{i:04}")).collect::<Vec<_>>(),
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
            .map(|i| generate_keypair(format!("{i}")))
            .collect::<Vec<_>>();
        let members = vec![
            create_member_with_governance_delegation(keys.clone(), 0, 1),
            create_member(keys.clone(), 1),
            create_member_with_governance_delegation(keys.clone(), 2, 3),
            create_member(keys.clone(), 3),
        ];
        let genesis_header = BlockHeader {
            author: PublicKey::zero(),
            prev_block_finalization_proof: FinalizationProof::genesis(),
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
            genesis_proof: FinalizationProof {
                round: 0,
                signatures: keys
                    .iter()
                    .map(|(_, private_key)| {
                        TypedSignature::sign(
                            &FinalizationSignTarget {
                                block_hash: genesis_header.to_hash256(),
                                round: 0,
                            },
                            private_key,
                        )
                        .unwrap()
                    })
                    .collect::<Vec<_>>(),
            },
            chain_name: "test-chain".to_string(),
        };
        let reserved_state = ReservedState {
            genesis_info,
            members,
            consensus_leader_order: (0..4).map(|i| format!("member-{i:04}")).collect::<Vec<_>>(),
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

    #[test]
    fn governance_set_with_expelled_members() {
        setup_test();
        let keys = (0..5)
            .map(|i| generate_keypair(format!("{i}")))
            .collect::<Vec<_>>();
        let members = vec![
            create_expelled_member_with_governance_delegation(keys.clone(), 0, 1),
            create_member(keys.clone(), 1),
            create_expelled_member_with_governance_delegation(keys.clone(), 2, 3),
            create_member(keys.clone(), 3),
            create_member_with_governance_delegation(keys.clone(), 4, 1),
        ];
        let genesis_header = BlockHeader {
            author: PublicKey::zero(),
            prev_block_finalization_proof: FinalizationProof::genesis(),
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
            genesis_proof: FinalizationProof {
                round: 0,
                signatures: keys
                    .iter()
                    .map(|(_, private_key)| {
                        TypedSignature::sign(
                            &FinalizationSignTarget {
                                block_hash: genesis_header.to_hash256(),
                                round: 0,
                            },
                            private_key,
                        )
                        .unwrap()
                    })
                    .collect::<Vec<_>>(),
            },
            chain_name: "test-chain".to_string(),
        };
        let reserved_state = ReservedState {
            genesis_info,
            members,
            consensus_leader_order: (0..5).map(|i| format!("member-{i:04}")).collect::<Vec<_>>(),
            version: "0.1.0".to_string(),
        };
        assert_eq!(
            reserved_state
                .get_governance_set()
                .unwrap()
                .into_iter()
                .collect::<HashSet<_>>(),
            vec![(keys[1].0.clone(), 2), (keys[3].0.clone(), 1),]
                .into_iter()
                .collect::<HashSet<_>>()
        );
    }

    #[test]
    fn test_apply_delegate_on_governance_and_consensus_success() {
        // given
        setup_test();
        let (mut reserved_state, keys) = generate_standard_genesis(4);

        // delegator: member-0000, delegatee: member-0002
        let delegator = reserved_state.members[0].clone();
        let delegator_private_key = keys[0].1.clone();
        let delegatee = reserved_state.members[2].clone();

        // when
        let data: DelegationTransactionData = DelegationTransactionData {
            delegator: delegator.name,
            delegatee: delegatee.name.clone(),
            governance: true,
            block_height: 0,
            timestamp: 0,
            chain_name: reserved_state.genesis_info.chain_name.clone(),
        };
        let proof = TypedSignature::sign(&data, &delegator_private_key).unwrap();

        let tx = TxDelegate { data, proof };
        let new_state = reserved_state.apply_delegate(&tx);

        // then
        assert!(new_state.is_ok());
        let new_state = new_state.unwrap();
        let new_state_validator_set = new_state.get_validator_set();
        let new_state_governance_set = new_state.get_governance_set();

        assert_eq!(
            new_state.members[0].governance_delegatee.as_ref().unwrap(),
            &delegatee.name
        );

        assert_eq!(
            new_state.members[0].consensus_delegatee.as_ref().unwrap(),
            &delegatee.name
        );

        assert_eq!(
            new_state_validator_set
                .unwrap()
                .into_iter()
                .find(|v| v.0 == delegatee.public_key)
                .unwrap()
                .1,
            2
        );

        assert_eq!(
            new_state_governance_set
                .unwrap()
                .into_iter()
                .find(|v| v.0 == delegatee.public_key)
                .unwrap()
                .1,
            2
        );
    }

    #[test]
    fn test_apply_delegate_on_consensus_success_with_expelled_members() {
        // given
        setup_test();
        let (mut reserved_state, keys) = generate_standard_genesis(5); // Increased to 5 members

        // Create expelled members
        reserved_state.members[0] =
            create_expelled_member_with_governance_delegation(keys.clone(), 0, 2);
        reserved_state.members[1] =
            create_expelled_member_with_governance_delegation(keys.clone(), 1, 2);
        reserved_state.members[3] = create_expelled_member(keys.clone(), 3);
        reserved_state.members[4] =
            create_expelled_member_with_governance_delegation(keys.clone(), 4, 2);

        let delegator = reserved_state.members[0].clone();
        let delegator_private_key = keys[0].1.clone();
        let existing_delegatee = reserved_state.members[2].clone();
        let non_existing_delegatee = reserved_state.members[3].clone();

        // when
        let data: DelegationTransactionData = DelegationTransactionData {
            delegator: delegator.name,
            delegatee: existing_delegatee.name.clone(),
            governance: true,
            block_height: 0,
            timestamp: 0,
            chain_name: reserved_state.genesis_info.chain_name.clone(),
        };
        let proof = TypedSignature::sign(&data, &delegator_private_key).unwrap();

        let tx = TxDelegate { data, proof };
        let new_state = reserved_state.apply_delegate(&tx);

        // then
        assert!(new_state.is_ok());
        let new_state = new_state.unwrap();
        let new_state_governance_set = new_state.get_governance_set().unwrap();

        assert_eq!(
            new_state_governance_set
                .iter()
                .find(|v| v.0 == existing_delegatee.public_key)
                .unwrap()
                .1,
            1
        );

        // assert that non_existing_delegatee should not exists on the governance_set
        assert!(!new_state_governance_set
            .iter()
            .any(|v| v.0 == non_existing_delegatee.public_key));

        // make sure expelled members are not part of the governance or validator sets
        assert!(!new_state_governance_set
            .iter()
            .any(|v| v.0 == keys[1].0 || v.0 == keys[4].0));
    }

    #[test]
    fn test_apply_delegate_on_consensus_success() {
        // given
        setup_test();
        let (mut state, keys) = generate_standard_genesis(3);

        // delegator: member-0000, delegatee: member-0002
        let delegator = state.members[0].clone();
        let delegator_private_key = keys[0].1.clone();
        let delegatee = state.members[2].clone();

        // when
        let data: DelegationTransactionData = DelegationTransactionData {
            delegator: delegator.name,
            delegatee: delegatee.name.clone(),
            governance: false,
            block_height: 0,
            timestamp: 0,
            chain_name: state.genesis_info.chain_name.clone(),
        };
        let proof = TypedSignature::sign(&data, &delegator_private_key).unwrap();

        let tx = TxDelegate { data, proof };

        let new_state = state.apply_delegate(&tx);

        // then
        assert!(new_state.is_ok());
        let new_state = new_state.unwrap();
        let new_state_validator_set = new_state.get_validator_set();
        let new_state_governance_set = new_state.get_governance_set();

        assert_eq!(
            new_state.members[0].consensus_delegatee.as_ref().unwrap(),
            &delegatee.name
        );

        assert_eq!(new_state.members[0].governance_delegatee, None);

        assert_eq!(
            new_state_validator_set
                .unwrap()
                .into_iter()
                .find(|v| v.0 == delegatee.public_key)
                .unwrap()
                .1,
            2
        );

        assert_eq!(
            new_state_governance_set
                .unwrap()
                .into_iter()
                .find(|v| v.0 == delegatee.public_key)
                .unwrap()
                .1,
            1
        );
    }

    #[test]
    fn test_apply_delegate_on_consensus_failure() {
        let (mut state, keys) = generate_standard_genesis(1);

        let delegator = state.members[0].clone();
        let delegator_private_key = keys[0].1.clone();

        let data: DelegationTransactionData = DelegationTransactionData {
            // delegator and delegatee are the same
            delegator: delegator.name.clone(),
            delegatee: delegator.name,
            governance: false,
            block_height: 0,
            timestamp: 0,
            chain_name: state.genesis_info.chain_name.clone(),
        };
        let proof = TypedSignature::sign(&data, &delegator_private_key).unwrap();

        let tx = TxDelegate { data, proof };

        let new_state = state.apply_delegate(&tx);

        if new_state.is_ok() {
            panic!("it must fail when the delegator and the delegatee are the same");
        }
    }

    #[test]
    fn test_apply_undelegate_on_governance_and_consensus_success() {
        // given
        setup_test();
        let (mut reserved_state, keys) = generate_delegated_genesis(4, true);

        // delegator: member-0000, delegatee: member-0002
        let delegator = reserved_state.members[0].clone();
        let delegator_private_key = keys[0].1.clone();
        let delegatee = reserved_state.members[2].clone();

        // when
        let data = UndelegationTransactionData {
            delegator: delegator.name,
            block_height: 0,
            timestamp: 0,
            chain_name: reserved_state.genesis_info.chain_name.clone(),
        };

        let proof = TypedSignature::sign(&data, &delegator_private_key).unwrap();

        let tx = TxUndelegate { data, proof };
        let undelegated_state = reserved_state.apply_undelegate(&tx);

        // then
        assert!(undelegated_state.is_ok());
        let undelegated_state = undelegated_state.unwrap();
        let new_state_validator_set = undelegated_state.get_validator_set();
        let new_state_governance_set = undelegated_state.get_governance_set();

        assert_eq!(undelegated_state.members[0].consensus_delegatee, None);
        assert_eq!(undelegated_state.members[0].governance_delegatee, None);

        assert_eq!(
            new_state_validator_set
                .unwrap()
                .into_iter()
                .find(|v| v.0 == delegatee.public_key)
                .unwrap()
                .1,
            1
        );

        assert_eq!(
            new_state_governance_set
                .unwrap()
                .into_iter()
                .find(|v| v.0 == delegatee.public_key)
                .unwrap()
                .1,
            1
        );
    }

    #[test]
    fn test_apply_undelegate_on_consensus_success() {
        // given
        setup_test();
        let (mut reserved_state, keys) = generate_delegated_genesis(4, false);

        // delegator: member-0000, delegatee: member-0002
        let delegator = reserved_state.members[0].clone();
        let delegator_private_key = keys[0].1.clone();
        let delegatee = reserved_state.members[2].clone();

        // when
        let data = UndelegationTransactionData {
            delegator: delegator.name,
            block_height: 0,
            timestamp: 0,
            chain_name: reserved_state.genesis_info.chain_name.clone(),
        };

        let proof = TypedSignature::sign(&data, &delegator_private_key).unwrap();

        let tx = TxUndelegate { data, proof };
        let undelegated_state = reserved_state.apply_undelegate(&tx);

        // then
        assert!(undelegated_state.is_ok());
        let undelegated_state = undelegated_state.unwrap();
        let new_state_validator_set = undelegated_state.get_validator_set();
        let new_state_governance_set = undelegated_state.get_governance_set();

        assert_eq!(undelegated_state.members[0].consensus_delegatee, None);
        assert_eq!(undelegated_state.members[0].governance_delegatee, None);

        assert_eq!(
            new_state_validator_set
                .unwrap()
                .into_iter()
                .find(|v| v.0 == delegatee.public_key)
                .unwrap()
                .1,
            1
        );

        assert_eq!(
            new_state_governance_set
                .unwrap()
                .into_iter()
                .find(|v| v.0 == delegatee.public_key)
                .unwrap()
                .1,
            1
        );
    }
}
