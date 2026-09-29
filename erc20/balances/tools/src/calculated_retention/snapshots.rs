//! Compile-time snapshots prevent a stale diagnostic binary from attributing
//! unchanged-on-disk but newer code or fixtures to its own execution.
use super::{binding, historical};
use anyhow::{ensure, Result};
use std::{fs, path::Path};
macro_rules! input {
    ($path:literal) => {
        ($path, include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../", $path)).as_slice())
    };
}
pub fn inputs() -> Vec<(&'static str, &'static [u8])> {
    let mut inputs = vec![
        input!("Cargo.toml"),
        input!("Cargo.lock"),
        input!("rust-toolchain.toml"),
        input!("erc20/balances/Cargo.toml"),
        input!("erc20/balances/tools/Cargo.toml"),
        input!("erc20/balances/tools/src/lib.rs"),
        input!("erc20/balances/tools/src/bin/replay_calculated_retention.rs"),
        input!("erc20/balances/tools/src/calculated_retention.rs"),
        input!("erc20/balances/tools/src/calculated_retention/binding.rs"),
        input!("erc20/balances/tools/src/calculated_retention/collect.rs"),
        input!("erc20/balances/tools/src/calculated_retention/historical.rs"),
        input!("erc20/balances/tools/src/calculated_retention/journal.rs"),
        input!("erc20/balances/tools/src/calculated_retention/source.rs"),
        input!("erc20/balances/tools/src/calculated_retention/snapshots.rs"),
        input!("erc20/balances/tools/src/lbp_rewards.rs"),
        input!("erc20/balances/tools/src/lbp_rewards/decay.rs"),
        input!("erc20/balances/tools/src/lbp_rewards/fixture.rs"),
        input!("erc20/balances/tools/src/reflection.rs"),
        input!("erc20/balances/tools/src/reflection/fixture.rs"),
        input!("erc20/balances/tools/src/data.rs"),
        input!("erc20/balances/tools/src/rpc.rs"),
        input!("erc20/balances/tools/src/survey.rs"),
        input!("erc20/balances/src/lib.rs"),
        input!("erc20/balances/src/persist.rs"),
        input!("common/persist/src/lib.rs"),
        input!("common/retention/Cargo.toml"),
        input!("common/retention/src/lib.rs"),
        input!("common/retention/src/protocol.rs"),
        input!("common/retention/src/positions.rs"),
        input!("erc20/balances/tools/tests/calculated_retention.rs"),
        input!("erc20/balances/tools/tests/calculated_retention_historical.rs"),
        input!("erc20/balances/tools/tests/calculated_retention_replay.rs"),
        input!("erc20/balances/tools/tests/fixtures/calculated-retention/journal.jsonl"),
        (
            "erc20/balances/tools/tests/fixtures/calculated-retention/lbp-source.json",
            binding::CAPTURES[0].3,
        ),
        (
            "erc20/balances/tools/tests/fixtures/calculated-retention/hlbp-source.json",
            binding::CAPTURES[1].3,
        ),
        (
            "erc20/balances/tools/tests/fixtures/calculated-retention/babydoge-source.json",
            binding::CAPTURES[2].3,
        ),
        (
            "erc20/balances/tools/tests/fixtures/calculated-retention/tenset-source.json",
            binding::CAPTURES[3].3,
        ),
    ];
    inputs.extend(historical::ORIGINALS.iter().map(|v| (v.path, v.raw)));
    inputs
}
pub fn verify(repo: &Path) -> Result<()> {
    for (path, compiled) in inputs() {
        ensure!(fs::read(repo.join(path))? == compiled, "stale compiled input {path}");
    }
    Ok(())
}
