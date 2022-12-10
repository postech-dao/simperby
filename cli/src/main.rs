mod cli;

use clap::Parser;
use cli::*;
use eyre::{eyre, Result};
use simperby_node::{
    simperby_common::*, simperby_repository::CommitHash, CommitInfo, Config, SimperbyApi,
};

fn to_commit_hash(s: &str) -> Result<CommitHash> {
    let hash = hex::decode(s).map_err(|_| eyre!("invalid hash"))?;
    let hash = hash
        .as_slice()
        .try_into()
        .map_err(|_| eyre!("a hash must be in 20 bytes"))?;
    Ok(CommitHash { hash })
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args = cli::Cli::parse();
    let path = args.path.display().to_string();
    let config: Config =
        serde_json::from_str(&tokio::fs::read_to_string(&format!("{}/config.json", path)).await?)?;

    match args.command {
        Commands::Sync {
            last_finalization_proof: _,
        } => todo!(),
        Commands::Clean { .. } => todo!(),
        Commands::Create(CreateCommands::Agenda) => todo!(),
        Commands::Create(CreateCommands::Block) => todo!(),
        Commands::Show { commit } => show(config, &path, commit).await?,
        Commands::Consensus { show: _ } => todo!(),
        Commands::Serve => todo!(),
        Commands::Fetch => todo!(),
        Commands::Sign(SignCommands::Custom { hash }) => {
            let hash = Hash256::from_array(
                hex::decode(hash)?
                    .as_slice()
                    .try_into()
                    .map_err(|_| eyre!("a hash must be in 32 bytes"))?,
            );
            println!(
                "{}",
                Signature::sign(hash, &config.private_key).map_err(|_| eyre!("failed to sign"))?
            );
        }
        _ => unimplemented!(),
    }
    #[allow(unreachable_code)]
    Ok(())
}

/// For every type of commit,
/// 1. Show the content.
/// 2. Show the hash of it.
///
/// For an agenda, show the governance status.
/// For a block, show the consensus status projected on this block.
/// For an extra-agenda transaction and a chat log, TODO.
async fn show(config: Config, path: &str, commit_hash: String) -> Result<()> {
    let node = simperby_node::initialize(config, path).await?;
    let result = node.show(to_commit_hash(&commit_hash)?).await?;
    match result {
        CommitInfo::Block { block_header, .. } => {
            println!("hash: {}", block_header.to_hash256());
            // TODO
        }
        _ => todo!(),
    }
    Ok(())
}
