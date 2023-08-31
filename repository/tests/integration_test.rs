use simperby_core::*;
use simperby_network::{dms, ClientNetworkConfig, Dms};
use simperby_repository::{format::from_semantic_commit, raw::*, server::*, *};
use simperby_test_suite::*;
use std::sync::Arc;
use tokio::sync::RwLock;

async fn sync_dms(client_nodes: &mut [(DistributedRepository, ClientNetworkConfig, String)]) {
    for (drepo, network_config, _) in client_nodes.iter_mut() {
        drepo.flush().await.unwrap();
        dms::DistributedMessageSet::broadcast(drepo.get_dms().unwrap(), network_config)
            .await
            .unwrap();
    }
    for (drepo, network_config, _) in client_nodes.iter_mut() {
        dms::DistributedMessageSet::fetch(drepo.get_dms().unwrap(), network_config)
            .await
            .unwrap();
        drepo.update(false).await.unwrap();
    }
    for (_, _, client_node_dir) in client_nodes {
        println!("\n\n");
        simperby_test_suite::run_command(format!(
            "cd {client_node_dir} && git log --all --decorate --oneline --graph"
        ))
        .await;
    }
}

#[tokio::test]
#[ignore]
async fn sync_by_dms() {
    setup_test();

    let network_id = "repository".to_string();
    let ((server_network_config, server_private_key), client_network_configs_and_keys, members, _) =
        setup_server_client_nodes(network_id.clone(), 4).await;
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
        Some(Arc::new(RwLock::new(
            create_test_dms(
                network_id.clone(),
                members.clone(),
                server_private_key.clone(),
            )
            .await,
        ))),
        Arc::new(RwLock::new(
            RawRepository::open(&server_node_dir).await.unwrap(),
        )),
        config.clone(),
        None,
    )
    .await
    .unwrap();
    let serve_task = tokio::spawn(async move {
        let task = tokio::spawn(Dms::serve(
            server_node_repo.get_dms().unwrap(),
            server_network_config,
        ));
        sleep_ms(5000).await;
        task.abort();
        let _ = task.await;
        server_node_repo.update(false).await.unwrap();
        assert_eq!(
            server_node_repo
                .read_last_finalization_info()
                .await
                .unwrap()
                .header
                .height,
            1
        );
    });

    let mut client_nodes = Vec::new();
    for (network_config, private_key) in client_network_configs_and_keys {
        let client_node_dir = create_temp_dir();
        simperby_test_suite::run_command(format!("cp -a {server_node_dir}/. {client_node_dir}/"))
            .await;

        client_nodes.push((
            DistributedRepository::new(
                Some(Arc::new(RwLock::new(
                    create_test_dms(network_id.clone(), members.clone(), private_key.clone()).await,
                ))),
                Arc::new(RwLock::new(
                    RawRepository::open(&client_node_dir).await.unwrap(),
                )),
                config.clone(),
                Some(keys[0].1.clone()),
            )
            .await
            .unwrap(),
            network_config,
            client_node_dir,
        ));
    }

    // Step 0: create an agenda and let the client push that
    let (agenda, agenda_commit) = client_nodes[0]
        .0
        .create_agenda(rs.query_name(&keys[0].0).unwrap())
        .await
        .unwrap();
    sync_dms(client_nodes.as_mut_slice()).await;
    for (client_node, _, _) in &client_nodes {
        assert_eq!(
            client_node.read_agendas().await.unwrap(),
            vec![(agenda_commit, agenda.to_hash256())]
        );
    }

    let agenda_proof_commit = client_nodes[0]
        .0
        .approve(
            &agenda.to_hash256(),
            keys.iter()
                .map(|(_, private_key)| TypedSignature::sign(&agenda, private_key).unwrap())
                .collect(),
            simperby_core::utils::get_timestamp(),
        )
        .await
        .unwrap();
    simperby_test_suite::run_command(format!(
        "cd {} && git reset --hard {agenda_proof_commit}",
        client_nodes[0].2
    ))
    .await;

    // Step 1: create a block and let the client push that
    let (block, block_commit) = client_nodes[0]
        .0
        .create_block(keys[0].0.clone())
        .await
        .unwrap();
    sync_dms(client_nodes.as_mut_slice()).await;
    for (client_node, _, _) in &client_nodes {
        assert_eq!(
            client_node.read_blocks().await.unwrap(),
            vec![(block_commit, block.to_hash256())]
        );
    }

    let agenda_proof = from_semantic_commit(
        client_nodes[0]
            .0
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
    sync_dms(client_nodes.as_mut_slice()).await;
    for (client_node, _, _) in &client_nodes {
        assert_eq!(
            client_node
                .read_governance_approved_agendas()
                .await
                .unwrap(),
            vec![(agenda_proof_commit, agenda_proof.to_hash256()),]
        );
    }

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
    client_nodes[0]
        .0
        .finalize(
            block_commit,
            FinalizationProof {
                signatures,
                round: 0,
            },
        )
        .await
        .unwrap();
    sync_dms(client_nodes.as_mut_slice()).await;
    for (client_node, _, _) in &client_nodes {
        assert_eq!(
            client_node
                .read_last_finalization_info()
                .await
                .unwrap()
                .header,
            block
        );
    }
    serve_task.await.unwrap();
}

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
        None,
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
        None,
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
        .create_agenda(rs.query_name(&keys[3].0).unwrap())
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
        .create_block(keys[3].0.clone())
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
        None,
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
        None,
        Arc::new(RwLock::new(
            RawRepository::open(&client_node_dir).await.unwrap(),
        )),
        config,
        Some(keys[0].1.clone()),
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
        vec![(agenda_proof_commit, agenda_proof.to_hash256()),]
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

async fn sync_each_other(paths: &[String], client_drepos: &mut [DistributedRepository]) {
    for client_drepo in client_drepos.iter_mut() {
        client_drepo.broadcast().await.unwrap();
    }
    sleep_ms(200).await;
    for path in paths.iter() {
        simperby_test_suite::run_command(format!("cd {path} && git fetch --all --prune")).await;
    }
    sleep_ms(200).await;
    for client_drepo in client_drepos.iter_mut() {
        client_drepo.sync_all().await.unwrap();
    }
    sleep_ms(200).await;
}

// Make two blocks with multiple agendas.
#[tokio::test]
async fn sync_by_push_and_fetch() {
    setup_test();
    let port = dispense_port();
    let (rs, keys) = test_utils::generate_standard_genesis(4);
    let config = Config {
        long_range_attack_distance: 1,
    };

    // Setup repository and server.
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
        None,
        Arc::new(RwLock::new(
            RawRepository::open(&server_node_dir).await.unwrap(),
        )),
        config.clone(),
        None,
    )
    .await
    .unwrap();

    // Setup clients.
    let mut client_dirs = Vec::new();
    let mut client_drepos = Vec::new();
    for (_, key) in keys.iter().take(3) {
        let client_node_dir = create_temp_dir();
        simperby_test_suite::run_command(format!("cp -a {server_node_dir}/. {client_node_dir}/"))
            .await;
        simperby_test_suite::run_command(format!(
            "cd {client_node_dir} && git remote add peer git://127.0.0.1:{port}/"
        ))
        .await;

        let client_node_repo = DistributedRepository::new(
            None,
            Arc::new(RwLock::new(
                RawRepository::open(&client_node_dir).await.unwrap(),
            )),
            config.clone(),
            Some(key.clone()),
        )
        .await
        .unwrap();
        client_dirs.push(client_node_dir);
        client_drepos.push(client_node_repo);
    }

    // Run server.
    let _git_server = simperby_repository::server::run_server(
        &server_node_dir,
        port,
        PushVerifier::VerifierExecutable(build_simple_git_server()),
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Make a first block with multiple agendas, Step 0 ~ 2.
    // Step 0: create multiple agendas at different times and let the client update that
    let (agenda1, agenda_commit1) = client_drepos[0]
        .create_agenda(rs.query_name(&keys[0].0).unwrap())
        .await
        .unwrap();

    sync_each_other(&client_dirs, &mut client_drepos).await;
    for drepo in client_drepos.iter_mut() {
        drepo.vote(agenda_commit1).await.unwrap();
    }
    let (_agenda2, _agenda_commit2) = client_drepos[1]
        .create_agenda(rs.query_name(&keys[1].0).unwrap())
        .await
        .unwrap();
    sync_each_other(&client_dirs, &mut client_drepos).await;
    let (_agenda3, _agenda_commit3) = client_drepos[2]
        .create_agenda(rs.query_name(&keys[2].0).unwrap())
        .await
        .unwrap();
    for (timestamp, client_drepo) in client_drepos.iter_mut().enumerate() {
        client_drepo
            .approve(
                &agenda1.to_hash256(),
                keys.iter()
                    .map(|(_, private_key)| TypedSignature::sign(&agenda1, private_key).unwrap())
                    .collect(),
                timestamp.try_into().unwrap(),
            )
            .await
            .unwrap();
    }
    assert_eq!(server_node_repo.read_agendas().await.unwrap().len(), 2);
    assert_eq!(client_drepos[0].read_agendas().await.unwrap().len(), 2);
    assert_eq!(client_drepos[1].read_agendas().await.unwrap().len(), 2);
    assert_eq!(client_drepos[2].read_agendas().await.unwrap().len(), 3);

    // Step 1: create a block and let the client push that
    let (block, block_commit) = client_drepos[0]
        .create_block(keys[0].0.clone())
        .await
        .unwrap();
    sync_each_other(&client_dirs, &mut client_drepos).await;
    assert_eq!(
        server_node_repo.read_blocks().await.unwrap(),
        vec![(block_commit, block.to_hash256())]
    );
    for client_drepo in client_drepos.iter().take(3) {
        assert_eq!(
            client_drepo.read_blocks().await.unwrap(),
            vec![(block_commit, block.to_hash256())]
        );
    }

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
    client_drepos[0]
        .finalize(
            block_commit,
            FinalizationProof {
                signatures,
                round: 0,
            },
        )
        .await
        .unwrap();
    sync_each_other(&client_dirs, &mut client_drepos).await;
    assert_eq!(
        server_node_repo
            .read_last_finalization_info()
            .await
            .unwrap()
            .header,
        block
    );
    for client_drepo in client_drepos.iter().take(3) {
        assert_eq!(
            client_drepo
                .read_last_finalization_info()
                .await
                .unwrap()
                .header,
            block
        );
    }

    // Make a second block, Step 3 ~ 5.
    // Step 3: create an agenda and let the client update that
    let (agenda1, agenda_commit1) = client_drepos[1]
        .create_agenda(rs.query_name(&keys[1].0).unwrap())
        .await
        .unwrap();
    sync_each_other(&client_dirs, &mut client_drepos).await;
    assert_eq!(
        server_node_repo.read_agendas().await.unwrap(),
        vec![(agenda_commit1, agenda1.to_hash256())]
    );
    for client_drepo in client_drepos.iter().take(3) {
        assert_eq!(
            client_drepo.read_agendas().await.unwrap(),
            vec![(agenda_commit1, agenda1.to_hash256())]
        );
    }

    for drepo in client_drepos.iter_mut() {
        drepo.vote(agenda_commit1).await.unwrap();
    }
    sync_each_other(&client_dirs, &mut client_drepos).await;
    for (timestamp, client_drepo) in client_drepos.iter_mut().enumerate() {
        client_drepo
            .approve(
                &agenda1.to_hash256(),
                keys.iter()
                    .map(|(_, private_key)| TypedSignature::sign(&agenda1, private_key).unwrap())
                    .collect(),
                timestamp.try_into().unwrap(),
            )
            .await
            .unwrap();
    }

    // Step 4: create a block and let the client push that
    let (block, block_commit) = client_drepos[1]
        .create_block(keys[1].0.clone())
        .await
        .unwrap();
    sync_each_other(&client_dirs, &mut client_drepos).await;
    assert_eq!(
        server_node_repo.read_blocks().await.unwrap(),
        vec![(block_commit, block.to_hash256())]
    );
    for client_drepo in client_drepos.iter().take(3) {
        assert_eq!(
            client_drepo.read_blocks().await.unwrap(),
            vec![(block_commit, block.to_hash256())]
        );
    }

    // Step 5: finalize a block and let the client update that
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
    client_drepos[1]
        .finalize(
            block_commit,
            FinalizationProof {
                signatures,
                round: 0,
            },
        )
        .await
        .unwrap();
    sync_each_other(&client_dirs, &mut client_drepos).await;
    assert_eq!(
        server_node_repo
            .read_last_finalization_info()
            .await
            .unwrap()
            .header,
        block
    );
    for client_drepo in client_drepos.iter().take(3) {
        assert_eq!(
            client_drepo
                .read_last_finalization_info()
                .await
                .unwrap()
                .header,
            block
        );
    }
}
