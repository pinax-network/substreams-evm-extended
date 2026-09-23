//! Retained-state check for packaged `native_balances` output (#7).
//!
//! A verified checkpoint is produced with `eth_getBalance` at the canonical
//! hash of the block before the window and loaded through
//! `evm_retention::Ledger::seed_checkpoint`. The window's packaged rows are
//! then applied block by block, including blocks without output (their clock
//! comes from the header), and at the chosen blocks every tracked account's
//! retained value is compared with `eth_getBalance` at that block's hash.
//! Tracked accounts are those emitted in the window plus optional extra
//! accounts the window never emits: their checkpoint value must survive
//! unchanged, because absence of a row means no persisted change, never zero.
//! Host-only; the RPC URL comes from `RPC_URL` and is never written out.
use crate::live::Chain;
use anyhow::{ensure, Context, Result};
use clap::Args;
use evm_retention::{Clock, Domain, Key, Ledger, Lookup};
use proto::pb::evm::balances::v1 as balances;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct RetentionCheck {
    /// `substreams run … --final-blocks-only -o jsonl --bytes-encoding hex` output files.
    #[arg(long, required = true)]
    pub events: Vec<PathBuf>,
    /// First block of the window; the checkpoint is taken at `from - 1`.
    #[arg(long)]
    pub from: u64,
    /// Last block of the window (inclusive).
    #[arg(long)]
    pub to: u64,
    /// Extra accounts (one `0x…` address per line) tracked from the checkpoint.
    #[arg(long)]
    pub extra_accounts: Option<PathBuf>,
    /// Compare retained values with RPC at this many evenly spaced blocks
    /// (the last block always included).
    #[arg(long, default_value_t = 4)]
    pub checks: u64,
    #[arg(long, default_value_t = 200)]
    pub batch: usize,
    /// Fresh output directory for `report.json`.
    #[arg(long)]
    pub output: PathBuf,
}

fn bytes(hex_str: &str) -> Result<Vec<u8>> {
    hex::decode(hex_str.trim_start_matches("0x")).context("hex")
}

pub fn run(args: RetentionCheck) -> Result<bool> {
    let rpc = crate::live::rpc_from_env()?;
    check(&args, &rpc)
}

pub fn check(args: &RetentionCheck, rpc: &dyn Chain) -> Result<bool> {
    ensure!(
        !args.output.exists() || std::fs::read_dir(&args.output)?.next().is_none(),
        "output directory must be fresh"
    );
    ensure!(args.from > 0 && args.to >= args.from && args.batch > 0 && args.checks > 0, "invalid window");
    std::fs::create_dir_all(&args.output)?;
    let mut rows: BTreeMap<u64, Vec<(String, String)>> = BTreeMap::new();
    for path in &args.events {
        for line in std::fs::read_to_string(path)?.lines().filter(|l| !l.trim().is_empty()) {
            let row: Value = serde_json::from_str(line)?;
            let n = row["@block"].as_u64().context("@block")?;
            if n < args.from || n > args.to {
                continue;
            }
            let list: Vec<(String, String)> = row["@data"]["balances"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|b| {
                    Ok((
                        b["address"].as_str().context("address")?.to_lowercase(),
                        b["amount"].as_str().context("amount")?.to_string(),
                    ))
                })
                .collect::<Result<_>>()?;
            ensure!(rows.insert(n, list).is_none(), "block {n} delivered twice");
        }
    }
    let emitted: BTreeSet<String> = rows.values().flatten().map(|(a, _)| a.clone()).collect();
    let mut extra: BTreeSet<String> = BTreeSet::new();
    if let Some(path) = &args.extra_accounts {
        for line in std::fs::read_to_string(path)?.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let a = line.to_lowercase();
            if !emitted.contains(&a) {
                extra.insert(a);
            }
        }
    }
    let accounts: Vec<String> = emitted.iter().chain(extra.iter()).cloned().collect();

    // Canonical headers of the checkpoint block and the window.
    let numbers: Vec<u64> = (args.from - 1..=args.to).collect();
    let mut headers: BTreeMap<u64, (String, String)> = BTreeMap::new();
    for chunk in numbers.chunks(args.batch) {
        let calls: Vec<(String, Value)> = chunk
            .iter()
            .map(|n| ("eth_getBlockByNumber".to_string(), json!([format!("{n:#x}"), false])))
            .collect();
        for (n, h) in chunk.iter().zip(rpc.batch(&calls)?) {
            headers.insert(
                *n,
                (
                    h["hash"].as_str().context("hash")?.to_string(),
                    h["parentHash"].as_str().context("parent")?.to_string(),
                ),
            );
        }
    }
    let balances_at = |n: u64| -> Result<Vec<String>> {
        let mut out = Vec::with_capacity(accounts.len());
        for chunk in accounts.chunks(args.batch) {
            let calls: Vec<(String, Value)> = chunk
                .iter()
                .map(|a| ("eth_getBalance".to_string(), json!([a, {"blockHash": headers[&n].0, "requireCanonical": true}])))
                .collect();
            for v in rpc.batch(&calls)? {
                out.push(crate::live::quantity(&v)?);
            }
        }
        Ok(out)
    };

    // Seed the checkpoint at `from - 1`.
    let checkpoint = args.from - 1;
    let mut ledger = Ledger::new(64, Domain::Balances);
    let checkpoint_hash = bytes(&headers[&checkpoint].0)?;
    let seeded = balances_at(checkpoint)?;
    for (account, value) in accounts.iter().zip(&seeded) {
        ledger
            .seed_checkpoint(
                Key {
                    contract: None,
                    address: bytes(account)?,
                },
                value,
                checkpoint,
                &checkpoint_hash,
                "eth_getBalance at the checkpoint block hash (RPC_URL)",
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;
    }

    let span = args.to - args.from + 1;
    let mut at: BTreeSet<u64> = (1..args.checks).map(|i| args.from + span * i / args.checks - 1).collect();
    at.insert(args.to);
    let (mut compared, mut matched, mut unknown) = (0u64, 0u64, 0u64);
    let mut mismatches = Vec::new();
    let (mut applied_rows, mut empty_blocks) = (0u64, 0u64);
    let mut per_check = Vec::new();
    for n in args.from..=args.to {
        let (hash, parent) = &headers[&n];
        let clock = Clock {
            number: n,
            hash: bytes(hash)?,
            parent_hash: bytes(parent)?,
        };
        let list: Vec<balances::Balance> = rows
            .get(&n)
            .into_iter()
            .flatten()
            .map(|(a, v)| {
                Ok(balances::Balance {
                    contract: None,
                    address: bytes(a)?,
                    amount: v.clone(),
                })
            })
            .collect::<Result<_>>()?;
        if list.is_empty() {
            empty_blocks += 1;
        }
        applied_rows += list.len() as u64;
        ledger.apply(&clock, &list).map_err(|e| anyhow::anyhow!("block {n}: {e}"))?;
        if at.contains(&n) {
            let reference = balances_at(n)?;
            let (mut ok, mut bad) = (0u64, 0u64);
            for (account, want) in accounts.iter().zip(reference) {
                compared += 1;
                let key = Key {
                    contract: None,
                    address: bytes(account)?,
                };
                match ledger.lookup(&key) {
                    Lookup::Known(entry) if entry.value == want => {
                        matched += 1;
                        ok += 1;
                    }
                    Lookup::Known(entry) => {
                        bad += 1;
                        mismatches.push(json!({"block": n, "account": account, "retained": entry.value, "rpc": want}));
                    }
                    other => {
                        unknown += 1;
                        bad += 1;
                        mismatches.push(json!({"block": n, "account": account, "retained": format!("{other:?}"), "rpc": want}));
                    }
                }
            }
            per_check.push(json!({"block": n, "accounts": accounts.len(), "matched": ok, "mismatched": bad}));
        }
    }
    let passed = mismatches.is_empty() && unknown == 0;
    let report = json!({
        "tool": "native-balances-tools retention-check",
        "status": if passed { "passed" } else { "failed" },
        "window": {"from": args.from, "to": args.to, "blocks": span, "blocks_without_output": empty_blocks, "applied_rows": applied_rows},
        "checkpoint": {"block": checkpoint, "hash": headers[&checkpoint].0, "evidence": "eth_getBalance at the checkpoint block hash", "accounts": accounts.len()},
        "accounts": {"emitted_in_window": emitted.len(), "extra_never_emitted": extra.len()},
        "comparisons": {"blocks": per_check, "compared": compared, "matched": matched, "unknown": unknown, "mismatches": mismatches.len(), "examples": mismatches.iter().take(20).collect::<Vec<_>>()},
        "ledger": ledger.report(),
        "limits": "Retention is shown for the tracked accounts from a checkpoint at one block hash over one contiguous final window; it says nothing about accounts outside the checkpoint, which stay unknown.",
    });
    std::fs::write(args.output.join("report.json"), serde_json::to_string_pretty(&report)? + "\n")?;
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"status": report["status"], "window": report["window"], "accounts": report["accounts"], "comparisons": {"compared": compared, "matched": matched, "unknown": unknown, "mismatches": report["comparisons"]["mismatches"]}})
        )?
    );
    Ok(passed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::bail;

    /// A chain where account `aa` goes 5 -> 7 at block 11 and `bb` never changes.
    struct Fake {
        lie: bool,
    }
    impl Chain for Fake {
        fn batch(&self, calls: &[(String, Value)]) -> Result<Vec<Value>> {
            calls
                .iter()
                .map(|(method, params)| match method.as_str() {
                    "eth_getBlockByNumber" => {
                        let n = u64::from_str_radix(params[0].as_str().unwrap().trim_start_matches("0x"), 16).unwrap();
                        Ok(json!({"hash": format!("0x{n:064x}"), "parentHash": format!("0x{:064x}", n - 1)}))
                    }
                    "eth_getBalance" => {
                        let account = params[0].as_str().unwrap();
                        let n = u64::from_str_radix(&params[1]["blockHash"].as_str().unwrap()[2..], 16).unwrap();
                        let v = if account.ends_with("aa") {
                            if n >= 11 {
                                7
                            } else {
                                5
                            }
                        } else {
                            9 + u64::from(self.lie && n >= 12)
                        };
                        Ok(json!(format!("{v:#x}")))
                    }
                    other => bail!("unexpected {other}"),
                })
                .collect()
        }
    }
    fn setup(dir: &std::path::Path, output: &str) -> RetentionCheck {
        let events = dir.join("events.jsonl");
        // Block 11 changes `aa`; block 12 has no output line at all.
        let line =
            json!({"@module": "map_events", "@block": 11, "@data": {"balances": [{"address": "0x00000000000000000000000000000000000000aa", "amount": "7"}]}});
        std::fs::write(&events, line.to_string()).unwrap();
        let extra = dir.join("extra.txt");
        std::fs::write(&extra, "0x00000000000000000000000000000000000000bb\n").unwrap();
        RetentionCheck {
            events: vec![events],
            from: 10,
            to: 12,
            extra_accounts: Some(extra),
            checks: 3,
            batch: 2,
            output: dir.join(output),
        }
    }

    #[test]
    fn checkpoint_values_survive_blocks_without_rows_and_changes_are_applied() {
        let tmp = tempfile::tempdir().unwrap();
        let args = setup(tmp.path(), "ok");
        assert!(check(&args, &Fake { lie: false }).unwrap());
        let report: Value = serde_json::from_str(&std::fs::read_to_string(args.output.join("report.json")).unwrap()).unwrap();
        assert_eq!(report["window"]["blocks_without_output"], 2);
        assert_eq!(report["comparisons"]["compared"], 6);
        assert_eq!(report["ledger"]["checkpoint_seeded_holders"], 1);
    }

    #[test]
    fn an_unobserved_change_is_a_retention_mismatch_not_a_silent_pass() {
        let tmp = tempfile::tempdir().unwrap();
        let args = setup(tmp.path(), "bad");
        assert!(!check(&args, &Fake { lie: true }).unwrap());
    }
}
