use simperby_core::*;
use simperby_governance::*;
use simperby_network::*;
use simperby_test_suite::*;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::test]
async fn basic_1() {
    setup_test();

    let network_id = "governance-basic-1".to_string();
    let ((server_network_config, server_private_key), client_network_configs_and_keys, members) =
        setup_server_client_nodes(network_id.clone(), 3).await;

    let mut server_node = Governance::new(Arc::new(RwLock::new(
        create_test_dms(network_id.clone(), members.clone(), server_private_key).await,
    )))
    .await
    .unwrap();

    let mut client_nodes = Vec::new();
    for (network_config, private_key) in client_network_configs_and_keys.iter() {
        client_nodes.push((
            Governance::new(Arc::new(RwLock::new(
                create_test_dms(network_id.clone(), members.clone(), private_key.clone()).await,
            )))
            .await
            .unwrap(),
            network_config,
        ));
    }

    let agenda_hash = Hash256::hash("agenda");
    server_node.vote(agenda_hash).await.unwrap();

    let serve_task = tokio::spawn(async move {
        let task = tokio::spawn(dms::serve(server_node.get_dms(), server_network_config));
        sleep_ms(5000).await;
        task.abort();
        let _ = task.await;
        assert_eq!(
            server_node.read().await.unwrap().votes[&agenda_hash].len(),
            4
        );
    });
    sleep_ms(500).await;
    for (node, network_config) in client_nodes.iter_mut() {
        node.vote(agenda_hash).await.unwrap();
        node.flush().await.unwrap();
        dms::DistributedMessageSet::broadcast(node.get_dms(), network_config)
            .await
            .unwrap();
    }
    sleep_ms(500).await;
    for (node, network_config) in client_nodes.iter_mut() {
        dms::DistributedMessageSet::fetch(node.get_dms(), network_config)
            .await
            .unwrap();
        node.update().await.unwrap();
        assert_eq!(node.read().await.unwrap().votes[&agenda_hash].len(), 4);
    }
    serve_task.await.unwrap();
}
