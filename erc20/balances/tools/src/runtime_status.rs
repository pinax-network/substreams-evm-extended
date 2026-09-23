//! Select, for a later live range, the caller-qualified profiles whose token
//! runtime and dependency bindings still match. A profile that no longer
//! matches is excluded with its reason, never changed or loosened: this
//! partitions an existing layout file, it neither qualifies nor promotes one.
use crate::{cli::record_run, data::sha256, rpc::*};
use anyhow::{ensure, Context, Result};
use clap::Args;
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Args)]
pub struct RuntimeStatus {
    /// JSON array of caller-qualified token layouts; no built-in token list.
    #[arg(long)]
    pub layouts: PathBuf,
    #[arg(long)]
    pub start: u64,
    #[arg(long, default_value_t = 64)]
    pub blocks: u64,
    #[arg(long)]
    pub output: PathBuf,
}

pub struct Partition {
    /// The input entries that still match, unchanged and in input order.
    pub kept: Vec<Value>,
    pub excluded: Vec<Value>,
}

/// Records whether any request failed in transport, so that a network error
/// aborts the run instead of excluding a profile.
struct Watched<'a> {
    rpc: &'a dyn Rpc,
    failed: AtomicBool,
}
impl Rpc for Watched<'_> {
    fn request(&self, payload: Value) -> Result<Value> {
        self.rpc.request(payload).inspect_err(|_| self.failed.store(true, Ordering::SeqCst))
    }
}

/// `None` when the profile still matches, the mismatch otherwise.
fn check(rpc: &dyn Rpc, start: u64, stop: u64, layout: &erc20_balances::layout::VerifiedLayout) -> Result<Option<String>> {
    let watched = Watched {
        rpc,
        failed: AtomicBool::new(false),
    };
    match qualify_layout_runtime(&watched, start, stop, layout) {
        Ok(()) => Ok(None),
        Err(error) => {
            ensure!(
                !watched.failed.load(Ordering::SeqCst),
                "RPC transport failed while checking 0x{}: {error:#}",
                hex::encode(&layout.contract)
            );
            Ok(Some(format!("{error:#}")))
        }
    }
}

/// A profile is excluded only when the same mismatch repeats on a second check.
pub fn partition(rpc: &dyn Rpc, text: &str, start: u64, stop: u64) -> Result<Partition> {
    let entries: Vec<Value> = serde_json::from_str(text)?;
    let layouts = erc20_balances::layout::parse(text)?;
    ensure!(entries.len() == layouts.len(), "layout entries differ from parsed profiles");
    let mut partition = Partition {
        kept: Vec::new(),
        excluded: Vec::new(),
    };
    for (entry, layout) in entries.into_iter().zip(&layouts) {
        let contract = format!("0x{}", hex::encode(&layout.contract));
        let mismatch = match check(rpc, start, stop, layout)? {
            None => None,
            Some(reason) => {
                let again = check(rpc, start, stop, layout)?;
                ensure!(again.is_none() || again.as_ref() == Some(&reason), "inconsistent runtime result for {contract}");
                again
            }
        };
        match mismatch {
            None => partition.kept.push(entry),
            Some(reason) => partition.excluded.push(json!({"contract":contract,"reason":reason})),
        }
    }
    Ok(partition)
}

pub fn run(args: RuntimeStatus) -> Result<bool> {
    ensure!(args.start > 0 && args.blocks > 0, "choose a positive start and block count");
    let stop = args.start.checked_add(args.blocks).context("range overflow")?;
    record_run(
        &args.output,
        json!({"status":"incomplete","start":args.start,"blocks":args.blocks,
        "scope":"Runtime and dependency bindings before the range and at its last block; profiles are neither loosened nor promoted"}),
        |report| {
            let text = fs::read_to_string(&args.layouts)?;
            report["layouts_sha256"] = json!(sha256(&args.layouts)?);
            let rpc = HttpRpc::from_env();
            ensure_finalized(&rpc, stop)?;
            let partition = partition(&rpc, &text, args.start, stop)?;
            ensure!(!partition.kept.is_empty(), "no configured profile still matches");
            let kept = args.output.join("layouts.json");
            fs::write(&kept, serde_json::to_string(&partition.kept)?)?;
            report["configured_tokens"] = json!(partition.kept.len() + partition.excluded.len());
            report["kept_tokens"] = json!(partition.kept.len());
            report["kept_layouts_sha256"] = json!(sha256(&kept)?);
            report["checked_blocks"] = json!([args.start - 1, stop - 1]);
            report["excluded"] = json!(partition.excluded);
            report["status"] = json!("partitioned");
            Ok(())
        },
    )
}
