//! This module is temporary and will be removed in the future.
#![allow(dead_code)]

use simperby_common::*;
use simperby_network::Peer;
use simperby_node::{simperby_repository::CommitHash, *};
use simperby_test_suite::*;
use std::io::{self, BufRead};
use tokio::io::AsyncWriteExt;

fn get_input() {
    let mut _buf = String::new();
    std::io::stdin().read_line(&mut _buf).unwrap();
}

fn get_commit_hash() -> CommitHash {
    loop {
        let mut buf = String::new();
        io::stdin().lock().read_line(&mut buf).unwrap();
        let x = buf.trim().to_owned();
        if x.len() != 40 {
            println!("INVALID LENGTH: {}", x.len());
        } else if hex::decode(&x).is_err() {
            println!("INVALID HEX");
        } else {
            return CommitHash {
                hash: hex::decode(x).unwrap().as_slice().try_into().unwrap(),
            };
        }
        println!("TRY AGAIN");
    }
}

async fn setup_peer(path: &str, peers: &[Peer]) {
    let mut file = tokio::fs::File::create(format!("{}/peers.json", path))
        .await
        .unwrap();
    file.write_all(&serde_spb::to_vec(&peers).unwrap())
        .await
        .unwrap();
    file.flush().await.unwrap();
}

pub async fn run_genesis_proposer(private_key: &str) {
    let private_key = PrivateKey::from_array(
        hex::decode(private_key)
            .unwrap()
            .as_slice()
            .try_into()
            .unwrap(),
    )
    .unwrap();

    let dir = create_temp_dir();
    println!("----------------------------DIRECTORY: {}", dir);
    setup_peer(&dir, &[]).await;
    run_command(format!("mkdir -p {}/repository", dir)).await;
    run_command(format!(
        "cd {}/repository && git clone https://github.com/postech-dao/pdao.git repo",
        dir
    ))
    .await;
    run_command(format!(
        "cd {}/repository/repo && git branch work origin/work && git branch fp origin/fp",
        dir
    ))
    .await;
    run_command(format!(
        "cd {}/repository/repo && git remote remove origin",
        dir
    ))
    .await;

    println!("STEP 0");
    let mut node = initialize(
        Config {
            chain_name: "PDAO-mainnet".to_owned(),
            public_key: private_key.public_key(),
            private_key,
            broadcast_interval_ms: None,
            fetch_interval_ms: None,
            public_repo_url: vec![],
            governance_port: 1155,
            consensus_port: 1166,
            repository_port: 1177,
        },
        &dir,
    )
    .await
    .unwrap();
    println!(
        "NETWORK -----------{}",
        serde_json::to_string(&node.network_config().ports).unwrap()
    );
    println!("PRESS ENTER TO CREATE AN AGENDA --------");
    get_input();
    node.create_agenda().await.unwrap();
    run_command(format!("cd {}/repository/repo && git show", dir)).await;

    println!("PRESS ENTER TO RUN SERVER -------- [A]");
    get_input();
    println!("SERVE STARTED -------- [A]");
    let mut node = node.serve(10000).await.unwrap();
    println!("SERVE FINISHED");
    node.fetch().await.unwrap();

    println!("STEP 1");
    run_command(format!(
        "cd {}/repository/repo && git branch -f work HEAD",
        dir
    ))
    .await;
    node.create_block().await.unwrap();
    node.progress_for_consensus().await.unwrap();
    println!("PRESS ENTER TO RUN SERVER -------- [B]");
    get_input();
    println!("SERVE STARTED -------- [B]");
    let mut node = node.serve(10000).await.unwrap();
    println!("SERVE FINISHED");

    println!("STEP 2");
    let _ = node.progress_for_consensus().await;
    println!("PRESS ENTER TO RUN SERVER -------- [C]");
    get_input();
    println!("SERVE STARTED -------- [C]");
    let mut node = node.serve(10000).await.unwrap();
    println!("SERVE FINISHED");

    println!("STEP 3");
    let _ = node.progress_for_consensus().await;
    println!("PRESS ENTER TO RUN SERVER -------- [D]");
    get_input();
    println!("SERVE STARTED -------- [D]");
    let _node = node.serve(10000).await.unwrap();
    println!("SERVE FINISHED");

    run_command(format!(
        "cd {}/repository/repo && git log --all --decorate --oneline --graph",
        dir
    ))
    .await;
}

pub async fn run_genesis_non_proposer(private_key: &str) {
    let private_key = PrivateKey::from_array(
        hex::decode(private_key)
            .unwrap()
            .as_slice()
            .try_into()
            .unwrap(),
    )
    .unwrap();

    let dir = create_temp_dir();
    println!("----------------------------DIRECTORY: {}", dir);
    setup_peer(&dir, &[]).await;
    run_command(format!("mkdir -p {}/repository", dir)).await;
    run_command(format!(
        "cd {}/repository && git clone https://github.com/postech-dao/pdao.git repo",
        dir
    ))
    .await;
    run_command(format!(
        "cd {}/repository/repo && git branch work origin/work && git branch fp origin/fp",
        dir
    ))
    .await;
    run_command(format!(
        "cd {}/repository/repo && git remote remove origin",
        dir
    ))
    .await;
    let ports =
        r#"{"dms-consensus-0-3f22f0a0":1166,"repository":1177,"dms-governance-0-3f22f0a0":1155}"#;
    let ports = serde_json::from_str(ports).unwrap();
    setup_peer(
        &dir,
        &[Peer {
            public_key: PublicKey::from_array(
                hex::decode("02e827d75e7586ab36f5c48c088b6b6c1f81fbb34272a4e53fd0dd6b56b19e15dd")
                    .unwrap()
                    .as_slice()
                    .try_into()
                    .unwrap(),
            )
            .unwrap(),
            name: "proposer".to_owned(),
            address: "43.201.28.183:1".parse().unwrap(),
            ports,
            message: "123".to_owned(),
            recently_seen_timestamp: 0,
        }],
    )
    .await;

    let mut node = initialize(
        Config {
            chain_name: "PDAO-mainnet".to_owned(),
            public_key: private_key.public_key(),
            private_key,
            broadcast_interval_ms: None,
            fetch_interval_ms: None,
            public_repo_url: vec![],
            governance_port: 1155,
            consensus_port: 1166,
            repository_port: 1177,
        },
        &dir,
    )
    .await
    .unwrap();
    println!("PUT AGENDA COMMIT HASH AFTER THE [A] FLAG");
    let commit = get_commit_hash();
    node.fetch().await.unwrap();
    node.vote(commit).await.unwrap();

    println!("PRESS ENTER AFTER THE [B] FLAG");
    get_input();
    node.fetch().await.unwrap();
    node.fetch().await.unwrap();
    let _ = node.progress_for_consensus().await;

    println!("PRESS ENTER AFTER THE [C] FLAG");
    get_input();
    node.fetch().await.unwrap();
    let _ = node.progress_for_consensus().await;

    println!("PRESS ENTER AFTER THE [D] FLAG");
    get_input();
    node.fetch().await.unwrap();
    let _ = node.progress_for_consensus().await;

    run_command(format!(
        "cd {}/repository/repo && git log --all --decorate --oneline --graph",
        dir
    ))
    .await;
}
