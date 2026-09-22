//! One RPC-free `map_events` for explicitly qualified Aave V3 aToken epochs.
//!
//! Output is `evm.balance_state.v1.Events` (see `docs/balance-state-contract.md`):
//! * `HolderBasis` rows for every persisted write to an aToken's
//!   `_userState[holder]` word, carrying the scaled balance (low
//!   `basis_bits` bits) before and after the block's writes;
//! * `GlobalState` rows for the bound reserve's `liquidityIndex`,
//!   `currentLiquidityRate` and `lastUpdateTimestamp` in Pool storage and for
//!   the aToken's scaled `_totalSupply`;
//! * one `BlockClock` per block, plus `ModelEpoch` and `Dependency` rows at the
//!   activation block and on the configured heartbeat.
//!
//! `balanceOf(user)` is `scaled.rayMulFloor(getNormalizedIncome(reserve))` for
//! aToken revision 4 and later (half-up before). The map emits the inputs; the
//! consumer evaluates at its selected canonical clock. Nothing is emitted for a
//! holder whose word was not written, and a global update never fans out into
//! synthetic holder rows.
//!
//! Fail-closed rules: unlisted producer versions, code changes on the Pool,
//! an aToken or their implementations, writes to any bound implementation
//! pointer slot, unresolved aToken writes, ambiguous or discontinuous writes
//! all fail the block. Pool writes outside the bound reserve structs are not
//! part of the model and are ignored.
use evm_persist as persist;
use proto::pb::evm::balance_state::v1 as pb;
use serde::Deserialize;
use sha2::Digest;
use std::collections::BTreeMap;
use substreams::{errors::Error, scalar::BigInt};
use substreams_ethereum::pb::eth::v2 as eth;
use tiny_keccak::{Hasher, Keccak};

pub const PACKAGE: &str = "aave_balance_state";
pub const SPEC_REVISION: u32 = 1;
/// Producer versions whose execution ordinals are qualified (version 3 has
/// broken system-call ordinals and is refused by the contract).
pub const QUALIFIED_PRODUCER_VERSIONS: [i32; 2] = [4, 5];
/// Storage word offsets inside `DataTypes.ReserveData` that the model binds.
const RESERVE_INDEX_RATE_OFFSET: u8 = 1;
const RESERVE_CLOCK_OFFSET: u8 = 3;
/// Number of words occupied by `DataTypes.ReserveData` (pinned source layout).
const RESERVE_WORDS: u8 = 10;

fn require(ok: bool, message: &str) -> Result<(), Error> {
    if ok {
        Ok(())
    } else {
        Err(Error::msg(message.to_string()))
    }
}
pub fn keccak(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0; 32];
    let mut hasher = Keccak::v256();
    hasher.update(bytes);
    hasher.finalize(&mut out);
    out
}
fn word(bytes: &[u8]) -> Result<[u8; 32], Error> {
    require(bytes.len() <= 32, "storage word exceeds 32 bytes")?;
    let mut out = [0; 32];
    out[32 - bytes.len()..].copy_from_slice(bytes);
    Ok(out)
}
fn hex_bytes(s: &str, len: usize, what: &str) -> Result<Vec<u8>, Error> {
    let bytes = hex::decode(s.strip_prefix("0x").unwrap_or(s)).map_err(|_| Error::msg(format!("invalid hex for {what}")))?;
    require(bytes.len() == len, &format!("{what} must be {len} bytes"))?;
    Ok(bytes)
}
fn slot(s: &str, what: &str) -> Result<[u8; 32], Error> {
    Ok(hex_bytes(s, 32, what)?.try_into().unwrap())
}
fn mapping_key(key: &[u8], base: &[u8; 32]) -> [u8; 32] {
    let mut preimage = [0u8; 64];
    preimage[32 - key.len()..32].copy_from_slice(key);
    preimage[32..].copy_from_slice(base);
    keccak(&preimage)
}
fn add_offset(base: &[u8; 32], offset: u8) -> [u8; 32] {
    let mut out = *base;
    let mut carry = offset as u16;
    for byte in out.iter_mut().rev() {
        let sum = *byte as u16 + carry;
        *byte = sum as u8;
        carry = sum >> 8;
        if carry == 0 {
            break;
        }
    }
    out
}
/// Offset of `key` from `base` when `key` lies within `[base, base + words)`.
fn struct_offset(key: &[u8; 32], base: &[u8; 32], words: u8) -> Option<u8> {
    (0..words).find(|offset| add_offset(base, *offset) == *key)
}
/// Unsigned value of bits `[offset, offset + width)` of a big-endian word.
pub fn bits(word: &[u8; 32], offset: u32, width: u32) -> BigInt {
    assert!(offset + width <= 256 && width > 0);
    let mut out = [0u8; 32];
    for bit in 0..width {
        let source = offset + bit;
        let byte = word[31 - (source / 8) as usize];
        if (byte >> (source % 8)) & 1 == 1 {
            out[31 - (bit / 8) as usize] |= 1 << (bit % 8);
        }
    }
    BigInt::from_unsigned_bytes_be(&out)
}

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub chain_id: u64,
    pub producer_versions: Vec<i32>,
    /// Blocks between REAFFIRMED epoch rows after activation; 0 disables.
    #[serde(default)]
    pub heartbeat_blocks: u64,
    #[serde(default)]
    pub pool: Option<PoolConfig>,
    #[serde(default)]
    pub markets: Vec<MarketConfig>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolConfig {
    pub address: String,
    /// Base slot of `mapping(address => ReserveData) _reserves` in Pool storage.
    pub reserves_slot: String,
    /// Proxy implementation pointer slot on the Pool proxy and its expected value.
    pub implementation_slot: String,
    pub implementation: String,
    pub source_pin: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketConfig {
    pub atoken: String,
    pub underlying: String,
    pub epoch: u32,
    pub model_id: String,
    pub source_pin: String,
    pub implementation_revision: String,
    pub activation_block: u64,
    /// First execution ordinal of `activation_block` at which the epoch
    /// applies; earlier writes in that block belong to the previous epoch.
    #[serde(default)]
    pub activation_ordinal: u64,
    pub balance_decimals: u32,
    /// "floor" (aToken revision 4 and later) or "half_up" (earlier).
    pub rounding: String,
    /// Width of the scaled balance inside `_userState[user]`: 120 since v3.4,
    /// 128 before.
    pub basis_bits: u32,
    pub user_state_slot: String,
    pub total_supply_slot: String,
    #[serde(default)]
    pub other_slots: Vec<String>,
    #[serde(default)]
    pub other_mapping_slots: Vec<String>,
    pub implementation_slot: String,
    pub implementation: String,
}

#[derive(Clone, Debug)]
pub struct Pool {
    pub address: Vec<u8>,
    pub reserves_slot: [u8; 32],
    pub implementation_slot: [u8; 32],
    pub implementation: Vec<u8>,
    pub source_pin: String,
}
#[derive(Clone, Debug)]
pub struct Market {
    pub atoken: Vec<u8>,
    pub underlying: Vec<u8>,
    pub reserve_base: [u8; 32],
    pub epoch: u32,
    pub model_id: String,
    pub source_pin: String,
    pub implementation_revision: String,
    pub activation_block: u64,
    pub activation_ordinal: u64,
    pub balance_decimals: u32,
    pub rounding: pb::Rounding,
    pub basis_bits: u32,
    pub user_state_slot: [u8; 32],
    pub total_supply_slot: [u8; 32],
    pub other_slots: Vec<[u8; 32]>,
    pub other_mapping_slots: Vec<[u8; 32]>,
    pub implementation_slot: [u8; 32],
    pub implementation: Vec<u8>,
}
#[derive(Clone, Debug)]
pub struct Config {
    pub chain_id: u64,
    pub producer_versions: Vec<i32>,
    pub heartbeat_blocks: u64,
    pub pool: Option<Pool>,
    pub markets: Vec<Market>,
    pub parameters_sha256: String,
}

impl Market {
    /// Whether this epoch applies to an effect at `(block, ordinal)`.
    pub fn active_at(&self, block: u64, ordinal: u64) -> bool {
        block > self.activation_block || (block == self.activation_block && ordinal >= self.activation_ordinal)
    }
}

pub fn parse(params: &str) -> Result<Config, Error> {
    let raw: Params = serde_json::from_str(params).map_err(|e| Error::msg(format!("invalid aave balance-state params: {e}")))?;
    require(
        !raw.producer_versions.is_empty() && raw.producer_versions.iter().all(|v| QUALIFIED_PRODUCER_VERSIONS.contains(v)),
        "producer_versions must be a non-empty subset of the qualified Extended versions 4 and 5",
    )?;
    require(raw.chain_id > 0, "chain_id required")?;
    let pool = match &raw.pool {
        Some(p) => Some(Pool {
            address: hex_bytes(&p.address, 20, "pool address")?,
            reserves_slot: slot(&p.reserves_slot, "pool reserves_slot")?,
            implementation_slot: slot(&p.implementation_slot, "pool implementation_slot")?,
            implementation: hex_bytes(&p.implementation, 20, "pool implementation")?,
            source_pin: p.source_pin.clone(),
        }),
        None => None,
    };
    require(raw.markets.is_empty() || pool.is_some(), "markets require a pool binding")?;
    let mut markets = Vec::new();
    for m in &raw.markets {
        let pool = pool.as_ref().unwrap();
        let underlying = hex_bytes(&m.underlying, 20, "market underlying")?;
        let rounding = match m.rounding.as_str() {
            "floor" => pb::Rounding::Floor,
            "half_up" => pb::Rounding::HalfUp,
            _ => return Err(Error::msg("market rounding must be floor or half_up")),
        };
        require(m.basis_bits == 120 || m.basis_bits == 128, "basis_bits must be 120 or 128")?;
        require(m.epoch > 0 && m.activation_block > 0, "epoch and activation_block must be positive")?;
        require(!m.model_id.is_empty() && !m.source_pin.is_empty(), "model_id and source_pin required")?;
        let market = Market {
            atoken: hex_bytes(&m.atoken, 20, "market atoken")?,
            reserve_base: mapping_key(&underlying, &pool.reserves_slot),
            underlying,
            epoch: m.epoch,
            model_id: m.model_id.clone(),
            source_pin: m.source_pin.clone(),
            implementation_revision: m.implementation_revision.clone(),
            activation_block: m.activation_block,
            activation_ordinal: m.activation_ordinal,
            balance_decimals: m.balance_decimals,
            rounding,
            basis_bits: m.basis_bits,
            user_state_slot: slot(&m.user_state_slot, "user_state_slot")?,
            total_supply_slot: slot(&m.total_supply_slot, "total_supply_slot")?,
            other_slots: m.other_slots.iter().map(|s| slot(s, "other_slots")).collect::<Result<_, _>>()?,
            other_mapping_slots: m.other_mapping_slots.iter().map(|s| slot(s, "other_mapping_slots")).collect::<Result<_, _>>()?,
            implementation_slot: slot(&m.implementation_slot, "market implementation_slot")?,
            implementation: hex_bytes(&m.implementation, 20, "market implementation")?,
        };
        require(
            market.atoken != pool.address && market.atoken != market.underlying,
            "aToken must differ from pool and underlying",
        )?;
        require(
            market.user_state_slot != market.total_supply_slot
                && market.user_state_slot != market.implementation_slot
                && market.total_supply_slot != market.implementation_slot
                && !market.other_slots.contains(&market.user_state_slot)
                && !market.other_slots.contains(&market.total_supply_slot)
                && !market.other_mapping_slots.contains(&market.user_state_slot),
            "market slots overlap",
        )?;
        require(markets.iter().all(|other: &Market| other.atoken != market.atoken), "duplicate aToken")?;
        markets.push(market);
    }
    Ok(Config {
        chain_id: raw.chain_id,
        producer_versions: raw.producer_versions,
        heartbeat_blocks: raw.heartbeat_blocks,
        pool,
        markets,
        parameters_sha256: hex::encode(sha2::Sha256::digest(params.as_bytes())),
    })
}

// ---------------------------------------------------------------------------
// Persisted writes
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Write {
    address: Vec<u8>,
    key: [u8; 32],
    old: [u8; 32],
    new: [u8; 32],
    ordinal: u64,
    scope: persist::Scope,
    tx_hash: Vec<u8>,
    tx_index: u32,
    call_index: u32,
}
#[derive(Clone, Debug)]
struct CodeChanged {
    address: Vec<u8>,
    new_hash: Vec<u8>,
    ordinal: u64,
    scope: persist::Scope,
    tx_hash: Vec<u8>,
    tx_index: u32,
    call_index: u32,
}
#[derive(Default)]
struct Collected {
    writes: Vec<Write>,
    /// Equal-value writes; only STORAGE_POINTER slots consume them.
    noops: Vec<Write>,
    codes: Vec<CodeChanged>,
    errors: Vec<String>,
}
impl persist::Sink for Collected {
    fn storage(&mut self, c: &eth::StorageChange, ctx: persist::Ctx) {
        self.storage_change(c, ctx, false);
    }
    fn storage_noop(&mut self, c: &eth::StorageChange, ctx: persist::Ctx) {
        self.storage_change(c, ctx, true);
    }
    fn balance(&mut self, _: &eth::BalanceChange, _: persist::Ctx) {}
    fn nonce(&mut self, _: &eth::NonceChange, _: persist::Ctx) {}
    fn code(&mut self, c: &eth::CodeChange, ctx: persist::Ctx) {
        self.push_code(c, ctx);
    }
}
impl Collected {
    fn storage_change(&mut self, c: &eth::StorageChange, ctx: persist::Ctx, noop: bool) {
        match (word(&c.key), word(&c.old_value), word(&c.new_value)) {
            (Ok(key), Ok(old), Ok(new)) => (if noop { &mut self.noops } else { &mut self.writes }).push(Write {
                address: c.address.clone(),
                key,
                old,
                new,
                ordinal: c.ordinal,
                scope: ctx.scope,
                tx_hash: ctx.tx_hash.to_vec(),
                tx_index: ctx.tx_index,
                call_index: ctx.call_index,
            }),
            _ => self.errors.push(format!("malformed storage change at 0x{}", hex::encode(&c.address))),
        }
    }
    fn push_code(&mut self, c: &eth::CodeChange, ctx: persist::Ctx) {
        self.codes.push(CodeChanged {
            address: c.address.clone(),
            new_hash: c.new_hash.clone(),
            ordinal: c.ordinal,
            scope: ctx.scope,
            tx_hash: ctx.tx_hash.to_vec(),
            tx_index: ctx.tx_index,
            call_index: ctx.call_index,
        });
    }
}

/// Writes to one key, reduced in ordinal order with old/new continuity.
#[derive(Clone, Debug)]
pub struct Reduced {
    pub address: Vec<u8>,
    pub key: [u8; 32],
    pub old: [u8; 32],
    pub new: [u8; 32],
    pub first_ordinal: u64,
    pub ordinal: u64,
    pub count: u32,
    pub scope: persist::Scope,
    pub tx_hash: Vec<u8>,
    pub tx_index: u32,
    pub call_index: u32,
}
fn reduce(mut writes: Vec<Write>) -> Result<Vec<Reduced>, Error> {
    writes.sort_by_key(|w| w.ordinal);
    let mut rows: BTreeMap<(Vec<u8>, [u8; 32]), Reduced> = BTreeMap::new();
    for w in writes {
        let at = || format!("0x{} key 0x{}", hex::encode(&w.address), hex::encode(w.key));
        require(w.ordinal > 0, &format!("persisted storage write at {} has no execution ordinal", at()))?;
        match rows.get_mut(&(w.address.clone(), w.key)) {
            Some(r) => {
                require(
                    w.ordinal > r.ordinal,
                    &format!(
                        "ambiguous storage execution order at {}: ordinal {} repeats after {}",
                        at(),
                        w.ordinal,
                        r.ordinal
                    ),
                )?;
                require(
                    w.old == r.new,
                    &format!(
                        "discontinuous storage writes at {}: ordinal {} starts from 0x{} but ordinal {} ended at 0x{}",
                        at(),
                        w.ordinal,
                        hex::encode(w.old),
                        r.ordinal,
                        hex::encode(r.new)
                    ),
                )?;
                r.new = w.new;
                r.ordinal = w.ordinal;
                r.count += 1;
                r.tx_hash = w.tx_hash;
                r.tx_index = w.tx_index;
                r.call_index = w.call_index;
                r.scope = w.scope;
            }
            None => {
                rows.insert(
                    (w.address.clone(), w.key),
                    Reduced {
                        address: w.address,
                        key: w.key,
                        old: w.old,
                        new: w.new,
                        first_ordinal: w.ordinal,
                        ordinal: w.ordinal,
                        count: 1,
                        scope: w.scope,
                        tx_hash: w.tx_hash,
                        tx_index: w.tx_index,
                        call_index: w.call_index,
                    },
                );
            }
        }
    }
    Ok(rows.into_values().collect())
}
fn scope_of(scope: persist::Scope) -> pb::Scope {
    match scope {
        persist::Scope::SystemCall => pb::Scope::SystemCall,
        persist::Scope::Block => pb::Scope::Block,
        _ => pb::Scope::Transaction,
    }
}
fn mapping_has_base(mut key: [u8; 32], preimages: &BTreeMap<[u8; 32], Vec<u8>>, expected: &[u8; 32]) -> bool {
    let mut visited = std::collections::BTreeSet::new();
    while visited.insert(key) {
        let Some(preimage) = preimages.get(&key).filter(|p| p.len() == 64) else {
            return false;
        };
        let base: [u8; 32] = preimage[32..].try_into().unwrap();
        if &base == expected {
            return true;
        }
        key = base;
    }
    false
}

pub fn validate_block(block: &eth::Block, config: &Config) -> Result<u64, Error> {
    require(
        block.detail_level == eth::block::DetailLevel::DetaillevelExtended as i32,
        "Extended blocks required",
    )?;
    require(config.producer_versions.contains(&block.ver), "Extended producer version not qualified")?;
    let header = block.header.as_ref().ok_or_else(|| Error::msg("missing header"))?;
    require(
        block.hash.len() == 32 && header.parent_hash.len() == 32 && header.state_root.len() == 32,
        "invalid block identity",
    )?;
    require(header.number == block.number && block.number > 0, "header number mismatch")?;
    let timestamp = header.timestamp.as_ref().ok_or_else(|| Error::msg("missing timestamp"))?;
    require(timestamp.seconds >= 0, "negative timestamp")?;
    for tx in &block.transaction_traces {
        require((1..=3).contains(&tx.status) && !tx.calls.is_empty(), "incomplete transaction persistence data")?;
    }
    Ok(timestamp.seconds as u64)
}

// ---------------------------------------------------------------------------
// Projection
// ---------------------------------------------------------------------------

fn base_row(config: &Config, market: &Market, r: &Reduced) -> pb::GlobalState {
    pb::GlobalState {
        chain_id: config.chain_id,
        market: market.atoken.clone(),
        epoch: market.epoch,
        observation: pb::Observation::ObservedWrite as i32,
        boundary: pb::Boundary::EndOfBlock as i32,
        scope: scope_of(r.scope) as i32,
        ordinal: r.ordinal,
        first_ordinal: r.first_ordinal,
        change_count: r.count,
        transaction_index: r.tx_index,
        transaction_hash: r.tx_hash.clone(),
        call_index: r.call_index,
        storage_contract: r.address.clone(),
        storage_slot: r.key.to_vec(),
        raw_previous_word: r.old.to_vec(),
        raw_word: r.new.to_vec(),
        ..Default::default()
    }
}
/// One decoded field of a persisted word.
struct Field<'a> {
    field: pb::StateField,
    key: &'a [u8],
    offset: u32,
    width: u32,
    scale: &'a str,
}
fn field_row(config: &Config, market: &Market, r: &Reduced, f: Field) -> pb::GlobalState {
    pb::GlobalState {
        field: f.field as i32,
        key: f.key.to_vec(),
        value: bits(&r.new, f.offset, f.width).to_string(),
        previous_value: bits(&r.old, f.offset, f.width).to_string(),
        scale: f.scale.into(),
        bit_offset: f.offset,
        bit_width: f.width,
        ..base_row(config, market, r)
    }
}
const RAY: &str = "1000000000000000000000000000";

fn epoch_row(config: &Config, market: &Market, kind: pb::EpochEventKind) -> pb::ModelEpoch {
    pb::ModelEpoch {
        chain_id: config.chain_id,
        market: market.atoken.clone(),
        epoch: market.epoch,
        kind: kind as i32,
        family: pb::ModelFamily::AaveV3Atoken as i32,
        model_id: market.model_id.clone(),
        source_pin: market.source_pin.clone(),
        implementation_revision: market.implementation_revision.clone(),
        basis_kind: pb::BasisKind::ScaledBalance as i32,
        basis_scale: RAY.into(),
        balance_rounding: market.rounding as i32,
        basis_bit_offset: 0,
        basis_bit_width: market.basis_bits,
        basis_signed: false,
        implementation: market.implementation.clone(),
        implementation_slot: market.implementation_slot.to_vec(),
        activation_block: market.activation_block,
        activation_ordinal: market.activation_ordinal,
        balance_asset: market.underlying.clone(),
        balance_decimals: market.balance_decimals,
        basis_carryover: true,
        global_carryover: true,
        scope: pb::Scope::Epoch as i32,
        ..Default::default()
    }
}
fn invalidation(config: &Config, market: &Market, reason: pb::InvalidationReason, r: &Write) -> pb::ModelEpoch {
    pb::ModelEpoch {
        reason: reason as i32,
        scope: scope_of(r.scope) as i32,
        ordinal: r.ordinal,
        transaction_index: r.tx_index,
        transaction_hash: r.tx_hash.clone(),
        call_index: r.call_index,
        evidence_contract: r.address.clone(),
        evidence_slot: r.key.to_vec(),
        evidence_previous_word: r.old.to_vec(),
        evidence_word: r.new.to_vec(),
        ..epoch_row(config, market, pb::EpochEventKind::Invalidated)
    }
}

pub fn project(block: &eth::Block, config: &Config) -> Result<pb::Events, Error> {
    let timestamp = validate_block(block, config)?;
    let header = block.header.as_ref().unwrap();
    let active: Vec<&Market> = config.markets.iter().filter(|m| m.activation_block <= block.number).collect();
    let mut events = pb::Events::default();

    let mut collected = Collected::default();
    persist::collect_block(block, &mut collected)?;
    require(collected.errors.is_empty(), "malformed persisted storage change")?;
    if let Some(pool) = &config.pool {
        // Code changes on the Pool, its implementation, an aToken or its
        // implementation invalidate the affected epochs with evidence; the
        // block's other writes are still decoded.
        for c in &collected.codes {
            for market in active.iter().filter(|m| m.active_at(block.number, c.ordinal)) {
                let reason = if c.address == market.atoken || c.address == market.implementation {
                    pb::InvalidationReason::CodeChange
                } else if c.address == pool.address || c.address == pool.implementation {
                    pb::InvalidationReason::DependencyCodeChange
                } else {
                    continue;
                };
                events.epochs.push(pb::ModelEpoch {
                    reason: reason as i32,
                    scope: scope_of(c.scope) as i32,
                    ordinal: c.ordinal,
                    transaction_index: c.tx_index,
                    transaction_hash: c.tx_hash.clone(),
                    call_index: c.call_index,
                    evidence_contract: c.address.clone(),
                    evidence_code_hash: c.new_hash.clone(),
                    ..epoch_row(config, market, pb::EpochEventKind::Invalidated)
                });
            }
        }
        // Preimages are discovery hints, verified before use; only persisted
        // writes become rows.
        let mut preimages = BTreeMap::new();
        for call in block.system_calls.iter().chain(block.transaction_traces.iter().flat_map(|tx| &tx.calls)) {
            if !active
                .iter()
                .any(|m| call.address == m.atoken || call.storage_changes.iter().any(|c| c.address == m.atoken))
            {
                continue;
            }
            for (key, value) in &call.keccak_preimages {
                let key = hex::decode(key).map_err(|_| Error::msg("invalid preimage hex"))?;
                let value = hex::decode(value).map_err(|_| Error::msg("invalid preimage hex"))?;
                require(key.len() == 32 && keccak(&value).as_slice() == key, "invalid Keccak preimage")?;
                preimages.insert(word(&key)?, value);
            }
        }
        // Each epoch owns only the effects at or after its activation
        // position. Reserve words are keyed by the market's underlying, so a
        // Pool write is owned by the market whose reserve it belongs to.
        let owned = |w: &Write| {
            if w.address == pool.address {
                if w.key == pool.implementation_slot {
                    active.iter().any(|m| m.active_at(block.number, w.ordinal))
                } else {
                    active
                        .iter()
                        .any(|m| m.active_at(block.number, w.ordinal) && struct_offset(&w.key, &m.reserve_base, RESERVE_WORDS).is_some())
                }
            } else {
                active.iter().any(|m| w.address == m.atoken && m.active_at(block.number, w.ordinal))
            }
        };
        let is_pointer = |w: &Write| {
            (w.address == pool.address && w.key == pool.implementation_slot) || active.iter().any(|m| w.address == m.atoken && w.key == m.implementation_slot)
        };
        let mut relevant: Vec<Write> = collected.writes.into_iter().filter(&owned).collect();
        // STORAGE_POINTER contract: any persisted write to a pointer slot
        // invalidates, including a write back to the same value.
        relevant.extend(collected.noops.into_iter().filter(|w| is_pointer(w) && owned(w)));
        // Validate continuity first, then evidence each pointer transition:
        // an in-block excursion X->Z->X must not be concealed by reduction.
        let reduced = reduce(relevant.clone())?;
        for w in relevant.iter().filter(|w| is_pointer(w)) {
            if w.address == pool.address {
                for market in active.iter().filter(|m| m.active_at(block.number, w.ordinal)) {
                    events
                        .epochs
                        .push(invalidation(config, market, pb::InvalidationReason::DependencyPointerWrite, w));
                }
            } else if let Some(market) = active.iter().find(|m| w.address == m.atoken) {
                events
                    .epochs
                    .push(invalidation(config, market, pb::InvalidationReason::ImplementationPointerWrite, w));
            }
        }
        for r in reduced {
            if r.address == pool.address {
                if r.key == pool.implementation_slot {
                    continue; // Every pointer write was invalidated above.
                }
                for market in &active {
                    let Some(offset) = struct_offset(&r.key, &market.reserve_base, RESERVE_WORDS) else {
                        continue;
                    };
                    match offset {
                        RESERVE_INDEX_RATE_OFFSET => {
                            events.global_state.push(field_row(
                                config,
                                market,
                                &r,
                                Field {
                                    field: pb::StateField::AaveLiquidityIndex,
                                    key: &market.underlying,
                                    offset: 0,
                                    width: 128,
                                    scale: RAY,
                                },
                            ));
                            events.global_state.push(field_row(
                                config,
                                market,
                                &r,
                                Field {
                                    field: pb::StateField::AaveCurrentLiquidityRate,
                                    key: &market.underlying,
                                    offset: 128,
                                    width: 128,
                                    scale: RAY,
                                },
                            ));
                        }
                        RESERVE_CLOCK_OFFSET => {
                            events.global_state.push(field_row(
                                config,
                                market,
                                &r,
                                Field {
                                    field: pb::StateField::AaveLastUpdateTimestamp,
                                    key: &market.underlying,
                                    offset: 128,
                                    width: 40,
                                    scale: "1",
                                },
                            ));
                        }
                        // configuration, variable-debt index/rate, addresses,
                        // treasury accrual, virtual balance: bound layout,
                        // not balance inputs of this model.
                        _ => {}
                    }
                }
                continue;
            }
            let market = active.iter().find(|m| m.atoken == r.address).unwrap();
            if r.key == market.implementation_slot {
                continue; // Every pointer write was invalidated above.
            }
            if let Some(holder) = preimages
                .get(&r.key)
                .filter(|p| p.len() == 64 && p[..12] == [0; 12] && p[32..] == market.user_state_slot)
                .map(|p| p[12..32].to_vec())
            {
                events.holder_basis.push(pb::HolderBasis {
                    chain_id: config.chain_id,
                    market: market.atoken.clone(),
                    holder,
                    epoch: market.epoch,
                    basis_kind: pb::BasisKind::ScaledBalance as i32,
                    value: bits(&r.new, 0, market.basis_bits).to_string(),
                    previous_value: bits(&r.old, 0, market.basis_bits).to_string(),
                    observation: pb::Observation::ObservedWrite as i32,
                    boundary: pb::Boundary::EndOfBlock as i32,
                    scope: scope_of(r.scope) as i32,
                    ordinal: r.ordinal,
                    first_ordinal: r.first_ordinal,
                    change_count: r.count,
                    transaction_index: r.tx_index,
                    transaction_hash: r.tx_hash.clone(),
                    call_index: r.call_index,
                    storage_contract: r.address.clone(),
                    storage_slot: r.key.to_vec(),
                    raw_previous_word: r.old.to_vec(),
                    raw_word: r.new.to_vec(),
                    bit_offset: 0,
                    bit_width: market.basis_bits,
                    signed: false,
                });
            } else if r.key == market.total_supply_slot {
                events.global_state.push(field_row(
                    config,
                    market,
                    &r,
                    Field {
                        field: pb::StateField::AaveScaledTotalSupply,
                        key: &market.atoken,
                        offset: 0,
                        width: 256,
                        scale: "1",
                    },
                ));
            } else if market.other_slots.contains(&r.key) || market.other_mapping_slots.iter().any(|base| mapping_has_base(r.key, &preimages, base)) {
                // Reviewed non-balance storage.
            } else {
                return Err(Error::msg(format!(
                    "unresolved storage for aToken 0x{} at key 0x{}; refusing incomplete balance state",
                    hex::encode(&r.address),
                    hex::encode(r.key)
                )));
            }
        }
        for market in &active {
            let kind = if block.number == market.activation_block {
                pb::EpochEventKind::Bound
            } else if config.heartbeat_blocks > 0 && (block.number - market.activation_block) % config.heartbeat_blocks == 0 {
                pb::EpochEventKind::Reaffirmed
            } else {
                continue;
            };
            events.epochs.push(pb::ModelEpoch {
                // A BOUND row applies from its activation position, after any
                // invalidation of the previous epoch earlier in the block.
                ordinal: if kind == pb::EpochEventKind::Bound { market.activation_ordinal } else { 0 },
                ..epoch_row(config, market, kind)
            });
            let dependency = |role: pb::DependencyRole, contract: &Vec<u8>, binding: pb::BindingKind, pin: &str| pb::Dependency {
                chain_id: config.chain_id,
                market: market.atoken.clone(),
                epoch: market.epoch,
                kind: kind as i32,
                role: role as i32,
                contract: contract.clone(),
                depth: 1,
                binding: binding as i32,
                activation_block: market.activation_block,
                source_pin: pin.into(),
                ..Default::default()
            };
            events.dependencies.push(pb::Dependency {
                pointer_contract: pool.address.clone(),
                pointer_slot: pool.implementation_slot.to_vec(),
                pointer_value: word(&pool.implementation)?.to_vec(),
                ..dependency(pb::DependencyRole::Pool, &pool.address, pb::BindingKind::StoragePointer, &pool.source_pin)
            });
            events.dependencies.push(pb::Dependency {
                pointer_contract: market.atoken.clone(),
                pointer_slot: market.implementation_slot.to_vec(),
                pointer_value: word(&market.implementation)?.to_vec(),
                parent: Vec::new(),
                ..dependency(
                    pb::DependencyRole::Implementation,
                    &market.implementation,
                    pb::BindingKind::StoragePointer,
                    &market.source_pin,
                )
            });
            events.dependencies.push(dependency(
                pb::DependencyRole::Underlying,
                &market.underlying,
                pb::BindingKind::Declared,
                &market.source_pin,
            ));
        }
    }
    events
        .holder_basis
        .sort_by(|a, b| (&a.market, &a.holder, a.ordinal).cmp(&(&b.market, &b.holder, b.ordinal)));
    events
        .global_state
        .sort_by(|a, b| (&a.market, a.field, &a.key, a.ordinal).cmp(&(&b.market, b.field, &b.key, b.ordinal)));
    events.epochs.sort_by(|a, b| {
        (
            &a.market,
            a.epoch,
            a.ordinal,
            a.kind,
            a.reason,
            &a.evidence_contract,
            &a.evidence_slot,
            &a.evidence_code_hash,
        )
            .cmp(&(
                &b.market,
                b.epoch,
                b.ordinal,
                b.kind,
                b.reason,
                &b.evidence_contract,
                &b.evidence_slot,
                &b.evidence_code_hash,
            ))
    });
    events
        .dependencies
        .sort_by(|a, b| (&a.market, a.epoch, a.role, &a.contract).cmp(&(&b.market, b.epoch, b.role, &b.contract)));
    events.clocks.push(pb::BlockClock {
        chain_id: config.chain_id,
        number: block.number,
        hash: block.hash.clone(),
        parent_hash: header.parent_hash.clone(),
        timestamp,
        state_root: header.state_root.clone(),
        producer_version: block.ver as u32,
        spec_revision: SPEC_REVISION,
        package: PACKAGE.into(),
        package_version: env!("CARGO_PKG_VERSION").into(),
        parameters_sha256: config.parameters_sha256.clone(),
        epoch_count: events.epochs.len() as u32,
        dependency_count: events.dependencies.len() as u32,
        alias_count: 0,
        holder_basis_count: events.holder_basis.len() as u32,
        global_state_count: events.global_state.len() as u32,
    });
    Ok(events)
}

// The SDK macro generates raw-pointer parameter decoding and discards function
// attributes; the export exists only in the WASM build so host crates can link
// this crate next to other balance packages.
#[cfg(target_arch = "wasm32")]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
mod handler {
    use super::*;
    #[substreams::handlers::map]
    fn map_events(params: String, block: eth::Block) -> Result<pb::Events, Error> {
        project(&block, &parse(&params)?)
    }
}

#[cfg(test)]
mod tests;
