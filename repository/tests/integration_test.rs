use simperby_core::*;
use simperby_repository::{raw::*, *};
use simperby_test_suite::*;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::test]
async fn basic_1() {
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
