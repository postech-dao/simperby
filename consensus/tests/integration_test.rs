use common::{
    crypto::{Signature, TypedSignature},
    BlockHeight, FinalizationProof, PrivateKey, Timestamp,
};
#[allow(unused_imports)]
use log::debug;
use simperby_common::{
    self as common,
    crypto::{Hash256, PublicKey},
    BlockHeader, VotingPower,
};
use simperby_consensus::{Consensus, ConsensusMessage, Precommit, Prevote, ProgressResult};
use simperby_network::{
    primitives::Storage, storage::StorageImpl, NetworkConfig, SharedKnownPeers,
};
use simperby_test_suite as test_suite;
use std::fmt::Debug;
use std::iter::once;
use test_suite::*;
use vetomint::ConsensusParams;

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
    let network_id = format!("consensus_{testname}");
    let dms_key = network_id.clone();
    (network_id, dms_key)
}

/// This may panic.
fn assert_eq_unordered<T: Eq + PartialEq + Debug>(expected: &Vec<T>, actual: &Vec<T>) {
    let result = expected.len() == actual.len()
        && actual.iter().all(|a| expected.contains(a))
        && expected.iter().all(|e| actual.contains(e));
    if !result {
        panic!("assert_eq_unordered failed: \nexpected: {expected:?}\nactual: {actual:?}");
    }
}

fn prevote(block_hash: Hash256, privkey: &PrivateKey) -> Prevote {
    TypedSignature::sign(&format!("{}-{}", block_hash, "prevote"), privkey).unwrap()
}

fn precommit(block_hash: Hash256, privkey: &PrivateKey) -> Precommit {
    TypedSignature::new(
        Signature::sign(block_hash, privkey).unwrap(),
        privkey.public_key(),
    )
}

/// This may panic.
fn _verify_fp(_fp: FinalizationProof) {
    unimplemented!();
}

#[tokio::test]
#[ignore]
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
        Some(server_config.private_key.clone()),
    )
    .await
    .unwrap();
    let mut other_nodes = Vec::new();
    for config in &other_configs {
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

    // [Step 1]
    // Action: The server node proposes a block
    // Expected: The server node will propose a block and prevotes on it.
    // Initial block candidate is set to 0 by default, so we progress without setting a new candidate.
    let timestamp = get_timestamp();
    let mut result = Vec::new();
    result.extend(
        server_node
            .set_proposal_candidate(dummy_block_hash, timestamp)
            .await
            .unwrap(),
    );
    result.extend(server_node.progress(get_timestamp()).await.unwrap());
    let expected = vec![
        ProgressResult::Proposed(0, dummy_block_hash, timestamp),
        ProgressResult::NonNilPreVoted(0, dummy_block_hash, timestamp),
    ];
    assert_eq!(result, expected);

    // Action: Non-server nodes fetch messages from the server and make progress.
    // Expected: Node 0, 1 will prevote, node 2, 3 will precommit.
    let serve_task = tokio::spawn(async { server_node.serve(5_000).await });
    for (i, other_node) in other_nodes.iter_mut().enumerate() {
        println!("Checking node #{i}");
        other_node.fetch().await.unwrap();
        let timestamp = get_timestamp();
        let result = other_node.progress(timestamp).await.unwrap();
        sleep_ms(200).await;
        let mut expected = vec![ProgressResult::NonNilPreVoted(
            0,
            dummy_block_hash,
            timestamp,
        )];
        // The nodes will broadcast precommits as well if they see prevotes over 2/3.
        if i == 2 || i == 3 {
            expected.push(ProgressResult::NonNilPreCommitted(
                0,
                dummy_block_hash,
                timestamp,
            ));
        }
        assert_eq!(result, expected);
    }
    let mut server_node = serve_task.await.unwrap().unwrap();

    // Check if prevotes and precommits are broadcasted well.
    let received_messages = server_node.read_messages().await.unwrap();
    let mut expected_received_messages = vec![
        (
            ConsensusMessage::Proposal {
                round: 0,
                valid_round: None,
                block_hash: dummy_block_hash,
            },
            server_config.public_key.clone(),
        ),
        (
            ConsensusMessage::NonNilPreVoted(
                0,
                dummy_block_hash,
                prevote(dummy_block_hash, &server_config.private_key),
            ),
            server_config.public_key.clone(),
        ),
    ];
    for (i, config) in other_configs.iter().enumerate() {
        expected_received_messages.push((
            ConsensusMessage::NonNilPreVoted(
                0,
                dummy_block_hash,
                prevote(dummy_block_hash, &config.private_key),
            ),
            config.public_key.clone(),
        ));
        if i == 2 || i == 3 {
            expected_received_messages.push((
                ConsensusMessage::NonNilPreCommitted(
                    0,
                    dummy_block_hash,
                    precommit(dummy_block_hash, &config.private_key),
                ),
                config.public_key.clone(),
            ));
        }
    }
    assert_eq_unordered(&expected_received_messages, &received_messages);

    // [Step 2]
    // Action: The server node progresses.
    // Expected: The server node will precommit.
    let timestamp = get_timestamp();
    let result = server_node.progress(timestamp).await.unwrap();
    let expected = vec![ProgressResult::NonNilPreCommitted(
        0,
        dummy_block_hash,
        timestamp,
    )];
    assert_eq!(result, expected);

    // Action: Non-server nodes fetch and progress.
    // Expected: Node 0, 1 will precommit and finalize, node 2, 3 will only finalize.
    let serve_task = tokio::spawn(async { server_node.serve(5_000).await });
    let mut finalization_proofs = Vec::new();
    for (i, other_node) in other_nodes.iter_mut().enumerate() {
        println!("Checking node #{i}");
        other_node.fetch().await.unwrap();
        let timestamp = get_timestamp();
        let result = other_node.progress(timestamp).await.unwrap();
        sleep_ms(200).await;
        for r in &result {
            debug!("{:?}", r);
        }
        let precommit = ProgressResult::NonNilPreCommitted(0, dummy_block_hash, timestamp);
        if i == 0 || i == 1 {
            assert_eq!(result.len(), 2);
            assert_eq!(result[0], precommit);
            match &result[1] {
                ProgressResult::Finalized(hash, time, proof) => {
                    assert_eq!(*hash, dummy_block_hash);
                    assert_eq!(*time, timestamp);
                    finalization_proofs.push(proof.clone());
                }
                _ => panic!("expect finalization"),
            }
        } else {
            assert_eq!(result.len(), 1);
            match &result[0] {
                ProgressResult::Finalized(hash, time, proof) => {
                    assert_eq!(*hash, dummy_block_hash);
                    assert_eq!(*time, timestamp);
                    finalization_proofs.push(proof.clone());
                }
                _ => panic!("expect finalization"),
            }
        }
    }
    let mut server_node = serve_task.await.unwrap().unwrap();

    let received_messages = server_node.read_messages().await.unwrap();
    // Check if precommits are broadcasted well.
    expected_received_messages.push((
        ConsensusMessage::NonNilPreCommitted(
            0,
            dummy_block_hash,
            precommit(dummy_block_hash, &server_config.private_key),
        ),
        server_config.public_key.clone(),
    ));
    for config in other_configs.iter().take(2) {
        expected_received_messages.push((
            ConsensusMessage::NonNilPreCommitted(
                0,
                dummy_block_hash,
                precommit(dummy_block_hash, &config.private_key),
            ),
            config.public_key.clone(),
        ));
    }
    assert_eq_unordered(&expected_received_messages, &received_messages);

    // [Step 3]
    // Action: The server node progresses.
    // Expected: The server node will finalize.
    let timestamp = get_timestamp();
    let result = server_node.progress(timestamp).await.unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        ProgressResult::Finalized(hash, time, proof) => {
            assert_eq!(*hash, dummy_block_hash);
            assert_eq!(*time, timestamp);
            finalization_proofs.push(proof.clone());
        }
        _ => panic!("expect finalization"),
    }

    // Action: Non-server nodes fetch and progress.
    // Expected: No operation.
    let serve_task = tokio::spawn(async { server_node.serve(3_000).await });
    for other_node in other_nodes.iter_mut() {
        other_node.fetch().await.unwrap();
        let timestamp = get_timestamp();
        let result = other_node.progress(timestamp).await.unwrap_err();
        assert_eq!(
            result.to_string(),
            "operation on finalized state".to_string()
        );
    }
    let _ = serve_task.await.unwrap().unwrap();

    // Todo: verify finalization proofs
}
