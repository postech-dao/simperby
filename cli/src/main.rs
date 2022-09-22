mod cli;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    println!("{:?}", args);
    Ok(())
}
