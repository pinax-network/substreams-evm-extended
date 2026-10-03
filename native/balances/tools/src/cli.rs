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
    /// Seed a verified `eth_getBalance` checkpoint into the retention ledger,
    /// apply the packaged window and compare retained values with RPC at
    /// the chosen blocks. Reads `RPC_URL` (never printed).
    RetentionCheck(crate::retention::RetentionCheck),
    /// Run the native ClickHouse sink layout (sink tables, legacy tables,
    /// bridge views, TTL) in `clickhouse local` on captured blocks and compare
    /// the legacy tables with values computed from the same blocks. No network.
    ClickhouseBridge(crate::clickhouse::ClickhouseBridge),
    /// Run the deployed substreams-evm `db_out` WASM (empty ERC-20 input) and
    /// the native package's `db_out` WASM on the same inputs; require identical
    /// outputs, logs and panics. No network.
    DbOutParity(crate::db_out_parity::DbOutParity),
}
pub fn run() -> Result<bool> {
    match Cli::parse().command {
        Commands::Replay(args) => crate::replay::run(args),
        Commands::LiveParity(args) => crate::live::run(args),
        Commands::RetentionCheck(args) => crate::retention::run(args),
        Commands::ClickhouseBridge(args) => crate::clickhouse::run(args),
        Commands::DbOutParity(args) => crate::db_out_parity::run(args),
    }
}
