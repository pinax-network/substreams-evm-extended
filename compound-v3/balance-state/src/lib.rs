//! One RPC-free `map_events` for explicitly qualified Compound III (Comet)
//! market epochs, emitting `evm.balance_state.v1.Events`.
//!
//! * `HolderBasis`: the signed int104 `UserBasic.principal` (low 104 bits of
//!   `userBasic[account]`, two's complement) before and after the block's
//!   persisted writes. Negative principal is borrow debt and keeps its sign.
//! * `GlobalState`: `baseSupplyIndex` and `baseBorrowIndex` (first market
//!   word), `totalSupplyBase`, `totalBorrowBase`, `lastAccrualTime` and
//!   `pauseFlags` (second market word), and the implementation immutables
//!   (kinks, per-second rate slopes and bases, scales) as qualified constants
//!   at the activation block and on the heartbeat.
//!
//! `balanceOf` is `principal > 0 ? principal * accruedSupplyIndex / 1e15 : 0`
//! where the accrued index projects the stored index to the evaluation
//! timestamp with the market's rates; an idle block moves every supplier's
//! balance without any write. The map emits inputs only; the consumer or
//! `conformance::comet` evaluates.
//!
//! Fail closed: unlisted producer versions, Comet or implementation code
//! changes, implementation-pointer writes, unresolved Comet writes, tied or
//! discontinuous writes fail the block. Storage slots are caller-qualified
//! parameters; the committed configuration infers them from the pinned
//! declaration order and marks them unverified against a compiler layout.
use evm_persist as persist;
use proto::pb::evm::balance_state::v1 as pb;
use serde::Deserialize;
use sha2::Digest;
use std::collections::BTreeMap;
use substreams::{errors::Error, scalar::BigInt};
use substreams_ethereum::pb::eth::v2 as eth;
use tiny_keccak::{Hasher, Keccak};

pub const PACKAGE: &str = "compound_v3_balance_state";
pub const SPEC_REVISION: u32 = 1;
const BASE_INDEX_SCALE: &str = "1000000000000000";
const FACTOR_SCALE: &str = "1000000000000000000";

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
        !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()),
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
    pub base_scale: String,
    pub base_index_scale: String,
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

pub fn parse(params: &str) -> Result<Config, Error> {
    let raw: Params = serde_json::from_str(params).map_err(|e| Error::msg(format!("invalid compound-v3 balance-state params: {e}")))?;
    require(raw.chain_id > 0, "chain_id required")?;
    require(
        !raw.producer_versions.is_empty() && raw.producer_versions.iter().all(|v| *v > 0),
        "qualified producer versions required",
    )?;
    let mut markets = Vec::new();
    for m in &raw.markets {
        require(m.epoch > 0 && m.activation_block > 0, "epoch and activation_block must be positive")?;
        require(!m.model_id.is_empty() && !m.source_pin.is_empty(), "model_id and source_pin required")?;
        let i = &m.immutables;
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
            (pb::StateField::CometBaseScale, decimal(&i.base_scale, "base_scale")?, "1"),
            (pb::StateField::CometBaseIndexScale, decimal(&i.base_index_scale, "base_index_scale")?, "1"),
            (pb::StateField::CometFactorScale, decimal(&i.factor_scale, "factor_scale")?, "1"),
        ];
        let market = Market {
            comet: hex_bytes(&m.comet, 20, "comet")?,
            base_token: hex_bytes(&m.base_token, 20, "base_token")?,
            base_decimals: m.base_decimals,
            epoch: m.epoch,
            model_id: m.model_id.clone(),
            source_pin: m.source_pin.clone(),
            activation_block: m.activation_block,
            implementation_slot: slot(&m.implementation_slot, "implementation_slot")?,
            implementation: hex_bytes(&m.implementation, 20, "implementation")?,
            indices_slot: slot(&m.indices_slot, "indices_slot")?,
            totals_slot: slot(&m.totals_slot, "totals_slot")?,
            user_basic_slot: slot(&m.user_basic_slot, "user_basic_slot")?,
            other_slots: m.other_slots.iter().map(|s| slot(s, "other_slots")).collect::<Result<_, _>>()?,
            other_mapping_slots: m.other_mapping_slots.iter().map(|s| slot(s, "other_mapping_slots")).collect::<Result<_, _>>()?,
            immutables,
        };
        let scalars = [market.indices_slot, market.totals_slot, market.implementation_slot, market.user_basic_slot];
        require(
            scalars.iter().enumerate().all(|(i, a)| scalars.iter().skip(i + 1).all(|b| a != b))
                && !market.other_slots.iter().any(|s| scalars.contains(s))
                && !market.other_mapping_slots.contains(&market.user_basic_slot),
            "market slots overlap",
        )?;
        require(markets.iter().all(|other: &Market| other.comet != market.comet), "duplicate market")?;
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
#[derive(Default)]
struct Collected {
    writes: Vec<Write>,
    codes: Vec<Vec<u8>>,
    errors: usize,
}
impl persist::Sink for Collected {
    fn storage(&mut self, c: &eth::StorageChange, ctx: persist::Ctx) {
        match (word(&c.key), word(&c.old_value), word(&c.new_value)) {
            (Ok(key), Ok(old), Ok(new)) => self.writes.push(Write {
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
    fn balance(&mut self, _: &eth::BalanceChange, _: persist::Ctx) {}
    fn nonce(&mut self, _: &eth::NonceChange, _: persist::Ctx) {}
    fn code(&mut self, c: &eth::CodeChange, _: persist::Ctx) {
        self.codes.push(c.address.clone());
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
        require(w.ordinal > 0, "persisted storage write has no execution ordinal")?;
        match rows.get_mut(&(w.address.clone(), w.key)) {
            Some(r) => {
                require(w.ordinal > r.ordinal, "ambiguous storage execution order")?;
                require(w.old == r.new, "discontinuous storage writes within block")?;
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

pub fn project(block: &eth::Block, config: &Config) -> Result<pb::Events, Error> {
    let timestamp = validate_block(block, config)?;
    let header = block.header.as_ref().unwrap();
    let active: Vec<&Market> = config.markets.iter().filter(|m| m.activation_block <= block.number).collect();
    let mut events = pb::Events::default();
    if !active.is_empty() {
        let mut collected = Collected::default();
        persist::collect_block(block, &mut collected)?;
        require(collected.errors == 0, "malformed persisted storage change")?;
        require(
            !collected.codes.iter().any(|a| active.iter().any(|m| *a == m.comet || *a == m.implementation)),
            "Comet or implementation code changed; requalify the epoch",
        )?;
        let mut preimages = BTreeMap::new();
        for call in block.system_calls.iter().chain(block.transaction_traces.iter().flat_map(|tx| &tx.calls)) {
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
        let relevant: Vec<Write> = collected.writes.into_iter().filter(|w| active.iter().any(|m| w.address == m.comet)).collect();
        for r in reduce(relevant)? {
            let market = active.iter().find(|m| m.comet == r.address).unwrap();
            require(r.key != market.implementation_slot, "Comet implementation pointer changed; requalify the epoch")?;
            if r.key == market.indices_slot {
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
                    let row = field_row(config, market, &r, Field { field, offset, width, scale });
                    if row.value != row.previous_value {
                        events.global_state.push(row);
                    }
                }
            } else if let Some(holder) = preimages
                .get(&r.key)
                .filter(|p| p.len() == 64 && p[..12] == [0; 12] && p[32..] == market.user_basic_slot)
                .map(|p| p[12..32].to_vec())
            {
                let value = signed_bits(&r.new, 0, 104);
                let previous = signed_bits(&r.old, 0, 104);
                // Tracking fields in the same word can change without the
                // principal; an unchanged principal is not a holder update.
                if value == previous {
                    continue;
                }
                events.holder_basis.push(pb::HolderBasis {
                    chain_id: config.chain_id,
                    market: market.comet.clone(),
                    holder,
                    epoch: market.epoch,
                    basis_kind: pb::BasisKind::SignedPrincipal as i32,
                    value: value.to_string(),
                    previous_value: previous.to_string(),
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
                // Reviewed non-balance storage (collateral totals, allowances,
                // nonces, collateral positions, liquidator points).
            } else {
                return Err(Error::msg(format!(
                    "unresolved storage for Comet 0x{} at key 0x{}; refusing incomplete balance state",
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
                balance_asset: market.base_token.clone(),
                balance_decimals: market.base_decimals,
                basis_carryover: true,
                global_carryover: true,
                scope: pb::Scope::Epoch as i32,
                ..Default::default()
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
    events.epochs.sort_by(|a, b| (&a.market, a.epoch).cmp(&(&b.market, b.epoch)));
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
