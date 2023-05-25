use simperby::types::{Auth, Config};
use simperby::*;
use simperby_core::*;
use simperby_network::Peer;
use simperby_repository::raw::RawRepository;
use simperby_test_suite::*;

fn setup_network(
    start_fi: &FinalizationInfo,
    server_public_key: PublicKey,
) -> (Vec<Peer>, ServerConfig) {
    let server_config = ServerConfig {
        governance_port: dispense_port(),
        consensus_port: dispense_port(),
        repository_port: dispense_port(),
        broadcast_interval_ms: Some(500),
        fetch_interval_ms: Some(500),
    };
    let peer = vec![Peer {
        public_key: server_public_key,
        name: "server".to_owned(),
        address: "127.0.0.1:1".parse().unwrap(),
        ports: vec![
            (
                format!("dms-governance-{}", start_fi.header.to_hash256()),
                server_config.governance_port,
            ),
            (
                format!("dms-consensus-{}", start_fi.header.to_hash256()),
                server_config.consensus_port,
            ),
            ("repository".to_owned(), server_config.repository_port),
        ]
        .into_iter()
        .collect(),
        message: "".to_owned(),
        recently_seen_timestamp: 0,
    }];
    (peer, server_config)
}

fn build_simperby_cli() -> String {
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("build");
    cmd.arg("--bin");
    cmd.arg("simperby-cli");
    cmd.arg("--release");
    cmd.current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../"));
    let output = cmd.output().unwrap();
    assert!(output.status.success());

    format!(
        "{}/../target/release/simperby-cli",
        env!("CARGO_MANIFEST_DIR").replace('\\', "/")
    )
}

async fn sync_each_other(cli_path: String, clients_path: Vec<String>) {
    for path in clients_path.clone() {
        run_command(format!("{cli_path} {path} broadcast")).await;
    }
    sleep_ms(200).await;
    for path in clients_path {
        run_command(format!("{cli_path} {path} update")).await;
    }
    sleep_ms(200).await;
}

#[tokio::test]
async fn cli() {
    setup_test();
    let (fi, keys) = test_utils::generate_fi(4);
    let (peers, server_config) = setup_network(&fi, keys[3].0.clone());
    let port = server_config.repository_port;

    // Setup a server.
    let server_dir = create_temp_dir();
    setup_pre_genesis_repository(&server_dir, fi.reserved_state.clone()).await;
    // Add push configs to server repository.
    run_command(format!(
        "cd {server_dir} && git config receive.advertisePushOptions true"
    ))
    .await;
    run_command(format!(
        "cd {server_dir} && git config sendpack.sideband false"
    ))
    .await;

    let cli_path = build_simperby_cli();
    run_command(format!("{cli_path} {server_dir} genesis")).await;
    run_command(format!("{cli_path} {server_dir} init")).await;

    // Setup clients.
    let mut clients_path = Vec::new();
    for (_, key) in keys.iter().take(3) {
        let dir = create_temp_dir();
        clients_path.push(dir.clone());
        run_command(format!("cp -a {server_dir}/. {dir}/")).await;

        let config = Config {
            peers: peers.clone(),
        };
        let config = serde_spb::to_string(&config).unwrap();
        let auth = Auth {
            private_key: key.clone(),
        };
        let auth = serde_spb::to_string(&auth).unwrap();
        tokio::fs::write(format!("{dir}/{}", ".simperby/config.json"), config.clone())
            .await
            .unwrap();
        tokio::fs::write(format!("{dir}/.simperby/auth.json"), auth.clone())
            .await
            .unwrap();

        run_command(format!(
            "cd {dir} && git remote add server git://127.0.0.1:{port}/"
        ))
        .await;
    }

    // Add files for cli.
    let config = Config { peers: Vec::new() };
    let config = serde_spb::to_string(&config).unwrap();
    let auth = Auth {
        private_key: keys[3].1.clone(),
    };
    let auth = serde_spb::to_string(&auth).unwrap();
    let server_config = serde_spb::to_string(&server_config).unwrap();
    tokio::fs::write(
        format!("{server_dir}/.simperby/config.json"),
        config.clone(),
    )
    .await
    .unwrap();
    tokio::fs::write(format!("{server_dir}/.simperby/auth.json"), auth.clone())
        .await
        .unwrap();
    tokio::fs::write(
        format!("{server_dir}/.simperby/server_config.json"),
        server_config.clone(),
    )
    .await
    .unwrap();

    // Start the server.
    let mut child = tokio::process::Command::new(cli_path.clone())
        .arg(server_dir)
        .arg("serve")
        .spawn()
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Step 1: create an agenda and propagate it
    log::info!("STEP 1");
    run_command(format!("{cli_path} {} create agenda", clients_path[0])).await;
    let raw_repo = RawRepository::open(clients_path[0].clone().as_str())
        .await
        .unwrap();
    let branches = raw_repo
        .list_branches()
        .await
        .unwrap()
        .into_iter()
        .filter(|b| b.starts_with("a-"))
        .collect::<Vec<String>>();
    let agenda_branch = branches.first().unwrap();

    sync_each_other(cli_path.clone(), clients_path.clone()).await;
    for path in clients_path.clone() {
        run_command(format!("{cli_path} {path} vote {agenda_branch}")).await;
    }
    sync_each_other(cli_path.clone(), clients_path.clone()).await;

    // Step 2: create block and run consensus
    log::info!("STEP 2");
    run_command(format!("{cli_path} {} create block", clients_path[0])).await;
    sync_each_other(cli_path.clone(), clients_path.clone()).await;
    for path in clients_path.clone() {
        run_command(format!("{cli_path} {path} consensus")).await;
    }
    sync_each_other(cli_path.clone(), clients_path.clone()).await;
    for path in clients_path.clone() {
        run_command(format!("{cli_path} {path} consensus")).await;
    }
    sync_each_other(cli_path.clone(), clients_path.clone()).await;
    for path in clients_path.clone() {
        run_command(format!("{cli_path} {path} consensus")).await;
    }
    sync_each_other(cli_path.clone(), clients_path.clone()).await;
    for path in clients_path.clone() {
        run_command(format!("{cli_path} {path} consensus")).await;
    }
    sync_each_other(cli_path.clone(), clients_path.clone()).await;

    // Step 3: check the result
    log::info!("STEP 3");
    for path in clients_path {
        let raw_repo = RawRepository::open(&path).await.unwrap();
        let finalized = raw_repo
            .locate_branch("finalized".to_owned())
            .await
            .unwrap();
        let title = raw_repo
            .read_semantic_commit(finalized)
            .await
            .unwrap()
            .title;
        assert_eq!(title, ">block: 1");
    }

    // Stop the server.
    child.kill().await.unwrap();
    child.wait().await.unwrap();
}
