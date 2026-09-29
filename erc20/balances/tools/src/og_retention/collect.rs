//! Local Extended-block decoding only. The saved-data driver verifies the raw
//! block digest against the historical manifest before calling this collector.
use super::{binding, At, BlockInput, Slot};
use crate::calculated_retention::{Code, Write};
use anyhow::{ensure, Context, Result};
use erc20_balances::persist::{self, Ctx, Sink};
use substreams_ethereum::pb::eth::v2 as eth;
/// The caller supplies an independently pinned original manifest record, not a
/// digest computed from the same untrusted block. Header equality alone cannot
/// detect changed trace bytes.
pub fn decode_original(raw: &[u8], number: u64, hash: super::Word, digest: &str) -> Result<BlockInput> {
    use prost::Message;
    ensure!(crate::calculated_retention::binding::sha(raw) == digest, "original PB digest differs");
    let block = eth::Block::decode(raw)?;
    ensure!(block.number == number && block.hash == hash, "original PB identity differs");
    decode(&block, digest)
}
#[derive(Default)]
struct Effects {
    storage: Vec<eth::StorageChange>,
    code: Vec<eth::CodeChange>,
}
impl Sink for Effects {
    fn storage(&mut self, c: &eth::StorageChange, _: Ctx) {
        self.storage.push(c.clone())
    }
    fn storage_noop(&mut self, c: &eth::StorageChange, _: Ctx) {
        self.storage.push(c.clone())
    }
    fn code(&mut self, c: &eth::CodeChange, _: Ctx) {
        self.code.push(c.clone())
    }
    fn balance(&mut self, _: &eth::BalanceChange, _: Ctx) {}
    fn nonce(&mut self, _: &eth::NonceChange, _: Ctx) {}
}
fn word(bytes: &[u8]) -> Result<[u8; 32]> {
    ensure!(bytes.len() <= 32, "oversized storage scalar");
    let mut out = [0; 32];
    out[32 - bytes.len()..].copy_from_slice(bytes);
    Ok(out)
}
pub fn decode(block: &eth::Block, source_sha256: &str) -> Result<BlockInput> {
    ensure!(crate::calculated_retention::binding::is_sha(source_sha256), "missing source digest");
    ensure!(
        block.detail_level == eth::block::DetailLevel::DetaillevelExtended as i32 && matches!(block.ver, 4 | 5),
        "Extended4/5 required"
    );
    let h = block.header.as_ref().context("missing header")?;
    ensure!(
        h.number == block.number && block.hash.len() == 32 && h.parent_hash.len() == 32,
        "header identity mismatch"
    );
    let time = h.timestamp.as_ref().context("missing timestamp")?;
    ensure!(time.seconds >= 0 && time.nanos == 0, "nonintegral EVM timestamp");
    ensure!(
        block.transaction_traces.iter().all(|t| !t.calls.is_empty()),
        "missing transaction persistence calls"
    );
    let mut effects = Effects::default();
    persist::collect_block(block, &mut effects).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let keep = |a: &[u8]| binding::codes().iter().any(|c| c.0.as_slice() == a);
    let mut writes = Vec::new();
    for c in effects.storage.iter().filter(|c| keep(&c.address)) {
        ensure!(c.key.len() == 32, "storage key width");
        writes.push(Write {
            slot: Slot {
                contract: c.address.as_slice().try_into().context("contract width")?,
                key: c.key.as_slice().try_into().unwrap(),
            },
            old: word(&c.old_value)?,
            new: word(&c.new_value)?,
            ordinal: c.ordinal,
        });
    }
    let mut codes = Vec::new();
    for c in effects.code.iter().filter(|c| keep(&c.address)) {
        codes.push(Code {
            contract: c.address.as_slice().try_into().context("code address width")?,
            old: c.old_hash.as_slice().try_into().context("old runtime hash width")?,
            new: c.new_hash.as_slice().try_into().context("new runtime hash width")?,
            ordinal: c.ordinal,
        });
    }
    // Equal ordinals on different keys are independent at end of block. Same-key
    // ties are rejected by the ledger; never invent an ordering for them.
    writes.sort_by_key(|w| w.ordinal);
    codes.sort_by_key(|c| c.ordinal);
    Ok(BlockInput {
        at: At {
            number: block.number,
            hash: block.hash.as_slice().try_into().unwrap(),
            parent_hash: Some(h.parent_hash.as_slice().try_into().unwrap()),
            timestamp: time.seconds as u64,
            producer_version: Some(block.ver as u32),
        },
        source_sha256: source_sha256.into(),
        writes,
        codes,
    })
}
