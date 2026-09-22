use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Offline replay of the RPC-free execution-facts map over captured Extended blocks")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}
#[derive(Subcommand)]
pub enum Commands {
    /// Project captured blocks, check storage context per frame type and
    /// tabulate what each producer version supplies. No network.
    Replay(crate::replay::Replay),
}
pub fn run() -> Result<bool> {
    match Cli::parse().command {
        Commands::Replay(args) => crate::replay::run(args),
    }
}
