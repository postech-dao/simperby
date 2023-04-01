use simperby_core::*;
use simperby_repository::{format::from_semantic_commit, raw::*, server::*, *};
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

    DistributedRepository::genesis(RawRepository::open(&server_node_dir).await.unwrap())
        .await
        .unwrap();
    let mut server_node_repo = DistributedRepository::new(
        Arc::new(RwLock::new(
            RawRepository::open(&server_node_dir).await.unwrap(),
        )),
        config.clone(),
        None,
    )
    .await
    .unwrap();
    let server_node_dir_clone = server_node_dir.clone();
    let git_server = tokio::spawn(async move {
        let _server = server::run_server_legacy(&server_node_dir_clone, port).await;
        sleep_ms(12000).await;
    });
    let client_node_dir = create_temp_dir();
    simperby_test_suite::run_command(format!("cp -a {server_node_dir}/. {client_node_dir}/")).await;

    simperby_test_suite::run_command(format!(
        "cd {client_node_dir} && git remote add peer git://127.0.0.1:{port}/"
    ))
    .await;

    let mut client_node_repo = DistributedRepository::new(
        Arc::new(RwLock::new(
            RawRepository::open(&client_node_dir).await.unwrap(),
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
    simperby_test_suite::run_command(format!("cd {client_node_dir} && git fetch --all")).await;
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
        "cd {server_node_dir} && git branch -f work {agenda_proof}"
    ))
    .await;

    // Step 1: create a block and let the client update that
    let (block, block_commit) = server_node_repo
        .create_block(keys[0].0.clone())
        .await
        .unwrap();
    simperby_test_suite::run_command(format!("cd {client_node_dir} && git fetch --all")).await;
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

    simperby_test_suite::run_command(format!("cd {client_node_dir} && git fetch --all")).await;
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
        long_range_attack_distance: 1,
    };
    let server_node_dir = create_temp_dir();
    setup_pre_genesis_repository(&server_node_dir, rs.clone()).await;
    // Add push configs to server repository.
    simperby_test_suite::run_command(format!(
        "cd {server_node_dir} && git config receive.advertisePushOptions true"
    ))
    .await;
    simperby_test_suite::run_command(format!(
        "cd {server_node_dir} && git config sendpack.sideband false"
    ))
    .await;

    DistributedRepository::genesis(RawRepository::open(&server_node_dir).await.unwrap())
        .await
        .unwrap();
    let server_node_repo = DistributedRepository::new(
        Arc::new(RwLock::new(
            RawRepository::open(&server_node_dir).await.unwrap(),
        )),
        config.clone(),
        None,
    )
    .await
    .unwrap();

    let _git_server = simperby_repository::server::run_server(
        &server_node_dir,
        port,
        PushVerifier::VerifierExecutable(build_simple_git_server()),
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let client_node_dir = create_temp_dir();
    simperby_test_suite::run_command(format!("cp -a {server_node_dir}/. {client_node_dir}/")).await;

    simperby_test_suite::run_command(format!(
        "cd {client_node_dir} && git remote add peer git://127.0.0.1:{port}/"
    ))
    .await;
    let mut client_node_repo = DistributedRepository::new(
        Arc::new(RwLock::new(
            RawRepository::open(&client_node_dir).await.unwrap(),
        )),
        config,
        Some(keys[1].1.clone()),
    )
    .await
    .unwrap();

    // Step 0: create an agenda and let the client push that
    let (agenda, agenda_commit) = client_node_repo
        .create_agenda(rs.query_name(&keys[0].0).unwrap())
        .await
        .unwrap();
    client_node_repo.broadcast().await.unwrap();
    assert_eq!(
        server_node_repo.read_agendas().await.unwrap(),
        vec![(agenda_commit, agenda.to_hash256())]
    );
    let agenda_proof_commit = client_node_repo
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
        "cd {client_node_dir} && git reset --hard {agenda_proof_commit}"
    ))
    .await;

    // Step 1: create a block and let the client push that
    let (block, block_commit) = client_node_repo
        .create_block(keys[0].0.clone())
        .await
        .unwrap();
    client_node_repo.broadcast().await.unwrap();
    assert_eq!(
        server_node_repo.read_blocks().await.unwrap(),
        vec![(block_commit, block.to_hash256())]
    );

    let agenda_proof = from_semantic_commit(
        client_node_repo
            .get_raw()
            .read()
            .await
            .read_semantic_commit(agenda_proof_commit)
            .await
            .unwrap(),
    )
    .unwrap();
    let agenda_proof = match agenda_proof {
        Commit::AgendaProof(agenda_proof) => Ok(agenda_proof),
        _ => Err("not an agenda proof commit"),
    }
    .unwrap();
    assert_eq!(
        server_node_repo
            .read_governance_approved_agendas()
            .await
            .unwrap(),
        vec![
            (agenda_proof_commit, agenda_proof.to_hash256()),
            (agenda_commit, agenda.to_hash256())
        ]
    );

    // Step 2: finalize a block and let the client push that
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
    client_node_repo
        .finalize(
            block_commit,
            FinalizationProof {
                signatures,
                round: 0,
            },
        )
        .await
        .unwrap();
    client_node_repo.broadcast().await.unwrap();
    assert_eq!(
        server_node_repo
            .read_last_finalization_info()
            .await
            .unwrap()
            .header,
        block
    );
}
