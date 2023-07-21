use crate::*;

pub fn generate_fi(member_number: usize) -> (FinalizationInfo, Vec<(PublicKey, PrivateKey)>) {
    let (rs, keys) = generate_standard_genesis(member_number);
    let fi = FinalizationInfo {
        header: rs.genesis_info.header.clone(),
        commit_hash: CommitHash::zero(),
        proof: rs.genesis_info.genesis_proof.clone(),
        reserved_state: rs,
    };
    (fi, keys)
}

/// Generates a standard test chain config returning the genesis reserved-state
/// and the associated key pairs of the members.
pub fn generate_standard_genesis(
    member_number: usize,
) -> (ReservedState, Vec<(PublicKey, PrivateKey)>) {
    let keys = (0..member_number)
        .map(|i| generate_keypair(format!("{i}")))
        .collect::<Vec<_>>();
    let members = keys
        .iter()
        .enumerate()
        .map(|(i, (public_key, _))| Member {
            public_key: public_key.clone(),
            // lexicographically ordered
            name: format!("member-{i:04}"),
            governance_voting_power: 1,
            consensus_voting_power: 1,
            governance_delegatee: None,
            consensus_delegatee: None,
            expelled: false,
        })
        .collect::<Vec<_>>();
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
        version: SIMPERBY_CORE_PROTOCOL_VERSION.to_string(),
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
    (
        ReservedState {
            genesis_info,
            members,
            consensus_leader_order: (0..member_number)
                .map(|i| format!("member-{i:04}"))
                .collect::<Vec<_>>(),
            version: SIMPERBY_CORE_PROTOCOL_VERSION.to_string(),
        },
        keys,
    )
}

/// Generates a standard test chain config returning the genesis reserved-state
/// and the associated key pairs of the members.
///
/// member-0000 delegates to member-0002 for governance and consensus if `governance` is true and
/// member-0000 delegates to member-0002 for consensus only if `governance` is false.
pub fn generate_delegated_genesis(
    member_number: usize,
    governance: bool,
) -> (ReservedState, Vec<(PublicKey, PrivateKey)>) {
    let keys = (0..member_number)
        .map(|i| generate_keypair(format!("{i}")))
        .collect::<Vec<_>>();
    // member-0000 delegates to member-0002
    let members = keys
        .iter()
        .enumerate()
        .map(|(i, (public_key, _))| Member {
            public_key: public_key.clone(),
            // lexicographically ordered
            name: format!("member-{i:04}"),
            governance_voting_power: 1,
            consensus_voting_power: 1,
            governance_delegatee: if i == 0 && governance {
                Some("member-0002".into())
            } else {
                None
            },
            consensus_delegatee: if i == 0 {
                Some("member-0002".into())
            } else {
                None
            },
            expelled: false,
        })
        .collect::<Vec<_>>();
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
    (
        ReservedState {
            genesis_info,
            members,
            consensus_leader_order: (1..member_number)
                .map(|i| format!("member-{i:04}"))
                .collect::<Vec<_>>(),
            version: "0.1.0".to_string(),
        },
        keys,
    )
}
