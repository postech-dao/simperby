use simperby::types::{Auth, Config};
use simperby::*;
use simperby_core::*;
use simperby_test_suite::*;

fn generate_server_config() -> ServerConfig {
    ServerConfig {
        peers_port: dispense_port(),
        governance_port: dispense_port(),
        consensus_port: dispense_port(),
        repository_port: dispense_port(),
        broadcast_interval_ms: Some(500),
        fetch_interval_ms: Some(500),
    }
}

async fn sync_each_other(clients: &mut [Client]) {
    for client in clients.iter_mut() {
        client.broadcast().await.unwrap();
    }
    sleep_ms(200).await;
    for client in clients.iter_mut() {
        client.update().await.unwrap();
    }
    sleep_ms(200).await;
}

fn build_simple_git_server() -> String {
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("build");
    cmd.arg("--bin");
    cmd.arg("simple_git_server");
    cmd.arg("--release");
    cmd.current_dir(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../repository/src/bin"
    ));
    let output = cmd.output().unwrap();
    assert!(output.status.success());

    format!(
        "{}/../target/release/simple_git_server",
        env!("CARGO_MANIFEST_DIR").replace('\\', "/")
    )
}

#[tokio::test]
async fn normal_1() {
    setup_test();
    let (fi, keys) = test_utils::generate_fi(4);
    let server_config = generate_server_config();

    // Setup repository and server
    let server_dir = create_temp_dir();
    setup_pre_genesis_repository(&server_dir, fi.reserved_state.clone()).await;
    Client::genesis(&server_dir).await.unwrap();
    Client::init(&server_dir).await.unwrap();
    // Add push configs to server repository.
    run_command(format!(
        "cd {server_dir} && git config receive.advertisePushOptions true"
    ))
    .await;
    run_command(format!(
        "cd {server_dir} && git config sendpack.sideband false"
    ))
    .await;

    // Setup clients
    let mut clients = Vec::new();
    for (_, key) in keys.iter().take(3) {
        let dir = create_temp_dir();
        run_command(format!("cp -a {server_dir}/. {dir}/")).await;
        let auth = Auth {
            private_key: key.clone(),
        };
        let port = server_config.peers_port;
        let mut client = Client::open(&dir, Config {}, auth).await.unwrap();
        client
            .add_peer(
                fi.reserved_state.members[3].name.clone(),
                format!("127.0.0.1:{port}").parse().unwrap(),
            )
            .await
            .unwrap();
        clients.push(client);
    }

    // Run server
    let auth = Auth {
        private_key: keys[3].1.clone(),
    };
    let server_config_ = server_config.clone();
    let server_dir_ = server_dir.clone();
    tokio::spawn(async move {
        let client = Client::open(&server_dir_, Config {}, auth).await.unwrap();
        let task = client
            .serve(
                server_config_,
                simperby_repository::server::PushVerifier::VerifierExecutable(
                    build_simple_git_server(),
                ),
            )
            .await
            .unwrap();
        task.await.unwrap().unwrap();
    });

    // Setup peer network
    sleep_ms(200).await;
    for client in clients.iter_mut() {
        client.update_peer().await.unwrap();
    }

    // Step 1: create an agenda and propagate it
    log::info!("STEP 1");
    let (_, agenda_commit) = clients[0]
        .repository_mut()
        .create_agenda(fi.reserved_state.members[0].name.clone())
        .await
        .unwrap();

    sync_each_other(&mut clients).await;
    for client in clients.iter_mut().take(3) {
        client.vote(agenda_commit).await.unwrap();
    }
    sync_each_other(&mut clients).await;

    // Step 2: create block and run consensus
    log::info!("STEP 2");
    let proposer_public_key = clients[0].auth().private_key.public_key();
    clients[0]
        .repository_mut()
        .create_block(proposer_public_key)
        .await
        .unwrap();
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut().take(3) {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut().take(3) {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut().take(3) {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut().take(3) {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;

    // Step 3: check the result
    for client in clients {
        let raw_repo = client.repository().get_raw();
        let raw_repo_ = raw_repo.read().await;
        let finalized = raw_repo_
            .locate_branch("finalized".to_owned())
            .await
            .unwrap();
        let title = raw_repo_
            .read_semantic_commit(finalized)
            .await
            .unwrap()
            .title;
        assert_eq!(title, ">block: 1");
    }
}
