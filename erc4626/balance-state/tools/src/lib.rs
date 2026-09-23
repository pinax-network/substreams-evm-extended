//! Same-block getter parity for packaged `erc4626_balance_state` output of
//! the `aave-static-atoken-lm` model.
//!
//! Reads `substreams run -o jsonl --bytes-encoding hex` output of the packed
//! map and checks, at each row's exact block hash:
//!
//! * the clock against the header;
//! * each `HolderBasis` share count against `vault.balanceOf(holder)`;
//! * `ERC4626_TOTAL_SUPPLY` against `vault.totalSupply()`;
//! * the carried Aave reserve words against `Pool.getReserveData(asset)`;
//! * the conversion: `conformance::erc4626::StataTokenLm` over the emitted
//!   shares and the same-block reserve words against the vault's own
//!   `convertToAssets(shares)` and `rate()`.
//!
//! It also records the vault and Pool implementation pointers, their code
//! hashes and `STATIC__ATOKEN_LM_REVISION` at the first and last block.
//! Host-only; the RPC URL comes from `RPC_URL` and is never written out.
#![cfg(not(target_arch = "wasm32"))]
use anyhow::{bail, ensure, Context, Result};
use clap::Parser;
use conformance::{aave::Reserve, erc4626::StataTokenLm};
use erc4626_balance_state::{keccak, Model};
use num_bigint::BigUint;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(about = "Same-block getter parity for packaged erc4626_balance_state output (static aToken model)")]
pub struct Cli {
    /// `substreams run … -o jsonl --bytes-encoding hex` output files.
    #[arg(long, required = true)]
    pub events: Vec<PathBuf>,
    /// The vault parameters passed to `map_events`.
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
        agent: ureq::AgentBuilder::new().timeout(Duration::from_secs(60)).redirects(0).build(),
        url: std::env::var("RPC_URL").context("RPC_URL is required (it is never printed)")?,
        key: std::env::var("RPC_API_KEY").or_else(|_| std::env::var("SUBSTREAMS_API_KEY")).ok(),
    };
    check(&args, &rpc)
}

pub fn selector(signature: &str) -> String {
    hex::encode(&keccak(signature.as_bytes())[..4])
}
fn arg(address: &str) -> String {
    format!("{:0>64}", address.trim_start_matches("0x").to_lowercase())
}
fn uint(value: &BigUint) -> String {
    format!("{:0>64}", value.to_str_radix(16))
}
fn word(bytes: &[u8], index: usize) -> Result<BigUint> {
    Ok(BigUint::from_bytes_be(
        bytes.get(index * 32..(index + 1) * 32).context("ABI word out of range")?,
    ))
}
fn text<'a>(v: &'a Value, field: &str) -> Result<&'a str> {
    v[field].as_str().with_context(|| format!("missing `{field}`"))
}
fn hex0x(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
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

pub fn check(args: &Cli, rpc: &dyn Chain) -> Result<bool> {
    ensure!(
        !args.output.exists() || std::fs::read_dir(&args.output)?.next().is_none(),
        "output directory must be fresh"
    );
    std::fs::create_dir_all(&args.output)?;
    let config = erc4626_balance_state::parse(&std::fs::read_to_string(&args.params)?).map_err(|e| anyhow::anyhow!("{e}"))?;
    ensure!(config.vaults.len() == 1, "exactly one bound vault is supported");
    let vault = &config.vaults[0];
    let Model::AaveStaticAToken {
        pool,
        implementation_slot: pool_slot,
        implementation: pool_implementation,
        ..
    } = &vault.model
    else {
        bail!("only the aave-static-atoken-lm model is supported");
    };
    let vault_address = hex0x(&vault.vault);
    let pool_address = hex0x(pool);
    let asset = hex0x(&vault.asset);

    let mut blocks: BTreeMap<u64, Value> = BTreeMap::new();
    let mut lines: BTreeMap<u64, String> = BTreeMap::new();
    for path in &args.events {
        for line in std::fs::read_to_string(path)?.lines().filter(|l| !l.trim().is_empty()) {
            let row: Value = serde_json::from_str(line).with_context(|| format!("invalid JSON line in {}", path.display()))?;
            let data = row["@data"].clone();
            let clock = data["clocks"].as_array().and_then(|c| c.first()).context("event without a clock")?;
            let n: u64 = text(clock, "number")?.parse()?;
            ensure!(blocks.insert(n, data).is_none(), "block {n} delivered twice");
            lines.insert(n, line.to_string());
        }
    }
    ensure!(!blocks.is_empty(), "no events");
    let mut digest = Sha256::new();
    for line in lines.values() {
        digest.update(line.as_bytes());
        digest.update(b"\n");
    }

    let (mut clocks, mut shares, mut supply, mut reserves, mut assets, mut rates) = (
        Tally::default(),
        Tally::default(),
        Tally::default(),
        Tally::default(),
        Tally::default(),
        Tally::default(),
    );
    let mut rows: BTreeMap<String, u64> = BTreeMap::new();
    let mut epochs = Vec::new();
    let mut holders = BTreeSet::new();
    let reserve_data = selector("getReserveData(address)");
    for (n, data) in &blocks {
        let clock = &data["clocks"][0];
        let hash = text(clock, "hash")?.to_string();
        let header = rpc.call("eth_getBlockByNumber", json!([format!("{n:#x}"), false]))?;
        let ctx = json!({"hash": hash});
        clocks.check("block hash", *n, hash.clone(), text(&header, "hash")?.to_string(), ctx.clone());
        clocks.check(
            "parent hash",
            *n,
            text(clock, "parentHash")?.to_string(),
            text(&header, "parentHash")?.to_string(),
            ctx,
        );
        let timestamp: u64 = text(clock, "timestamp")?.parse()?;
        for (kind, list) in [
            ("holder_basis", "holderBasis"),
            ("global_state", "globalState"),
            ("epochs", "epochs"),
            ("dependencies", "dependencies"),
        ] {
            *rows.entry(kind.into()).or_default() += data[list].as_array().map_or(0, |a| a.len() as u64);
        }
        for e in data["epochs"].as_array().into_iter().flatten() {
            epochs.push(json!({"block": n, "kind": e["kind"], "reason": e["reason"]}));
        }
        let mut reserve: Option<Vec<u8>> = None;
        let mut reserve_at = || -> Result<Vec<u8>> {
            if let Some(r) = &reserve {
                return Ok(r.clone());
            }
            let r = rpc.eth_call(&pool_address, format!("0x{reserve_data}{}", arg(&asset)), &hash)?;
            ensure!(r.len() >= 15 * 32, "getReserveData returned {} bytes", r.len());
            reserve = Some(r.clone());
            Ok(r)
        };
        for g in data["globalState"].as_array().into_iter().flatten() {
            let field = text(g, "field")?;
            let value = text(g, "value")?.to_string();
            let ctx = json!({"field": field, "key": g["key"]});
            match field {
                "STATE_FIELD_ERC4626_TOTAL_SUPPLY" => {
                    let r = rpc.eth_call(&vault_address, format!("0x{}", selector("totalSupply()")), &hash)?;
                    supply.check(field, *n, value, word(&r, 0)?.to_string(), ctx);
                }
                "STATE_FIELD_AAVE_LIQUIDITY_INDEX" | "STATE_FIELD_AAVE_CURRENT_LIQUIDITY_RATE" | "STATE_FIELD_AAVE_LAST_UPDATE_TIMESTAMP" => {
                    ensure!(text(g, "key")? == asset, "reserve row keyed by another asset");
                    let index = match field {
                        "STATE_FIELD_AAVE_LIQUIDITY_INDEX" => 1,
                        "STATE_FIELD_AAVE_CURRENT_LIQUIDITY_RATE" => 2,
                        _ => 6,
                    };
                    reserves.check(field, *n, value, word(&reserve_at()?, index)?.to_string(), ctx);
                }
                _ => {}
            }
        }
        for h in data["holderBasis"].as_array().into_iter().flatten() {
            let holder = text(h, "holder")?;
            holders.insert(holder.to_string());
            let count: BigUint = text(h, "value")?.parse()?;
            let ctx = json!({"holder": holder});
            let r = rpc.eth_call(&vault_address, format!("0x70a08231{}", arg(holder)), &hash)?;
            shares.check("balanceOf", *n, count.to_string(), word(&r, 0)?.to_string(), ctx.clone());
            let rd = reserve_at()?;
            let model = StataTokenLm {
                reserve: Reserve {
                    liquidity_index: word(&rd, 1)?,
                    current_liquidity_rate: word(&rd, 2)?,
                    last_update_timestamp: u64::try_from(word(&rd, 6)?).context("timestamp")?,
                },
                reserve_active_and_unpaused: None,
            };
            let converted = model.convert_to_assets(&count, timestamp).map_err(|e| anyhow::anyhow!("{e:?}"))?;
            let r = rpc.eth_call(&vault_address, format!("0x{}{}", selector("convertToAssets(uint256)"), uint(&count)), &hash)?;
            assets.check(
                "convertToAssets via conformance::erc4626",
                *n,
                converted.to_string(),
                word(&r, 0)?.to_string(),
                ctx.clone(),
            );
            let rate = model.rate(timestamp).map_err(|e| anyhow::anyhow!("{e:?}"))?;
            let r = rpc.eth_call(&vault_address, format!("0x{}", selector("rate()")), &hash)?;
            rates.check("rate via conformance::aave", *n, rate.to_string(), word(&r, 0)?.to_string(), ctx);
        }
    }

    let first = text(&blocks.values().next().unwrap()["clocks"][0], "hash")?.to_string();
    let last = text(&blocks.values().last().unwrap()["clocks"][0], "hash")?.to_string();
    let mut identities = Vec::new();
    let mut identity_ok = true;
    let mut pointers = vec![("pool".to_string(), pool_address.clone(), *pool_slot, pool_implementation.clone())];
    if let (Some(slot), Some(implementation)) = (vault.implementation_slot, &vault.implementation) {
        pointers.push(("vault".to_string(), vault_address.clone(), slot, implementation.clone()));
    } else {
        identity_ok = false;
    }
    for (label, proxy, slot, expected) in &pointers {
        for block in [&first, &last] {
            let reference = json!({"blockHash": block, "requireCanonical": true});
            let pointer = rpc.call("eth_getStorageAt", json!([proxy, hex0x(slot), reference]))?;
            let pointer = pointer.as_str().context("storage")?.trim_start_matches("0x").to_string();
            let found = format!("0x{}", &format!("{pointer:0>64}")[24..]);
            let expected = hex0x(expected);
            identity_ok &= found == expected;
            let code = rpc.call("eth_getCode", json!([expected, reference]))?;
            let code = hex::decode(code.as_str().context("code")?.trim_start_matches("0x"))?;
            identities.push(json!({"contract": label, "proxy": proxy, "block_hash": block, "implementation_expected": expected, "implementation_found": found, "implementation_code_hash": hex0x(&keccak(&code))}));
        }
    }
    let r = rpc.eth_call(&vault_address, format!("0x{}", selector("STATIC__ATOKEN_LM_REVISION()")), &last)?;
    let revision = word(&r, 0)?.to_string();
    identity_ok &= revision == vault.implementation_revision;

    let mismatches = [&clocks, &shares, &supply, &reserves, &assets, &rates]
        .iter()
        .map(|t| t.mismatches.len())
        .sum::<usize>();
    let passed = mismatches == 0 && identity_ok;
    let report = json!({
        "tool": "erc4626-balance-state-tools live-parity",
        "status": if passed { "passed" } else { "failed" },
        "events_sha256": hex::encode(digest.finalize()),
        "package": {"spkg_sha256": hex::encode(Sha256::digest(std::fs::read(&args.spkg)?)), "params_file_sha256": hex::encode(Sha256::digest(std::fs::read(&args.params)?))},
        "endpoints": {"substreams": args.endpoint, "rpc": "RPC_URL (credential-bearing; not recorded)"},
        "blocks": {"delivered": blocks.len(), "first": blocks.keys().next(), "last": blocks.keys().last()},
        "rows": rows,
        "distinct_holders": holders.len(),
        "checks": {"clock": clocks.json(), "shares": shares.json(), "total_supply": supply.json(), "reserve_words": reserves.json(), "convert_to_assets": assets.json(), "rate": rates.json()},
        "epochs": epochs,
        "identities": identities,
        "revision": {"declared": vault.implementation_revision, "rpc": revision},
        "limits": "Same-block parity for the delivered blocks and the holders written in them only; `maxWithdraw`/`maxRedeem` limits and reward accounting are not checked.",
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
mod tests {
    use super::*;

    #[test]
    fn selectors_and_encodings_match_the_known_values() {
        assert_eq!(selector("rate()"), "2c4e722e");
        assert_eq!(selector("convertToAssets(uint256)"), "07a2d13a");
        assert_eq!(selector("STATIC__ATOKEN_LM_REVISION()"), "fa714610");
        assert_eq!(uint(&BigUint::from(255u8)), format!("{:0>64}", "ff"));
        assert_eq!(arg("0xABcd"), format!("{:0>64}", "abcd"));
    }

    struct Fake {
        shares: u128,
    }
    impl Chain for Fake {
        fn call(&self, method: &str, params: Value) -> Result<Value> {
            let ray: u128 = 10u128.pow(27);
            let w = |v: u128| format!("{v:064x}");
            Ok(match method {
                "eth_getBlockByNumber" => json!({"hash": format!("0x{:064x}", 1), "parentHash": format!("0x{:064x}", 2)}),
                "eth_getStorageAt" => {
                    let implementation = if params[0].as_str() == Some("0x6807dc923806fe8fd134338eabca509979a7e0cb") {
                        "5e2b0fcc5b9734c7ec0a03401ee9e6805f783b6d"
                    } else {
                        "1d69c48a35ddd241e72a31db0e637676d89fc553"
                    };
                    json!(format!("0x{implementation:0>64}"))
                }
                "eth_getCode" => json!("0x6000"),
                "eth_call" => {
                    let data = params[0]["data"].as_str().unwrap().trim_start_matches("0x").to_string();
                    let out = if data.starts_with(&selector("getReserveData(address)")) {
                        let mut words = vec![w(0); 15];
                        words[1] = w(ray + ray / 10);
                        words[6] = w(1_790_000_000);
                        words.concat()
                    } else if data.starts_with("70a08231") {
                        w(self.shares)
                    } else if data.starts_with(&selector("convertToAssets(uint256)")) {
                        w(1_000 * 11 / 10)
                    } else if data.starts_with(&selector("rate()")) {
                        w(ray + ray / 10)
                    } else if data.starts_with(&selector("STATIC__ATOKEN_LM_REVISION()")) {
                        w(2)
                    } else if data.starts_with(&selector("totalSupply()")) {
                        w(5_000)
                    } else {
                        bail!("unexpected call {data}")
                    };
                    json!(format!("0x{out}"))
                }
                other => bail!("unexpected {other}"),
            })
        }
    }
    fn setup(dir: &std::path::Path, output: &str) -> Cli {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let mut params: Value = serde_json::from_str(&std::fs::read_to_string(root.join("tests/fixtures/bsc-stata-usdt-epoch.json")).unwrap()).unwrap();
        params["vaults"][0]["implementation"] = json!("0x1d69c48a35ddd241e72a31db0e637676d89fc553");
        let params_path = dir.join("params.json");
        std::fs::write(&params_path, params.to_string()).unwrap();
        let line = json!({"@module": "map_events", "@block": 10, "@data": {
            "clocks": [{"number": "10", "hash": format!("0x{:064x}", 1), "parentHash": format!("0x{:064x}", 2), "timestamp": "1790000000"}],
            "holderBasis": [{"holder": "0x00000000000000000000000000000000000000aa", "value": "1000"}],
            "globalState": [{"field": "STATE_FIELD_ERC4626_TOTAL_SUPPLY", "value": "5000"}]
        }});
        let events = dir.join(format!("{output}.jsonl"));
        std::fs::write(&events, line.to_string()).unwrap();
        let spkg = dir.join("p.spkg");
        std::fs::write(&spkg, b"spkg").unwrap();
        Cli {
            events: vec![events],
            params: params_path,
            spkg,
            endpoint: "bsc.substreams.example:443".into(),
            output: dir.join(output),
        }
    }

    #[test]
    fn matching_getters_pass_and_a_share_mismatch_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let args = setup(tmp.path(), "ok");
        assert!(check(&args, &Fake { shares: 1_000 }).unwrap());
        let report: Value = serde_json::from_str(&std::fs::read_to_string(args.output.join("report.json")).unwrap()).unwrap();
        for (check, count) in [("shares", 1), ("convert_to_assets", 1), ("rate", 1), ("total_supply", 1), ("clock", 2)] {
            assert_eq!(report["checks"][check]["checked"], count, "{check}");
        }
        let args = setup(tmp.path(), "bad");
        assert!(!check(&args, &Fake { shares: 999 }).unwrap());
    }
}
