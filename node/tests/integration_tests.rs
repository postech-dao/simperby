use simperby_common::*;
use simperby_network::Peer;
use simperby_node::{genesis, *};
use simperby_repository::raw::RawRepository;
use simperby_test_suite::*;
use tokio::io::AsyncWriteExt;

fn generate_config(key: PrivateKey, chain_name: String) -> Config {
    Config {
        chain_name,
        public_key: key.public_key(),
        private_key: key,
        broadcast_interval_ms: None,
        fetch_interval_ms: None,
        public_repo_url: vec![],
        governance_port: dispense_port(),
        consensus_port: dispense_port(),
        repository_port: dispense_port(),
    }
}

async fn setup_peer(path: &str, peers: &[Peer]) {
    let mut file = tokio::fs::File::create(format!("{path}/peers.json"))
        .await
        .unwrap();
    file.write_all(serde_spb::to_string(&peers).unwrap().as_bytes())
        .await
        .unwrap();
    file.flush().await.unwrap();
}

#[tokio::test]
async fn normal_1() {
    setup_test();
    let (rs, keys) = generate_standard_genesis(5);
    let chain_name = "normal_1".to_owned();

    let configs = keys
        .iter()
        .map(|(_, private_key)| generate_config(private_key.clone(), chain_name.clone()))
        .collect::<Vec<_>>();

    // Step 0: initialize each's repo
    let server_dir = create_temp_dir();
    setup_peer(&server_dir, &[]).await;
    setup_pre_genesis_repository(&server_dir, rs.clone()).await;
    genesis(configs[0].clone(), &server_dir).await.unwrap();
    let mut proposer_node = initialize(configs[0].clone(), &server_dir).await.unwrap();
    let mut other_nodes = Vec::new();
    for config in configs[1..=4].iter() {
        let dir = create_temp_dir();
        copy_repository(&server_dir, &dir).await;
        setup_peer(
            &dir,
            &[Peer {
                public_key: configs[0].public_key.clone(),
                name: "proposer".to_owned(),
                address: "127.0.0.1:1".parse().unwrap(),
                ports: proposer_node.network_config().ports.clone(),
                message: "123".to_owned(),
                recently_seen_timestamp: 0,
            }],
        )
        .await;
        other_nodes.push(initialize(config.clone(), &dir).await.unwrap());
    }

    // Step 1: create an agenda and propagate it
    log::info!("STEP 1");
    proposer_node.create_agenda().await.unwrap();
    let agenda_commit = proposer_node
        .get_raw_repo_mut()
        .locate_branch("work".to_owned())
        .await
        .unwrap();
    let serve = tokio::spawn(async move { proposer_node.serve(5000).await.unwrap() });
    sleep_ms(500).await;
    for node in other_nodes.iter_mut() {
        node.fetch().await.unwrap();
        node.vote(agenda_commit).await.unwrap();
        node.broadcast().await.unwrap();
    }
    let mut proposer_node = serve.await.unwrap();
    // currently calling `fetch()` is the only way to notice governance approval
    proposer_node.fetch().await.unwrap();
    // TODO: it is not guaranteed that `HEAD` is on the agenda proof.
    run_command(format!(
        "cd {server_dir}/repository/repo && git branch -f work HEAD"
    ))
    .await;

    // Step 2: create block and run prevote phase
    log::info!("STEP 2");
    proposer_node.create_block().await.unwrap();
    proposer_node.progress_for_consensus().await.unwrap();
    proposer_node.broadcast().await.unwrap();
    let serve = tokio::spawn(async move { proposer_node.serve(5000).await.unwrap() });
    sleep_ms(500).await;
    for node in other_nodes.iter_mut() {
        node.fetch().await.unwrap();
    }
    for node in other_nodes.iter_mut() {
        node.fetch().await.unwrap();
    }
    for node in other_nodes.iter_mut() {
        node.progress_for_consensus().await.unwrap();
        node.broadcast().await.unwrap();
    }
    let mut proposer_node = serve.await.unwrap();
    proposer_node.progress_for_consensus().await.unwrap();
    proposer_node.broadcast().await.unwrap();

    // Step 3: Run precommit phase
    log::info!("STEP 3");
    let serve = tokio::spawn(async move { proposer_node.serve(5000).await.unwrap() });
    sleep_ms(500).await;
    for node in other_nodes.iter_mut() {
        node.fetch().await.unwrap();
    }
    for node in other_nodes.iter_mut() {
        let _ = node.progress_for_consensus().await.unwrap();
        node.broadcast().await.unwrap();
    }
    let mut proposer_node = serve.await.unwrap();
    let _ = proposer_node.progress_for_consensus().await.unwrap();
    proposer_node.broadcast().await.unwrap();

    // Step 4: Propagate finalized proof
    log::info!("STEP 4");
    let serve = tokio::spawn(async move { proposer_node.serve(5000).await.unwrap() });
    sleep_ms(500).await;
    for node in other_nodes.iter_mut() {
        node.fetch().await.unwrap();
    }
    for node in other_nodes.iter_mut() {
        let _ = node.progress_for_consensus().await;
        node.broadcast().await.unwrap();
    }
    let mut proposer_node = serve.await.unwrap();
    let _ = proposer_node.progress_for_consensus().await;
    proposer_node.broadcast().await.unwrap();

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
