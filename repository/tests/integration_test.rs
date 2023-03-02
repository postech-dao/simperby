use simperby_core::*;
use simperby_repository::{raw::*, *};
use simperby_test_suite::*;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::test]
async fn sync_by_fetch() {
    setup_test();
    let port = dispense_port();
    let (rs, keys) = test_utils::generate_standard_genesis(4);
    let config = Config {
        long_range_attack_distance: 1,
    };
    let server_node_dir = create_temp_dir();
    setup_pre_genesis_repository(&server_node_dir, rs.clone()).await;

    let mut server_node_repo = DistributedRepository::new(
        Arc::new(RwLock::new(
            RawRepository::open(&format!("{server_node_dir}/repository"))
                .await
                .unwrap(),
        )),
        config.clone(),
        None,
    )
    .await
    .unwrap();
    server_node_repo.genesis().await.unwrap();

    let server_node_dir_clone = server_node_dir.clone();
    let git_server = tokio::spawn(async move {
        let _server =
            server::run_server_legacy(&format!("{server_node_dir_clone}/repository"), port).await;
        sleep_ms(12000).await;
    });

    let client_node_dir = create_temp_dir();
    simperby_test_suite::run_command(format!(
        "cp -r {server_node_dir}/repository {client_node_dir}/repository"
    ))
    .await;

    simperby_test_suite::run_command(format!(
        "cd {client_node_dir}/repository && git remote add peer git://127.0.0.1:{port}/"
    ))
    .await;
    let mut client_node_repo = DistributedRepository::new(
        Arc::new(RwLock::new(
            RawRepository::open(&format!("{client_node_dir}/repository"))
                .await
                .unwrap(),
        )),
        config.clone(),
        None,
    )
    .await
    .unwrap();

    // Step 0: create an agenda and let the client update that
    let (agenda, agenda_commit) = server_node_repo
        .create_agenda(rs.query_name(&keys[0].0).unwrap())
        .await
        .unwrap();
    simperby_test_suite::run_command(format!(
        "cd {client_node_dir}/repository && git fetch --all"
    ))
    .await;
    client_node_repo.sync_all().await.unwrap();
    assert_eq!(
        client_node_repo.read_agendas().await.unwrap(),
        vec![(agenda_commit, agenda.to_hash256())]
    );
    let agenda_proof = server_node_repo
        .approve(
            &agenda.to_hash256(),
            keys.iter()
                .map(|(_, private_key)| TypedSignature::sign(&agenda, private_key).unwrap())
                .collect(),
            0,
        )
        .await
        .unwrap();
    simperby_test_suite::run_command(format!(
        "cd {server_node_dir}/repository && git branch -f work {agenda_proof}"
    ))
    .await;

    // Step 1: create a block and let the client update that
    let (block, block_commit) = server_node_repo
        .create_block(keys[0].0.clone())
        .await
        .unwrap();
    simperby_test_suite::run_command(format!(
        "cd {client_node_dir}/repository && git fetch --all"
    ))
    .await;
    client_node_repo.sync_all().await.unwrap();
    assert_eq!(
        client_node_repo.read_blocks().await.unwrap(),
        vec![(block_commit, block.to_hash256())]
    );

    // Step 2: finalize a block and let the client update that
    let signatures = keys
        .iter()
        .map(|(_, private_key)| {
            TypedSignature::sign(
                &FinalizationSignTarget {
                    round: 0,
                    block_hash: block.to_hash256(),
                },
                private_key,
            )
            .unwrap()
        })
        .collect();
    server_node_repo
        .finalize(
            block_commit,
            FinalizationProof {
                signatures,
                round: 0,
            },
        )
        .await
        .unwrap();

    simperby_test_suite::run_command(format!(
        "cd {client_node_dir}/repository && git fetch --all"
    ))
    .await;
    client_node_repo.sync_all().await.unwrap();
    assert_eq!(
        client_node_repo
            .read_last_finalization_info()
            .await
            .unwrap()
            .header,
        block
    );

    git_server.await.unwrap();
}

/// Builds `simple_git_server.rs` and returns the path of the executable.
fn build_simple_git_server() -> String {
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("build");
    cmd.arg("--bin");
    cmd.arg("simple_git_server");
    cmd.arg("--release");
    let output = cmd.output().unwrap();
    assert!(output.status.success());
    format!(
        "{}/../target/release/simple_git_server",
        env!("CARGO_MANIFEST_DIR").replace('\\', "/")
    )
}

#[tokio::test]
async fn sync_by_push() {
    setup_test();
    let port = dispense_port();

    let (rs, keys) = test_utils::generate_standard_genesis(4);
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
    // Add push configs to server repository.
    simperby_test_suite::run_command(format!(
        "cd {server_node_dir}/repository/repo && git config receive.advertisePushOptions true"
    ))
    .await;
    simperby_test_suite::run_command(format!(
        "cd {server_node_dir}/repository/repo && git config sendpack.sideband false"
    ))
    .await;

    let mut server_node_repo = DistributedRepository::new(
        RawRepositoryImpl::open(&format!("{server_node_dir}/repository/repo"))
            .await
            .unwrap(),
        config.clone(),
        peers.clone(),
        None,
    )
    .await
    .unwrap();
    server_node_repo.genesis().await.unwrap();

    let _git_server =
        simperby_repository::server::run_server(&server_node_dir, port, &build_simple_git_server())
            .await;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let client_node_dir = create_temp_dir();
    simperby_test_suite::run_command(format!(
        "cd {client_node_dir} && mkdir repository && cd repository && cp -r {server_node_dir}/repository/repo {client_node_dir}/repository"
    ))
    .await;
    let mut client_node_repo = DistributedRepository::new(
        RawRepositoryImpl::open(&format!("{client_node_dir}/repository/repo"))
            .await
            .unwrap(),
        config,
        peers.clone(),
        Some(keys[1].1.clone()),
    )
    .await
    .unwrap();

    // Step 0: create an agenda and let the client push that
    let (agenda, agenda_commit) = client_node_repo
        .create_agenda(keys[0].0.clone())
        .await
        .unwrap();
    client_node_repo.broadcast().await.unwrap();
    assert_eq!(
        server_node_repo.get_agendas().await.unwrap(),
        vec![(agenda_commit, agenda.to_hash256())]
    );
    let agenda_proof = client_node_repo
        .approve(
            &agenda.to_hash256(),
            keys.iter()
                .map(|(_, private_key)| TypedSignature::sign(&agenda, private_key).unwrap())
                .collect(),
        )
        .await
        .unwrap();
    simperby_test_suite::run_command(format!(
        "cd {client_node_dir}/repository/repo && git branch -f work {agenda_proof}"
    ))
    .await;

    // Step 1: create a block and let the client push that
    let (block, block_commit) = client_node_repo
        .create_block(keys[0].0.clone())
        .await
        .unwrap();
    client_node_repo.broadcast().await.unwrap();
    assert_eq!(
        server_node_repo.get_blocks().await.unwrap(),
        vec![(block_commit, block.to_hash256())]
    );

    // Step 2: finalize a block and let the client push that
    let block_proof = keys
        .iter()
        .map(|(_, private_key)| TypedSignature::sign(&block, private_key).unwrap())
        .collect();
    client_node_repo
        .sync(&block.to_hash256(), &block_proof)
        .await
        .unwrap();
    client_node_repo.broadcast().await.unwrap();
    assert_eq!(
        server_node_repo
            .get_last_finalized_block_header()
            .await
            .unwrap(),
        block
    );
}
