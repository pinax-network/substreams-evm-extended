//! Replay caller-qualified layouts natively over complete consecutive captured
//! Extended blocks and name every profile the mapper refuses. The package halts
//! the whole stream on its first refusal, so one unreviewed write in one token
//! stops every other token. A refused profile is recorded with its first
//! refusing block and reason and left out of the rest of the replay; it is
//! never loosened. The kept file lists the profiles that a package run over the
//! same blocks processes without halting.
use crate::cli::record_run;
use crate::data::sha256;
use anyhow::{anyhow, ensure, Context, Result};
use clap::Args;
use erc20_balances::layout::{self, VerifiedLayout};
use prost::Message;
use serde_json::{json, Value};
use std::{fs, path::PathBuf};
use substreams_ethereum::pb::eth::v2 as eth;

#[derive(Args)]
pub struct RefusalScan {
    /// JSON array of caller-qualified token layouts; no built-in token list.
    #[arg(long)]
    pub layouts: PathBuf,
    /// Complete consecutive Extended blocks, one `<number>.pb` file each.
    #[arg(long)]
    pub block_dir: PathBuf,
    #[arg(long)]
    pub output: PathBuf,
}

pub struct Scan {
    /// The input entries never refused, unchanged and in input order.
    pub kept: Vec<Value>,
    pub refused: Vec<Value>,
    pub blocks: u64,
    pub emitted_rows: u64,
}

/// Replays `blocks` in order. A refusal that remains with no profile
/// configured belongs to the block, not a token, and stops the scan.
pub fn scan(blocks: impl IntoIterator<Item = Result<eth::Block>>, text: &str) -> Result<Scan> {
    let mut entries: Vec<Value> = serde_json::from_str(text)?;
    let mut layouts: Vec<VerifiedLayout> = layout::parse(text)?;
    ensure!(entries.len() == layouts.len(), "layout entries differ from parsed profiles");
    let mut result = Scan {
        kept: Vec::new(),
        refused: Vec::new(),
        blocks: 0,
        emitted_rows: 0,
    };
    let mut previous: Option<(u64, Vec<u8>)> = None;
    for block in blocks {
        let block = block?;
        let parent = &block.header.as_ref().context("missing header")?.parent_hash;
        if let Some((number, hash)) = &previous {
            ensure!(
                block.number == number + 1 && parent == hash,
                "captured blocks have a gap or fork at {}",
                block.number
            );
        }
        ensure!(
            block.detail_level == eth::block::DetailLevel::DetaillevelExtended as i32,
            "Extended block required at {}",
            block.number
        );
        let events = match erc20_balances::project(&block, &layouts) {
            Ok(events) => events,
            Err(combined) => {
                erc20_balances::project(&block, &[]).map_err(|error| anyhow!("block {} is refused without any profile: {error}", block.number))?;
                let failing = (0..layouts.len())
                    .filter_map(|i| {
                        erc20_balances::project(&block, std::slice::from_ref(&layouts[i]))
                            .err()
                            .map(|e| (i, e.to_string()))
                    })
                    .collect::<Vec<_>>();
                ensure!(
                    !failing.is_empty(),
                    "block {} is refused ({combined}) but no profile is refused alone",
                    block.number
                );
                for (i, reason) in failing.iter().rev() {
                    let contract = format!("0x{}", hex::encode(&layouts[*i].contract));
                    let mut refusal = json!({"contract":contract,"block":block.number,"reason":reason});
                    if let Some(key) = reason.split(" at key ").nth(1).and_then(|tail| tail.split(';').next()) {
                        refusal["slot"] = slot_origin(&block, &layouts[*i].contract, key);
                    }
                    result.refused.push(refusal);
                    entries.remove(*i);
                    layouts.remove(*i);
                }
                erc20_balances::project(&block, &layouts)
                    .map_err(|error| anyhow!("block {} is still refused without its refused profiles: {error}", block.number))?
            }
        };
        result.emitted_rows += events.balances.len() as u64;
        result.blocks += 1;
        previous = Some((block.number, block.hash.clone()));
    }
    ensure!(result.blocks > 0, "no captured blocks");
    result.refused.sort_by_key(|r| (r["block"].as_u64(), r["contract"].as_str().map(str::to_owned)));
    result.kept = entries;
    Ok(result)
}

fn bare(hex: &str) -> String {
    hex.trim_start_matches("0x").to_ascii_lowercase()
}

/// Where a refused key comes from: the writes to it, and the chain of mapping
/// bases recovered from the block's Keccak preimages, ending at the first slot
/// without one (a declared slot or a compile-time constant such as ERC-7201).
pub fn slot_origin(block: &eth::Block, contract: &[u8], key: &str) -> Value {
    let calls = block.transaction_traces.iter().flat_map(|t| t.calls.iter().map(move |c| (t, c)));
    let mut preimages = std::collections::BTreeMap::new();
    let mut writes = Vec::new();
    for (trace, call) in calls {
        for (hash, preimage) in &call.keccak_preimages {
            preimages.insert(bare(hash), bare(preimage));
        }
        for change in call
            .storage_changes
            .iter()
            .filter(|c| c.address == contract && bare(&hex::encode(&c.key)) == bare(key))
        {
            writes.push(json!({"transaction":format!("0x{}", hex::encode(&trace.hash)),"call_index":call.index,
                "old":format!("0x{}", hex::encode(&change.old_value)),"new":format!("0x{}", hex::encode(&change.new_value))}));
        }
    }
    let (mut chain, mut current) = (Vec::new(), bare(key));
    while let Some(preimage) = preimages.get(&current) {
        if preimage.len() != 128 || chain.len() == 8 {
            chain.push(json!({"hash":format!("0x{current}"),"preimage_bytes":preimage.len() / 2}));
            break;
        }
        chain.push(json!({"mapping_key":format!("0x{}", &preimage[..64]),"base":format!("0x{}", &preimage[64..])}));
        current = preimage[64..].to_string();
    }
    json!({"key":format!("0x{}", bare(key)),"writes":writes,"mapping_chain":chain,"root":format!("0x{current}")})
}

/// `<number>.pb` files in block order; each file's block must carry its name.
pub fn block_files(dir: &PathBuf) -> Result<Vec<(u64, PathBuf)>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_none_or(|e| e != "pb") {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).context("invalid block file name")?;
        files.push((
            stem.parse::<u64>().with_context(|| format!("block file {stem}.pb is not named by number"))?,
            path,
        ));
    }
    files.sort();
    Ok(files)
}

pub fn run(args: RefusalScan) -> Result<bool> {
    record_run(
        &args.output,
        json!({"status":"incomplete","scope":"Native mapper replay of captured Extended blocks; a refused profile is excluded from the rest of the replay, never loosened"}),
        |report| {
            let text = fs::read_to_string(&args.layouts)?;
            report["layouts_sha256"] = json!(sha256(&args.layouts)?);
            let files = block_files(&args.block_dir)?;
            let (first, last) = (files.first().context("no captured blocks")?.0, files.last().unwrap().0);
            let mut hashes = (String::new(), String::new());
            let blocks = files.iter().map(|(number, path)| {
                let block = eth::Block::decode(fs::read(path)?.as_slice())?;
                ensure!(block.number == *number, "{} holds block {}", path.display(), block.number);
                let hash = format!("0x{}", hex::encode(&block.hash));
                if *number == first {
                    hashes.0 = hash.clone();
                }
                hashes.1 = hash;
                Ok(block)
            });
            let scan = scan(blocks, &text)?;
            let kept = args.output.join("layouts.json");
            fs::write(&kept, serde_json::to_string(&scan.kept)?)?;
            report["start"] = json!(first);
            report["stop_exclusive"] = json!(last + 1);
            report["blocks"] = json!(scan.blocks);
            report["first_hash"] = json!(hashes.0);
            report["last_hash"] = json!(hashes.1);
            report["configured_tokens"] = json!(scan.kept.len() + scan.refused.len());
            report["kept_tokens"] = json!(scan.kept.len());
            report["kept_layouts_sha256"] = json!(sha256(&kept)?);
            report["native_emitted_rows"] = json!(scan.emitted_rows);
            report["refused"] = json!(scan.refused);
            report["status"] = json!("scanned");
            Ok(())
        },
    )
}
