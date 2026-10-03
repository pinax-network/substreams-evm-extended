//! Replays an RPC-free package on Extended blocks the way the Substreams
//! engine chains its modules: the packaged native and ERC-20 WASM maps produce
//! the `db_out` inputs, then the reference and candidate `db_out` run on them.
//!
//! Each map output is also compared with the host build of the same crate and,
//! when given, with the engine's recorded ERC-20 output for the same blocks.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Result};
use erc20_balances::layout::VerifiedLayout;
use prost::Message;
use proto::pb::evm::balances::v1 as pb;
use serde::Serialize;
use sha2::{Digest, Sha256};
use substreams_database_change::pb::database::DatabaseChanges;
use substreams_ethereum::pb::eth::v2 as eth;

use crate::{
    compare::{ERC20_MAP, NATIVE_MAP, RPC_IMPORT},
    corpus::{self, clock_of},
    spkg::Package,
    wasm::{import_modules, MapModule, Outcome},
};

struct Map {
    module: MapModule,
    params: Vec<u8>,
}

impl Map {
    fn load(package: &Package, name: &str) -> Result<Self> {
        let module = package.module(name)?;
        let binary = package.binary(module)?;
        if import_modules(binary)?.contains(RPC_IMPORT) {
            bail!("{name} imports RPC host functions; pipeline replay needs an RPC-free package");
        }
        Ok(Map {
            module: MapModule::new(binary, &module.binary_entrypoint)?,
            params: module.params().unwrap_or_default().as_bytes().to_vec(),
        })
    }

    /// The map's encoded output, or the panic message of a refused block.
    fn run(&self, block: &[u8]) -> Result<Result<Vec<u8>, String>> {
        let outcome: Outcome = self.module.call(&[&self.params, block])?;
        Ok(match (outcome.panic, outcome.trapped) {
            (Some(panic), _) => Err(panic.message),
            (None, true) => Err("trapped without a registered panic".into()),
            (None, false) => Ok(outcome.output.unwrap_or_default()),
        })
    }
}

#[derive(Debug, Default, Serialize)]
pub struct Replay {
    pub blocks: usize,
    pub first_block: Option<u64>,
    pub last_block: Option<u64>,
    /// First block a packaged map refused; the engine stops the stream there.
    pub first_refusal: Option<(u64, String, String)>,
    pub refused_blocks: usize,
    pub native_rows: usize,
    pub erc20_rows: usize,
    pub erc20_tokens: usize,
    /// Packaged map outputs that differ from the host build of the same crate.
    pub native_host_mismatches: Vec<u64>,
    pub erc20_host_mismatches: Vec<u64>,
    /// Blocks whose packaged ERC-20 output differs from the engine recording.
    pub erc20_engine_mismatches: Vec<u64>,
    pub erc20_engine_blocks_compared: usize,
    pub db_out_identical: usize,
    pub db_out_differing: Vec<u64>,
    pub table_changes: BTreeMap<String, usize>,
    /// SHA-256 over the length-prefixed candidate `db_out` outputs, in block order.
    pub db_out_outputs_sha256: String,
}

/// What one block contributed; merged in block order.
enum BlockOutcome {
    Refused {
        number: u64,
        map: &'static str,
        reason: String,
    },
    Replayed {
        number: u64,
        native_rows: usize,
        erc20_rows: usize,
        tokens: Vec<Vec<u8>>,
        native_host_match: bool,
        erc20_host_match: bool,
        engine_match: Option<bool>,
        /// The candidate's output when both `db_out`s agree.
        db_out: Option<Vec<u8>>,
    },
}

/// One thread's instances of the candidate's maps and both `db_out`s.
struct Replayer<'a> {
    native: Map,
    erc20: Map,
    reference_db: MapModule,
    candidate_db: MapModule,
    layouts: &'a [VerifiedLayout],
    engine: Option<&'a BTreeMap<u64, pb::Events>>,
}

impl Replayer<'_> {
    fn block(&self, path: &Path) -> Result<BlockOutcome> {
        let bytes = std::fs::read(path)?;
        let block = eth::Block::decode(bytes.as_slice())?;
        let number = block.number;
        let (native_out, erc20_out) = match (self.native.run(&bytes)?, self.erc20.run(&bytes)?) {
            (Ok(n), Ok(e)) => (n, e),
            (Err(reason), _) => return Ok(BlockOutcome::Refused { number, map: "native", reason }),
            (_, Err(reason)) => return Ok(BlockOutcome::Refused { number, map: "erc20", reason }),
        };
        let erc20_events = pb::Events::decode(erc20_out.as_slice())?;
        let clock = clock_of(&block)?.encode_to_vec();
        let inputs: [&[u8]; 4] = [b"hex", &clock, &native_out, &erc20_out];
        let (left, right) = (self.reference_db.call(&inputs)?, self.candidate_db.call(&inputs)?);
        Ok(BlockOutcome::Replayed {
            number,
            native_rows: pb::Events::decode(native_out.as_slice())?.balances.len(),
            erc20_rows: erc20_events.balances.len(),
            tokens: erc20_events.balances.iter().filter_map(|b| b.contract.clone()).collect(),
            native_host_match: corpus::native_events(&block).ok().map(|e| e.encode_to_vec()) == Some(native_out),
            erc20_host_match: corpus::erc20_events(&block, self.layouts).ok().map(|e| e.encode_to_vec()) == Some(erc20_out),
            engine_match: self.engine.map(|engine| engine.get(&number).cloned().unwrap_or_default() == erc20_events),
            db_out: left.same_as(&right).then(|| right.output.unwrap_or_default()),
        })
    }
}

/// Replays `blocks` (Extended block files, in order) through `candidate`'s
/// maps and both `db_out`s, spread over the available cores.
pub fn replay(reference: &Package, candidate: &Package, blocks: &[PathBuf], layouts: &[VerifiedLayout], engine_erc20: Option<&Path>) -> Result<Replay> {
    let engine: Option<BTreeMap<u64, pb::Events>> = engine_erc20
        .map(|path| Ok::<_, anyhow::Error>(corpus::read_jsonl_events(path)?.into_iter().collect()))
        .transpose()?;
    let replayer = || -> Result<Replayer<'_>> {
        Ok(Replayer {
            native: Map::load(candidate, NATIVE_MAP)?,
            erc20: Map::load(candidate, ERC20_MAP)?,
            reference_db: crate::compare::db_out(reference)?,
            candidate_db: crate::compare::db_out(candidate)?,
            layouts,
            engine: engine.as_ref(),
        })
    };
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = blocks.len().div_ceil(threads).max(1);
    let outcomes: Vec<BlockOutcome> = std::thread::scope(|scope| {
        let handles: Vec<_> = blocks
            .chunks(chunk)
            .map(|part| {
                scope.spawn(|| {
                    let replayer = replayer()?;
                    part.iter().map(|path| replayer.block(path)).collect::<Result<Vec<_>>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().map_err(|_| anyhow!("replay thread panicked"))?)
            .collect::<Result<Vec<_>>>()
    })?
    .into_iter()
    .flatten()
    .collect();

    let mut report = Replay::default();
    let mut tokens = std::collections::BTreeSet::new();
    let mut digest = Sha256::new();
    for outcome in outcomes {
        report.blocks += 1;
        match outcome {
            BlockOutcome::Refused { number, map, reason } => {
                report.first_block.get_or_insert(number);
                report.last_block = Some(number);
                report.refused_blocks += 1;
                report.first_refusal.get_or_insert((number, map.into(), reason));
            }
            BlockOutcome::Replayed {
                number,
                native_rows,
                erc20_rows,
                tokens: block_tokens,
                native_host_match,
                erc20_host_match,
                engine_match,
                db_out,
            } => {
                report.first_block.get_or_insert(number);
                report.last_block = Some(number);
                report.native_rows += native_rows;
                report.erc20_rows += erc20_rows;
                tokens.extend(block_tokens);
                if !native_host_match {
                    report.native_host_mismatches.push(number);
                }
                if !erc20_host_match {
                    report.erc20_host_mismatches.push(number);
                }
                if let Some(matched) = engine_match {
                    report.erc20_engine_blocks_compared += 1;
                    if !matched {
                        report.erc20_engine_mismatches.push(number);
                    }
                }
                let Some(output) = db_out else {
                    report.db_out_differing.push(number);
                    continue;
                };
                report.db_out_identical += 1;
                digest.update((output.len() as u64).to_be_bytes());
                digest.update(&output);
                for change in DatabaseChanges::decode(output.as_slice())?.table_changes {
                    *report.table_changes.entry(change.table).or_default() += 1;
                }
            }
        }
    }
    report.erc20_tokens = tokens.len();
    report.db_out_outputs_sha256 = hex::encode(digest.finalize());
    Ok(report)
}

impl Replay {
    pub fn passed(&self) -> bool {
        self.blocks > 0
            && self.first_refusal.is_none()
            && self.native_host_mismatches.is_empty()
            && self.erc20_host_mismatches.is_empty()
            && self.erc20_engine_mismatches.is_empty()
            && self.db_out_differing.is_empty()
    }
}
