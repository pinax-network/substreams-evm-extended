//! Portable raw-effects journal. Its digest is independently supplied; values
//! originate from the PB-bound collector, never from comparison observations.
use super::{binding, historical, At, BlockInput, Ledger, Slot};
use crate::calculated_retention::binding::sha;
use anyhow::{ensure, Context, Result};
use serde_json::Value;
pub const BYTES: &[u8] = include_bytes!("../../tests/fixtures/og-retention/journal.jsonl");
pub const SHA256: &str = "9f21ff66c6a5383cc64fcb6b9b76b01ae61943eb16a2e5de11cc963196daceb3";
pub fn verify(raw: &[u8]) -> Result<Vec<BlockInput>> {
    ensure!(raw == BYTES && sha(raw) == SHA256, "fixed original-derived OG journal differs");
    decode(raw, SHA256)
}
pub const START: u64 = 122288108;
pub const STOP: u64 = 122289030;
pub const MAX_BYTES: usize = 64 * 1024 * 1024;
pub fn decode(raw: &[u8], digest: &str) -> Result<Vec<BlockInput>> {
    ensure!(raw.len() <= MAX_BYTES && sha(raw) == digest, "journal size/digest");
    let inputs: Vec<BlockInput> = std::str::from_utf8(raw)?
        .lines()
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    ensure!(inputs.len() == (STOP - START) as usize, "journal count");
    let manifest: Vec<Value> = serde_json::from_slice(binding::artifact_bytes("original-block-manifest.json")?)?;
    let mut at = historical::checkpoints()?.remove(0).at;
    for (input, m) in inputs.iter().zip(manifest.iter().skip((START - 122288006) as usize)) {
        ensure!(
            input.at.number == at.number + 1
                && input.at.parent_hash == Some(at.hash)
                && input.at.timestamp >= at.timestamp
                && input.at.producer_version == Some(5),
            "journal clock"
        );
        ensure!(
            m["block"] == input.at.number && binding::hexword(&m["hash"])? == input.at.hash && m["sha256"] == input.source_sha256,
            "journal original PB linkage"
        );
        at = input.at.clone();
    }
    ensure!(at.number == STOP - 1, "journal terminal height");
    Ok(inputs)
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PoolObservation {
    pub at: At,
    pub words: Vec<([u8; 32], Slot)>,
    pub expected: String,
}
pub fn pool_observations() -> Result<Vec<PoolObservation>> {
    use super::decode::{holder_key, scalar};
    let report = binding::artifact("pool-scan-report.json")?;
    let compact = binding::original("pool-range.json")?;
    let bytes = binding::artifact_bytes("pool-snapshots.jsonl")?;
    ensure!(
        report["snapshots_sha256"] == sha(bytes) && compact["source_snapshots_sha256"] == sha(bytes),
        "pool source links"
    );
    let mut keys: Vec<_> = (0..12).map(|i| holder_key(binding::pool_a(), 9, i)).collect();
    keys.extend((0..4).map(|i| holder_key(binding::pool_a(), 11, i)));
    keys.push(holder_key(binding::pool_a(), 0, 0));
    keys.push(scalar(binding::token(), 29));
    keys.extend(binding::pointers().into_iter().map(|(s, _)| s));
    let source_keys = report["storage_slots"].as_array().context("pool key declarations")?;
    ensure!(source_keys.len() == keys.len(), "pool key count");
    for (s, k) in source_keys.iter().zip(&keys) {
        ensure!(
            super::address(s["contract"].as_str().context("pool key address")?)? == k.contract && binding::hexword(&s["key"])? == k.key,
            "independent pool key derivation"
        );
    }
    let rows: Vec<Value> = std::str::from_utf8(bytes)?
        .lines()
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    let cs = compact["rows"].as_array().context("compact pool rows")?;
    let manifest: Vec<Value> = serde_json::from_slice(binding::artifact_bytes("original-block-manifest.json")?)?;
    ensure!(rows.len() == 1024 && cs.len() == 1024 && manifest.len() == 1024, "full captured pool interval");
    let mut last = None;
    let mut result = vec![];
    for (i, ((r, c), m)) in rows.iter().zip(cs).zip(manifest).enumerate() {
        let number = 122288006 + i as u64;
        let at = At {
            number,
            hash: binding::hexword(&r["hash"])?,
            parent_hash: Some(binding::hexword(&r["parent_hash"])?),
            timestamp: u64::from_str_radix(
                r["timestamp"].as_str().context("pool clock")?.strip_prefix("0x").context("pool clock prefix")?,
                16,
            )?,
            producer_version: None,
        };
        ensure!(
            r["block"] == number
                && c["block"] == number
                && m["block"] == number
                && r["hash"] == c["hash"]
                && r["hash"] == m["hash"]
                && r["parent_hash"] == c["parent_hash"]
                && r["timestamp"] == c["timestamp"]
                && r["rpc_balance"] == c["expected_pool_balance"]
                && r["raw_matches_rpc"] == true,
            "pool capture/compact identity"
        );
        if let Some(hash) = last {
            ensure!(at.parent_hash == Some(hash), "pool capture chain");
        }
        last = Some(at.hash);
        let values = r["storage_values"].as_array().context("pool raw words")?;
        ensure!(values.len() == 21, "pool word count");
        ensure!(
            values[16] == c["raw_pool_balance"]
                && values[..16] == compact["record_and_cursors"].as_array().context("pool record")?[..]
                && values[17..] == compact["epoch_and_dependency_addresses"].as_array().context("pool pointers")?[..],
            "compact raw pool words"
        );
        let expected = r["rpc_balance"].as_str().context("pool expectation")?.to_string();
        ensure!(
            super::value(&binding::hexword(&values[16])?).to_string() == expected,
            "captured terminal raw pool expectation"
        );
        ensure!(
            binding::hexword(&values[4])? == [0; 32] && binding::hexword(&values[5])? == [0; 32],
            "saved terminal rate scope"
        );
        result.push(PoolObservation {
            at,
            words: values
                .iter()
                .zip(&keys)
                .map(|(v, k)| Ok((binding::hexword(v)?, k.clone())))
                .collect::<Result<_>>()?,
            expected,
        });
    }
    Ok(result)
}
pub fn check_pool(ledger: &Ledger, observation: &PoolObservation) -> Result<()> {
    ensure!(
        ledger.at().number == observation.at.number && ledger.at().hash == observation.at.hash && ledger.at().timestamp == observation.at.timestamp,
        "pool observation boundary"
    );
    for (word, slot) in &observation.words {
        ensure!(
            ledger.fact(slot).is_some_and(|f| f.word == *word),
            "retained pool fact differs or missing: {slot:?}"
        );
    }
    Ok(())
}

/// The original five holder checkpoints stay untouched. Only the replay joins
/// independently captured raw pool words at the identical first boundary.
#[derive(Clone, Debug, serde::Serialize)]
pub struct JoinedCheckpoint {
    pub checkpoint: super::Checkpoint,
    pub provenance: Value,
}
pub fn join_initial(original: &super::Checkpoint, pool: &PoolObservation) -> Result<JoinedCheckpoint> {
    let expected = historical::checkpoints()?.remove(0);
    ensure!(*original == expected, "join requires exact original earliest checkpoint");
    let rows = pool_observations()?;
    let captured = rows
        .iter()
        .find(|r| r.at.number == original.at.number)
        .context("missing exact pool join boundary")?;
    ensure!(
        pool == captured && pool.at.number == original.at.number && pool.at.hash == original.at.hash && pool.at.timestamp == original.at.timestamp,
        "join pool raw boundary differs"
    );
    let allowed: Vec<_> = (0..4).map(|i| super::decode::holder_key(binding::pool_a(), 11, i)).collect();
    let mut overlaps = vec![];
    let mut additions = vec![];
    for (value, slot) in &pool.words {
        if let Some(f) = original.facts.iter().find(|f| f.slot == *slot) {
            ensure!(f.word == *value, "join overlapping raw fact differs");
            overlaps.push(slot.clone());
        } else {
            ensure!(allowed.contains(slot), "join unexpected missing raw fact");
            additions.push((slot.clone(), *value));
        }
    }
    ensure!(
        overlaps.len() == 17 && additions.len() == 4 && allowed.iter().all(|s| additions.iter().any(|(a, _)| a == s)),
        "join exact17overlap/fourcursors"
    );
    let provenance = serde_json::json!({"scope":"same-boundary raw join, never getter or later-state seeding","original_checkpoint_sha256":sha(&serde_json::to_vec(original)?),"original_artifact":{"path":"tests/fixtures/og-model/historical.json","sha256":binding::ORIGINALS[0].sha256,"slots":original.facts.iter().map(|f|&f.slot).collect::<Vec<_>>()},"pool_artifact":{"path":"tools/tests/fixtures/og-retention/pool-snapshots.jsonl","sha256":sha(binding::artifact_bytes("pool-snapshots.jsonl")?),"at":pool.at,"overlapping_slots":overlaps,"new_raw_facts":additions}});
    let evidence = format!(
        "same-boundary raw checkpoint join: historical={} pool={} provenance={}",
        binding::ORIGINALS[0].sha256,
        sha(binding::artifact_bytes("pool-snapshots.jsonl")?),
        sha(&serde_json::to_vec(&provenance)?)
    );
    let mut checkpoint = original.clone();
    checkpoint.evidence = evidence.clone();
    for f in &mut checkpoint.facts {
        f.origin = super::Origin::Checkpoint { evidence: evidence.clone() };
    }
    for (slot, word) in additions {
        checkpoint.universe.push(slot.clone());
        checkpoint.facts.push(super::Fact {
            slot,
            word,
            at: checkpoint.at.clone(),
            epoch: checkpoint.binding.epoch,
            origin: super::Origin::Checkpoint { evidence: evidence.clone() },
        });
    }
    checkpoint.universe.sort();
    checkpoint.facts.sort_by(|a, b| a.slot.cmp(&b.slot));
    ensure!(checkpoint.facts.len() == 157 && checkpoint.universe.len() == 157, "joined raw count");
    super::Ledger::from_checkpoint(checkpoint.clone(), super::Limits::default())?;
    Ok(JoinedCheckpoint { checkpoint, provenance })
}
