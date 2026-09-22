//! Offline replay of `evm/executions` over captured `<height>.pb` Extended
//! blocks. It establishes three things from saved data, with no network:
//!
//! 1. the map projects every qualified block, including its receipt/trace log
//!    agreement check, and projects it deterministically with one clock;
//! 2. storage context per frame type: a `DELEGATE` or `CALLCODE` frame writes
//!    the storage of its `caller`, every other frame the storage of its
//!    `address`, which is what `Call.caller`/`Call.address` mean to consumers;
//! 3. a capability matrix per `Block.ver`: which facts each producer version
//!    actually supplies, so a consumer knows what an absent field means.
use anyhow::{ensure, Context, Result};
use clap::Args;
use evm_executions::{parse, project};
use prost::Message;
use proto::pb::evm::executions::v1 as pb;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use substreams_ethereum::pb::eth::v2 as eth;

#[derive(Args, Debug)]
pub struct Replay {
    /// Directories of captured `<height>.pb` Extended blocks; may be repeated.
    /// The same height in two directories must be byte-identical.
    #[arg(long, required = true)]
    pub blocks: Vec<PathBuf>,
    /// Extended producer versions accepted by the map, as in its parameters.
    #[arg(long, default_value = "4,5", value_delimiter = ',')]
    pub producer_versions: Vec<i32>,
    /// Chain id written into the map parameters; the block carries none.
    #[arg(long, default_value_t = 56)]
    pub chain_id: u64,
    /// Label recorded in the report.
    #[arg(long, default_value = "bsc")]
    pub network: String,
    #[arg(long)]
    pub output: PathBuf,
}

const MAX_EXAMPLES: usize = 20;

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn sha256_file(path: &Path) -> Result<String> {
    Ok(sha256_bytes(&fs::read(path).with_context(|| format!("read {}", path.display()))?))
}
fn hex0x(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn source_hashes() -> Result<Value> {
    let tools = Path::new(env!("CARGO_MANIFEST_DIR"));
    let package = tools.parent().context("tools dir")?;
    let root = package.parent().and_then(Path::parent).context("workspace root")?;
    Ok(json!({
        "evm/executions/src/lib.rs": sha256_file(&package.join("src/lib.rs"))?,
        "common/persist/src/lib.rs": sha256_file(&root.join("common/persist/src/lib.rs"))?,
    }))
}

/// Enumerate `<height>.pb` files across directories, deduplicating identical
/// captures of the same height and rejecting conflicting ones.
pub fn enumerate(dirs: &[PathBuf]) -> Result<(BTreeMap<u64, PathBuf>, usize)> {
    let mut files: BTreeMap<u64, (PathBuf, String)> = BTreeMap::new();
    let mut duplicates = 0;
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
                duplicates += 1;
                continue;
            }
            files.insert(height, (path, digest));
        }
    }
    ensure!(!files.is_empty(), "no <height>.pb block files found");
    Ok((files.into_iter().map(|(h, (p, _))| (h, p)).collect(), duplicates))
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

fn call_type_name(raw: i32) -> String {
    eth::CallType::try_from(raw)
        .map(|t| t.as_str_name().to_string())
        .unwrap_or_else(|_| format!("UNKNOWN_{raw}"))
}
fn code_kind_name(raw: i32) -> String {
    pb::CodeChangeKind::try_from(raw)
        .map(|k| k.as_str_name().to_string())
        .unwrap_or_else(|_| format!("UNKNOWN_{raw}"))
}
fn scope_name(raw: i32) -> String {
    pb::Scope::try_from(raw)
        .map(|s| s.as_str_name().to_string())
        .unwrap_or_else(|_| format!("UNKNOWN_{raw}"))
}
fn bump(map: &mut BTreeMap<String, u64>, key: impl Into<String>, by: u64) {
    *map.entry(key.into()).or_default() += by;
}

/// Storage context of a frame: DELEGATECALL and CALLCODE run the callee's
/// code against the caller's storage; every other frame uses its own address.
pub fn storage_context(call: &eth::Call) -> &[u8] {
    if call.call_type == eth::CallType::Delegate as i32 || call.call_type == eth::CallType::Callcode as i32 {
        &call.caller
    } else {
        &call.address
    }
}

#[derive(Default)]
struct Capabilities {
    blocks: u64,
    transactions: u64,
    transaction_types_raw: BTreeMap<String, u64>,
    frames: u64,
    frames_by_call_type: BTreeMap<String, u64>,
    delegated_frames: u64,
    set_code_authorizations: u64,
    blob_transactions: u64,
    transactions_with_blob_gas: u64,
    system_calls: u64,
    block_level_balance_changes: u64,
    block_level_code_changes: u64,
    frames_with_keccak_preimages: u64,
    storage_changes: u64,
    equal_value_storage_changes: u64,
    logs_in_reverted_frames: u64,
    zero_begin_ordinal_frames: u64,
}
impl Capabilities {
    fn observe(&mut self, block: &eth::Block) {
        self.blocks += 1;
        self.system_calls += block.system_calls.len() as u64;
        self.block_level_balance_changes += block.balance_changes.len() as u64;
        self.block_level_code_changes += block.code_changes.len() as u64;
        let frames = block.transaction_traces.iter().flat_map(|tx| &tx.calls).chain(&block.system_calls);
        for call in frames {
            self.frames += 1;
            bump(&mut self.frames_by_call_type, call_type_name(call.call_type), 1);
            self.delegated_frames += u64::from(call.address_delegates_to.as_ref().is_some_and(|a| !a.is_empty()));
            self.frames_with_keccak_preimages += u64::from(!call.keccak_preimages.is_empty());
            self.storage_changes += call.storage_changes.len() as u64;
            self.equal_value_storage_changes += call.storage_changes.iter().filter(|c| c.old_value == c.new_value).count() as u64;
            if call.state_reverted {
                self.logs_in_reverted_frames += call.logs.len() as u64;
            }
            self.zero_begin_ordinal_frames += u64::from(call.begin_ordinal == 0);
        }
        for tx in &block.transaction_traces {
            self.transactions += 1;
            bump(&mut self.transaction_types_raw, tx.r#type.to_string(), 1);
            self.set_code_authorizations += tx.set_code_authorizations.len() as u64;
            self.blob_transactions += u64::from(!tx.blob_hashes.is_empty());
            self.transactions_with_blob_gas += u64::from(tx.blob_gas.is_some());
        }
    }
    fn report(&self) -> Value {
        json!({
            "blocks": self.blocks,
            "transactions": self.transactions,
            "transaction_types_raw": self.transaction_types_raw,
            "frames": self.frames,
            "frames_by_call_type": self.frames_by_call_type,
            "delegated_frames_eip7702": self.delegated_frames,
            "set_code_authorizations": self.set_code_authorizations,
            "blob_transactions": self.blob_transactions,
            "transactions_with_blob_gas": self.transactions_with_blob_gas,
            "system_calls": self.system_calls,
            "block_level_balance_changes": self.block_level_balance_changes,
            "block_level_code_changes": self.block_level_code_changes,
            "frames_with_keccak_preimages": self.frames_with_keccak_preimages,
            "storage_changes": self.storage_changes,
            "equal_value_storage_changes": self.equal_value_storage_changes,
            "logs_in_reverted_frames": self.logs_in_reverted_frames,
            "zero_begin_ordinal_frames": self.zero_begin_ordinal_frames,
        })
    }
}

#[derive(Default)]
struct Totals {
    transactions: u64,
    calls: u64,
    calls_persisted: u64,
    logs: u64,
    logs_persisted: u64,
    receipt_logs: u64,
    code_changes: BTreeMap<String, u64>,
    set_code_applied: u64,
    set_code_discarded: u64,
    created_contracts: u64,
}
impl Totals {
    fn add(&mut self, events: &pb::Events) {
        self.transactions += events.transactions.len() as u64;
        self.calls += events.calls.len() as u64;
        self.calls_persisted += events.calls.iter().filter(|c| c.persisted).count() as u64;
        self.logs += events.logs.len() as u64;
        self.logs_persisted += events.logs.iter().filter(|l| l.persisted).count() as u64;
        self.receipt_logs += events.transactions.iter().map(|t| u64::from(t.receipt_log_count)).sum::<u64>();
        for c in &events.code_changes {
            let key = format!(
                "{}/{}/{}",
                scope_name(c.scope),
                code_kind_name(c.kind),
                if c.persisted { "persisted" } else { "attempted" }
            );
            bump(&mut self.code_changes, key, 1);
        }
        self.set_code_applied += events.set_code_authorizations.iter().filter(|a| a.applied).count() as u64;
        self.set_code_discarded += events.set_code_authorizations.iter().filter(|a| a.discarded).count() as u64;
        self.created_contracts += events.transactions.iter().filter(|t| !t.created_contract.is_empty()).count() as u64;
    }
}

#[derive(Default)]
struct StorageContextCheck {
    frames: u64,
    writes: u64,
    by_call_type: BTreeMap<String, u64>,
    violations: u64,
    examples: Vec<Value>,
}
impl StorageContextCheck {
    fn check(&mut self, block: &eth::Block) {
        let frames = block
            .transaction_traces
            .iter()
            .flat_map(|tx| tx.calls.iter().map(move |c| (Some(tx), c)))
            .chain(block.system_calls.iter().map(|c| (None, c)));
        for (tx, call) in frames {
            if call.storage_changes.is_empty() {
                continue;
            }
            self.frames += 1;
            bump(&mut self.by_call_type, call_type_name(call.call_type), call.storage_changes.len() as u64);
            let context = storage_context(call);
            for change in &call.storage_changes {
                self.writes += 1;
                if change.address != context {
                    self.violations += 1;
                    if self.examples.len() < MAX_EXAMPLES {
                        self.examples.push(json!({
                            "block": block.number,
                            "transaction": tx.map(|t| hex0x(&t.hash)),
                            "call_index": call.index,
                            "call_type": call_type_name(call.call_type),
                            "caller": hex0x(&call.caller),
                            "address": hex0x(&call.address),
                            "storage_address": hex0x(&change.address),
                            "ordinal": change.ordinal,
                        }));
                    }
                }
            }
        }
    }
}

pub fn run(args: Replay) -> Result<bool> {
    fs::create_dir_all(&args.output)?;
    let params = json!({"chain_id": args.chain_id, "producer_versions": args.producer_versions}).to_string();
    let config = parse(&params).map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let (files, duplicates) = enumerate(&args.blocks)?;
    let mut heights = BTreeSet::new();
    let mut refused: BTreeMap<String, u64> = BTreeMap::new();
    let mut errors: Vec<Value> = Vec::new();
    let mut error_count = 0u64;
    let mut determinism = (0u64, 0u64);
    let mut clocks = (0u64, 0u64);
    let mut totals = Totals::default();
    let mut context = StorageContextCheck::default();
    let mut capabilities: BTreeMap<String, Capabilities> = BTreeMap::new();
    for (&height, path) in &files {
        let block = eth::Block::decode(fs::read(path)?.as_slice()).with_context(|| format!("decode {}", path.display()))?;
        ensure!(block.number == height, "{} holds block {}", path.display(), block.number);
        capabilities.entry(block.ver.to_string()).or_default().observe(&block);
        if !args.producer_versions.contains(&block.ver) {
            bump(&mut refused, block.ver.to_string(), 1);
            continue;
        }
        heights.insert(height);
        context.check(&block);
        match project(&block, &config) {
            Ok(events) => {
                determinism.0 += 1;
                let again = project(&block, &config).map_err(|e| anyhow::anyhow!("{e:?}"))?;
                if again.encode_to_vec() != events.encode_to_vec() {
                    determinism.1 += 1;
                }
                clocks.0 += 1;
                let clock_ok = events.clocks.len() == 1 && events.clocks[0].number == height && events.clocks[0].hash == block.hash;
                clocks.1 += u64::from(!clock_ok);
                totals.add(&events);
            }
            Err(error) => {
                error_count += 1;
                if errors.len() < MAX_EXAMPLES {
                    errors.push(json!({"block": height, "error": format!("{error:?}")}));
                }
            }
        }
    }
    let passed = error_count == 0 && context.violations == 0 && determinism.1 == 0 && clocks.1 == 0;
    let report = json!({
        "tool": "evm-executions-tools replay",
        "mode": "offline_saved_data_only",
        "rpc_in_replay": 0,
        "network": args.network,
        "parameters": serde_json::from_str::<Value>(&params)?,
        "parameters_sha256": config.parameters_sha256,
        "sources": source_hashes()?,
        "inputs": {
            "directories": args.blocks.iter().map(|d| d.display().to_string()).collect::<Vec<_>>(),
            "block_files": files.len(),
            "duplicate_identical_captures": duplicates,
        },
        "status": if passed { "passed" } else { "failed" },
        "blocks": {
            "projected": heights.len(),
            "ranges": ranges(&heights),
            "refused_unqualified_producer_version": refused,
            "projection_errors": error_count,
            "projection_error_examples": errors,
        },
        "determinism": {"checked": determinism.0, "mismatches": determinism.1},
        "clocks": {"checked": clocks.0, "mismatches": clocks.1},
        "storage_context": {
            "rule": "DELEGATE and CALLCODE frames write the storage of Call.caller; every other frame writes the storage of Call.address",
            "frames_with_writes": context.frames,
            "writes_checked": context.writes,
            "writes_by_call_type": context.by_call_type,
            "violations": context.violations,
            "violation_examples": context.examples,
        },
        "rows": {
            "transactions": totals.transactions,
            "calls": totals.calls,
            "calls_persisted": totals.calls_persisted,
            "logs": totals.logs,
            "logs_persisted": totals.logs_persisted,
            "logs_attempted_in_reverted_frames": totals.logs - totals.logs_persisted,
            "receipt_logs": totals.receipt_logs,
            "code_changes_by_scope_kind_persistence": totals.code_changes,
            "set_code_authorizations_applied": totals.set_code_applied,
            "set_code_authorizations_discarded": totals.set_code_discarded,
            "root_created_contracts": totals.created_contracts,
        },
        "capabilities_by_producer_version": capabilities.iter().map(|(v, c)| (v.clone(), c.report())).collect::<Map<String, Value>>(),
    });
    let text = serde_json::to_string_pretty(&report)?;
    fs::write(args.output.join("report.json"), &text)?;
    println!("{text}");
    Ok(passed)
}
