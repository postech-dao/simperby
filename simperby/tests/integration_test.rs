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

/// Make only one block without server participation.
#[ignore]
#[tokio::test]
async fn normal_1() {
    setup_test();
    let (fi, keys) = test_utils::generate_fi(4);
    let server_config = generate_server_config();

    // Setup repository and server.
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

    // Setup clients.
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

    // Run server.
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

    // Setup peer network.
    sleep_ms(500).await;
    for client in clients.iter_mut() {
        client.update_peer().await.unwrap();
    }

    // Step 1: create an agenda and propagate it.
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

    // Step 2: create block and run consensus.
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

    // Step 3: check the result.
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

/// Make two blocks with server participation.
#[ignore]
#[tokio::test]
async fn normal_2() {
    setup_test();
    let (fi, keys) = test_utils::generate_fi(4);
    let server_config = generate_server_config();

    // Setup repository and server.
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

    // Setup clients.
    let mut clients = Vec::new();
    for (_, key) in keys.iter() {
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

    // Run server.
    let auth = Auth {
        private_key: keys[3].1.clone(),
    };
    let client = Client::open(&server_dir.clone(), Config {}, auth.clone())
        .await
        .unwrap();
    let server_task =
        client
            .serve(
                server_config.clone(),
                simperby_repository::server::PushVerifier::VerifierExecutable(
                    build_simple_git_server(),
                ),
            )
            .await
            .unwrap();

    // Setup peer network.
    sleep_ms(500).await;
    for client in clients.iter_mut() {
        client.update_peer().await.unwrap();
    }

    // Make a first block, Step 1 ~ 3.
    // Step 1: create an agenda and propagate it.
    log::info!("STEP 1");
    let (_, agenda_commit) = clients[0]
        .repository_mut()
        .create_agenda(fi.reserved_state.members[0].name.clone())
        .await
        .unwrap();

    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.vote(agenda_commit).await.unwrap();
    }
    sync_each_other(&mut clients).await;

    // Step 2: create block and run consensus.
    log::info!("STEP 2");
    let proposer_public_key = clients[0].auth().private_key.public_key();
    clients[0]
        .repository_mut()
        .create_block(proposer_public_key)
        .await
        .unwrap();
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;

    // Step 3: check the result.
    log::info!("STEP 3");
    for client in clients.iter() {
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

    // Stop and restart the server.
    server_task.abort();

    run_command(format!(
        "cd {server_dir}/.simperby/governance/dms/ && rm state.json"
    ))
    .await;
    run_command(format!(
        "cd {server_dir}/.simperby/consensus/dms/ && rm state.json"
    ))
    .await;
    run_command(format!(
        "cd {server_dir}/.simperby/consensus/state/ && rm state.json"
    ))
    .await;
    tokio::spawn(async move {
        let client = Client::open(&server_dir, Config {}, auth).await.unwrap();
        let task = client
            .serve(
                server_config,
                simperby_repository::server::PushVerifier::VerifierExecutable(
                    build_simple_git_server(),
                ),
            )
            .await
            .unwrap();
        task.await.unwrap().unwrap();
    });

    // Setup peer network.
    sleep_ms(500).await;
    for client in clients.iter_mut() {
        client.update_peer().await.unwrap();
    }

    // Make a second block, Step 4 ~ 6.
    // Step 4: create an agenda and propagate it.
    log::info!("STEP 4");
    let (_, agenda_commit) = clients[1]
        .repository_mut()
        .create_agenda(fi.reserved_state.members[1].name.clone())
        .await
        .unwrap();

    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.vote(agenda_commit).await.unwrap();
    }
    sync_each_other(&mut clients).await;

    // Step 5: create block and run consensus.
    log::info!("STEP 5");
    let proposer_public_key = clients[1].auth().private_key.public_key();
    clients[1]
        .repository_mut()
        .create_block(proposer_public_key)
        .await
        .unwrap();
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;

    // Step 6: check the result.
    log::info!("STEP 6");
    for client in clients.iter() {
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
        assert_eq!(title, ">block: 2");
    }
}

/// Make two blocks with server participation and premade one-block repository.
#[ignore]
#[tokio::test]
async fn normal_2_premade() {
    setup_test();
    let (fi, keys) = test_utils::generate_fi(4);
    let server_config = generate_server_config();

    // Setup repository and server.
    let server_dir = create_temp_dir();
    make_repository_with_one_block(fi.clone(), keys.clone(), server_dir.clone()).await;

    // Setup clients.
    let mut clients = Vec::new();
    for (_, key) in keys.iter() {
        let dir = create_temp_dir();
        run_command(format!("cp -a {server_dir}/. {dir}/")).await;
        let auth = Auth {
            private_key: key.clone(),
        };
        let port = server_config.peers_port;
        run_command(format!(
            "cd {dir}/.simperby/governance/dms/ && rm state.json"
        ))
        .await;
        run_command(format!(
            "cd {dir}/.simperby/consensus/dms/ && rm state.json"
        ))
        .await;
        run_command(format!(
            "cd {dir}/.simperby/consensus/state/ && rm state.json"
        ))
        .await;
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

    // Add push configs to server repository.
    run_command(format!(
        "cd {server_dir} && git config receive.advertisePushOptions true"
    ))
    .await;
    run_command(format!(
        "cd {server_dir} && git config sendpack.sideband false"
    ))
    .await;

    run_command(format!(
        "cd {server_dir}/.simperby/governance/dms/ && rm state.json"
    ))
    .await;
    run_command(format!(
        "cd {server_dir}/.simperby/consensus/dms/ && rm state.json"
    ))
    .await;
    run_command(format!(
        "cd {server_dir}/.simperby/consensus/state/ && rm state.json"
    ))
    .await;

    // Run server.
    let auth = Auth {
        private_key: keys[3].1.clone(),
    };
    let client = Client::open(&server_dir.clone(), Config {}, auth.clone())
        .await
        .unwrap();
    let server_task =
        client
            .serve(
                server_config.clone(),
                simperby_repository::server::PushVerifier::VerifierExecutable(
                    build_simple_git_server(),
                ),
            )
            .await
            .unwrap();

    // Setup peer network.
    sleep_ms(500).await;
    for client in clients.iter_mut() {
        client.update_peer().await.unwrap();
    }

    sync_each_other(&mut clients).await;

    // Make a first block, Step 1 ~ 3.
    // Step 1: create an agenda and propagate it.
    log::info!("STEP 1");
    let (_, agenda_commit) = clients[0]
        .repository_mut()
        .create_agenda(fi.reserved_state.members[0].name.clone())
        .await
        .unwrap();

    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.vote(agenda_commit).await.unwrap();
    }
    sync_each_other(&mut clients).await;

    // Step 2: create block and run consensus.
    log::info!("STEP 2");
    let proposer_public_key = clients[0].auth().private_key.public_key();
    clients[0]
        .repository_mut()
        .create_block(proposer_public_key)
        .await
        .unwrap();
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;

    // Step 3: check the result.
    log::info!("STEP 3");
    for client in clients.iter() {
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
        assert_eq!(title, ">block: 2");
    }

    // Stop and restart the server.
    server_task.abort();

    run_command(format!(
        "cd {server_dir}/.simperby/governance/dms/ && rm state.json"
    ))
    .await;
    run_command(format!(
        "cd {server_dir}/.simperby/consensus/dms/ && rm state.json"
    ))
    .await;
    run_command(format!(
        "cd {server_dir}/.simperby/consensus/state/ && rm state.json"
    ))
    .await;
    tokio::spawn(async move {
        let client = Client::open(&server_dir, Config {}, auth).await.unwrap();
        let task = client
            .serve(
                server_config,
                simperby_repository::server::PushVerifier::VerifierExecutable(
                    build_simple_git_server(),
                ),
            )
            .await
            .unwrap();
        task.await.unwrap().unwrap();
    });

    // Setup peer network.
    sleep_ms(500).await;
    for client in clients.iter_mut() {
        client.update_peer().await.unwrap();
    }

    // Make a second block, Step 4 ~ 6.
    // Step 4: create an agenda and propagate it.
    log::info!("STEP 4");
    let (_, agenda_commit) = clients[1]
        .repository_mut()
        .create_agenda(fi.reserved_state.members[1].name.clone())
        .await
        .unwrap();

    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.vote(agenda_commit).await.unwrap();
    }
    sync_each_other(&mut clients).await;

    // Step 5: create block and run consensus.
    log::info!("STEP 5");
    let proposer_public_key = clients[1].auth().private_key.public_key();
    clients[1]
        .repository_mut()
        .create_block(proposer_public_key)
        .await
        .unwrap();
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;
    for client in clients.iter_mut() {
        client.progress_for_consensus().await.unwrap();
    }
    sync_each_other(&mut clients).await;

    // Step 6: check the result.
    log::info!("STEP 6");
    for client in clients.iter() {
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
        assert_eq!(title, ">block: 3");
    }
}

async fn make_repository_with_one_block(
    fi: FinalizationInfo,
    keys: Vec<(PublicKey, PrivateKey)>,
    dir: String,
) {
    use simperby_core::verify::CommitSequenceVerifier;
    use simperby_repository::format::{fp_to_semantic_commit, to_semantic_commit};
    use simperby_repository::{
        BRANCH_NAME_HASH_DIGITS, FINALIZED_BRANCH_NAME, FP_BRANCH_NAME, TAG_NAME_HASH_DIGITS,
    };

    // Setup clients
    setup_pre_genesis_repository(&dir, fi.reserved_state.clone()).await;
    Client::genesis(&dir).await.unwrap();
    Client::init(&dir).await.unwrap();
    let auth = Auth {
        private_key: keys[3].1.clone(),
    };
    let mut client = Client::open(&dir, Config {}, auth).await.unwrap();

    let rs = fi.reserved_state;
    let genesis_info = rs.genesis_info.clone();
    let genesis_header = rs.genesis_info.header.clone();

    let mut csv = CommitSequenceVerifier::new(genesis_header.clone(), rs.clone()).unwrap();

    // Create agenda commit
    let transactions = Vec::new();
    let agenda = Agenda {
        height: 1,
        author: rs.query_name(&keys[3].0).unwrap(),
        timestamp: 0,
        transactions_hash: Agenda::calculate_transactions_hash(&transactions),
        previous_block_hash: csv.get_header().to_hash256(),
    };
    let agenda_commit = Commit::Agenda(agenda.clone());
    let semantic_commit = to_semantic_commit(&agenda_commit, rs.clone()).unwrap();

    let raw = client.repository_mut().get_raw();
    raw.write().await.checkout_clean().await.unwrap();
    let result = raw
        .write()
        .await
        .create_semantic_commit(semantic_commit)
        .await
        .unwrap();
    let mut agenda_branch_name = agenda_commit.to_hash256().to_string();
    agenda_branch_name.truncate(BRANCH_NAME_HASH_DIGITS);
    let agenda_branch_name = format!("a-{agenda_branch_name}");
    raw.write()
        .await
        .create_branch(agenda_branch_name.clone(), result)
        .await
        .unwrap();
    csv.apply_commit(&agenda_commit).unwrap();

    // Create tag
    let mut vote_tag_name = agenda_commit.to_hash256().to_string();
    vote_tag_name.truncate(TAG_NAME_HASH_DIGITS);
    let vote_tag_name = format!("vote-{vote_tag_name}");
    raw.write()
        .await
        .create_tag(vote_tag_name, result)
        .await
        .unwrap();

    // Create agenda proof commit
    for i in (0..3).rev() {
        let commit_hash = raw
            .read()
            .await
            .locate_branch(agenda_branch_name.clone())
            .await
            .unwrap();
        raw.write()
            .await
            .checkout_detach(commit_hash)
            .await
            .unwrap();
        let agenda_proof = AgendaProof {
            height: 1,
            agenda_hash: agenda.to_hash256(),
            proof: keys
                .iter()
                .map(|(_, private_key)| TypedSignature::sign(&agenda, private_key).unwrap())
                .collect::<Vec<_>>(),
            timestamp: i,
        };
        let agenda_proof_commit = Commit::AgendaProof(agenda_proof.clone());
        let semantic_commit = to_semantic_commit(&agenda_proof_commit, rs.clone()).unwrap();
        let result = raw
            .write()
            .await
            .create_semantic_commit(semantic_commit)
            .await
            .unwrap();

        let agenda_proof_branch_name = format!(
            "a-{}",
            &agenda_proof_commit.to_hash256().to_string()[0..BRANCH_NAME_HASH_DIGITS]
        );
        raw.write()
            .await
            .create_branch(agenda_proof_branch_name.clone(), result)
            .await
            .unwrap();
    }
    raw.write()
        .await
        .delete_branch(agenda_branch_name.clone())
        .await
        .unwrap();

    // Create block commit
    let block_header = BlockHeader {
        author: keys[3].0.clone(),
        prev_block_finalization_proof: genesis_info.genesis_proof,
        previous_hash: csv.get_header().to_hash256(),
        height: 1,
        timestamp: 0,
        commit_merkle_root: BlockHeader::calculate_commit_merkle_root(
            &csv.get_total_commits()[1..],
        ),
        repository_merkle_root: Hash256::zero(),
        validator_set: genesis_info.header.validator_set.clone(),
        version: genesis_info.header.version,
    };

    let block_commit = Commit::Block(block_header.clone());
    let semantic_commit = to_semantic_commit(&block_commit, rs.clone()).unwrap();
    let head = raw.read().await.get_head().await.unwrap();
    raw.write().await.checkout_clean().await.unwrap();
    raw.write().await.checkout_detach(head).await.unwrap();
    let result = raw
        .write()
        .await
        .create_semantic_commit(semantic_commit)
        .await
        .unwrap();
    let mut block_branch_name = block_commit.to_hash256().to_string();
    block_branch_name.truncate(BRANCH_NAME_HASH_DIGITS);
    let block_branch_name = format!("b-{block_branch_name}");
    raw.write()
        .await
        .create_branch(block_branch_name.clone(), result)
        .await
        .unwrap();
    raw.write().await.checkout(block_branch_name).await.unwrap();

    raw.write()
        .await
        .move_branch(FINALIZED_BRANCH_NAME.to_string(), result)
        .await
        .unwrap();

    // Create fp commit
    let signatures = keys
        .iter()
        .map(|(_, private_key)| {
            TypedSignature::sign(
                &FinalizationSignTarget {
                    block_hash: block_header.to_hash256(),
                    round: 0,
                },
                private_key,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let fp = FinalizationProof {
        round: 0,
        signatures,
    };
    raw.write()
        .await
        .move_branch(FP_BRANCH_NAME.to_string(), result)
        .await
        .unwrap();
    raw.write()
        .await
        .checkout(FP_BRANCH_NAME.into())
        .await
        .unwrap();
    raw.write()
        .await
        .create_semantic_commit(fp_to_semantic_commit(&LastFinalizationProof {
            height: 1,
            proof: fp.clone(),
        }))
        .await
        .unwrap();
    let commit_hash = raw
        .read()
        .await
        .locate_branch(FINALIZED_BRANCH_NAME.into())
        .await
        .unwrap();
    raw.write()
        .await
        .checkout_detach(commit_hash)
        .await
        .unwrap();
}
