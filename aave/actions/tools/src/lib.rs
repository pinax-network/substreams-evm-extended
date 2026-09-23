//! Receipt-log parity for packaged `aave_actions` output.
//!
//! Every persisted `Action` the packed map emitted is compared with the
//! Pool's receipt logs returned by `eth_getLogs` for the same blocks, decoded
//! here from the committed compiled ABI (`tests/fixtures/pool-event-abi.json`,
//! aave-v3-origin v3.7.0), not with the map's decoder. The match is by
//! transaction hash and block-level log index in both directions: every RPC
//! log of a bound event must be a persisted row, and every persisted row must
//! be an RPC log. Attempted rows from reverted frames are counted separately;
//! receipts do not carry them. Clock hashes are checked against the headers
//! and the Pool's implementation pointer and code hash are recorded.
//! Host-only; the RPC URL comes from `RPC_URL` and is never written out.
#![cfg(not(target_arch = "wasm32"))]
use anyhow::{bail, ensure, Context, Result};
use clap::Parser;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

const ABI: &str = include_str!("../../tests/fixtures/pool-event-abi.json");

#[derive(Parser, Debug)]
#[command(about = "Receipt-log parity for packaged aave_actions output")]
pub struct Cli {
    /// `substreams run … -o jsonl --bytes-encoding hex` output files.
    #[arg(long, required = true)]
    pub events: Vec<PathBuf>,
    /// The Pool parameters passed to `map_events`.
    #[arg(long)]
    pub params: PathBuf,
    /// The packed `.spkg` that produced the events (hashed into the report).
    #[arg(long)]
    pub spkg: PathBuf,
    /// Substreams endpoint host label for the report (no credentials).
    #[arg(long)]
    pub endpoint: String,
    /// Blocks per `eth_getLogs` request.
    #[arg(long, default_value_t = 1000)]
    pub log_span: u64,
    /// Fresh output directory for `report.json`.
    #[arg(long)]
    pub output: PathBuf,
}

/// A JSON-RPC source; the live one reads `RPC_URL`, tests use a fake.
pub trait Chain {
    fn call(&self, method: &str, params: Value) -> Result<Value>;
}
struct Rpc {
    agent: ureq::Agent,
    url: String,
    key: Option<String>,
}
impl Chain for Rpc {
    fn call(&self, method: &str, params: Value) -> Result<Value> {
        let mut request = self.agent.post(&self.url);
        if let Some(key) = &self.key {
            request = request.set("X-Api-Key", key);
        }
        let response = match request.send_json(json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params})) {
            Ok(response) => response,
            Err(ureq::Error::Status(code, _)) => bail!("RPC {method}: HTTP {code}"),
            Err(_) => bail!("RPC {method}: transport failed"),
        };
        let row: Value = response.into_json().map_err(|_| anyhow::anyhow!("RPC {method}: invalid JSON"))?;
        ensure!(row["error"].is_null() && !row["result"].is_null(), "RPC {method} returned an error or null");
        Ok(row["result"].clone())
    }
}

pub fn run(args: Cli) -> Result<bool> {
    let rpc = Rpc {
        agent: ureq::AgentBuilder::new().timeout(Duration::from_secs(120)).redirects(0).build(),
        url: std::env::var("RPC_URL").context("RPC_URL is required (it is never printed)")?,
        key: std::env::var("RPC_API_KEY").or_else(|_| std::env::var("SUBSTREAMS_API_KEY")).ok(),
    };
    check(&args, &rpc)
}

/// One bound event of the compiled ABI.
struct Event {
    name: String,
    topic: String,
    inputs: Vec<(String, String, bool)>,
}
fn events() -> Result<Vec<Event>> {
    let doc: Value = serde_json::from_str(ABI)?;
    doc["v3_7_0"]["events"]
        .as_array()
        .context("abi events")?
        .iter()
        .map(|e| {
            let inputs: Vec<(String, String, bool)> = e["inputs"]
                .as_array()
                .context("inputs")?
                .iter()
                .map(|i| {
                    Ok((
                        i["name"].as_str().context("name")?.to_string(),
                        i["type"].as_str().context("type")?.to_string(),
                        i["indexed"].as_bool().context("indexed")?,
                    ))
                })
                .collect::<Result<_>>()?;
            let name = e["name"].as_str().context("event name")?.to_string();
            let signature = format!("{name}({})", inputs.iter().map(|i| i.1.as_str()).collect::<Vec<_>>().join(","));
            Ok(Event {
                topic: format!("0x{}", hex::encode(aave_actions::keccak(signature.as_bytes()))),
                name,
                inputs,
            })
        })
        .collect()
}

fn decimal(word: &[u8]) -> String {
    // Base-1e9 limbs of a big-endian word.
    let mut limbs: Vec<u64> = vec![0];
    for byte in word {
        let mut carry = u64::from(*byte);
        for l in limbs.iter_mut() {
            let v = *l * 256 + carry;
            *l = v % 1_000_000_000;
            carry = v / 1_000_000_000;
        }
        while carry > 0 {
            limbs.push(carry % 1_000_000_000);
            carry /= 1_000_000_000;
        }
    }
    let mut s = limbs.last().unwrap().to_string();
    for l in limbs.iter().rev().skip(1) {
        s.push_str(&format!("{l:09}"));
    }
    s
}
/// Decode one ABI value into the protojson form the package prints.
fn value(ty: &str, word: &[u8]) -> Result<Value> {
    ensure!(word.len() == 32, "short ABI word");
    Ok(match ty {
        "address" => {
            ensure!(word[..12].iter().all(|b| *b == 0), "unpadded address");
            json!(format!("0x{}", hex::encode(&word[12..])))
        }
        "bool" => {
            ensure!(word[..31].iter().all(|b| *b == 0) && word[31] <= 1, "bool out of range");
            json!(word[31] == 1)
        }
        "uint8" | "uint16" => {
            let bytes = if ty == "uint8" { 1 } else { 2 };
            ensure!(word[..32 - bytes].iter().all(|b| *b == 0), "{ty} out of range");
            json!(u64::from_be_bytes(word[24..].try_into().unwrap()))
        }
        "uint256" => json!(decimal(word)),
        other => bail!("unsupported ABI type {other}"),
    })
}
/// The receipt log decoded by name into the `Action` fields the package emits.
fn expected_action(event: &Event, log: &Value) -> Result<Map<String, Value>> {
    let topics: Vec<Vec<u8>> = log["topics"]
        .as_array()
        .context("topics")?
        .iter()
        .map(|t| hex::decode(t.as_str().unwrap_or_default().trim_start_matches("0x")).context("topic hex"))
        .collect::<Result<_>>()?;
    let data = hex::decode(log["data"].as_str().context("data")?.trim_start_matches("0x"))?;
    let indexed = event.inputs.iter().filter(|i| i.2).count();
    ensure!(topics.len() == 1 + indexed, "{}: {} topics", event.name, topics.len());
    ensure!(data.len() == 32 * (event.inputs.len() - indexed), "{}: {} data bytes", event.name, data.len());
    let (mut t, mut d) = (1, 0);
    let mut fields: BTreeMap<String, Value> = BTreeMap::new();
    for (name, ty, is_indexed) in &event.inputs {
        let word = if *is_indexed {
            t += 1;
            topics[t - 1].clone()
        } else {
            d += 1;
            data[(d - 1) * 32..d * 32].to_vec()
        };
        fields.insert(name.clone(), value(ty, &word)?);
    }
    let f = |n: &str| fields.get(n).cloned().with_context(|| format!("{}: no input {n}", event.name));
    let mut out = Map::new();
    let mut set = |k: &str, v: Value| {
        out.insert(k.to_string(), v);
    };
    set("kind", json!(format!("ACTION_KIND_{}", screaming(&event.name))));
    match event.name.as_str() {
        "Supply" => {
            set("reserve", f("reserve")?);
            set("actor", f("user")?);
            set("beneficiary", f("onBehalfOf")?);
            set("amount", f("amount")?);
            set("referralCode", f("referralCode")?);
        }
        "Withdraw" => {
            set("reserve", f("reserve")?);
            set("actor", f("user")?);
            set("beneficiary", f("user")?);
            set("recipient", f("to")?);
            set("amount", f("amount")?);
        }
        "Borrow" => {
            set("reserve", f("reserve")?);
            set("actor", f("user")?);
            set("beneficiary", f("onBehalfOf")?);
            set("amount", f("amount")?);
            set("interestRateMode", f("interestRateMode")?);
            set("borrowRate", f("borrowRate")?);
            set("referralCode", f("referralCode")?);
        }
        "Repay" => {
            set("reserve", f("reserve")?);
            set("actor", f("repayer")?);
            set("beneficiary", f("user")?);
            set("amount", f("amount")?);
            set("useAtokens", f("useATokens")?);
        }
        "LiquidationCall" => {
            set("reserve", f("debtAsset")?);
            set("actor", f("liquidator")?);
            set("beneficiary", f("user")?);
            set("amount", f("debtToCover")?);
            set("collateralAsset", f("collateralAsset")?);
            set("liquidatedCollateralAmount", f("liquidatedCollateralAmount")?);
            set("receiveAtoken", f("receiveAToken")?);
        }
        "FlashLoan" => {
            set("reserve", f("asset")?);
            set("actor", f("initiator")?);
            set("beneficiary", f("target")?);
            set("amount", f("amount")?);
            set("interestRateMode", f("interestRateMode")?);
            set("premium", f("premium")?);
            set("referralCode", f("referralCode")?);
        }
        other => bail!("unbound event {other}"),
    }
    Ok(out)
}
fn screaming(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_uppercase());
    }
    out
}
/// The value a protojson row carries for `field`, with proto3 defaults for
/// omitted fields (`0`, `false`, `""`).
fn emitted(row: &Value, field: &str, like: &Value) -> Value {
    match (&row[field], like) {
        (Value::Null, Value::Bool(_)) => json!(false),
        (Value::Null, Value::Number(_)) => json!(0),
        (Value::Null, Value::String(_)) => json!(""),
        (Value::Number(n), _) => json!(n.as_u64().unwrap_or_default()),
        (other, _) => other.clone(),
    }
}
fn quantity(v: &Value) -> Result<u64> {
    Ok(u64::from_str_radix(v.as_str().context("quantity")?.trim_start_matches("0x"), 16)?)
}

pub fn check(args: &Cli, rpc: &dyn Chain) -> Result<bool> {
    ensure!(
        !args.output.exists() || std::fs::read_dir(&args.output)?.next().is_none(),
        "output directory must be fresh"
    );
    ensure!(args.log_span > 0, "log span must be positive");
    std::fs::create_dir_all(&args.output)?;
    let config = aave_actions::parse(&std::fs::read_to_string(&args.params)?).map_err(|e| anyhow::anyhow!("{e}"))?;
    ensure!(config.pools.len() == 1, "exactly one bound Pool is supported");
    let pool = &config.pools[0];
    let pool_address = format!("0x{}", hex::encode(&pool.address));
    let bound = events()?;

    let mut blocks: BTreeMap<u64, Value> = BTreeMap::new();
    let mut digest = Sha256::new();
    let mut lines: BTreeMap<u64, String> = BTreeMap::new();
    for path in &args.events {
        for line in std::fs::read_to_string(path)?.lines().filter(|l| !l.trim().is_empty()) {
            let row: Value = serde_json::from_str(line).with_context(|| format!("invalid JSON line in {}", path.display()))?;
            let data = row["@data"].clone();
            let clock = data["clocks"].as_array().and_then(|c| c.first()).context("event without a clock")?;
            let n: u64 = clock["number"].as_str().context("clock number")?.parse()?;
            ensure!(blocks.insert(n, data).is_none(), "block {n} delivered twice");
            lines.insert(n, line.to_string());
        }
    }
    ensure!(!blocks.is_empty(), "no events");
    for line in lines.values() {
        digest.update(line.as_bytes());
        digest.update(b"\n");
    }

    let (mut clock_checked, mut clock_mismatches) = (0u64, Vec::new());
    let (mut matched, mut field_mismatches, mut missing_rows, mut extra_rows) = (0u64, Vec::new(), Vec::new(), Vec::new());
    let (mut persisted_rows, mut attempted_rows, mut rpc_logs) = (0u64, 0u64, 0u64);
    let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
    let numbers: Vec<u64> = blocks.keys().copied().collect();
    let topics: Vec<String> = bound.iter().map(|e| e.topic.clone()).collect();
    // Group delivered blocks into spans for eth_getLogs, then match per block.
    let mut i = 0;
    while i < numbers.len() {
        let from = numbers[i];
        let mut to = from;
        while i + 1 < numbers.len() && numbers[i + 1] < from + args.log_span {
            i += 1;
            to = numbers[i];
        }
        i += 1;
        let logs = rpc.call(
            "eth_getLogs",
            json!([{"fromBlock": format!("{from:#x}"), "toBlock": format!("{to:#x}"), "address": pool_address, "topics": [topics]}]),
        )?;
        let mut by_block: BTreeMap<u64, Vec<Value>> = BTreeMap::new();
        for log in logs.as_array().context("eth_getLogs result")? {
            ensure!(!log["removed"].as_bool().unwrap_or(false), "removed log returned");
            by_block.entry(quantity(&log["blockNumber"])?).or_default().push(log.clone());
        }
        for n in numbers.iter().filter(|n| (from..=to).contains(*n)) {
            let data = &blocks[n];
            let clock = &data["clocks"][0];
            let header = rpc.call("eth_getBlockByNumber", json!([format!("{n:#x}"), false]))?;
            clock_checked += 1;
            if clock["hash"] != header["hash"] || clock["parentHash"] != header["parentHash"] {
                clock_mismatches.push(json!({"block": n, "emitted": clock["hash"], "rpc": header["hash"]}));
            }
            let mut rows: BTreeMap<(String, u64), &Value> = BTreeMap::new();
            for row in data["actions"].as_array().into_iter().flatten() {
                if row["persisted"].as_bool() == Some(true) {
                    persisted_rows += 1;
                    let key = (
                        row["transactionHash"].as_str().unwrap_or_default().to_string(),
                        row["blockIndex"].as_u64().unwrap_or(0),
                    );
                    ensure!(rows.insert(key, row).is_none(), "two persisted rows for one log at block {n}");
                    *kinds.entry(row["kind"].as_str().unwrap_or_default().to_string()).or_default() += 1;
                } else {
                    attempted_rows += 1;
                }
            }
            let mut seen = BTreeSet::new();
            for log in by_block.get(n).into_iter().flatten() {
                rpc_logs += 1;
                ensure!(log["blockHash"] == header["hash"], "log block hash differs from the header at {n}");
                let topic0 = log["topics"][0].as_str().unwrap_or_default();
                let event = bound.iter().find(|e| e.topic == topic0).context("log with an unbound topic")?;
                let key = (log["transactionHash"].as_str().context("log tx")?.to_string(), quantity(&log["logIndex"])?);
                seen.insert(key.clone());
                let Some(row) = rows.get(&key) else {
                    missing_rows.push(json!({"block": n, "transaction": key.0, "log_index": key.1, "event": event.name}));
                    continue;
                };
                let expected = expected_action(event, log)?;
                let mut differs = Vec::new();
                for (field, want) in &expected {
                    let got = emitted(row, field, want);
                    if &got != want {
                        differs.push(json!({"field": field, "emitted": got, "rpc": want}));
                    }
                }
                if row["pool"].as_str() != Some(pool_address.as_str()) {
                    differs.push(json!({"field": "pool", "emitted": row["pool"], "rpc": pool_address}));
                }
                if differs.is_empty() {
                    matched += 1;
                } else {
                    field_mismatches.push(json!({"block": n, "transaction": key.0, "log_index": key.1, "differences": differs}));
                }
            }
            for key in rows.keys().filter(|k| !seen.contains(*k)) {
                extra_rows.push(json!({"block": n, "transaction": key.0, "block_index": key.1}));
            }
        }
    }

    // The bound implementation and its code hash at the first and last block.
    let mut identities = Vec::new();
    let mut identity_ok = true;
    for n in [numbers[0], *numbers.last().unwrap()] {
        let hash = blocks[&n]["clocks"][0]["hash"].clone();
        let reference = json!({"blockHash": hash, "requireCanonical": true});
        let pointer = rpc.call(
            "eth_getStorageAt",
            json!([pool_address, format!("0x{}", hex::encode(pool.implementation_slot)), reference]),
        )?;
        let pointer = pointer.as_str().context("storage")?.trim_start_matches("0x");
        let found = format!("0x{}", &format!("{pointer:0>64}")[24..]);
        let expected = format!("0x{}", hex::encode(&pool.implementation));
        identity_ok &= found == expected;
        let code = rpc.call("eth_getCode", json!([expected, reference]))?;
        let code = hex::decode(code.as_str().context("code")?.trim_start_matches("0x"))?;
        identities.push(json!({"block": n, "block_hash": hash, "implementation_expected": expected, "implementation_found": found, "implementation_code_hash": format!("0x{}", hex::encode(aave_actions::keccak(&code)))}));
    }

    let passed = clock_mismatches.is_empty() && field_mismatches.is_empty() && missing_rows.is_empty() && extra_rows.is_empty() && identity_ok;
    let report = json!({
        "tool": "aave-actions-tools live-parity",
        "status": if passed { "passed" } else { "failed" },
        "events_sha256": hex::encode(digest.finalize()),
        "package": {"spkg_sha256": hex::encode(Sha256::digest(std::fs::read(&args.spkg)?)), "params_file_sha256": hex::encode(Sha256::digest(std::fs::read(&args.params)?)), "abi": "tests/fixtures/pool-event-abi.json v3_7_0"},
        "endpoints": {"substreams": args.endpoint, "rpc": "RPC_URL (credential-bearing; not recorded)"},
        "blocks": {"delivered": numbers.len(), "first": numbers.first(), "last": numbers.last()},
        "rows": {"persisted": persisted_rows, "attempted_in_reverted_frames": attempted_rows, "persisted_by_kind": kinds},
        "checks": {
            "clock": {"checked": clock_checked, "mismatches": clock_mismatches.len(), "examples": clock_mismatches.iter().take(10).collect::<Vec<_>>()},
            "receipt_logs": {"rpc_logs": rpc_logs, "matched": matched, "field_mismatches": field_mismatches.len(), "rpc_log_without_row": missing_rows.len(), "row_without_rpc_log": extra_rows.len(),
                "examples": field_mismatches.iter().chain(missing_rows.iter()).chain(extra_rows.iter()).take(20).collect::<Vec<_>>()},
        },
        "pool_identity": identities,
        "limits": "Persisted rows only; attempted rows of reverted frames are not in receipts and are counted, not checked. Qualifies the delivered blocks and the bound Pool only.",
    });
    std::fs::write(args.output.join("report.json"), serde_json::to_string_pretty(&report)? + "\n")?;
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"status": report["status"], "blocks": report["blocks"]["delivered"], "rows": report["rows"], "checks": report["checks"], "identity_ok": identity_ok})
        )?
    );
    Ok(passed)
}

#[cfg(test)]
mod tests;
