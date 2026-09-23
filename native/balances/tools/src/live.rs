//! Same-block parity for packaged `native_balances` output.
//!
//! Reads `substreams run -o jsonl --bytes-encoding hex` output of the packed
//! map streamed with `--final-blocks-only`, binds each delivered block number
//! to its canonical hash through `eth_getBlockByNumber`, and checks emitted
//! rows against `eth_getBalance(address, {blockHash})` in JSON-RPC batches.
//! Every row of the blocks in `--full-from..=--full-to` is checked; outside
//! that range every `--sample-every`-th row (by account order) is checked.
//! Optionally compares the rows with an offline replay of saved blocks
//! (`rows.jsonl` of `native-balances-tools replay`) block for block.
//! Host-only; the RPC URL comes from `RPC_URL` and is never written out.
use anyhow::{bail, ensure, Context, Result};
use clap::Args;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Args, Debug)]
pub struct LiveParity {
    /// `substreams run … --final-blocks-only -o jsonl --bytes-encoding hex` output files.
    #[arg(long, required = true)]
    pub events: Vec<PathBuf>,
    /// The packed `.spkg` that produced the events (hashed into the report).
    #[arg(long)]
    pub spkg: PathBuf,
    /// Substreams endpoint host label for the report (no credentials).
    #[arg(long)]
    pub endpoint: String,
    /// Offline replay rows (`rows.jsonl`) of saved blocks to compare with.
    #[arg(long)]
    pub saved_rows: Option<PathBuf>,
    /// Check every row of blocks in this inclusive range.
    #[arg(long, default_value_t = 0)]
    pub full_from: u64,
    #[arg(long, default_value_t = 0)]
    pub full_to: u64,
    /// Outside the full range, check every n-th row of each block (0: none).
    #[arg(long, default_value_t = 0)]
    pub sample_every: usize,
    /// Requests per JSON-RPC batch.
    #[arg(long, default_value_t = 200)]
    pub batch: usize,
    /// Fresh output directory for `report.json`.
    #[arg(long)]
    pub output: PathBuf,
}

/// A JSON-RPC source; the live one reads `RPC_URL`, tests use a fake.
pub trait Chain {
    /// One request per `(method, params)`, answered in order.
    fn batch(&self, calls: &[(String, Value)]) -> Result<Vec<Value>>;
}

struct Rpc {
    agent: ureq::Agent,
    url: String,
    key: Option<String>,
}
impl Chain for Rpc {
    fn batch(&self, calls: &[(String, Value)]) -> Result<Vec<Value>> {
        let payload: Vec<Value> = calls
            .iter()
            .enumerate()
            .map(|(id, (method, params))| json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .collect();
        let mut request = self.agent.post(&self.url);
        if let Some(key) = &self.key {
            request = request.set("X-Api-Key", key);
        }
        let response = match request.send_json(Value::Array(payload)) {
            Ok(response) => response,
            Err(ureq::Error::Status(code, _)) => bail!("RPC batch: HTTP {code}"),
            Err(_) => bail!("RPC batch: transport failed"),
        };
        let rows: Value = response.into_json().map_err(|_| anyhow::anyhow!("RPC batch: invalid JSON"))?;
        let rows = rows.as_array().context("RPC batch: not an array")?;
        ensure!(rows.len() == calls.len(), "RPC batch: {} answers for {} requests", rows.len(), calls.len());
        let mut out = vec![Value::Null; calls.len()];
        for row in rows {
            let id = row["id"].as_u64().context("RPC batch: missing id")? as usize;
            ensure!(id < calls.len() && out[id].is_null(), "RPC batch: invalid or duplicate id");
            ensure!(
                row["error"].is_null() && !row["result"].is_null(),
                "RPC {} returned an error or null",
                calls[id].0
            );
            out[id] = row["result"].clone();
        }
        Ok(out)
    }
}

fn quantity(v: &Value) -> Result<String> {
    let hex = v.as_str().context("quantity")?.trim_start_matches("0x");
    ensure!(
        !hex.is_empty() && hex.len() <= 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid quantity"
    );
    // Base-1e9 limbs; a limb times 16 plus a carry fits in u64.
    let mut digits: Vec<u64> = vec![0];
    for c in hex.chars() {
        let mut carry = u64::from(c.to_digit(16).unwrap());
        for d in digits.iter_mut() {
            let v = *d * 16 + carry;
            *d = v % 1_000_000_000;
            carry = v / 1_000_000_000;
        }
        while carry > 0 {
            digits.push(carry % 1_000_000_000);
            carry /= 1_000_000_000;
        }
    }
    let mut s = digits.last().unwrap().to_string();
    for d in digits.iter().rev().skip(1) {
        s.push_str(&format!("{d:09}"));
    }
    Ok(s)
}

#[derive(Default)]
struct Tally {
    checked: u64,
    matched: u64,
    mismatches: Vec<Value>,
}
impl Tally {
    fn check(&mut self, what: &str, block: u64, emitted: String, reference: String, context: Value) {
        self.checked += 1;
        if emitted == reference {
            self.matched += 1;
        } else {
            self.mismatches
                .push(json!({"check": what, "block": block, "emitted": emitted, "reference": reference, "context": context}));
        }
    }
    fn json(&self) -> Value {
        json!({"checked": self.checked, "matched": self.matched, "mismatches": self.mismatches.len(), "mismatch_examples": self.mismatches.iter().take(20).collect::<Vec<_>>()})
    }
}

pub fn run(args: LiveParity) -> Result<bool> {
    let rpc = Rpc {
        agent: ureq::AgentBuilder::new().timeout(Duration::from_secs(120)).redirects(0).build(),
        url: std::env::var("RPC_URL").context("RPC_URL is required (it is never printed)")?,
        key: std::env::var("RPC_API_KEY").or_else(|_| std::env::var("SUBSTREAMS_API_KEY")).ok(),
    };
    check(&args, &rpc)
}

/// The parity check over any chain source; writes `report.json` and returns
/// whether every check matched.
pub fn check(args: &LiveParity, rpc: &dyn Chain) -> Result<bool> {
    ensure!(
        !args.output.exists() || std::fs::read_dir(&args.output)?.next().is_none(),
        "output directory must be fresh"
    );
    ensure!(args.batch > 0, "batch must be positive");
    std::fs::create_dir_all(&args.output)?;
    // block -> account -> amount, in delivered order.
    let mut blocks: BTreeMap<u64, BTreeMap<String, String>> = BTreeMap::new();
    let mut lines: BTreeMap<u64, String> = BTreeMap::new();
    for path in &args.events {
        for line in std::fs::read_to_string(path)?.lines().filter(|l| !l.trim().is_empty()) {
            let row: Value = serde_json::from_str(line).with_context(|| format!("invalid JSON line in {}", path.display()))?;
            let n = row["@block"].as_u64().context("line without @block")?;
            let mut accounts = BTreeMap::new();
            for b in row["@data"]["balances"].as_array().into_iter().flatten() {
                ensure!(b.get("contract").is_none_or(|c| c.is_null()), "native row with a contract at block {n}");
                let address = b["address"].as_str().context("address")?.to_lowercase();
                let amount = b["amount"].as_str().context("amount")?.to_string();
                ensure!(
                    accounts.insert(address.clone(), amount).is_none(),
                    "account {address} emitted twice at block {n}"
                );
            }
            ensure!(blocks.insert(n, accounts).is_none(), "block {n} delivered twice");
            lines.insert(n, line.to_string());
        }
    }
    ensure!(!blocks.is_empty(), "no events");
    let mut digest = Sha256::new();
    for line in lines.values() {
        digest.update(line.as_bytes());
        digest.update(b"\n");
    }
    let events_sha256 = hex::encode(digest.finalize());

    // Canonical hashes of every delivered block.
    let numbers: Vec<u64> = blocks.keys().copied().collect();
    let mut hashes: BTreeMap<u64, String> = BTreeMap::new();
    for chunk in numbers.chunks(args.batch) {
        let calls: Vec<(String, Value)> = chunk
            .iter()
            .map(|n| ("eth_getBlockByNumber".to_string(), json!([format!("{n:#x}"), false])))
            .collect();
        for (n, header) in chunk.iter().zip(rpc.batch(&calls)?) {
            let number = u64::from_str_radix(header["number"].as_str().context("header number")?.trim_start_matches("0x"), 16)?;
            ensure!(number == *n, "RPC returned block {number} for {n}");
            hashes.insert(*n, header["hash"].as_str().context("header hash")?.to_string());
        }
    }

    // Saved controls: the packaged rows equal the offline replay of the same blocks.
    let mut saved = Tally::default();
    let mut saved_blocks = 0u64;
    if let Some(path) = &args.saved_rows {
        let mut replay: BTreeMap<u64, BTreeMap<String, String>> = BTreeMap::new();
        for line in std::fs::read_to_string(path)?.lines().filter(|l| !l.trim().is_empty()) {
            let r: Value = serde_json::from_str(line)?;
            let n = r["block"].as_u64().context("saved row block")?;
            replay.entry(n).or_default().insert(
                r["address"].as_str().context("saved address")?.to_lowercase(),
                r["amount"].as_str().context("saved amount")?.to_string(),
            );
        }
        for (n, emitted) in &blocks {
            let Some(expected) = replay.get(n) else { continue };
            saved_blocks += 1;
            let accounts: BTreeSet<&String> = emitted.keys().chain(expected.keys()).collect();
            for account in accounts {
                saved.check(
                    "packaged row equals the offline replay of the saved block",
                    *n,
                    emitted.get(account).cloned().unwrap_or_else(|| "<absent>".into()),
                    expected.get(account).cloned().unwrap_or_else(|| "<absent>".into()),
                    json!({"account": account}),
                );
            }
        }
    }

    // Same-block RPC balances.
    let mut selected: Vec<(u64, String, String)> = Vec::new();
    let (mut full_rows, mut sampled_rows) = (0u64, 0u64);
    for (n, accounts) in &blocks {
        let full = *n >= args.full_from && *n <= args.full_to && args.full_to > 0;
        for (i, (account, amount)) in accounts.iter().enumerate() {
            if full {
                full_rows += 1;
                selected.push((*n, account.clone(), amount.clone()));
            } else if args.sample_every > 0 && i % args.sample_every == 0 {
                sampled_rows += 1;
                selected.push((*n, account.clone(), amount.clone()));
            }
        }
    }
    let mut balances = Tally::default();
    for chunk in selected.chunks(args.batch) {
        let calls: Vec<(String, Value)> = chunk
            .iter()
            .map(|(n, account, _)| {
                (
                    "eth_getBalance".to_string(),
                    json!([account, {"blockHash": hashes[n], "requireCanonical": true}]),
                )
            })
            .collect();
        for ((n, account, amount), result) in chunk.iter().zip(rpc.batch(&calls)?) {
            balances.check(
                "eth_getBalance at the block hash",
                *n,
                amount.clone(),
                quantity(&result)?,
                json!({"account": account}),
            );
        }
    }

    let rows: u64 = blocks.values().map(|a| a.len() as u64).sum();
    let zero_rows: u64 = blocks.values().flat_map(|a| a.values()).filter(|v| v.as_str() == "0").count() as u64;
    let accounts: BTreeSet<&String> = blocks.values().flat_map(|a| a.keys()).collect();
    let passed = saved.mismatches.is_empty() && balances.mismatches.is_empty();
    let report = json!({
        "tool": "native-balances-tools live-parity",
        "status": if passed { "passed" } else { "failed" },
        "events_sha256": events_sha256,
        "package": {"spkg_sha256": hex::encode(Sha256::digest(std::fs::read(&args.spkg)?))},
        "endpoints": {"substreams": args.endpoint, "rpc": "RPC_URL (credential-bearing; not recorded)", "block_binding": "streamed with --final-blocks-only; each number bound to its canonical hash by eth_getBlockByNumber"},
        "blocks": {"delivered": blocks.len(), "first": numbers.first(), "last": numbers.last(), "ranges": ranges(&numbers)},
        "rows": {"emitted": rows, "known_zero": zero_rows, "distinct_accounts": accounts.len()},
        "selection": {"full_from": args.full_from, "full_to": args.full_to, "full_rows": full_rows, "sample_every": args.sample_every, "sampled_rows": sampled_rows},
        "checks": {"saved_controls": {"blocks": saved_blocks, "result": saved.json()}, "eth_get_balance": balances.json()},
        "limits": "Checks the emitted accounts only: accounts without a persisted change in a block are not emitted and not checked (absence is no observation). Parity outside the full range is sampled.",
    });
    std::fs::write(args.output.join("report.json"), serde_json::to_string_pretty(&report)? + "\n")?;
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"status": report["status"], "blocks": report["blocks"]["delivered"], "rows": report["rows"], "checks": report["checks"]})
        )?
    );
    Ok(passed)
}

fn ranges(numbers: &[u64]) -> Vec<[u64; 2]> {
    let mut out: Vec<[u64; 2]> = Vec::new();
    for &n in numbers {
        match out.last_mut() {
            Some(r) if r[1] + 1 == n => r[1] = n,
            _ => out.push([n, n]),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Answers headers and balances from a map; `wrong` corrupts one balance.
    struct Fake {
        balances: BTreeMap<String, u128>,
        wrong: Option<String>,
    }
    impl Chain for Fake {
        fn batch(&self, calls: &[(String, Value)]) -> Result<Vec<Value>> {
            calls
                .iter()
                .map(|(method, params)| match method.as_str() {
                    "eth_getBlockByNumber" => Ok(json!({"number": params[0], "hash": format!("0x{:064x}", 7)})),
                    "eth_getBalance" => {
                        let account = params[0].as_str().unwrap().to_string();
                        let v = self.balances[&account] + u128::from(self.wrong.as_deref() == Some(account.as_str()));
                        Ok(json!(format!("{v:#x}")))
                    }
                    other => bail!("unexpected {other}"),
                })
                .collect()
        }
    }
    fn line(block: u64, rows: &[(&str, &str)]) -> String {
        let balances: Vec<Value> = rows.iter().map(|(a, v)| json!({"address": a, "amount": v})).collect();
        json!({"@module": "map_events", "@block": block, "@data": {"balances": balances}}).to_string()
    }
    fn setup(dir: &std::path::Path, output: &str) -> LiveParity {
        let events = dir.join("events.jsonl");
        std::fs::write(
            &events,
            [
                line(
                    10,
                    &[
                        ("0x00000000000000000000000000000000000000aa", "5"),
                        ("0x00000000000000000000000000000000000000bb", "0"),
                    ],
                ),
                line(11, &[]),
            ]
            .join("\n"),
        )
        .unwrap();
        let saved = dir.join("rows.jsonl");
        std::fs::write(
            &saved,
            [
                json!({"block": 10, "address": "0x00000000000000000000000000000000000000aa", "amount": "5"}).to_string(),
                json!({"block": 10, "address": "0x00000000000000000000000000000000000000bb", "amount": "0"}).to_string(),
            ]
            .join("\n"),
        )
        .unwrap();
        let spkg = dir.join("p.spkg");
        std::fs::write(&spkg, b"spkg").unwrap();
        LiveParity {
            events: vec![events],
            spkg,
            endpoint: "bsc.substreams.example:443".into(),
            saved_rows: Some(saved),
            full_from: 10,
            full_to: 11,
            sample_every: 0,
            batch: 1,
            output: dir.join(output),
        }
    }
    fn fake(wrong: Option<&str>) -> Fake {
        Fake {
            balances: [
                ("0x00000000000000000000000000000000000000aa".to_string(), 5),
                ("0x00000000000000000000000000000000000000bb".to_string(), 0),
            ]
            .into(),
            wrong: wrong.map(str::to_string),
        }
    }

    #[test]
    fn quantities_convert_exactly_to_decimal() {
        assert_eq!(quantity(&json!("0x0")).unwrap(), "0");
        assert_eq!(quantity(&json!("0xde0b6b3a7640000")).unwrap(), "1000000000000000000");
        assert_eq!(
            quantity(&json!(format!("0x{}", "f".repeat(64)))).unwrap(),
            "115792089237316195423570985008687907853269984665640564039457584007913129639935"
        );
        assert!(quantity(&json!("0x")).is_err() && quantity(&json!(format!("0x1{}", "0".repeat(64)))).is_err());
    }

    #[test]
    fn matching_rows_pass_and_every_mismatch_is_reported() {
        let tmp = tempfile::tempdir().unwrap();
        let args = setup(tmp.path(), "ok");
        assert!(check(&args, &fake(None)).unwrap());
        let report: Value = serde_json::from_str(&std::fs::read_to_string(args.output.join("report.json")).unwrap()).unwrap();
        assert_eq!(report["checks"]["eth_get_balance"]["checked"], 2);
        assert_eq!(report["checks"]["saved_controls"]["result"]["checked"], 2);
        assert_eq!((report["rows"]["emitted"].as_u64(), report["rows"]["known_zero"].as_u64()), (Some(2), Some(1)));
        assert!(check(&args, &fake(None)).is_err(), "output must be fresh");
        let args = setup(tmp.path(), "bad");
        assert!(!check(&args, &fake(Some("0x00000000000000000000000000000000000000bb"))).unwrap());
        let report: Value = serde_json::from_str(&std::fs::read_to_string(args.output.join("report.json")).unwrap()).unwrap();
        assert_eq!(report["checks"]["eth_get_balance"]["mismatch_examples"][0]["reference"], "1");
    }

    #[test]
    fn a_saved_control_row_missing_from_the_package_is_a_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let mut args = setup(tmp.path(), "saved");
        let saved = tmp.path().join("rows-extra.jsonl");
        std::fs::write(
            &saved,
            json!({"block": 10, "address": "0x00000000000000000000000000000000000000cc", "amount": "1"}).to_string(),
        )
        .unwrap();
        args.saved_rows = Some(saved);
        assert!(!check(&args, &fake(None)).unwrap());
    }
}
