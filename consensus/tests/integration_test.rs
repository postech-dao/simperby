use simperby_consensus::*;
use simperby_core::*;
use simperby_network::*;
use simperby_test_suite::*;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::test]
async fn basic_1() {
    setup_test();

    let network_id = "consensus-basic-1".to_string();
    let ((server_network_config, server_private_key), client_network_configs_and_keys, members, fi) =
        setup_server_client_nodes(network_id.clone(), 4).await;
    let path = create_temp_dir();
    StorageImpl::create(&path).await.unwrap();
    let storage = StorageImpl::open(&path).await.unwrap();

    let mut server_node = Consensus::new(
        Arc::new(RwLock::new(
            create_test_dms(
                network_id.clone(),
                members.clone(),
                server_private_key.clone(),
            )
            .await,
        )),
        storage,
        fi.header.clone(),
        ConsensusParams {
            timeout_ms: 6000,
            repeat_round_for_first_leader: 10,
        },
        0,
        Some(server_private_key),
    )
    .await
    .unwrap();

    let mut client_nodes = Vec::new();
    for (network_config, private_key) in client_network_configs_and_keys {
        let path = create_temp_dir();
        StorageImpl::create(&path).await.unwrap();
        let storage = StorageImpl::open(&path).await.unwrap();

        client_nodes.push((
            Consensus::new(
                Arc::new(RwLock::new(
                    create_test_dms(network_id.clone(), members.clone(), private_key.clone()).await,
                )),
                storage,
                fi.header.clone(),
                ConsensusParams {
                    timeout_ms: 6000,
                    repeat_round_for_first_leader: 10,
                },
                0,
                Some(private_key.clone()),
            )
            .await
            .unwrap(),
            network_config,
        ));
    }

    let block_hash = Hash256::hash("block");
    server_node
        .register_verified_block_hash(block_hash)
        .await
        .unwrap();
    for (node, _) in client_nodes.iter_mut() {
        node.register_verified_block_hash(block_hash).await.unwrap();
    }

    let serve_task = tokio::spawn(async move {
        let task = tokio::spawn(Dms::serve(server_node.get_dms(), server_network_config));
        sleep_ms(5000).await;
        task.abort();
        let _ = task.await;
        server_node.update().await.unwrap();
        server_node.progress(0).await.unwrap();
        assert_eq!(
            server_node
                .check_finalized()
                .await
                .unwrap()
                .unwrap()
                .block_hash,
            block_hash
        );
    });

    async fn sync(client_nodes: &mut [(Consensus, ClientNetworkConfig)]) {
        for (node, network_config) in client_nodes.iter_mut() {
            node.flush().await.unwrap();
            dms::DistributedMessageSet::broadcast(node.get_dms(), network_config)
                .await
                .unwrap();
        }
        for (node, network_config) in client_nodes.iter_mut() {
            dms::DistributedMessageSet::fetch(node.get_dms(), network_config)
                .await
                .unwrap();
            node.update().await.unwrap();
        }
    }

    client_nodes[0]
        .0
        .set_proposal_candidate(block_hash, 0)
        .await
        .unwrap();
    // PROPOSE
    for (node, _) in client_nodes.iter_mut() {
        node.progress(0).await.unwrap();
    }
    sync(&mut client_nodes).await;
    // PREVOTE
    for (node, _) in client_nodes.iter_mut() {
        node.progress(0).await.unwrap();
    }
    sync(&mut client_nodes).await;
    // PRECOMMIT
    for (node, _) in client_nodes.iter_mut() {
        node.progress(0).await.unwrap();
    }
    sync(&mut client_nodes).await;
    // FINALIZE
    for (node, _) in client_nodes.iter_mut() {
        node.progress(0).await.unwrap();
    }
    for (node, _) in client_nodes.iter_mut() {
        assert_eq!(
            node.check_finalized().await.unwrap().unwrap().block_hash,
            block_hash
        );
    }
    serve_task.await.unwrap();
}
