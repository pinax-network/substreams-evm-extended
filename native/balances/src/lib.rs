//! One RPC-free `map_events`: the final persisted native account balance of
//! every account changed in an Extended block.
//!
//! Output reuses the shared `evm.balances.v1.Events` protobuf. `Balance.contract`
//! is absent (`None`) for native amounts and `Balance.amount` is the account's
//! balance after the last persisted change in the block, as an exact decimal
//! `uint256` string. Accounts without a persisted change in the block are not
//! emitted; absence is no observation, never zero.
//!
//! Balances are account state, reduced from the producer's persisted
//! `BalanceChange` records in execution order. They are not a sum of inferred
//! transfers: CALLCODE/DELEGATECALL value, value-bearing calls and transfer logs
//! are not consulted. Which records persist is decided by the shared
//! [`evm_persist`] rules (failed-transaction gas effects, reverted frames,
//! system calls, block-level records).
use evm_persist as persist;
use proto::pb::evm::balances::v1 as balances_pb;
use serde::Deserialize;
use std::collections::BTreeMap;
use substreams::{errors::Error, scalar::BigInt};
use substreams_ethereum::pb::eth::v2 as eth;

fn require(ok: bool, message: &str) -> Result<(), Error> {
    if ok {
        Ok(())
    } else {
        Err(Error::msg(message.to_string()))
    }
}

/// Explicit producer qualification. Nothing about the network or producer is
/// inferred from a block: `Block.ver` must be listed, and the manifest binds the
/// network. Other producer versions and networks need their own fixtures.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// Extended producer versions (`Block.ver`) replayed from saved fixtures.
    pub producer_versions: Vec<i32>,
}

pub fn parse_params(params: &str) -> Result<Params, Error> {
    let parsed: Params = serde_json::from_str(params).map_err(|e| Error::msg(format!("invalid native balance params: {e}")))?;
    require(!parsed.producer_versions.is_empty(), "no qualified Extended producer version configured")?;
    require(parsed.producer_versions.iter().all(|v| *v > 0), "invalid Extended producer version")?;
    Ok(parsed)
}

/// One account's persisted native balance movement within a block: the value
/// before its first persisted change and after its last one. Intermediate
/// values are reduced away; a net-zero account that changed is still reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub address: Vec<u8>,
    /// Balance before the first persisted change in this block.
    pub old_amount: String,
    /// Balance after the last persisted change in this block.
    pub amount: String,
    pub first_ordinal: u64,
    pub ordinal: u64,
    /// Number of persisted change records reduced into this row.
    pub records: u32,
}

/// A persisted native balance record with its persistence provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub address: Vec<u8>,
    pub old_value: Option<Vec<u8>>,
    pub new_value: Option<Vec<u8>>,
    pub ordinal: u64,
    pub reason: i32,
    pub scope: persist::Scope,
}

#[derive(Default)]
struct Collected {
    records: Vec<Record>,
}
impl persist::Sink for Collected {
    fn storage(&mut self, _: &eth::StorageChange, _: persist::Ctx) {}
    fn balance(&mut self, c: &eth::BalanceChange, ctx: persist::Ctx) {
        self.records.push(Record {
            address: c.address.clone(),
            old_value: c.old_value.as_ref().map(|v| v.bytes.clone()),
            new_value: c.new_value.as_ref().map(|v| v.bytes.clone()),
            ordinal: c.ordinal,
            reason: c.reason,
            scope: ctx.scope,
        });
    }
    fn nonce(&mut self, _: &eth::NonceChange, _: persist::Ctx) {}
    fn code(&mut self, _: &eth::CodeChange, _: persist::Ctx) {}
}

fn word(bytes: &[u8]) -> Result<[u8; 32], Error> {
    require(bytes.len() <= 32, "native balance exceeds uint256")?;
    let mut out = [0; 32];
    out[32 - bytes.len()..].copy_from_slice(bytes);
    Ok(out)
}
/// An absent `BigInt` message inside an observed `BalanceChange` encodes zero
/// under the pinned Ethereum protobuf contract. This applies only to a record
/// the producer emitted; it never initializes an untouched account.
fn value(bytes: Option<&[u8]>) -> Result<[u8; 32], Error> {
    word(bytes.unwrap_or(&[]))
}
fn amount(word: &[u8; 32]) -> String {
    BigInt::from_unsigned_bytes_be(word).to_string()
}

pub fn validate_block(block: &eth::Block, params: &Params) -> Result<(), Error> {
    require(
        block.detail_level == eth::block::DetailLevel::DetaillevelExtended as i32,
        "Extended blocks required",
    )?;
    require(
        params.producer_versions.contains(&block.ver),
        "Extended producer version not qualified for native balances",
    )?;
    let header = block.header.as_ref().ok_or_else(|| Error::msg("missing header"))?;
    require(
        block.hash.len() == 32 && header.parent_hash.len() == 32 && header.state_root.len() == 32,
        "invalid block identity",
    )?;
    require(header.number == block.number, "header number mismatch")?;
    // Genesis allocations are not a persisted execution record in a traced
    // block; supporting them needs a documented producer fixture first.
    require(block.number > 0, "genesis block not qualified for native balances")?;
    for tx in &block.transaction_traces {
        require((1..=3).contains(&tx.status) && !tx.calls.is_empty(), "incomplete transaction persistence data")?;
    }
    Ok(())
}

/// Balance-change reasons whose behavior inside a failed or reverted
/// transaction is pinned by the shared persistence rules: the three gas
/// reasons persist, the others revert with the execution frame. Any other
/// reason in a failed transaction's root call (for example BNB blob-fee
/// rewards, OP-stack deposit mints, or a reason this crate cannot name) has no
/// fixture, so the block fails instead of silently dropping a possible credit.
const REVERTIBLE_FAILED_REASONS: [eth::balance_change::Reason; 6] = [
    eth::balance_change::Reason::Transfer,
    eth::balance_change::Reason::TouchAccount,
    eth::balance_change::Reason::SuicideRefund,
    eth::balance_change::Reason::CallBalanceOverride,
    eth::balance_change::Reason::SuicideWithdraw,
    eth::balance_change::Reason::Burn,
];
fn validate_failed_transaction_reasons(block: &eth::Block) -> Result<(), Error> {
    for tx in block.transaction_traces.iter().filter(|tx| persist::is_failed(tx)) {
        let Some(root) = tx.calls.first() else { continue };
        for change in &root.balance_changes {
            let pinned =
                persist::is_gas_reason(change) || eth::balance_change::Reason::try_from(change.reason).is_ok_and(|r| REVERTIBLE_FAILED_REASONS.contains(&r));
            require(
                pinned,
                "failed transaction carries a balance-change reason without pinned persistence semantics",
            )?;
        }
    }
    Ok(())
}

/// Every persisted native balance record in the block, in array order, with
/// its persistence scope. Used by the host replay tool for the reason matrix.
pub fn records(block: &eth::Block, params: &Params) -> Result<Vec<Record>, Error> {
    validate_block(block, params)?;
    validate_failed_transaction_reasons(block)?;
    let mut collected = Collected::default();
    persist::collect_block(block, &mut collected)?;
    Ok(collected.records)
}

/// Reduce persisted records by execution ordinal into one row per account.
/// Every record needs a canonical 20-byte account, a positive ordinal, values
/// that fit `uint256`, a strictly increasing ordinal per account and old/new
/// continuity. Ambiguous or discontinuous producer data fails the block.
pub fn changes(block: &eth::Block, params: &Params) -> Result<Vec<Change>, Error> {
    let mut records = records(block, params)?;
    // Array order is not execution order: block-level records follow the
    // transactions in the message but can precede or follow them by ordinal.
    records.sort_by_key(|r| r.ordinal);
    let mut rows: BTreeMap<Vec<u8>, (Change, [u8; 32])> = BTreeMap::new();
    for record in records {
        require(record.address.len() == 20, "invalid native account address")?;
        require(record.ordinal > 0, "persisted native balance change has no execution ordinal")?;
        let old = value(record.old_value.as_deref())?;
        let new = value(record.new_value.as_deref())?;
        match rows.get_mut(&record.address) {
            Some((row, last)) => {
                require(record.ordinal > row.ordinal, "ambiguous native balance execution order")?;
                require(old == *last, "discontinuous native balance changes within block")?;
                row.amount = amount(&new);
                row.ordinal = record.ordinal;
                row.records += 1;
                *last = new;
            }
            None => {
                rows.insert(
                    record.address.clone(),
                    (
                        Change {
                            address: record.address,
                            old_amount: amount(&old),
                            amount: amount(&new),
                            first_ordinal: record.ordinal,
                            ordinal: record.ordinal,
                            records: 1,
                        },
                        new,
                    ),
                );
            }
        }
    }
    Ok(rows.into_values().map(|(row, _)| row).collect())
}

/// Final balances of every changed account, sorted by account, with an absent
/// contract. Includes the zero address, precompiles, system and burn-looking
/// addresses whenever the producer persisted a change for them.
pub fn project(block: &eth::Block, params: &Params) -> Result<balances_pb::Events, Error> {
    Ok(balances_pb::Events {
        balances: changes(block, params)?
            .into_iter()
            .map(|c| balances_pb::Balance {
                contract: None,
                address: c.address,
                amount: c.amount,
            })
            .collect(),
    })
}

// The SDK macro generates raw-pointer parameter decoding and discards function
// attributes. Keep its ABI-specific lint exception scoped to this wrapper. The
// exported `map_events` symbol exists only in the WASM build so host test
// binaries can link this crate next to `erc20-balances`, which exports the
// same C symbol name.
#[cfg(target_arch = "wasm32")]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
mod handler {
    use super::*;
    #[substreams::handlers::map]
    fn map_events(params: String, block: eth::Block) -> Result<balances_pb::Events, Error> {
        project(&block, &parse_params(&params)?)
    }
}

#[cfg(test)]
mod tests;
