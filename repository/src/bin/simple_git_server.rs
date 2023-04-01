use clap::{Parser, Subcommand};
use simperby_core::*;
use simperby_repository::{raw::*, *};
use std::sync::Arc;
use tokio::sync::RwLock;

/**
Welcome to the Simperby Simple Git Server!
*/
#[derive(Debug, Parser)]
#[clap(name = "simperby-simple-git-server")]
#[clap(about = "A Simperby simple git server CLI", long_about = None)]
pub struct Cli {
    pub path: std::path::PathBuf,
    #[clap(subcommand)]
    pub command: Commands,
}

/// Partially idential as the Simperby CLI.
#[derive(Debug, Subcommand)]
pub enum Commands {
    CheckPush {
        commit: String,
        branch: String,
        timestamp: u64,
        signature: String,
        signer: String,
    },
    NotifyPush {
        commit: String,
    },
    AfterPush {
        branch: String,
    },
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    let path = args.path.display().to_string();
    let raw = Arc::new(RwLock::new(RawRepository::open(&path).await.unwrap()));
    let config = Config {
        long_range_attack_distance: 1,
    };
    let mut drepo = simperby_repository::DistributedRepository::new(raw, config, None)
        .await
        .unwrap();

    match args.command {
        Commands::CheckPush {
            commit,
            branch,
            timestamp,
            signature,
            signer,
        } => {
            let commit_hash = CommitHash {
                hash: hex::decode(&commit).unwrap().try_into().unwrap(),
            };
            let signature: Signature = simperby_core::serde_spb::from_str(&signature).unwrap();
            let signer: PublicKey = simperby_core::serde_spb::from_str(&signer).unwrap();
            let typed_signature = TypedSignature::new(signature, signer);

            let result = drepo
                .test_push_eligibility(commit_hash, branch, timestamp as i64, typed_signature, 0)
                .await
                .unwrap();
            if result {
                std::process::exit(0);
            } else {
                std::process::exit(1);
            }
        }
        Commands::NotifyPush { commit } => {
            let commit_hash = CommitHash {
                hash: hex::decode(&commit).unwrap().try_into().unwrap(),
            };
            // TODO: handle inner Result<(), String>.
            let result = drepo.sync(commit_hash).await;
            match result {
                Ok(_) => std::process::exit(0),
                Err(_) => std::process::exit(1),
            }
        }
        Commands::AfterPush { branch } => {
            drepo
                .get_raw()
                .write()
                .await
                .delete_branch(branch)
                .await
                .unwrap();
        }
    }
    std::process::exit(0);
}
