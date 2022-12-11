//! This module is temporary and will be removed in the future.

use simperby_node::{simperby_repository::CommitHash, *};
use std::io::{self, BufRead};

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

pub async fn run_genesis_proposer(config: Config, path: &str) {
    println!("STEP 0");
    let mut node = initialize(config, path).await.unwrap();
    node.create_agenda().await.unwrap();

    println!("PRESS ENTER TO RUN SERVER -------- [A]");
    get_input();
    println!("SERVE STARTED -------- [A]");
    let mut node = node.serve(15000).await.unwrap();
    println!("SERVE FINISHED");

    println!("STEP 1");
    node.create_block().await.unwrap();
    node.progress_for_consensus().await.unwrap();
    println!("PRESS ENTER TO RUN SERVER -------- [B]");
    get_input();
    println!("SERVE STARTED -------- [B]");
    let mut node = node.serve(15000).await.unwrap();
    println!("SERVE FINISHED");

    println!("STEP 2");
    let _ = node.progress_for_consensus().await;
    println!("PRESS ENTER TO RUN SERVER -------- [C]");
    get_input();
    println!("SERVE STARTED -------- [C]");
    let mut node = node.serve(15000).await.unwrap();
    println!("SERVE FINISHED");

    println!("STEP 3");
    let _ = node.progress_for_consensus().await;
    println!("PRESS ENTER TO RUN SERVER -------- [D]");
    get_input();
    println!("SERVE STARTED -------- [D]");
    let node = node.serve(15000).await.unwrap();
    println!("SERVE FINISHED");

    let result = node.get_consensus_status().await;
    println!("{:?}", result);
}

pub async fn run_genesis_non_proposer(config: Config, path: &str) {
    let mut node = initialize(config, path).await.unwrap();
    println!("PUT AGENDA COMMIT HASH AFTER THE [A] FLAG");
    let commit = get_commit_hash();
    node.fetch().await.unwrap();
    node.vote(commit).await.unwrap();

    println!("PRESS ENTER AFTER THE [B] FLAG");
    get_input();
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

    let result = node.get_consensus_status().await;
    println!("{:?}", result);
}
