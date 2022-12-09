use simperby_common::*;
use simperby_governance::*;
use simperby_network::*;
use simperby_test_suite::*;

#[tokio::test]
async fn basic_1() {
    env_logger::init();
    let network_id = "governance-basic-1".to_string();
    let (server_network_config, client_network_configs, peer) =
        setup_server_client_nodes(network_id.clone(), 3).await;

    let mut server_node = Governance::open(
        create_test_dms(
            server_network_config.clone(),
            network_id.clone(),
            SharedKnownPeers::new_static(Default::default()),
        )
        .await,
        Some(server_network_config.private_key),
    )
    .await
    .unwrap();

    let mut client_nodes = Vec::new();
    for network_config in client_network_configs.iter() {
        client_nodes.push((
            Governance::open(
                create_test_dms(network_config.clone(), network_id.clone(), peer.clone()).await,
                Some(network_config.private_key.clone()),
            )
            .await
            .unwrap(),
            network_config,
        ));
    }

    let agenda_hash = Hash256::hash("agenda");
    server_node.vote(agenda_hash).await.unwrap();

    tokio::spawn(async move {
        let server_node = server_node.serve().await.unwrap();
        assert_eq!(
            server_node.read().await.unwrap().votes[&agenda_hash].len(),
            4
        );
    });

    sleep_ms(1000).await;

    for (node, _) in client_nodes.iter_mut() {
        node.vote(agenda_hash).await.unwrap();
    }
    sleep_ms(500).await;
    for (node, _) in client_nodes.iter_mut() {
        node.fetch().await.unwrap();
        assert_eq!(node.read().await.unwrap().votes[&agenda_hash].len(), 4);
    }
}
