use common::{BlockHeight, Timestamp};
use log::debug;
use simperby_common::{
    self as common,
    crypto::{Hash256, PublicKey},
    BlockHeader, VotingPower,
};
use simperby_consensus::{Consensus, ConsensusMessage, ProgressResult};
use simperby_network::{
    primitives::Storage, storage::StorageImpl, NetworkConfig, SharedKnownPeers,
};
use simperby_test_suite as test_suite;
use std::iter::once;
use test_suite::*;
use vetomint2::ConsensusParams;

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

fn configs_to_block_header(
    configs: Vec<&NetworkConfig>,
    voting_powers: Vec<VotingPower>,
) -> BlockHeader {
    let pubkeys = configs.iter().map(|config| config.public_key.clone());
    let validator_set: Vec<(PublicKey, u64)> = pubkeys.zip(voting_powers.iter().cloned()).collect();
    get_initial_block_header(validator_set)
}

async fn create_storage(dirname: String) -> StorageImpl {
    StorageImpl::create(&dirname).await.unwrap();
    StorageImpl::open(&dirname).await.unwrap()
}

fn get_network_id_and_dms_key(testname: &str) -> (String, String) {
    let network_id = format!("consensus_{}", testname);
    let dms_key = network_id.clone();
    (network_id, dms_key)
}

#[tokio::test]
async fn single_server_propose_1() {
    setup_test();
    let (network_id, dms_key) = get_network_id_and_dms_key("single_server_propose_1");

    let voting_powers = vec![1, 1, 1, 1, 1];
    let num_nodes = voting_powers.len();
    let params = ConsensusParams {
        timeout_ms: 60 * 1_000,
        repeat_round_for_first_leader: 100,
    };
    let round_zero_timestamp = get_timestamp();

    let (server_config, other_configs, peers) =
        setup_server_client_nodes(network_id.clone(), num_nodes - 1).await;

    let block_header = configs_to_block_header(
        once(&server_config).chain(&other_configs).collect(),
        voting_powers,
    );

    let mut server_node = Consensus::new(
        create_test_dms(
            server_config.clone(),
            dms_key.clone(),
            SharedKnownPeers::new_static(vec![]),
        )
        .await,
        create_storage(create_temp_dir()).await,
        block_header.clone(),
        params.clone(),
        round_zero_timestamp,
        Some(server_config.private_key),
    )
    .await
    .unwrap();
    let mut other_nodes = Vec::new();
    for config in other_configs {
        let consensus = Consensus::new(
            create_test_dms(config.clone(), dms_key.clone(), peers.clone()).await,
            create_storage(create_temp_dir()).await,
            block_header.clone(),
            params.clone(),
            round_zero_timestamp,
            Some(config.private_key.clone()),
        )
        .await
        .unwrap();
        other_nodes.push(consensus);
    }

    // Make a block to propose
    let dummy_block_hash = Hash256::hash("dummy_block");
    server_node
        .register_verified_block_hash(dummy_block_hash)
        .await
        .unwrap();
    for other_node in &mut other_nodes {
        other_node
            .register_verified_block_hash(dummy_block_hash)
            .await
            .unwrap();
    }

    // Make consensus
    // Initial block candidate is set to 0 by default, so we progress without setting a new candidate.
    let result = server_node.progress(get_timestamp()).await.unwrap();
    debug!("progress result: {:?}", result);
    assert!(result
        .iter()
        .any(|r| matches!(r, ProgressResult::Proposed(..))));

    let serve_task = tokio::spawn(async { server_node.serve(3_000).await });
    for other_node in &mut other_nodes {
        other_node.fetch().await.unwrap();
        let result = other_node.progress(get_timestamp()).await.unwrap();
        assert!(result
            .iter()
            .any(|r| matches!(r, ProgressResult::NonNilPreVoted(..))));
    }
    let server_node = serve_task.await.unwrap().unwrap();
    let messages = server_node.read_messages().await.unwrap();
    let prevotes = messages
        .iter()
        .filter(|message| matches!(message, ConsensusMessage::NonNilPreVoted(..)));
    assert_eq!(prevotes.count(), num_nodes)
}
