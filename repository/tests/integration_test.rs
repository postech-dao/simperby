use simperby_common::*;
use simperby_network::{Peer, SharedKnownPeers};
use simperby_repository::{raw::*, *};
use simperby_test_suite::*;

#[tokio::test]
async fn basic_1() {
    setup_test();
    let port = dispense_port();

    let (rs, keys) = generate_standard_genesis(4);
    let config = Config {
        mirrors: Vec::new(),
        long_range_attack_distance: 1,
    };
    let peers = vec![Peer {
        public_key: keys[0].0.clone(),
        name: "server-node".to_owned(),
        address: format!("127.0.0.1:{}", 1).parse().unwrap(),
        ports: vec![("repository".to_owned(), port)].into_iter().collect(),
        message: "".to_owned(),
        recently_seen_timestamp: 0,
    }];
    let peers = SharedKnownPeers::new_static(peers);

    let server_node_dir = create_temp_dir();
    setup_pre_genesis_repository(&server_node_dir, rs.clone()).await;

    let mut server_node_repo = DistributedRepository::new(
        RawRepositoryImpl::open(&format!("{}/repository/repo", server_node_dir))
            .await
            .unwrap(),
        config.clone(),
        peers.clone(),
    )
    .await
    .unwrap();
    server_node_repo.genesis().await.unwrap();

    let server_node_dir_clone = server_node_dir.clone();
    let git_server = tokio::spawn(async move {
        let _server =
            server::run_server(&format!("{}/repository", server_node_dir_clone), port).await;
        sleep_ms(12000).await;
    });

    let client_node_dir = create_temp_dir();
    run_command(format!(
        "cd {} && mkdir repository && cd repository && cp -r {}/repository/repo {}/repository",
        client_node_dir, server_node_dir, client_node_dir
    ))
    .await;
    let mut client_node_repo = DistributedRepository::new(
        RawRepositoryImpl::open(&format!("{}/repository/repo", client_node_dir))
            .await
            .unwrap(),
        config,
        peers.clone(),
    )
    .await
    .unwrap();

    // Step 0: create an agenda and let the client update that
    let (agenda, agenda_commit) = server_node_repo
        .create_agenda(keys[0].0.clone())
        .await
        .unwrap();
    client_node_repo.fetch().await.unwrap();
    assert_eq!(
        client_node_repo.get_agendas().await.unwrap(),
        vec![(agenda_commit, agenda.to_hash256())]
    );
    server_node_repo
        .approve(
            &agenda.to_hash256(),
            keys.iter()
                .map(|(_, private_key)| TypedSignature::sign(&agenda, private_key).unwrap())
                .collect(),
        )
        .await
        .unwrap();

    // Step 1: create a block and let the client update that
    let (block, block_commit) = server_node_repo
        .create_block(keys[0].0.clone())
        .await
        .unwrap();
    client_node_repo.fetch().await.unwrap();
    assert_eq!(
        client_node_repo.get_blocks().await.unwrap(),
        vec![(block_commit, block.to_hash256())]
    );

    // Step 2: finalize a block and let the client update that
    let block_proof = keys
        .iter()
        .map(|(_, private_key)| TypedSignature::sign(&block, private_key).unwrap())
        .collect();
    server_node_repo
        .sync(&block.to_hash256(), &block_proof)
        .await
        .unwrap();
    client_node_repo.fetch().await.unwrap();
    assert_eq!(
        client_node_repo
            .get_last_finalized_block_header()
            .await
            .unwrap(),
        block
    );

    git_server.await.unwrap();
}
