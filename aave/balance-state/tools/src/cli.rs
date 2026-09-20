use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Offline replay of the RPC-free Aave balance-state map over captured Extended blocks")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}
#[derive(Subcommand)]
pub enum Commands {
    /// Project captured blocks with an epoch configuration, check reserve and
    /// holder continuity across blocks and reproduce every stored index
    /// update with the conformance model. No network.
    Replay(crate::replay::Replay),
}
pub fn run() -> Result<bool> {
    match Cli::parse().command {
        Commands::Replay(args) => crate::replay::run(args),
    }
}
