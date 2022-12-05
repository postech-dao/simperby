mod cli;

use clap::Parser;
use cli::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    match args.command {
        Commands::Sync {
            last_finalization_proof: _,
        } => todo!(),
        Commands::Clean { .. } => todo!(),
        Commands::Create(CreateCommands::Agenda) => todo!(),
        Commands::Create(CreateCommands::Block) => todo!(),
        Commands::Show { commit } => show(commit).await?,
        Commands::Consensus { show: _ } => todo!(),
        Commands::Serve => todo!(),
        Commands::Fetch => todo!(),
        _ => unimplemented!(),
    }
    #[allow(unreachable_code)]
    Ok(())
}

async fn show(_commit_hash: String) -> anyhow::Result<()> {
    // For every type of commit,
    // 1. Show the content.
    // 2. Show the hash of it.
    //
    // For an agenda, show the governance status.
    // For a block, show the consensus status projected on this block.
    // For an extra-agenda transaction and a chat log, TODO.
    todo!()
}
