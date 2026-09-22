//! One RPC-free `map_events` for explicitly qualified Compound III (Comet)
//! market epochs, emitting `evm.balance_state.v1.Events`.
//!
//! * `HolderBasis`: the signed int104 `UserBasic.principal` (low 104 bits of
//!   `userBasic[account]`, two's complement) before the first and after the
//!   last persisted write of the block. A row is emitted for every written
//!   holder word, also when only the tracking fields in that word moved; the
//!   consumer compares `value` with `previous_value`. Negative principal is
//!   borrow debt and keeps its sign.
//! * `GlobalState`: every decoded field of a written market word,
//!   `baseSupplyIndex` and `baseBorrowIndex` (first word), `totalSupplyBase`,
//!   `totalBorrowBase`, `lastAccrualTime` and `pauseFlags` (second word), and
//!   the implementation immutables (kinks, per-second rate slopes and bases,
//!   scales) as qualified constants at the activation block and on the
//!   heartbeat.
//! * `ModelEpoch` `INVALIDATED` with evidence when the Comet's implementation
//!   pointer is written to another address or the Comet or implementation
//!   code changes; the block's other writes are still decoded.
//!
//! `balanceOf` is `principal > 0 ? principal * accruedSupplyIndex / 1e15 : 0`
//! where the accrued index projects the stored index to the evaluation
//! timestamp with the market's rates; an idle block moves every supplier's
//! balance without any write. The map emits inputs only; the consumer or
//! `conformance::comet` evaluates.
//!
//! Fail closed: producer versions other than 4 and 5, unresolved Comet writes,
//! tied or discontinuous writes and inconsistent parameters fail the block or
//! the parameters. Storage slots are caller-qualified; the committed
//! configuration infers them from the pinned declaration order (see
//! `docs/storage-layout-provenance.md`). Unstructured slots such as
//! `keccak256("comet.reentrancy.guard")` are reviewed by name.
use evm_persist as persist;
use proto::pb::evm::balance_state::v1 as pb;
use serde::Deserialize;
use sha2::Digest;
use std::collections::{BTreeMap, BTreeSet};
use substreams::{errors::Error, scalar::BigInt};
use substreams_ethereum::pb::eth::v2 as eth;
use tiny_keccak::{Hasher, Keccak};

pub const PACKAGE: &str = "compound_v3_balance_state";
pub const SPEC_REVISION: u32 = 2;
/// `CometCore.BASE_INDEX_SCALE`.
pub const BASE_INDEX_SCALE: &str = "1000000000000000";
/// `CometCore.FACTOR_SCALE`.
pub const FACTOR_SCALE: &str = "1000000000000000000";
/// Producer versions whose execution ordinals are qualified (version 3 has
/// broken system-call ordinals and is refused by the contract).
pub const QUALIFIED_PRODUCER_VERSIONS: [i32; 2] = [4, 5];

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
fn decimal(s: &str, what: &str) -> Result<String, Error> {
    require(
        !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()) && (s == "0" || !s.starts_with('0')),
        &format!("{what} must be a decimal integer"),
    )?;
    Ok(s.to_string())
}
/// Unsigned value of bits `[offset, offset + width)` of a big-endian word.
pub fn bits(word: &[u8; 32], offset: u32, width: u32) -> BigInt {
    assert!(offset + width <= 256 && width > 0);
    let mut out = [0u8; 32];
    for bit in 0..width {
        let source = offset + bit;
        if (word[31 - (source / 8) as usize] >> (source % 8)) & 1 == 1 {
            out[31 - (bit / 8) as usize] |= 1 << (bit % 8);
        }
    }
    BigInt::from_unsigned_bytes_be(&out)
}
/// Two's-complement signed value of bits `[offset, offset + width)`.
pub fn signed_bits(word: &[u8; 32], offset: u32, width: u32) -> BigInt {
    let unsigned = bits(word, offset, width);
    let sign_bit = (word[31 - ((offset + width - 1) / 8) as usize] >> ((offset + width - 1) % 8)) & 1 == 1;
    if sign_bit {
        let modulus = BigInt::from(1) << width as usize;
        unsigned - modulus
    } else {
        unsigned
    }
}
pub fn mapping_key(key: &[u8], base: &[u8; 32]) -> [u8; 32] {
    let mut preimage = [0u8; 64];
    preimage[32 - key.len()..32].copy_from_slice(key);
    preimage[32..].copy_from_slice(base);
    keccak(&preimage)
}

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub chain_id: u64,
    pub producer_versions: Vec<i32>,
    #[serde(default)]
    pub heartbeat_blocks: u64,
    #[serde(default)]
    pub markets: Vec<MarketConfig>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Immutables {
    pub supply_kink: String,
    pub supply_rate_slope_low: String,
    pub supply_rate_slope_high: String,
    pub supply_rate_base: String,
    pub borrow_kink: String,
    pub borrow_rate_slope_low: String,
    pub borrow_rate_slope_high: String,
    pub borrow_rate_base: String,
    /// Must equal `10^base_decimals` (`baseScale = 10 ** decimals` in the constructor).
    pub base_scale: String,
    /// Must equal `BASE_INDEX_SCALE` (1e15).
    pub base_index_scale: String,
    /// Must equal `FACTOR_SCALE` (1e18).
    pub factor_scale: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketConfig {
    pub comet: String,
    pub base_token: String,
    pub base_decimals: u32,
    pub epoch: u32,
    pub model_id: String,
    pub source_pin: String,
    pub activation_block: u64,
    /// First execution ordinal of `activation_block` at which the epoch
    /// applies; earlier writes in that block belong to the previous epoch
    /// (for example the upgrade write that installs this implementation).
    #[serde(default)]
    pub activation_ordinal: u64,
    pub implementation_slot: String,
    pub implementation: String,
    /// Word holding baseSupplyIndex | baseBorrowIndex | tracking indices.
    pub indices_slot: String,
    /// Word holding totalSupplyBase | totalBorrowBase | lastAccrualTime | pauseFlags.
    pub totals_slot: String,
    /// Base slot of `mapping(address => UserBasic) userBasic`.
    pub user_basic_slot: String,
    #[serde(default)]
    pub other_slots: Vec<String>,
    /// Reviewed unstructured-storage labels; the map stores `keccak256(label)`
    /// (for example `comet.reentrancy.guard`, written on every guarded call).
    #[serde(default)]
    pub other_slot_names: Vec<String>,
    #[serde(default)]
    pub other_mapping_slots: Vec<String>,
    pub immutables: Immutables,
}
#[derive(Clone, Debug)]
pub struct Market {
    pub comet: Vec<u8>,
    pub base_token: Vec<u8>,
    pub base_decimals: u32,
    pub epoch: u32,
    pub model_id: String,
    pub source_pin: String,
    pub activation_block: u64,
    pub activation_ordinal: u64,
    pub implementation_slot: [u8; 32],
    pub implementation: Vec<u8>,
    pub indices_slot: [u8; 32],
    pub totals_slot: [u8; 32],
    pub user_basic_slot: [u8; 32],
    pub other_slots: Vec<[u8; 32]>,
    pub other_mapping_slots: Vec<[u8; 32]>,
    /// (field, value, scale) constants of the bound implementation.
    pub immutables: Vec<(pb::StateField, String, &'static str)>,
}
#[derive(Clone, Debug)]
pub struct Config {
    pub chain_id: u64,
    pub producer_versions: Vec<i32>,
    pub heartbeat_blocks: u64,
    pub markets: Vec<Market>,
    pub parameters_sha256: String,
}

fn pow10(decimals: u32) -> String {
    let mut s = String::from("1");
    s.extend(std::iter::repeat_n('0', decimals as usize));
    s
}

impl Market {
    /// Whether this epoch applies to an effect at `(block, ordinal)`.
    pub fn active_at(&self, block: u64, ordinal: u64) -> bool {
        block > self.activation_block || (block == self.activation_block && ordinal >= self.activation_ordinal)
    }
}

pub fn parse(params: &str) -> Result<Config, Error> {
    let raw: Params = serde_json::from_str(params).map_err(|e| Error::msg(format!("invalid compound-v3 balance-state params: {e}")))?;
    require(raw.chain_id > 0, "chain_id required")?;
    require(
        !raw.producer_versions.is_empty() && raw.producer_versions.iter().all(|v| QUALIFIED_PRODUCER_VERSIONS.contains(v)),
        "producer_versions must be a non-empty subset of the qualified Extended versions 4 and 5",
    )?;
    let mut markets: Vec<Market> = Vec::new();
    for m in &raw.markets {
        require(m.epoch > 0 && m.activation_block > 0, "epoch and activation_block must be positive")?;
        require(!m.model_id.is_empty() && !m.source_pin.is_empty(), "model_id and source_pin required")?;
        require(m.base_decimals <= 18, "base_decimals exceeds MAX_BASE_DECIMALS (18)")?;
        let i = &m.immutables;
        require(
            decimal(&i.base_index_scale, "base_index_scale")? == BASE_INDEX_SCALE,
            "base_index_scale must equal the pinned BASE_INDEX_SCALE 1e15",
        )?;
        require(
            decimal(&i.factor_scale, "factor_scale")? == FACTOR_SCALE,
            "factor_scale must equal the pinned FACTOR_SCALE 1e18",
        )?;
        require(
            decimal(&i.base_scale, "base_scale")? == pow10(m.base_decimals),
            "base_scale must equal 10^base_decimals",
        )?;
        let immutables = vec![
            (pb::StateField::CometSupplyKink, decimal(&i.supply_kink, "supply_kink")?, FACTOR_SCALE),
            (
                pb::StateField::CometSupplyRateSlopeLow,
                decimal(&i.supply_rate_slope_low, "supply_rate_slope_low")?,
                FACTOR_SCALE,
            ),
            (
                pb::StateField::CometSupplyRateSlopeHigh,
                decimal(&i.supply_rate_slope_high, "supply_rate_slope_high")?,
                FACTOR_SCALE,
            ),
            (
                pb::StateField::CometSupplyRateBase,
                decimal(&i.supply_rate_base, "supply_rate_base")?,
                FACTOR_SCALE,
            ),
            (pb::StateField::CometBorrowKink, decimal(&i.borrow_kink, "borrow_kink")?, FACTOR_SCALE),
            (
                pb::StateField::CometBorrowRateSlopeLow,
                decimal(&i.borrow_rate_slope_low, "borrow_rate_slope_low")?,
                FACTOR_SCALE,
            ),
            (
                pb::StateField::CometBorrowRateSlopeHigh,
                decimal(&i.borrow_rate_slope_high, "borrow_rate_slope_high")?,
                FACTOR_SCALE,
            ),
            (
                pb::StateField::CometBorrowRateBase,
                decimal(&i.borrow_rate_base, "borrow_rate_base")?,
                FACTOR_SCALE,
            ),
            (pb::StateField::CometBaseScale, i.base_scale.clone(), "1"),
            (pb::StateField::CometBaseIndexScale, i.base_index_scale.clone(), "1"),
            (pb::StateField::CometFactorScale, i.factor_scale.clone(), "1"),
        ];
        let mut other_slots: Vec<[u8; 32]> = m.other_slots.iter().map(|s| slot(s, "other_slots")).collect::<Result<_, _>>()?;
        for name in &m.other_slot_names {
            require(!name.is_empty(), "empty slot name")?;
            other_slots.push(keccak(name.as_bytes()));
        }
        let market = Market {
            comet: hex_bytes(&m.comet, 20, "comet")?,
            base_token: hex_bytes(&m.base_token, 20, "base_token")?,
            base_decimals: m.base_decimals,
            epoch: m.epoch,
            model_id: m.model_id.clone(),
            source_pin: m.source_pin.clone(),
            activation_block: m.activation_block,
            activation_ordinal: m.activation_ordinal,
            implementation_slot: slot(&m.implementation_slot, "implementation_slot")?,
            implementation: hex_bytes(&m.implementation, 20, "implementation")?,
            indices_slot: slot(&m.indices_slot, "indices_slot")?,
            totals_slot: slot(&m.totals_slot, "totals_slot")?,
            user_basic_slot: slot(&m.user_basic_slot, "user_basic_slot")?,
            other_slots,
            other_mapping_slots: m.other_mapping_slots.iter().map(|s| slot(s, "other_mapping_slots")).collect::<Result<_, _>>()?,
            immutables,
        };
        let mut all = vec![market.indices_slot, market.totals_slot, market.implementation_slot, market.user_basic_slot];
        all.extend(market.other_slots.iter().copied());
        all.extend(market.other_mapping_slots.iter().copied());
        require(all.iter().collect::<BTreeSet<_>>().len() == all.len(), "market slots overlap")?;
        require(markets.iter().all(|other| other.comet != market.comet), "duplicate market")?;
        markets.push(market);
    }
    Ok(Config {
        chain_id: raw.chain_id,
        producer_versions: raw.producer_versions,
        heartbeat_blocks: raw.heartbeat_blocks,
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
    /// Equal-value writes. Persistence drops them as balance effects, but a
    /// STORAGE_POINTER binding invalidates on any write to its slot.
    noops: Vec<Write>,
    codes: Vec<CodeChanged>,
    errors: usize,
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
            _ => self.errors += 1,
        }
    }
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
    let mut visited = BTreeSet::new();
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

struct Field<'a> {
    field: pb::StateField,
    offset: u32,
    width: u32,
    scale: &'a str,
}
fn field_row(config: &Config, market: &Market, r: &Reduced, f: Field) -> pb::GlobalState {
    pb::GlobalState {
        chain_id: config.chain_id,
        market: market.comet.clone(),
        epoch: market.epoch,
        field: f.field as i32,
        value: bits(&r.new, f.offset, f.width).to_string(),
        previous_value: bits(&r.old, f.offset, f.width).to_string(),
        scale: f.scale.into(),
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
        bit_offset: f.offset,
        bit_width: f.width,
        ..Default::default()
    }
}
fn epoch_row(config: &Config, market: &Market, kind: pb::EpochEventKind) -> pb::ModelEpoch {
    pb::ModelEpoch {
        chain_id: config.chain_id,
        market: market.comet.clone(),
        epoch: market.epoch,
        kind: kind as i32,
        family: pb::ModelFamily::CompoundV3Comet as i32,
        model_id: market.model_id.clone(),
        source_pin: market.source_pin.clone(),
        basis_kind: pb::BasisKind::SignedPrincipal as i32,
        basis_scale: BASE_INDEX_SCALE.into(),
        balance_rounding: pb::Rounding::Floor as i32,
        basis_bit_offset: 0,
        basis_bit_width: 104,
        basis_signed: true,
        implementation: market.implementation.clone(),
        implementation_slot: market.implementation_slot.to_vec(),
        activation_block: market.activation_block,
        activation_ordinal: market.activation_ordinal,
        balance_asset: market.base_token.clone(),
        balance_decimals: market.base_decimals,
        // Storage (principals, indices, totals) persists across a Comet
        // implementation upgrade, so retained holder basis stays evaluable.
        basis_carryover: true,
        // Every Comet epoch is a new implementation with new rate immutables
        // (a rate-model replacement): retained GlobalState rows do not carry;
        // the constants are re-declared on BOUND.
        global_carryover: false,
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
    if !active.is_empty() {
        let mut collected = Collected::default();
        persist::collect_block(block, &mut collected)?;
        require(collected.errors == 0, "malformed persisted storage change")?;
        let mut preimages = BTreeMap::new();
        for call in block.system_calls.iter().chain(block.transaction_traces.iter().flat_map(|tx| &tx.calls)) {
            // Delegatecall frames carry `Call.address == implementation` while
            // their storage changes are on the Comet proxy: match on either.
            if !active
                .iter()
                .any(|m| call.address == m.comet || call.storage_changes.iter().any(|c| c.address == m.comet))
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
        // An epoch owns only the effects at or after its activation position;
        // earlier writes of the activation block belong to the previous epoch.
        let owner = |w: &Write| active.iter().find(|m| w.address == m.comet && m.active_at(block.number, w.ordinal)).copied();
        let mut relevant: Vec<Write> = collected.writes.into_iter().filter(|w| owner(w).is_some()).collect();
        // STORAGE_POINTER contract: any persisted write to the pointer slot
        // invalidates, including a write back to the same value.
        relevant.extend(collected.noops.into_iter().filter(|w| owner(w).is_some_and(|m| w.key == m.implementation_slot)));
        // Validate continuity first, then evidence every pointer transition
        // individually: reducing X->Z->X to its end points would conceal a
        // temporary upgrade that ran within this block.
        let reduced = reduce(relevant.clone())?;
        for w in &relevant {
            let market = owner(w).unwrap();
            if w.key == market.implementation_slot {
                events
                    .epochs
                    .push(invalidation(config, market, pb::InvalidationReason::ImplementationPointerWrite, w));
            }
        }
        for r in reduced {
            let market = active.iter().find(|m| m.comet == r.address).unwrap();
            if r.key == market.implementation_slot {
                // Every pointer write was invalidated above.
            } else if r.key == market.indices_slot {
                for (field, offset) in [(pb::StateField::CometBaseSupplyIndex, 0), (pb::StateField::CometBaseBorrowIndex, 64)] {
                    events.global_state.push(field_row(
                        config,
                        market,
                        &r,
                        Field {
                            field,
                            offset,
                            width: 64,
                            scale: BASE_INDEX_SCALE,
                        },
                    ));
                }
                // Tracking indices (bits 128..256) are reward state, not balance inputs.
            } else if r.key == market.totals_slot {
                for (field, offset, width, scale) in [
                    (pb::StateField::CometTotalSupplyBase, 0, 104, "1"),
                    (pb::StateField::CometTotalBorrowBase, 104, 104, "1"),
                    (pb::StateField::CometLastAccrualTime, 208, 40, "1"),
                    (pb::StateField::CometPauseFlags, 248, 8, "1"),
                ] {
                    events.global_state.push(field_row(config, market, &r, Field { field, offset, width, scale }));
                }
            } else if let Some(holder) = preimages
                .get(&r.key)
                .filter(|p| p.len() == 64 && p[..12] == [0; 12] && p[32..] == market.user_basic_slot)
                .map(|p| p[12..32].to_vec())
            {
                events.holder_basis.push(pb::HolderBasis {
                    chain_id: config.chain_id,
                    market: market.comet.clone(),
                    holder,
                    epoch: market.epoch,
                    basis_kind: pb::BasisKind::SignedPrincipal as i32,
                    value: signed_bits(&r.new, 0, 104).to_string(),
                    previous_value: signed_bits(&r.old, 0, 104).to_string(),
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
                    bit_width: 104,
                    signed: true,
                });
            } else if market.other_slots.contains(&r.key) || market.other_mapping_slots.iter().any(|base| mapping_has_base(r.key, &preimages, base)) {
                // Reviewed non-balance storage: reentrancy guard, collateral
                // totals, allowances, nonces, collateral positions, liquidator
                // points.
            } else {
                return Err(Error::msg(format!(
                    "unresolved storage for Comet 0x{} at key 0x{}; refusing incomplete balance state",
                    hex::encode(&r.address),
                    hex::encode(r.key)
                )));
            }
        }
        for c in &collected.codes {
            for market in active
                .iter()
                .filter(|m| (c.address == m.comet || c.address == m.implementation) && m.active_at(block.number, c.ordinal))
            {
                events.epochs.push(pb::ModelEpoch {
                    reason: pb::InvalidationReason::CodeChange as i32,
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
            events.dependencies.push(pb::Dependency {
                chain_id: config.chain_id,
                market: market.comet.clone(),
                epoch: market.epoch,
                kind: kind as i32,
                role: pb::DependencyRole::Implementation as i32,
                contract: market.implementation.clone(),
                depth: 1,
                binding: pb::BindingKind::StoragePointer as i32,
                pointer_contract: market.comet.clone(),
                pointer_slot: market.implementation_slot.to_vec(),
                pointer_value: word(&market.implementation)?.to_vec(),
                activation_block: market.activation_block,
                source_pin: market.source_pin.clone(),
                ..Default::default()
            });
            events.dependencies.push(pb::Dependency {
                chain_id: config.chain_id,
                market: market.comet.clone(),
                epoch: market.epoch,
                kind: kind as i32,
                role: pb::DependencyRole::Underlying as i32,
                contract: market.base_token.clone(),
                depth: 1,
                binding: pb::BindingKind::Declared as i32,
                activation_block: market.activation_block,
                source_pin: market.source_pin.clone(),
                ..Default::default()
            });
            // Rate and scale immutables are baked into the implementation;
            // every governance rate change is a new implementation.
            for (field, value, scale) in &market.immutables {
                events.global_state.push(pb::GlobalState {
                    chain_id: config.chain_id,
                    market: market.comet.clone(),
                    epoch: market.epoch,
                    field: *field as i32,
                    value: value.clone(),
                    scale: (*scale).into(),
                    observation: pb::Observation::QualifiedConstant as i32,
                    boundary: pb::Boundary::Declaration as i32,
                    scope: pb::Scope::Epoch as i32,
                    storage_contract: market.implementation.clone(),
                    ..Default::default()
                });
            }
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
