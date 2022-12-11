use simperby_common::*;
use simperby_node::*;
use simperby_repository::raw::RawRepository;
use simperby_test_suite::*;

fn generate_config(key: PrivateKey, chain_name: String) -> Config {
    Config {
        chain_name,
        public_key: key.public_key(),
        private_key: key,
        broadcast_interval_ms: None,
        fetch_interval_ms: None,
        public_repo_url: vec![],
    }
}

#[tokio::test]
#[ignore]
async fn normal_1() {
    let (rs, keys) = generate_standard_genesis(4);
    let chain_name = "normal_1".to_owned();

    let configs = keys
        .iter()
        .map(|(_, private_key)| generate_config(private_key.clone(), chain_name.clone()))
        .collect::<Vec<_>>();

    // Step 0: initialize each's repo
    let server_dir = create_temp_dir();
    setup_pre_genesis_repository(&server_dir, rs.clone()).await;
    let mut proposer_node = initialize(configs[0].clone(), &server_dir).await.unwrap();
    let mut other_nodes = Vec::new();
    for config in configs[1..=3].iter() {
        let dir = create_temp_dir();
        copy_repository(&server_dir, &dir).await;
        other_nodes.push(initialize(config.clone(), &dir).await.unwrap());
    }

    // Step 1: create an agenda and propagate it
    proposer_node.create_agenda().await.unwrap();
    let agenda_commit = proposer_node
        .get_raw_repo_mut()
        .locate_branch("work".to_owned())
        .await
        .unwrap();
    let serve = tokio::spawn(async move { proposer_node.serve().await.unwrap() });
    sleep_ms(1000).await;
    for node in other_nodes.iter_mut() {
        node.fetch().await.unwrap();
        node.vote(agenda_commit).await.unwrap();
    }
    let mut proposer_node = serve.await.unwrap();

    // Step 2: create block and run prevote phase
    proposer_node.create_block().await.unwrap();
    proposer_node.progress_for_consensus().await.unwrap();
    let serve = tokio::spawn(async move { proposer_node.serve().await.unwrap() });
    sleep_ms(1000).await;
    for node in other_nodes.iter_mut() {
        node.fetch().await.unwrap();
        node.progress_for_consensus().await.unwrap();
    }
    let mut proposer_node = serve.await.unwrap();
    proposer_node.progress_for_consensus().await.unwrap();

    // Step 3: Run precommit phase
    let serve = tokio::spawn(async move { proposer_node.serve().await.unwrap() });
    sleep_ms(1000).await;
    for node in other_nodes.iter_mut() {
        node.fetch().await.unwrap();
        node.progress_for_consensus().await.unwrap();
    }
    let mut proposer_node = serve.await.unwrap();
    proposer_node.progress_for_consensus().await.unwrap();

    // Step 4: Propagate finalized proof
    let serve = tokio::spawn(async move { proposer_node.serve().await.unwrap() });
    sleep_ms(1000).await;
    for node in other_nodes.iter_mut() {
        node.fetch().await.unwrap();
    }
    let proposer_node = serve.await.unwrap();

    for node in std::iter::once(proposer_node).chain(other_nodes.into_iter()) {
        let finalized = node
            .get_raw_repo()
            .locate_branch("finalized".to_owned())
            .await
            .unwrap();
        let title = node
            .get_raw_repo()
            .read_semantic_commit(finalized)
            .await
            .unwrap()
            .title;
        assert_eq!(title, ">block: 1");
    }
}
