use clap::Parser;
use simperby_cli::cli::Cli;
fn main() {
    _ = Cli::parse_from(["--help"]);
}
