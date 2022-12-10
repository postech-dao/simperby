use common::{BlockHeight, Timestamp};
use simperby_common::{
    self as common,
    crypto::{Hash256, PublicKey},
    BlockHeader, VotingPower,
};
use test_suite::*;
use simperby_consensus::{Consensus, ProgressResult};
use simperby_network::{
    primitives::Storage, storage::StorageImpl};
use simperby_test_suite as test_suite;
use vetomint2::ConsensusParams;
use log::debug;

fn get_initial_block_header(validator_set: Vec<(PublicKey, VotingPower)>) -> BlockHeader {
    BlockHeader {
        author: PublicKey::zero(),
        prev_block_finalization_proof: Vec::new(),
        previous_hash: Hash256::zero(),
        height: 0 as BlockHeight,
        timestamp: 0 as Timestamp,
        commit_merkle_root: Hash256::zero(),
        repository_merkle_root: Hash256::zero(),
        validator_set,
        version: "0.0.0".to_string(),
    }
}

async fn create_storage(dirname: String) -> StorageImpl {
    StorageImpl::create(&dirname).await.unwrap();
    StorageImpl::open(&dirname).await.unwrap()
}

#[tokio::test]
async fn single_server_propose_1() {
    setup_test();
    let network_id = format!("consensus_{}", "single_server_propose_1");
    let dms_key = network_id.clone();

    let voting_powers = vec![1, 1, 1, 1, 1];
    let num_nodes = voting_powers.len();
    let params = ConsensusParams { timeout_ms: 60 * 1_000 , repeat_round_for_first_leader: 100 };
    let round_zero_timestamp = get_timestamp();

    let (server_config, other_configs, peers) =
        setup_server_client_nodes(network_id.clone(), num_nodes - 1).await;
    let mut pubkeys = vec![server_config.public_key.clone()];
    pubkeys.extend(
        other_configs
            .iter()
            .cloned()
            .map(|config| config.public_key)
    );

    let validator_set: Vec<(PublicKey, u64)> = pubkeys
        .iter()
        .cloned()
        .zip(voting_powers.iter().cloned())
        .collect();
    let block_header = get_initial_block_header(validator_set);

    let mut server_node = Consensus::new(
        create_test_dms(server_config.clone(), dms_key.clone(), peers.clone()).await,
        create_storage(create_temp_dir()).await,
        block_header.clone(),
        params.clone(),
        round_zero_timestamp,
        Some(server_config.private_key),
    )
    .await
    .unwrap();
    let mut other_nodes = Vec::new();
    for index in 0..(num_nodes - 1) {
        let consensus = Consensus::new(
            create_test_dms(
                other_configs[index].clone(),
                dms_key.clone(),
                peers.clone(),
            )
            .await,
            create_storage(create_temp_dir()).await,
            block_header.clone(),
            params.clone(),
            round_zero_timestamp,
            Some(other_configs[index].private_key.clone()),
        )
        .await
        .unwrap();
        other_nodes.push(consensus);
    }

    // Make a block to propose
    let dummy_block_hash = Hash256::hash("dummy_block");
    server_node.register_verified_block_hash(dummy_block_hash).await.unwrap();
    for other_node in &mut other_nodes {
        other_node.register_verified_block_hash(dummy_block_hash).await.unwrap();
    }

    // Make consensus
    server_node.set_proposal_candidate(dummy_block_hash, get_timestamp()).await.unwrap();
    let result = server_node.progress(get_timestamp()).await.unwrap();
    debug!("progress result: {:?}", result);
    assert!(result.iter().any(|r| match r {
        ProgressResult::Proposed(..) => true,
        _ => false,
    }));
}
