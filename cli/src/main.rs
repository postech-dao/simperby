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
        Commands::Show { commit: _ } => todo!(),
        Commands::Consensus { show: _ } => todo!(),
        Commands::Relay => todo!(),
        Commands::Fetch => todo!(),
        _ => unimplemented!(),
    }
    #[allow(unreachable_code)]
    Ok(())
}
