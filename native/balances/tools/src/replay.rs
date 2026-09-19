//! Offline replay: reduce captured Extended block files with the production
//! map logic, check per-account continuity across consecutive blocks, and
//! compare against saved RPC oracles and historical prototype rows.
//!
//! This is saved-data evidence for the Rust reducer. It does not execute the
//! packaged WASM, does not contact any network and does not qualify an SPKG.
use anyhow::{ensure, Context, Result};
use clap::Args;
use native_balances::{changes, records, Change, Params};
use prost::Message;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};
use substreams_ethereum::pb::eth::v2 as eth;

#[derive(Args, Debug)]
pub struct Replay {
    /// Directories of captured `<height>.pb` Extended blocks; may be repeated.
    /// The same height in two directories must be byte-identical.
    #[arg(long, required = true)]
    pub blocks: Vec<PathBuf>,
    /// Extended producer versions accepted by the map, as in its parameters.
    #[arg(long, default_value = "5", value_delimiter = ',')]
    pub producer_versions: Vec<i32>,
    /// Saved RPC oracle files shaped like `bsc-122260950.json` (rpc_checks
    /// rows with an empty contract are native balances).
    #[arg(long)]
    pub oracle: Vec<PathBuf>,
    /// Historical prototype native-row fixtures (old/new amount and ordinal).
    #[arg(long)]
    pub prototype_rows: Vec<PathBuf>,
    /// Label recorded in the report; the block data itself carries no chain id.
    #[arg(long, default_value = "bsc")]
    pub network: String,
    #[arg(long)]
    pub output: PathBuf,
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn sha256_file(path: &Path) -> Result<String> {
    Ok(sha256_bytes(&fs::read(path)?))
}
fn hex0x(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn source_hashes() -> Result<Value> {
    let tools = Path::new(env!("CARGO_MANIFEST_DIR"));
    let map = tools.parent().context("tools dir")?.join("src/lib.rs");
    let persist = tools.parent().unwrap().parent().unwrap().parent().unwrap().join("common/persist/src/lib.rs");
    Ok(json!({
        "native/balances/src/lib.rs": sha256_file(&map)?,
        "common/persist/src/lib.rs": sha256_file(&persist)?,
    }))
}

/// Enumerate `<height>.pb` files across directories, deduplicating identical
/// captures of the same height and rejecting conflicting ones.
pub fn enumerate(dirs: &[PathBuf]) -> Result<BTreeMap<u64, PathBuf>> {
    let mut files: BTreeMap<u64, (PathBuf, String)> = BTreeMap::new();
    for dir in dirs {
        ensure!(dir.is_dir(), "not a directory: {}", dir.display());
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            let Some(stem) = path.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".pb")) else {
                continue;
            };
            let Ok(height) = stem.parse::<u64>() else {
                continue;
            };
            let digest = sha256_file(&path)?;
            if let Some((existing, existing_digest)) = files.get(&height) {
                ensure!(
                    existing_digest == &digest,
                    "conflicting captures of block {height}: {} and {}",
                    existing.display(),
                    path.display()
                );
                continue;
            }
            files.insert(height, (path, digest));
        }
    }
    ensure!(!files.is_empty(), "no <height>.pb block files found");
    Ok(files.into_iter().map(|(h, (p, _))| (h, p)).collect())
}

fn ranges(heights: &BTreeSet<u64>) -> Vec<[u64; 2]> {
    let mut out: Vec<[u64; 2]> = Vec::new();
    for &h in heights {
        match out.last_mut() {
            Some(last) if last[1] == h => last[1] = h + 1,
            _ => out.push([h, h + 1]),
        }
    }
    out
}

fn reason_name(reason: i32) -> String {
    eth::balance_change::Reason::try_from(reason)
        .map(|r| r.as_str_name().to_string())
        .unwrap_or_else(|_| format!("UNKNOWN_{reason}"))
}

#[derive(Default)]
struct Ledger {
    /// Last observed (height, final amount) per account.
    last: BTreeMap<Vec<u8>, (u64, String)>,
    /// First height of the current contiguous run of replayed blocks.
    run_start: u64,
    checks: u64,
    mismatches: Vec<Value>,
    skipped_gaps: u64,
}
impl Ledger {
    fn observe(&mut self, height: u64, rows: &[Change]) {
        for row in rows {
            if let Some((last_height, last_amount)) = self.last.get(&row.address) {
                if *last_height >= self.run_start {
                    self.checks += 1;
                    if *last_amount != row.old_amount {
                        self.mismatches.push(json!({
                            "address": hex0x(&row.address), "previous_block": last_height, "previous_amount": last_amount,
                            "block": height, "old_amount": row.old_amount,
                        }));
                    }
                } else {
                    self.skipped_gaps += 1;
                }
            }
            self.last.insert(row.address.clone(), (height, row.amount.clone()));
        }
    }
}

struct OracleFile {
    path: PathBuf,
    block: u64,
    hash: Option<String>,
    rows: BTreeMap<String, String>,
}
fn load_oracles(paths: &[PathBuf]) -> Result<Vec<OracleFile>> {
    let mut out = Vec::new();
    for path in paths {
        let v: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
        let block = v["block_number"].as_u64().context("oracle block_number")?;
        let mut rows = BTreeMap::new();
        for check in v["rpc_checks"].as_array().context("oracle rpc_checks")? {
            if check["contract"].as_str() != Some("") {
                continue;
            }
            let address = check["address"].as_str().context("oracle address")?.to_lowercase();
            let balance = check["balance"].as_str().context("oracle balance")?.to_string();
            ensure!(rows.insert(address, balance).is_none(), "duplicate oracle row");
        }
        ensure!(!rows.is_empty(), "oracle has no native rows: {}", path.display());
        out.push(OracleFile {
            path: path.clone(),
            block,
            hash: v["block_hash"].as_str().map(str::to_lowercase),
            rows,
        });
    }
    Ok(out)
}
struct PrototypeFile {
    path: PathBuf,
    block: u64,
    hash: Option<String>,
    rows: BTreeMap<String, (String, String, u64)>,
}
fn load_prototypes(paths: &[PathBuf]) -> Result<Vec<PrototypeFile>> {
    let mut out = Vec::new();
    for path in paths {
        let v: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
        let block = v["block"]["number"].as_u64().context("prototype block number")?;
        let mut rows = BTreeMap::new();
        for row in v["native_rows"].as_array().context("prototype native_rows")? {
            let address = row["address"].as_str().context("address")?.to_lowercase();
            let old = row["old_amount"].as_str().context("old_amount")?.to_string();
            let new = row["amount"].as_str().context("amount")?.to_string();
            let ordinal = row["ordinal"].as_u64().context("ordinal")?;
            ensure!(rows.insert(address, (old, new, ordinal)).is_none(), "duplicate prototype row");
        }
        out.push(PrototypeFile {
            path: path.clone(),
            block,
            hash: v["block"]["hash"].as_str().map(str::to_lowercase),
            rows,
        });
    }
    Ok(out)
}

fn compare_rows(expected: &BTreeMap<String, String>, actual: &[Change]) -> Value {
    let actual: BTreeMap<String, &Change> = actual.iter().map(|r| (hex0x(&r.address), r)).collect();
    let mut matched = 0u64;
    let mut mismatched = Vec::new();
    let mut missing = Vec::new();
    for (address, amount) in expected {
        match actual.get(address) {
            Some(row) if &row.amount == amount => matched += 1,
            Some(row) => mismatched.push(json!({"address": address, "expected": amount, "actual": row.amount})),
            None => missing.push(address.clone()),
        }
    }
    let extra: Vec<_> = actual.keys().filter(|a| !expected.contains_key(*a)).cloned().collect();
    json!({
        "expected_rows": expected.len(), "matched": matched, "mismatched": mismatched,
        "missing_in_output": missing, "extra_in_output": extra,
        "status": if mismatched.is_empty() && missing.is_empty() && extra.is_empty() { "match" } else { "mismatch" },
    })
}

pub fn run(args: Replay) -> Result<bool> {
    let started = Instant::now();
    ensure!(
        !args.output.exists(),
        "output directory exists; use a fresh directory: {}",
        args.output.display()
    );
    fs::create_dir_all(&args.output)?;
    let params = Params {
        producer_versions: args.producer_versions.clone(),
    };
    ensure!(!params.producer_versions.is_empty(), "at least one producer version required");
    let mut report = json!({
        "status": "incomplete",
        "tool_language": "Rust",
        "scope": "Offline Rust reduction of captured Extended blocks; not packaged WASM execution, not network qualification",
        "network_label": args.network,
        "producer_versions": params.producer_versions,
        "source_sha256": source_hashes()?,
        "rpc_in_replay": false,
    });
    let result = replay(&args, &params, &mut report);
    if let Err(error) = &result {
        report["status"] = json!("incomplete");
        report["failure"] = json!(format!("{error:#}"));
    }
    report["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
    fs::write(args.output.join("report.json"), serde_json::to_string_pretty(&report)? + "\n")?;
    let mut summary = report.clone();
    summary.as_object_mut().unwrap().remove("directories");
    println!("{}", serde_json::to_string_pretty(&summary)?);
    result?;
    Ok(report["status"] == "replayed")
}

fn replay(args: &Replay, params: &Params, report: &mut Value) -> Result<()> {
    let files = enumerate(&args.blocks)?;
    let oracles = load_oracles(&args.oracle)?;
    let prototypes = load_prototypes(&args.prototype_rows)?;
    let mut directories = Vec::new();
    for dir in &args.blocks {
        let count = fs::read_dir(dir)?
            .filter(|e| e.as_ref().is_ok_and(|e| e.path().extension().is_some_and(|x| x == "pb")))
            .count();
        directories.push(json!({"path": dir.display().to_string(), "pb_files": count}));
    }
    report["directories"] = json!(directories);
    let heights: BTreeSet<u64> = files.keys().copied().collect();
    report["blocks"] = json!(files.len());
    report["ranges"] = json!(ranges(&heights));

    let mut blocks_out = fs::File::create(args.output.join("blocks.jsonl"))?;
    let mut rows_out = fs::File::create(args.output.join("rows.jsonl"))?;
    let mut ledger = Ledger::default();
    let mut previous: Option<(u64, Vec<u8>)> = None;
    let mut clock_links = 0u64;
    let mut clock_mismatches = Vec::new();
    let mut reason_matrix: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    let mut total_rows = 0u64;
    let mut zero_final_rows = 0u64;
    let mut zero_address_rows = 0u64;
    let mut total_records = 0u64;
    let mut max_records_per_row = 0u32;
    let mut accounts: BTreeSet<Vec<u8>> = BTreeSet::new();
    let mut versions: BTreeMap<i32, u64> = BTreeMap::new();
    let mut empty_blocks = 0u64;
    let mut projection_errors = Vec::new();
    let mut unqualified_version_blocks = Vec::new();
    let mut oracle_results = Vec::new();
    let mut prototype_results = Vec::new();

    for (&height, path) in &files {
        let bytes = fs::read(path)?;
        let block = eth::Block::decode(bytes.as_slice()).with_context(|| format!("decode {}", path.display()))?;
        ensure!(block.number == height, "file {} holds block {}", path.display(), block.number);
        let header = block.header.as_ref().context("missing header")?;
        if let Some((prev_height, prev_hash)) = &previous {
            if *prev_height + 1 == height {
                clock_links += 1;
                if &header.parent_hash != prev_hash {
                    clock_mismatches.push(json!({"block": height, "parent_hash": hex0x(&header.parent_hash), "previous_hash": hex0x(prev_hash)}));
                }
            } else {
                ledger.run_start = height;
            }
        } else {
            ledger.run_start = height;
        }
        previous = Some((height, block.hash.clone()));
        *versions.entry(block.ver).or_default() += 1;
        if !params.producer_versions.contains(&block.ver) {
            // The map refuses unqualified producer versions by design; record
            // the refusal separately from a reduction error.
            unqualified_version_blocks.push(json!({"block": height, "ver": block.ver}));
            writeln!(
                blocks_out,
                "{}",
                json!({"block": height, "hash": hex0x(&block.hash), "sha256": sha256_bytes(&bytes), "ver": block.ver, "refused": "unqualified producer version"})
            )?;
            // A refused block was not reduced, so continuity across it is unknown.
            ledger.run_start = height + 1;
            continue;
        }

        let rows = match changes(&block, params) {
            Ok(rows) => rows,
            Err(error) => {
                projection_errors.push(json!({"block": height, "error": error.to_string()}));
                writeln!(
                    blocks_out,
                    "{}",
                    json!({"block": height, "hash": hex0x(&block.hash), "sha256": sha256_bytes(&bytes), "error": error.to_string()})
                )?;
                continue;
            }
        };
        for record in records(&block, params)? {
            *reason_matrix
                .entry(format!("{:?}", record.scope))
                .or_default()
                .entry(reason_name(record.reason))
                .or_default() += 1;
            total_records += 1;
        }
        if rows.is_empty() {
            empty_blocks += 1;
        }
        for row in &rows {
            total_rows += 1;
            if row.amount == "0" {
                zero_final_rows += 1;
            }
            if row.address.iter().all(|b| *b == 0) {
                zero_address_rows += 1;
            }
            max_records_per_row = max_records_per_row.max(row.records);
            accounts.insert(row.address.clone());
            writeln!(
                rows_out,
                "{}",
                json!({"block": height, "address": hex0x(&row.address), "old_amount": row.old_amount, "amount": row.amount,
                    "first_ordinal": row.first_ordinal, "ordinal": row.ordinal, "records": row.records})
            )?;
        }
        ledger.observe(height, &rows);
        writeln!(
            blocks_out,
            "{}",
            json!({"block": height, "hash": hex0x(&block.hash), "parent_hash": hex0x(&header.parent_hash), "ver": block.ver,
                "sha256": sha256_bytes(&bytes), "rows": rows.len(), "transactions": block.transaction_traces.len()})
        )?;
        for oracle in oracles.iter().filter(|o| o.block == height) {
            let mut result = compare_rows(&oracle.rows, &rows);
            result["file"] = json!(oracle.path.display().to_string());
            result["block"] = json!(height);
            result["hash_bound"] = json!(oracle.hash.as_deref() == Some(&hex0x(&block.hash)));
            oracle_results.push(result);
        }
        for prototype in prototypes.iter().filter(|p| p.block == height) {
            let actual: BTreeMap<String, &Change> = rows.iter().map(|r| (hex0x(&r.address), r)).collect();
            let mut matched = 0u64;
            let mut mismatched = Vec::new();
            for (address, (old, new, ordinal)) in &prototype.rows {
                match actual.get(address) {
                    Some(r) if &r.old_amount == old && &r.amount == new && r.ordinal == *ordinal => matched += 1,
                    Some(r) => mismatched
                        .push(json!({"address": address, "expected": [old, new, ordinal], "actual": [r.old_amount.clone(), r.amount.clone(), r.ordinal]})),
                    None => mismatched.push(json!({"address": address, "expected": [old, new, ordinal], "actual": null})),
                }
            }
            let extra = actual.len() as u64 - matched - mismatched.iter().filter(|m| !m["actual"].is_null()).count() as u64;
            prototype_results.push(json!({
                "file": prototype.path.display().to_string(), "block": height,
                "hash_bound": prototype.hash.as_deref() == Some(&hex0x(&block.hash)),
                "expected_rows": prototype.rows.len(), "matched": matched, "mismatched": mismatched, "extra_in_output": extra,
                "status": if mismatched.is_empty() && extra == 0 { "match" } else { "mismatch" },
            }));
        }
    }
    for oracle in &oracles {
        if !heights.contains(&oracle.block) {
            oracle_results.push(json!({"file": oracle.path.display().to_string(), "block": oracle.block, "status": "block_not_replayed"}));
        }
    }
    for prototype in &prototypes {
        if !heights.contains(&prototype.block) {
            prototype_results.push(json!({"file": prototype.path.display().to_string(), "block": prototype.block, "status": "block_not_replayed"}));
        }
    }
    blocks_out.flush()?;
    rows_out.flush()?;

    report["producer_versions_observed"] = json!(versions);
    report["unqualified_version_blocks"] = json!({
        "count": unqualified_version_blocks.len(),
        "scope": "blocks the map refuses because Block.ver is not in producer_versions; not reduced, not an error of the reducer",
        "blocks": unqualified_version_blocks,
    });
    report["replayed_blocks"] = json!(files.len() - report["unqualified_version_blocks"]["count"].as_u64().unwrap() as usize);
    report["records"] = json!(total_records);
    report["reason_matrix"] = json!(reason_matrix);
    report["rows"] = json!(total_rows);
    report["zero_final_rows"] = json!(zero_final_rows);
    report["zero_address_rows"] = json!(zero_address_rows);
    report["distinct_accounts"] = json!(accounts.len());
    report["max_records_per_row"] = json!(max_records_per_row);
    report["empty_output_blocks"] = json!(empty_blocks);
    report["projection_errors"] = json!(projection_errors);
    report["clock_links"] = json!({"checked": clock_links, "mismatches": clock_mismatches});
    report["continuity"] = json!({
        "scope": "old_amount of an account equals its last emitted amount when every intervening block was replayed",
        "checks": ledger.checks, "mismatches": ledger.mismatches, "skipped_across_gaps": ledger.skipped_gaps,
    });
    report["oracle"] = json!(oracle_results);
    report["prototype"] = json!(prototype_results);
    report["outputs"] = json!({
        "blocks.jsonl": sha256_file(&args.output.join("blocks.jsonl"))?,
        "rows.jsonl": sha256_file(&args.output.join("rows.jsonl"))?,
    });
    let ok = report["projection_errors"].as_array().unwrap().is_empty()
        && report["clock_links"]["mismatches"].as_array().unwrap().is_empty()
        && report["continuity"]["mismatches"].as_array().unwrap().is_empty()
        && oracle_results.iter().all(|o| o["status"] == "match")
        && prototype_results.iter().all(|p| p["status"] == "match");
    // Mismatches are a recorded result, not a tool failure; only I/O, decode
    // or malformed-input errors leave the report incomplete.
    report["status"] = json!(if ok { "replayed" } else { "mismatch" });
    Ok(())
}
