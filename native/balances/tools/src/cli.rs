use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Offline replay of the RPC-free native balance map over captured Extended blocks")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}
#[derive(Subcommand)]
pub enum Commands {
    /// Reduce captured Extended block files, check cross-block continuity and
    /// compare against saved RPC and historical prototype rows. No network.
    Replay(crate::replay::Replay),
}
pub fn run() -> Result<bool> {
    match Cli::parse().command {
        Commands::Replay(args) => crate::replay::run(args),
    }
}
