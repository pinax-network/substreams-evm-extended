use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Offline replay and live same-block parity for the RPC-free Aave balance-state map")]
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
    /// Check packaged `substreams run` output against RPC getters at each
    /// row's exact block hash, and record the bound runtime identities.
    /// Reads `RPC_URL` (never printed).
    LiveParity(crate::live::LiveParity),
}
pub fn run() -> Result<bool> {
    match Cli::parse().command {
        Commands::Replay(args) => crate::replay::run(args),
        Commands::LiveParity(args) => crate::live::run(args),
    }
}
