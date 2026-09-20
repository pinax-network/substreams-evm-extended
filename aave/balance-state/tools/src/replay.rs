//! Offline replay: run the production projection over captured blocks, check
//! per-key continuity across consecutive blocks, and reproduce every stored
//! liquidity-index update from the previous index, rate and clock observed in
//! the same block with the exact conformance arithmetic. Saved-data evidence
//! for the Rust map; not packaged WASM execution, not RPC qualification.
use aave_balance_state::{parse, project};
use anyhow::{ensure, Context, Result};
use clap::Args;
use conformance::aave::{next_liquidity_index, Reserve};
use num_bigint::BigUint;
use prost::Message;
use proto::pb::evm::balance_state::v1 as pb;
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
    #[arg(long, required = true)]
    pub blocks: Vec<PathBuf>,
    /// Epoch configuration JSON passed as the map parameters.
    #[arg(long)]
    pub params: PathBuf,
    #[arg(long)]
    pub output: PathBuf,
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn hex0x(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn big(s: &str) -> Result<BigUint> {
    s.parse::<BigUint>().map_err(|_| anyhow::anyhow!("not a decimal integer: {s}"))
}

pub fn enumerate(dirs: &[PathBuf]) -> Result<BTreeMap<u64, PathBuf>> {
    let mut files: BTreeMap<u64, (PathBuf, String)> = BTreeMap::new();
    for dir in dirs {
        ensure!(dir.is_dir(), "not a directory: {}", dir.display());
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            let Some(height) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".pb"))
                .and_then(|n| n.parse::<u64>().ok())
            else {
                continue;
            };
            let digest = sha256_bytes(&fs::read(&path)?);
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

#[derive(Default)]
struct Continuity {
    last: BTreeMap<Vec<u8>, (u64, String)>,
    run_start: u64,
    checks: u64,
    mismatches: Vec<Value>,
    skipped: u64,
}
impl Continuity {
    fn observe(&mut self, height: u64, key: Vec<u8>, previous: &str, value: &str, label: &str) {
        if let Some((last_height, last_value)) = self.last.get(&key) {
            if *last_height >= self.run_start {
                self.checks += 1;
                if last_value != previous {
                    self.mismatches.push(json!({"kind": label, "key": hex0x(&key), "previous_block": last_height, "previous_value": last_value, "block": height, "old_value": previous}));
                }
            } else {
                self.skipped += 1;
            }
        }
        self.last.insert(key, (height, value.to_string()));
    }
}

pub fn run(args: Replay) -> Result<bool> {
    let started = Instant::now();
    ensure!(
        !args.output.exists(),
        "output directory exists; use a fresh directory: {}",
        args.output.display()
    );
    fs::create_dir_all(&args.output)?;
    let params = fs::read_to_string(&args.params)?;
    let config = parse(&params).map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut report = json!({
        "status": "incomplete",
        "tool_language": "Rust",
        "scope": "Offline Rust projection of captured Extended blocks with the aave/balance-state map; not packaged WASM execution, not RPC qualification",
        "parameters_file": args.params.display().to_string(),
        "parameters_sha256": config.parameters_sha256,
        "chain_id": config.chain_id,
        "producer_versions": config.producer_versions,
        "markets": config.markets.iter().map(|m| json!({"atoken": hex0x(&m.atoken), "underlying": hex0x(&m.underlying), "epoch": m.epoch, "model_id": m.model_id})).collect::<Vec<_>>(),
        "rpc_in_replay": false,
    });
    let result = replay(&args, &params, &mut report);
    if let Err(error) = &result {
        report["status"] = json!("incomplete");
        report["failure"] = json!(format!("{error:#}"));
    }
    report["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
    fs::write(args.output.join("report.json"), serde_json::to_string_pretty(&report)? + "\n")?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    result?;
    Ok(report["status"] == "replayed")
}

fn replay(args: &Replay, params: &str, report: &mut Value) -> Result<()> {
    let config = parse(params).map_err(|e| anyhow::anyhow!("{e}"))?;
    let files = enumerate(&args.blocks)?;
    let heights: BTreeSet<u64> = files.keys().copied().collect();
    let mut rows_out = fs::File::create(args.output.join("rows.jsonl"))?;
    let mut previous: Option<(u64, Vec<u8>)> = None;
    let mut clock_links = 0u64;
    let mut clock_mismatches = Vec::new();
    let mut holders = Continuity::default();
    let mut globals = Continuity::default();
    let mut projection_errors = Vec::new();
    let mut unqualified_version_blocks = Vec::new();
    let mut holder_rows = 0u64;
    let mut global_rows = 0u64;
    let mut epoch_rows = 0u64;
    let mut known_zero_rows = 0u64;
    let mut blocks_with_rows = 0u64;
    let mut accounts: BTreeSet<Vec<u8>> = BTreeSet::new();
    let mut field_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut oracle_checks = 0u64;
    let mut oracle_mismatches = Vec::new();
    let mut oracle_unchecked = Vec::new();
    let mut replayed = 0u64;

    for (&height, path) in &files {
        let bytes = fs::read(path)?;
        let block = eth::Block::decode(bytes.as_slice()).with_context(|| format!("decode {}", path.display()))?;
        ensure!(block.number == height, "file {} holds block {}", path.display(), block.number);
        let header = block.header.as_ref().context("missing header")?;
        match &previous {
            Some((prev_height, prev_hash)) if *prev_height + 1 == height => {
                clock_links += 1;
                if &header.parent_hash != prev_hash {
                    clock_mismatches.push(json!({"block": height, "parent_hash": hex0x(&header.parent_hash), "previous_hash": hex0x(prev_hash)}));
                }
            }
            _ => {
                holders.run_start = height;
                globals.run_start = height;
            }
        }
        previous = Some((height, block.hash.clone()));
        if !config.producer_versions.contains(&block.ver) {
            unqualified_version_blocks.push(json!({"block": height, "ver": block.ver}));
            holders.run_start = height + 1;
            globals.run_start = height + 1;
            continue;
        }
        let events = match project(&block, &config) {
            Ok(events) => events,
            Err(error) => {
                projection_errors.push(json!({"block": height, "error": error.to_string()}));
                holders.run_start = height + 1;
                globals.run_start = height + 1;
                continue;
            }
        };
        replayed += 1;
        ensure!(
            events.clocks.len() == 1 && events.clocks[0].number == height,
            "block {height} must carry exactly one clock row"
        );
        let clock = &events.clocks[0];
        ensure!(
            (clock.holder_basis_count as usize, clock.global_state_count as usize, clock.epoch_count as usize)
                == (events.holder_basis.len(), events.global_state.len(), events.epochs.len()),
            "clock row counts disagree with rows at block {height}"
        );
        if !events.holder_basis.is_empty() || !events.global_state.is_empty() {
            blocks_with_rows += 1;
        }
        epoch_rows += events.epochs.len() as u64;
        for h in &events.holder_basis {
            holder_rows += 1;
            if h.value == "0" {
                known_zero_rows += 1;
            }
            accounts.insert(h.holder.clone());
            let key = [h.market.as_slice(), h.holder.as_slice()].concat();
            holders.observe(height, key, &h.previous_value, &h.value, "holder");
            writeln!(
                rows_out,
                "{}",
                json!({"block": height, "table": "holder_basis", "market": hex0x(&h.market), "holder": hex0x(&h.holder), "previous": h.previous_value, "value": h.value, "ordinal": h.ordinal, "count": h.change_count})
            )?;
        }
        // Reserve oracle: the index written in this block must equal the
        // model's projection from the previous index, rate and clock.
        let mut by_market: BTreeMap<Vec<u8>, BTreeMap<i32, &pb::GlobalState>> = BTreeMap::new();
        for g in &events.global_state {
            global_rows += 1;
            let name = pb::StateField::try_from(g.field)
                .map(|f| f.as_str_name().to_string())
                .unwrap_or_else(|_| format!("UNKNOWN_{}", g.field));
            *field_counts.entry(name).or_default() += 1;
            let key = [g.market.as_slice(), &g.field.to_be_bytes(), g.key.as_slice()].concat();
            globals.observe(height, key, &g.previous_value, &g.value, "global");
            by_market.entry(g.market.clone()).or_default().insert(g.field, g);
            writeln!(
                rows_out,
                "{}",
                json!({"block": height, "table": "global_state", "market": hex0x(&g.market), "field": g.field, "key": hex0x(&g.key), "previous": g.previous_value, "value": g.value, "ordinal": g.ordinal, "count": g.change_count})
            )?;
        }
        for (market, fields) in &by_market {
            let Some(index) = fields.get(&(pb::StateField::AaveLiquidityIndex as i32)) else {
                continue;
            };
            let rate = fields.get(&(pb::StateField::AaveCurrentLiquidityRate as i32));
            let clock_field = fields.get(&(pb::StateField::AaveLastUpdateTimestamp as i32));
            let (Some(rate), Some(clock_field)) = (rate, clock_field) else {
                oracle_unchecked
                    .push(json!({"block": height, "market": hex0x(market), "reason": "index written without rate or clock write in the same block"}));
                continue;
            };
            let reserve = Reserve {
                liquidity_index: big(&index.previous_value)?,
                current_liquidity_rate: big(&rate.previous_value)?,
                last_update_timestamp: clock_field.previous_value.parse()?,
            };
            let now: u64 = clock_field.value.parse()?;
            ensure!(
                now == clock.timestamp,
                "reserve clock {now} differs from block timestamp {} at {height}",
                clock.timestamp
            );
            oracle_checks += 1;
            match next_liquidity_index(&reserve, now) {
                Ok(expected) if expected.to_string() == index.value => {}
                Ok(expected) => oracle_mismatches.push(json!({"block": height, "market": hex0x(market), "expected": expected.to_string(), "actual": index.value, "previous_index": index.previous_value, "previous_rate": rate.previous_value, "previous_clock": clock_field.previous_value, "clock": now})),
                Err(e) => oracle_mismatches.push(json!({"block": height, "market": hex0x(market), "error": format!("{e:?}")})),
            }
        }
    }
    rows_out.flush()?;
    report["blocks"] = json!(files.len());
    report["replayed_blocks"] = json!(replayed);
    report["ranges"] = json!(ranges(&heights));
    report["unqualified_version_blocks"] = json!(unqualified_version_blocks);
    report["projection_errors"] = json!(projection_errors);
    report["clock_links"] = json!({"checked": clock_links, "mismatches": clock_mismatches});
    report["blocks_with_rows"] = json!(blocks_with_rows);
    report["holder_basis_rows"] = json!(holder_rows);
    report["known_zero_holder_rows"] = json!(known_zero_rows);
    report["distinct_holders"] = json!(accounts.len());
    report["global_state_rows"] = json!(global_rows);
    report["global_state_fields"] = json!(field_counts);
    report["epoch_rows"] = json!(epoch_rows);
    report["continuity"] = json!({
        "scope": "previous_value equals the last emitted value of the same key when every intervening block was replayed",
        "holder_checks": holders.checks, "holder_mismatches": holders.mismatches, "holder_skipped_across_gaps": holders.skipped,
        "global_checks": globals.checks, "global_mismatches": globals.mismatches, "global_skipped_across_gaps": globals.skipped,
    });
    report["index_oracle"] = json!({
        "scope": "every stored liquidity index write reproduced from the previous index, rate and clock of the same block with conformance::aave::next_liquidity_index",
        "checks": oracle_checks, "mismatches": oracle_mismatches, "unchecked": oracle_unchecked,
    });
    report["outputs"] = json!({"rows.jsonl": sha256_bytes(&fs::read(args.output.join("rows.jsonl"))?)});
    let ok = projection_errors_empty(report)
        && clock_mismatches_empty(report)
        && holders.mismatches.is_empty()
        && globals.mismatches.is_empty()
        && oracle_mismatches.is_empty();
    report["status"] = json!(if ok { "replayed" } else { "mismatch" });
    Ok(())
}
fn projection_errors_empty(report: &Value) -> bool {
    report["projection_errors"].as_array().is_some_and(|a| a.is_empty())
}
fn clock_mismatches_empty(report: &Value) -> bool {
    report["clock_links"]["mismatches"].as_array().is_some_and(|a| a.is_empty())
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
pub fn source_hashes() -> Result<Value> {
    let tools = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = tools.parent().unwrap().parent().unwrap().parent().unwrap();
    Ok(json!({
        "aave/balance-state/src/lib.rs": sha256_bytes(&fs::read(tools.parent().unwrap().join("src/lib.rs"))?),
        "conformance/src/aave.rs": sha256_bytes(&fs::read(root.join("conformance/src/aave.rs"))?),
        "common/persist/src/lib.rs": sha256_bytes(&fs::read(root.join("common/persist/src/lib.rs"))?),
    }))
}
