//! Compile-time evidence closure: reject a stale executable even when the disk
//! inventory remains unchanged throughout its run.
use anyhow::{ensure, Result};
use std::{fs, path::Path};
macro_rules! input {
    ($p:literal) => {
        ($p, include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../", $p)).as_slice())
    };
}
pub fn inputs() -> Vec<(&'static str, &'static [u8])> {
    vec![
        input!("Cargo.toml"),
        input!("Cargo.lock"),
        input!("rust-toolchain.toml"),
        input!("erc20/balances/Cargo.toml"),
        input!("erc20/balances/tools/Cargo.toml"),
        input!("erc20/balances/tools/src/lib.rs"),
        input!("erc20/balances/tools/src/bin/replay_ybc_retention.rs"),
        input!("erc20/balances/tools/src/ybc_retention/mod.rs"),
        input!("erc20/balances/tools/src/ybc_retention/binding.rs"),
        input!("erc20/balances/tools/src/ybc_retention/decode.rs"),
        input!("erc20/balances/tools/src/ybc_retention/collect.rs"),
        input!("erc20/balances/tools/src/ybc_retention/historical.rs"),
        input!("erc20/balances/tools/src/ybc_retention/journal.rs"),
        input!("erc20/balances/tools/src/ybc_retention/snapshots.rs"),
        input!("erc20/balances/tools/src/ybc_rewards.rs"),
        input!("erc20/balances/tools/src/ybc_rewards/fixture.rs"),
        input!("erc20/balances/tools/src/calculated_retention.rs"),
        input!("erc20/balances/tools/src/calculated_retention/binding.rs"),
        input!("erc20/balances/src/lib.rs"),
        input!("erc20/balances/src/persist.rs"),
        input!("common/persist/src/lib.rs"),
        input!("common/retention/Cargo.toml"),
        input!("common/retention/src/lib.rs"),
        input!("proto/Cargo.toml"),
        input!("proto/src/lib.rs"),
        input!("proto/src/pb/mod.rs"),
        input!("proto/src/pb/evm.balances.v1.rs"),
        input!("erc20/balances/tools/tests/ybc_retention.rs"),
        input!("erc20/balances/tools/tests/fixtures/ybc-retention/token-source.json"),
        input!("erc20/balances/tools/tests/fixtures/ybc-retention/prestate.json"),
        input!("erc20/balances/tools/tests/fixtures/ybc-retention/calls.json"),
        input!("erc20/balances/tools/tests/fixtures/ybc-retention/parent-storage.json"),
        input!("erc20/balances/tools/tests/fixtures/ybc-retention/final-storage.json"),
        input!("erc20/balances/tools/tests/fixtures/ybc-retention/historical.json"),
        input!("erc20/balances/tools/tests/fixtures/ybc-retention/overrides.json"),
        input!("erc20/balances/tools/tests/fixtures/ybc-retention/journal.jsonl"),
        input!("erc20/balances/tools/tests/fixtures/ybc-retention/original-block-manifest.json"),
    ]
}
pub fn verify(repo: &Path) -> Result<()> {
    for (path, compiled) in inputs() {
        ensure!(fs::read(repo.join(path))? == compiled, "stale compiled input {path}");
    }
    Ok(())
}
