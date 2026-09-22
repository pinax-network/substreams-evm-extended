//! Same-block getter parity for packaged `aave_balance_state` output.
//!
//! Reads `substreams run -o jsonl --bytes-encoding hex` output of the packed
//! map, then checks every emitted row against the chain at the row's exact
//! block hash through JSON-RPC:
//!
//! * the clock (hash, parent, timestamp, state root) against the header;
//! * each `HolderBasis` scaled balance against `aToken.scaledBalanceOf`;
//! * each reserve word against `Pool.getReserveData(asset)` and the scaled
//!   total supply against `aToken.scaledTotalSupply`;
//! * the observable balance: `conformance::aave::balance_of` over the
//!   emitted scaled balance and the same-block reserve words, against
//!   `aToken.balanceOf`.
//!
//! It also records the runtime identities the epochs bind (implementation
//! pointers, code hashes, `ATOKEN_REVISION`) at the first and last block.
//! Host-only; the RPC URL comes from `RPC_URL` and is never written out.
use anyhow::{bail, ensure, Context, Result};
use clap::Args;
use conformance::aave::{balance_of, Era, Reserve};
use num_bigint::BigUint;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Args, Debug)]
pub struct LiveParity {
    /// `substreams run … -o jsonl --bytes-encoding hex` output files.
    #[arg(long, required = true)]
    pub events: Vec<PathBuf>,
    /// The epoch configuration passed to `map_events`.
    #[arg(long)]
    pub params: PathBuf,
    /// The packed `.spkg` that produced the events (hashed into the report).
    #[arg(long)]
    pub spkg: PathBuf,
    /// Substreams endpoint host label for the report (no credentials).
    #[arg(long)]
    pub endpoint: String,
    /// Fresh output directory for `report.json`.
    #[arg(long)]
    pub output: PathBuf,
}

/// A JSON-RPC source; the live one reads `RPC_URL`, tests use a fake.
pub trait Chain {
    fn call(&self, method: &str, params: Value) -> Result<Value>;
    fn calls(&self) -> u64;
    fn eth_call(&self, to: &str, data: String, block_hash: &str) -> Result<Vec<u8>> {
        let result = self.call(
            "eth_call",
            json!([{"to": to, "data": data}, {"blockHash": block_hash, "requireCanonical": true}]),
        )?;
        hex::decode(result.as_str().context("eth_call result")?.trim_start_matches("0x")).context("eth_call hex")
    }
}

struct Rpc {
    agent: ureq::Agent,
    url: String,
    key: Option<String>,
    calls: std::cell::Cell<u64>,
}
impl Chain for Rpc {
    fn call(&self, method: &str, params: Value) -> Result<Value> {
        Rpc::call(self, method, params)
    }
    fn calls(&self) -> u64 {
        self.calls.get()
    }
}
impl Rpc {
    fn from_env() -> Result<Self> {
        Ok(Self {
            agent: ureq::AgentBuilder::new().timeout(Duration::from_secs(60)).redirects(0).build(),
            url: std::env::var("RPC_URL").context("RPC_URL is required (it is never printed)")?,
            key: std::env::var("RPC_API_KEY").or_else(|_| std::env::var("SUBSTREAMS_API_KEY")).ok(),
            calls: std::cell::Cell::new(0),
        })
    }
    fn call(&self, method: &str, params: Value) -> Result<Value> {
        self.calls.set(self.calls.get() + 1);
        let mut request = self.agent.post(&self.url);
        if let Some(key) = &self.key {
            request = request.set("X-Api-Key", key);
        }
        let response = match request.send_json(json!({"jsonrpc":"2.0","id":1,"method":method,"params":params})) {
            Ok(response) => response,
            Err(ureq::Error::Status(code, _)) => bail!("RPC {method}: HTTP {code}"),
            Err(_) => bail!("RPC {method}: transport failed"),
        };
        let row: Value = response.into_json().map_err(|_| anyhow::anyhow!("RPC {method}: invalid JSON"))?;
        ensure!(row["error"].is_null() && !row["result"].is_null(), "RPC {method} returned an error or null");
        Ok(row["result"].clone())
    }
}

fn keccak(bytes: &[u8]) -> [u8; 32] {
    aave_balance_state::keccak(bytes)
}
pub fn selector(signature: &str) -> String {
    hex::encode(&keccak(signature.as_bytes())[..4])
}
fn address_arg(address: &str) -> String {
    format!("{:0>64}", address.trim_start_matches("0x").to_lowercase())
}
fn word(bytes: &[u8], index: usize) -> Result<BigUint> {
    let chunk = bytes.get(index * 32..(index + 1) * 32).context("ABI word out of range")?;
    Ok(BigUint::from_bytes_be(chunk))
}
fn text<'a>(v: &'a Value, field: &str) -> Result<&'a str> {
    v[field].as_str().with_context(|| format!("missing `{field}`"))
}
fn number(v: &Value, field: &str) -> Result<u64> {
    match &v[field] {
        Value::String(s) => s.parse().with_context(|| format!("`{field}` is not a number")),
        Value::Number(n) => n.as_u64().with_context(|| format!("`{field}` is not a u64")),
        Value::Null => Ok(0),
        _ => bail!("`{field}` has an unexpected type"),
    }
}
fn big(v: &Value, field: &str) -> Result<BigUint> {
    text(v, field)?.parse().with_context(|| format!("`{field}` is not a decimal"))
}
fn sha256_file(path: &PathBuf) -> Result<String> {
    Ok(hex::encode(Sha256::digest(std::fs::read(path)?)))
}

#[derive(Default)]
struct Tally {
    checked: u64,
    matched: u64,
    mismatches: Vec<Value>,
}
impl Tally {
    fn check(&mut self, what: &str, block: u64, expected: String, actual: String, context: Value) {
        self.checked += 1;
        if expected == actual {
            self.matched += 1;
        } else {
            self.mismatches
                .push(json!({"check": what, "block": block, "emitted": expected, "rpc": actual, "context": context}));
        }
    }
    fn json(&self) -> Value {
        json!({"checked": self.checked, "matched": self.matched, "mismatches": self.mismatches.len(), "mismatch_examples": self.mismatches.iter().take(20).collect::<Vec<_>>()})
    }
}

pub fn run(args: LiveParity) -> Result<bool> {
    let rpc = Rpc::from_env()?;
    check(&args, &rpc)
}

/// The parity check over any chain source; writes `report.json` and returns
/// whether every check matched.
pub fn check(args: &LiveParity, rpc: &dyn Chain) -> Result<bool> {
    ensure!(
        !args.output.exists() || std::fs::read_dir(&args.output)?.next().is_none(),
        "output directory must be fresh"
    );
    std::fs::create_dir_all(&args.output)?;
    let params_text = std::fs::read_to_string(&args.params)?;
    let config = aave_balance_state::parse(&params_text).map_err(|e| anyhow::anyhow!("{e}"))?;
    let pool = config.pool.clone().context("the parameters bind no Pool")?;
    let markets: BTreeMap<String, &aave_balance_state::Market> = config.markets.iter().map(|m| (format!("0x{}", hex::encode(&m.atoken)), m)).collect();

    let mut blocks: BTreeMap<u64, Value> = BTreeMap::new();
    let mut lines: BTreeMap<u64, String> = BTreeMap::new();
    for path in &args.events {
        for line in std::fs::read_to_string(path)?.lines().filter(|l| !l.trim().is_empty()) {
            let row: Value = serde_json::from_str(line).with_context(|| format!("invalid JSON line in {}", path.display()))?;
            let data = row["@data"].clone();
            let clock = data["clocks"].as_array().and_then(|c| c.first()).context("event without a clock")?;
            let n = number(clock, "number")?;
            ensure!(blocks.insert(n, data).is_none(), "block {n} delivered twice");
            lines.insert(n, line.to_string());
        }
    }
    ensure!(!blocks.is_empty(), "no events");
    // Digest of the checked output: every JSON line in block order, each
    // followed by a newline.
    let mut digest = Sha256::new();
    for line in lines.values() {
        digest.update(line.as_bytes());
        digest.update(b"\n");
    }
    let events_sha256 = hex::encode(digest.finalize());

    let (mut clocks, mut holders, mut balances, mut reserves, mut supplies) =
        (Tally::default(), Tally::default(), Tally::default(), Tally::default(), Tally::default());
    let mut rows_by_kind: BTreeMap<String, u64> = BTreeMap::new();
    let mut epochs = Vec::new();
    let mut parameter_digests = BTreeSet::new();
    let mut packages = BTreeSet::new();
    let mut holder_set = BTreeSet::new();
    let reserve_data = selector("getReserveData(address)");
    for (n, data) in &blocks {
        let clock = &data["clocks"][0];
        let hash = text(clock, "hash")?.to_string();
        parameter_digests.insert(text(clock, "parametersSha256")?.to_string());
        packages.insert(format!("{} {}", text(clock, "package")?, text(clock, "packageVersion")?));
        let header = rpc.call("eth_getBlockByNumber", json!([format!("{n:#x}"), false]))?;
        let ctx = json!({"hash": hash});
        clocks.check("block hash", *n, hash.clone(), text(&header, "hash")?.to_string(), ctx.clone());
        clocks.check(
            "parent hash",
            *n,
            text(clock, "parentHash")?.to_string(),
            text(&header, "parentHash")?.to_string(),
            ctx.clone(),
        );
        clocks.check(
            "state root",
            *n,
            text(clock, "stateRoot")?.to_string(),
            text(&header, "stateRoot")?.to_string(),
            ctx.clone(),
        );
        let timestamp = number(clock, "timestamp")?;
        let header_ts = u64::from_str_radix(text(&header, "timestamp")?.trim_start_matches("0x"), 16)?;
        clocks.check("timestamp", *n, timestamp.to_string(), header_ts.to_string(), ctx.clone());
        for (kind, list) in [
            ("holder_basis", "holderBasis"),
            ("global_state", "globalState"),
            ("epochs", "epochs"),
            ("dependencies", "dependencies"),
        ] {
            *rows_by_kind.entry(kind.into()).or_default() += data[list].as_array().map_or(0, |a| a.len() as u64);
        }
        for e in data["epochs"].as_array().into_iter().flatten() {
            epochs.push(json!({"block": n, "kind": e["kind"], "reason": e["reason"], "market": e["market"]}));
        }
        let mut reserve_cache: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut reserve_of = |asset: &str| -> Result<Vec<u8>> {
            if let Some(r) = reserve_cache.get(asset) {
                return Ok(r.clone());
            }
            let r = rpc.eth_call(
                &format!("0x{}", hex::encode(&pool.address)),
                format!("0x{reserve_data}{}", address_arg(asset)),
                &hash,
            )?;
            ensure!(r.len() >= 15 * 32, "getReserveData returned {} bytes", r.len());
            reserve_cache.insert(asset.to_string(), r.clone());
            Ok(r)
        };
        for g in data["globalState"].as_array().into_iter().flatten() {
            let field = text(g, "field")?;
            let value = text(g, "value")?.to_string();
            let ctx = json!({"market": g["market"], "key": g["key"], "field": field});
            match field {
                "STATE_FIELD_AAVE_LIQUIDITY_INDEX" | "STATE_FIELD_AAVE_CURRENT_LIQUIDITY_RATE" | "STATE_FIELD_AAVE_LAST_UPDATE_TIMESTAMP" => {
                    let r = reserve_of(text(g, "key")?)?;
                    // ReserveDataLegacy: configuration 0, liquidityIndex 1,
                    // currentLiquidityRate 2, …, lastUpdateTimestamp 6.
                    let index = match field {
                        "STATE_FIELD_AAVE_LIQUIDITY_INDEX" => 1,
                        "STATE_FIELD_AAVE_CURRENT_LIQUIDITY_RATE" => 2,
                        _ => 6,
                    };
                    reserves.check(field, *n, value, word(&r, index)?.to_string(), ctx);
                }
                "STATE_FIELD_AAVE_SCALED_TOTAL_SUPPLY" => {
                    let atoken = text(g, "market")?;
                    let r = rpc.eth_call(atoken, format!("0x{}", selector("scaledTotalSupply()")), &hash)?;
                    supplies.check(field, *n, value, word(&r, 0)?.to_string(), ctx);
                }
                _ => {}
            }
        }
        for h in data["holderBasis"].as_array().into_iter().flatten() {
            let atoken = text(h, "market")?;
            let holder = text(h, "holder")?;
            holder_set.insert((atoken.to_string(), holder.to_string()));
            let scaled = big(h, "value")?;
            let ctx = json!({"market": atoken, "holder": holder});
            let r = rpc.eth_call(atoken, format!("0x{}{}", selector("scaledBalanceOf(address)"), address_arg(holder)), &hash)?;
            holders.check("scaledBalanceOf", *n, scaled.to_string(), word(&r, 0)?.to_string(), ctx.clone());
            let market = markets.get(atoken).context("row for an unbound aToken")?;
            let underlying = format!("0x{}", hex::encode(&market.underlying));
            let rd = reserve_of(&underlying)?;
            let reserve = Reserve {
                liquidity_index: word(&rd, 1)?,
                current_liquidity_rate: word(&rd, 2)?,
                last_update_timestamp: u64::try_from(word(&rd, 6)?).context("timestamp")?,
            };
            let era = if market.rounding == proto::pb::evm::balance_state::v1::Rounding::Floor {
                Era::Floor
            } else {
                Era::HalfUp
            };
            let evaluated = balance_of(&scaled, &reserve, timestamp, era).map_err(|e| anyhow::anyhow!("{e:?}"))?;
            let r = rpc.eth_call(atoken, format!("0x70a08231{}", address_arg(holder)), &hash)?;
            balances.check("balanceOf via conformance::aave", *n, evaluated.to_string(), word(&r, 0)?.to_string(), ctx);
        }
    }

    // Runtime identities at the first and last block.
    let first = text(&blocks.values().next().unwrap()["clocks"][0], "hash")?.to_string();
    let last = text(&blocks.values().last().unwrap()["clocks"][0], "hash")?.to_string();
    let mut identities = Vec::new();
    let mut identity_ok = true;
    let pool_address = format!("0x{}", hex::encode(&pool.address));
    let mut contracts = vec![("pool".to_string(), pool_address, pool.implementation_slot, pool.implementation.clone())];
    for (atoken, m) in &markets {
        contracts.push((format!("atoken {atoken}"), atoken.clone(), m.implementation_slot, m.implementation.clone()));
    }
    for (label, proxy, slot, expected) in &contracts {
        for block in [&first, &last] {
            let pointer = rpc.call(
                "eth_getStorageAt",
                json!([proxy, format!("0x{}", hex::encode(slot)), {"blockHash": block, "requireCanonical": true}]),
            )?;
            let pointer = pointer.as_str().context("storage")?.trim_start_matches("0x").to_string();
            let found = format!("0x{}", &format!("{pointer:0>64}")[24..]);
            let expected = format!("0x{}", hex::encode(expected));
            identity_ok &= found == expected;
            let proxy_code = rpc.call("eth_getCode", json!([proxy, {"blockHash": block, "requireCanonical": true}]))?;
            let implementation_code = rpc.call("eth_getCode", json!([expected, {"blockHash": block, "requireCanonical": true}]))?;
            let code_hash = |v: &Value| -> Result<String> {
                Ok(format!(
                    "0x{}",
                    hex::encode(keccak(&hex::decode(v.as_str().context("code")?.trim_start_matches("0x"))?))
                ))
            };
            identities.push(json!({
                "contract": label, "proxy": proxy, "block_hash": block,
                "implementation_expected": expected, "implementation_found": found,
                "proxy_code_hash": code_hash(&proxy_code)?, "implementation_code_hash": code_hash(&implementation_code)?,
            }));
        }
    }
    let mut revisions = Vec::new();
    for (atoken, m) in &markets {
        let r = rpc.eth_call(atoken, format!("0x{}", selector("ATOKEN_REVISION()")), &last)?;
        let revision = word(&r, 0)?.to_string();
        identity_ok &= revision == m.implementation_revision;
        revisions.push(json!({"atoken": atoken, "declared": m.implementation_revision, "rpc": revision}));
    }

    let total_mismatches = [&clocks, &holders, &balances, &reserves, &supplies]
        .iter()
        .map(|t| t.mismatches.len())
        .sum::<usize>();
    let passed = total_mismatches == 0 && identity_ok;
    let report = json!({
        "tool": "aave-balance-state-tools live-parity",
        "status": if passed { "passed" } else { "failed" },
        "events_sha256": events_sha256,
        "package": {"spkg_sha256": sha256_file(&args.spkg)?, "clock_package": packages, "parameters_sha256": parameter_digests, "params_file_sha256": sha256_file(&args.params)?},
        "endpoints": {"substreams": args.endpoint, "rpc": "RPC_URL (credential-bearing; not recorded)"},
        "blocks": {"delivered": blocks.len(), "first": blocks.keys().next(), "last": blocks.keys().last(), "numbers": blocks.keys().collect::<Vec<_>>()},
        "rows": rows_by_kind,
        "distinct_holders": holder_set.len(),
        "checks": {"clock": clocks.json(), "scaled_balance": holders.json(), "balance_of": balances.json(), "reserve_words": reserves.json(), "scaled_total_supply": supplies.json()},
        "epochs": epochs,
        "runtime_identities": identities,
        "atoken_revisions": revisions,
        "rpc_calls": rpc.calls(),
        "limits": "Same-block getter parity for the delivered blocks and the holders written in them only; it does not initialize or check holders without a row, and it does not qualify other markets, networks or intervals.",
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
