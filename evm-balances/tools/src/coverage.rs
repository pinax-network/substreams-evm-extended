//! Share of an RPC reference stream's ERC-20 activity that the configured
//! tokens cover. The RPC package emits every token with a Transfer; the
//! Extended map emits only configured ones.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};
use proto::pb::evm::balances::v1 as pb;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Coverage {
    /// Recorded blocks with output; the recording omits empty blocks.
    pub blocks_with_output: usize,
    pub first_block: Option<u64>,
    pub last_block: Option<u64>,
    pub configured_profiles: usize,
    pub rows: usize,
    pub configured_rows: usize,
    pub rows_percent: f64,
    pub tokens: usize,
    pub configured_tokens: usize,
    pub tokens_percent: f64,
    pub holder_pairs: usize,
    pub configured_holder_pairs: usize,
    pub holder_pairs_percent: f64,
}

/// Contract addresses of a layout file's profiles.
pub fn configured_contracts(layouts: &serde_json::Value) -> Result<BTreeSet<Vec<u8>>> {
    layouts
        .as_array()
        .ok_or_else(|| anyhow!("layouts are not an array"))?
        .iter()
        .map(|profile| {
            let contract = profile["contract"].as_str().ok_or_else(|| anyhow!("profile without contract"))?;
            Ok(hex::decode(contract.trim_start_matches("0x"))?)
        })
        .collect()
}

pub fn measure(reference: &[(u64, pb::Events)], configured: &BTreeSet<Vec<u8>>) -> Coverage {
    let mut holders: BTreeMap<Vec<u8>, BTreeSet<Vec<u8>>> = BTreeMap::new();
    let (mut rows, mut configured_rows) = (0, 0);
    for (_, events) in reference {
        for balance in &events.balances {
            let contract = balance.contract.clone().unwrap_or_default();
            rows += 1;
            configured_rows += usize::from(configured.contains(&contract));
            holders.entry(contract).or_default().insert(balance.address.clone());
        }
    }
    let covered = |c: &&Vec<u8>| configured.contains(*c);
    let configured_tokens = holders.keys().filter(covered).count();
    let holder_pairs = holders.values().map(BTreeSet::len).sum();
    let configured_holder_pairs = holders.iter().filter(|(c, _)| configured.contains(*c)).map(|(_, h)| h.len()).sum();
    let percent = |part: usize, whole: usize| {
        if whole == 0 {
            0.0
        } else {
            (1000.0 * part as f64 / whole as f64).round() / 10.0
        }
    };
    Coverage {
        blocks_with_output: reference.len(),
        first_block: reference.first().map(|(b, _)| *b),
        last_block: reference.last().map(|(b, _)| *b),
        configured_profiles: configured.len(),
        rows,
        configured_rows,
        rows_percent: percent(configured_rows, rows),
        tokens: holders.len(),
        configured_tokens,
        tokens_percent: percent(configured_tokens, holders.len()),
        holder_pairs,
        configured_holder_pairs,
        holder_pairs_percent: percent(configured_holder_pairs, holder_pairs),
    }
}
