use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Offline replay and live same-block parity for the RPC-free native balance map")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}
#[derive(Subcommand)]
pub enum Commands {
    /// Reduce captured Extended block files, check cross-block continuity and
    /// compare against saved RPC and historical prototype rows. No network.
    Replay(crate::replay::Replay),
    /// Check packaged `substreams run` output against saved controls and
    /// `eth_getBalance` at each block's canonical hash. Reads `RPC_URL`
    /// (never printed).
    LiveParity(crate::live::LiveParity),
}
pub fn run() -> Result<bool> {
    match Cli::parse().command {
        Commands::Replay(args) => crate::replay::run(args),
        Commands::LiveParity(args) => crate::live::run(args),
    }
}
