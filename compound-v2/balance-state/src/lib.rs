//! One RPC-free `map_events` for explicitly qualified Compound v2 cToken
//! epochs, emitting `evm.balance_state.v1.Events`.
//!
//! Three metrics stay distinct. The cToken ERC-20 `balanceOf` is the stored
//! share count (`accountTokens[holder]`, emitted as `HolderBasis` SHARES).
//! `exchangeRateStored` converts shares with the stored market words
//! (`GlobalState` totals, borrow index, accrual block, reserve factor, initial
//! rate) and the cross-contract cash. `balanceOfUnderlying` accrues first;
//! the map never accrues and never converts. The consumer evaluates with
//! `conformance::compound_v2` at a canonical block clock.
//!
//! Cash is `underlying.balanceOf(cToken)` for CErc20 and the cToken's native
//! balance for CEther. Both are cross-contract inputs that move without any
//! cToken write (donations, rebases, direct transfers). CErc20 cash is read
//! from the qualified underlying's balances mapping only when the parameters
//! bind that underlying model; CEther cash comes from persisted native balance
//! changes of the cToken address.
//!
//! Dependency changes emit `ModelEpoch` INVALIDATED rows with evidence instead
//! of synthetic balances: rate-model pointer writes, delegator implementation
//! pointer writes, and code changes on the cToken, its implementation, the
//! rate model or the underlying. Unresolved cToken writes fail the block.
use evm_persist as persist;
use proto::pb::evm::balance_state::v1 as pb;
use serde::Deserialize;
use sha2::Digest;
use std::collections::{BTreeMap, BTreeSet};
use substreams::{errors::Error, scalar::BigInt};
use substreams_ethereum::pb::eth::v2 as eth;
use tiny_keccak::{Hasher, Keccak};

pub const PACKAGE: &str = "compound_v2_balance_state";
pub const SPEC_REVISION: u32 = 1;
/// Producer versions whose execution ordinals are qualified (version 3 has
/// broken system-call ordinals and is refused by the contract).
pub const QUALIFIED_PRODUCER_VERSIONS: [i32; 2] = [4, 5];
/// `blocksPerYear` of BaseJumpRateModelV2 and WhitePaperInterestRateModel at the pin.
pub const BLOCKS_PER_YEAR: &str = "2102400";
const EXP_SCALE: &str = "1000000000000000000";
/// Words a mapping value struct may span (BorrowSnapshot has two).
const MAX_STRUCT_WORDS: u8 = 4;

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
pub fn mapping_key(key: &[u8], base: &[u8; 32]) -> [u8; 32] {
    let mut preimage = [0u8; 64];
    preimage[32 - key.len()..32].copy_from_slice(key);
    preimage[32..].copy_from_slice(base);
    keccak(&preimage)
}
fn sub_small(word: &[u8; 32], n: u8) -> Option<[u8; 32]> {
    let mut out = *word;
    let mut borrow = n as u16;
    for byte in out.iter_mut().rev() {
        if borrow == 0 {
            break;
        }
        let v = *byte as u16;
        if v >= borrow {
            *byte = (v - borrow) as u8;
            borrow = 0;
        } else {
            *byte = (v + 256 - borrow) as u8;
            borrow = 1;
        }
    }
    (borrow == 0).then_some(out)
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
fn unsigned(word: &[u8; 32]) -> String {
    BigInt::from_unsigned_bytes_be(word).to_string()
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
pub struct Slots {
    pub interest_rate_model: String,
    pub initial_exchange_rate_mantissa: String,
    pub reserve_factor_mantissa: String,
    pub accrual_block_number: String,
    pub borrow_index: String,
    pub total_borrows: String,
    pub total_reserves: String,
    pub total_supply: String,
    pub account_tokens: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnderlyingConfig {
    #[serde(default)]
    pub address: Option<String>,
    pub decimals: u32,
    /// "native" (CEther) or "erc20_mapping" (CErc20 with a qualified balances mapping).
    pub cash: String,
    #[serde(default)]
    pub balances_slot: Option<String>,
    /// Width in bits of the balance inside the mapping value word (default 256).
    /// FiatToken V2.2 keeps the blacklist flag in bit 255 and `_balanceOf`
    /// masks it, so USDC uses 255.
    #[serde(default)]
    pub value_bits: Option<u32>,
    #[serde(default)]
    pub implementation_slot: Option<String>,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub source_pin: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateModelConfig {
    pub address: String,
    pub model_id: String,
    pub source_pin: String,
    #[serde(default)]
    pub slots: BTreeMap<String, String>,
    #[serde(default)]
    pub constants: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketConfig {
    pub ctoken: String,
    /// "cerc20", "cether" or "cerc20_delegator".
    pub kind: String,
    pub ctoken_decimals: u32,
    pub epoch: u32,
    pub model_id: String,
    pub source_pin: String,
    pub activation_block: u64,
    /// First execution ordinal of `activation_block` at which the epoch
    /// applies; earlier writes in that block belong to the previous epoch.
    #[serde(default)]
    pub activation_ordinal: u64,
    #[serde(default)]
    pub implementation_slot: Option<String>,
    #[serde(default)]
    pub implementation: Option<String>,
    pub slots: Slots,
    #[serde(default)]
    pub other_slots: Vec<String>,
    #[serde(default)]
    pub other_mapping_slots: Vec<String>,
    pub underlying: UnderlyingConfig,
    pub rate_model: RateModelConfig,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cash {
    Native,
    Erc20Mapping {
        balances_slot: [u8; 32],
        implementation_slot: Option<[u8; 32]>,
        /// Bits `[0, value_bits)` of the mapping value word are the balance.
        value_bits: u32,
    },
}
#[derive(Clone, Debug)]
pub struct Market {
    pub ctoken: Vec<u8>,
    pub ctoken_decimals: u32,
    pub epoch: u32,
    pub model_id: String,
    pub source_pin: String,
    pub activation_block: u64,
    pub activation_ordinal: u64,
    pub implementation_slot: Option<[u8; 32]>,
    pub implementation: Option<Vec<u8>>,
    /// (slot, field, scale) scalar words of the cToken.
    pub scalars: Vec<([u8; 32], pb::StateField, &'static str)>,
    pub rate_model_slot: [u8; 32],
    pub account_tokens_slot: [u8; 32],
    pub other_slots: Vec<[u8; 32]>,
    pub other_mapping_slots: Vec<[u8; 32]>,
    pub underlying: Option<Vec<u8>>,
    pub underlying_decimals: u32,
    pub underlying_pin: String,
    pub cash: Cash,
    pub rate_model: Vec<u8>,
    pub rate_model_pin: String,
    pub rate_model_slots: Vec<([u8; 32], pb::StateField)>,
    pub rate_model_constants: Vec<(pb::StateField, String, &'static str)>,
}
#[derive(Clone, Debug)]
pub struct Config {
    pub chain_id: u64,
    pub producer_versions: Vec<i32>,
    pub heartbeat_blocks: u64,
    pub markets: Vec<Market>,
    pub parameters_sha256: String,
}

fn irm_field(name: &str) -> Result<(pb::StateField, &'static str), Error> {
    Ok(match name {
        "base_rate_per_block" => (pb::StateField::CompoundV2IrmBaseRatePerBlock, EXP_SCALE),
        "multiplier_per_block" => (pb::StateField::CompoundV2IrmMultiplierPerBlock, EXP_SCALE),
        "jump_multiplier_per_block" => (pb::StateField::CompoundV2IrmJumpMultiplierPerBlock, EXP_SCALE),
        "kink" => (pb::StateField::CompoundV2IrmKink, EXP_SCALE),
        "blocks_per_year" => (pb::StateField::CompoundV2IrmBlocksPerYear, "1"),
        other => return Err(Error::msg(format!("unknown rate model parameter `{other}`"))),
    })
}

impl Market {
    /// Whether this epoch applies to an effect at `(block, ordinal)`.
    pub fn active_at(&self, block: u64, ordinal: u64) -> bool {
        block > self.activation_block || (block == self.activation_block && ordinal >= self.activation_ordinal)
    }
    fn watches(&self, address: &[u8]) -> bool {
        address == self.ctoken || address == self.rate_model || self.implementation.as_deref() == Some(address) || self.underlying.as_deref() == Some(address)
    }
    /// STORAGE_POINTER slots: any persisted write, including a write back to
    /// the same value, invalidates the epoch.
    fn pointer_reason(&self, address: &[u8], key: &[u8; 32]) -> Option<pb::InvalidationReason> {
        if address == self.ctoken && Some(*key) == self.implementation_slot {
            Some(pb::InvalidationReason::ImplementationPointerWrite)
        } else if address == self.ctoken && *key == self.rate_model_slot {
            Some(pb::InvalidationReason::RateModelChange)
        } else if let Cash::Erc20Mapping {
            implementation_slot: Some(slot),
            ..
        } = &self.cash
        {
            (self.underlying.as_deref() == Some(address) && key == slot).then_some(pb::InvalidationReason::DependencyPointerWrite)
        } else {
            None
        }
    }
}

pub fn parse(params: &str) -> Result<Config, Error> {
    let raw: Params = serde_json::from_str(params).map_err(|e| Error::msg(format!("invalid compound-v2 balance-state params: {e}")))?;
    require(raw.chain_id > 0, "chain_id required")?;
    require(
        !raw.producer_versions.is_empty() && raw.producer_versions.iter().all(|v| QUALIFIED_PRODUCER_VERSIONS.contains(v)),
        "producer_versions must be a non-empty subset of the qualified Extended versions 4 and 5",
    )?;
    let mut markets: Vec<Market> = Vec::new();
    for m in &raw.markets {
        require(m.epoch > 0 && m.activation_block > 0, "epoch and activation_block must be positive")?;
        require(!m.model_id.is_empty() && !m.source_pin.is_empty(), "model_id and source_pin required")?;
        let ctoken = hex_bytes(&m.ctoken, 20, "ctoken")?;
        let (implementation_slot, implementation) = match m.kind.as_str() {
            "cerc20" | "cether" => {
                require(
                    m.implementation_slot.is_none() && m.implementation.is_none(),
                    "non-delegator markets take no implementation",
                )?;
                (None, None)
            }
            "cerc20_delegator" => (
                Some(slot(
                    m.implementation_slot
                        .as_deref()
                        .ok_or_else(|| Error::msg("delegator requires implementation_slot"))?,
                    "implementation_slot",
                )?),
                Some(hex_bytes(
                    m.implementation.as_deref().ok_or_else(|| Error::msg("delegator requires implementation"))?,
                    20,
                    "implementation",
                )?),
            ),
            other => return Err(Error::msg(format!("unknown market kind `{other}`"))),
        };
        let u = &m.underlying;
        let (underlying, cash) = match (m.kind.as_str(), u.cash.as_str()) {
            ("cether", "native") => {
                require(
                    u.address.is_none() && u.balances_slot.is_none() && u.implementation_slot.is_none() && u.value_bits.is_none(),
                    "native cash takes no underlying contract",
                )?;
                (None, Cash::Native)
            }
            ("cerc20" | "cerc20_delegator", "erc20_mapping") => {
                let address = hex_bytes(
                    u.address.as_deref().ok_or_else(|| Error::msg("erc20 cash requires the underlying address"))?,
                    20,
                    "underlying.address",
                )?;
                require(
                    u.model_id.as_deref().is_some_and(|s| !s.is_empty()) && u.source_pin.as_deref().is_some_and(|s| !s.is_empty()),
                    "erc20 cash requires a qualified underlying model_id and source_pin",
                )?;
                let balances_slot = slot(
                    u.balances_slot.as_deref().ok_or_else(|| Error::msg("erc20 cash requires balances_slot"))?,
                    "balances_slot",
                )?;
                let implementation_slot = u
                    .implementation_slot
                    .as_deref()
                    .map(|s| slot(s, "underlying.implementation_slot"))
                    .transpose()?;
                let value_bits = u.value_bits.unwrap_or(256);
                require((1..=256).contains(&value_bits), "underlying.value_bits must be within 1..=256")?;
                (
                    Some(address),
                    Cash::Erc20Mapping {
                        balances_slot,
                        implementation_slot,
                        value_bits,
                    },
                )
            }
            _ => return Err(Error::msg("cash kind does not match market kind")),
        };
        let s = &m.slots;
        let scalars = vec![
            (
                slot(&s.initial_exchange_rate_mantissa, "initial_exchange_rate_mantissa")?,
                pb::StateField::CompoundV2InitialExchangeRateMantissa,
                EXP_SCALE,
            ),
            (
                slot(&s.reserve_factor_mantissa, "reserve_factor_mantissa")?,
                pb::StateField::CompoundV2ReserveFactorMantissa,
                EXP_SCALE,
            ),
            (
                slot(&s.accrual_block_number, "accrual_block_number")?,
                pb::StateField::CompoundV2AccrualBlockNumber,
                "1",
            ),
            (slot(&s.borrow_index, "borrow_index")?, pb::StateField::CompoundV2BorrowIndex, EXP_SCALE),
            (slot(&s.total_borrows, "total_borrows")?, pb::StateField::CompoundV2TotalBorrows, "1"),
            (slot(&s.total_reserves, "total_reserves")?, pb::StateField::CompoundV2TotalReserves, "1"),
            (slot(&s.total_supply, "total_supply")?, pb::StateField::CompoundV2TotalSupply, "1"),
        ];
        let rate_model_slot = slot(&s.interest_rate_model, "interest_rate_model")?;
        let account_tokens_slot = slot(&s.account_tokens, "account_tokens")?;
        let other_slots: Vec<[u8; 32]> = m.other_slots.iter().map(|s| slot(s, "other_slots")).collect::<Result<_, _>>()?;
        let other_mapping_slots: Vec<[u8; 32]> = m.other_mapping_slots.iter().map(|s| slot(s, "other_mapping_slots")).collect::<Result<_, _>>()?;
        let mut all: Vec<[u8; 32]> = scalars.iter().map(|(k, _, _)| *k).collect();
        all.push(rate_model_slot);
        all.push(account_tokens_slot);
        all.extend(implementation_slot);
        all.extend(other_slots.iter().copied());
        all.extend(other_mapping_slots.iter().copied());
        require(all.iter().collect::<BTreeSet<_>>().len() == all.len(), "market slots overlap")?;
        let r = &m.rate_model;
        require(
            !r.model_id.is_empty() && !r.source_pin.is_empty(),
            "rate model model_id and source_pin required",
        )?;
        let mut rate_model_slots = Vec::new();
        for (name, s) in &r.slots {
            let (field, _) = irm_field(name)?;
            require(field != pb::StateField::CompoundV2IrmBlocksPerYear, "blocks_per_year is a constant, not a slot")?;
            rate_model_slots.push((slot(s, name)?, field));
        }
        require(
            rate_model_slots.iter().map(|(k, _)| k).collect::<BTreeSet<_>>().len() == rate_model_slots.len(),
            "rate model slots overlap",
        )?;
        let mut rate_model_constants = Vec::new();
        for (name, v) in &r.constants {
            let (field, scale) = irm_field(name)?;
            require(!r.slots.contains_key(name), "rate model parameter bound as both slot and constant")?;
            rate_model_constants.push((field, decimal(v, name)?, scale));
        }
        require(
            r.constants.get("blocks_per_year").map(String::as_str) == Some(BLOCKS_PER_YEAR),
            "rate model blocks_per_year constant must equal the pinned 2102400",
        )?;
        let market = Market {
            ctoken,
            ctoken_decimals: m.ctoken_decimals,
            epoch: m.epoch,
            model_id: m.model_id.clone(),
            source_pin: m.source_pin.clone(),
            activation_block: m.activation_block,
            activation_ordinal: m.activation_ordinal,
            implementation_slot,
            implementation,
            scalars,
            rate_model_slot,
            account_tokens_slot,
            other_slots,
            other_mapping_slots,
            underlying,
            underlying_decimals: u.decimals,
            underlying_pin: format!("{} {}", u.model_id.clone().unwrap_or_default(), u.source_pin.clone().unwrap_or_default())
                .trim()
                .to_string(),
            cash,
            rate_model: hex_bytes(&r.address, 20, "rate_model.address")?,
            rate_model_pin: format!("{} {}", r.model_id, r.source_pin),
            rate_model_slots,
            rate_model_constants,
        };
        require(markets.iter().all(|o| o.ctoken != market.ctoken), "duplicate market")?;
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
// Persisted effects
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Change {
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
    writes: Vec<Change>,
    /// Equal-value writes; only pointer slots consume them.
    noops: Vec<Change>,
    balances: Vec<Change>,
    codes: Vec<CodeChanged>,
    errors: usize,
}
fn change(address: &[u8], key: [u8; 32], old: &[u8], new: &[u8], ordinal: u64, ctx: persist::Ctx) -> Option<Change> {
    Some(Change {
        address: address.to_vec(),
        key,
        old: word(old).ok()?,
        new: word(new).ok()?,
        ordinal,
        scope: ctx.scope,
        tx_hash: ctx.tx_hash.to_vec(),
        tx_index: ctx.tx_index,
        call_index: ctx.call_index,
    })
}
impl persist::Sink for Collected {
    fn storage(&mut self, c: &eth::StorageChange, ctx: persist::Ctx) {
        match word(&c.key)
            .ok()
            .and_then(|key| change(&c.address, key, &c.old_value, &c.new_value, c.ordinal, ctx))
        {
            Some(w) => self.writes.push(w),
            None => self.errors += 1,
        }
    }
    fn storage_noop(&mut self, c: &eth::StorageChange, ctx: persist::Ctx) {
        match word(&c.key)
            .ok()
            .and_then(|key| change(&c.address, key, &c.old_value, &c.new_value, c.ordinal, ctx))
        {
            Some(w) => self.noops.push(w),
            None => self.errors += 1,
        }
    }
    fn balance(&mut self, c: &eth::BalanceChange, ctx: persist::Ctx) {
        let old = c.old_value.as_ref().map(|v| v.bytes.clone()).unwrap_or_default();
        let new = c.new_value.as_ref().map(|v| v.bytes.clone()).unwrap_or_default();
        match change(&c.address, [0; 32], &old, &new, c.ordinal, ctx) {
            Some(b) => self.balances.push(b),
            None => self.errors += 1,
        }
    }
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
fn reduce(mut changes: Vec<Change>, what: &str) -> Result<Vec<Reduced>, Error> {
    changes.sort_by_key(|w| w.ordinal);
    let mut rows: BTreeMap<(Vec<u8>, [u8; 32]), Reduced> = BTreeMap::new();
    for w in changes {
        let at = || format!("0x{} key 0x{}", hex::encode(&w.address), hex::encode(w.key));
        require(w.ordinal > 0, &format!("persisted {what} change at {} has no execution ordinal", at()))?;
        match rows.get_mut(&(w.address.clone(), w.key)) {
            Some(r) => {
                require(
                    w.ordinal > r.ordinal,
                    &format!(
                        "ambiguous {what} execution order at {}: ordinal {} repeats after {}",
                        at(),
                        w.ordinal,
                        r.ordinal
                    ),
                )?;
                require(
                    w.old == r.new,
                    &format!(
                        "discontinuous {what} changes at {}: ordinal {} starts from 0x{} but ordinal {} ended at 0x{}",
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
/// True when `key` is word `n < MAX_STRUCT_WORDS` of a value under mapping
/// `base`, through verified preimages (nested mappings chain to the base).
fn mapping_member(key: [u8; 32], preimages: &BTreeMap<[u8; 32], Vec<u8>>, base: &[u8; 32]) -> bool {
    (0..MAX_STRUCT_WORDS).filter_map(|n| sub_small(&key, n)).any(|mut k| {
        let mut visited = BTreeSet::new();
        while visited.insert(k) {
            let Some(p) = preimages.get(&k).filter(|p| p.len() == 64) else {
                return false;
            };
            let parent: [u8; 32] = p[32..].try_into().unwrap();
            if &parent == base {
                return true;
            }
            k = parent;
        }
        false
    })
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

fn global_row(config: &Config, market: &Market, r: &Reduced, field: pb::StateField, scale: &str, key: Vec<u8>) -> pb::GlobalState {
    pb::GlobalState {
        chain_id: config.chain_id,
        market: market.ctoken.clone(),
        epoch: market.epoch,
        field: field as i32,
        key,
        value: unsigned(&r.new),
        previous_value: unsigned(&r.old),
        scale: scale.into(),
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
        bit_width: 256,
        ..Default::default()
    }
}
fn epoch_row(config: &Config, market: &Market, kind: pb::EpochEventKind) -> pb::ModelEpoch {
    pb::ModelEpoch {
        chain_id: config.chain_id,
        market: market.ctoken.clone(),
        epoch: market.epoch,
        kind: kind as i32,
        family: pb::ModelFamily::CompoundV2Ctoken as i32,
        model_id: market.model_id.clone(),
        source_pin: market.source_pin.clone(),
        basis_kind: pb::BasisKind::Shares as i32,
        basis_scale: "1".into(),
        balance_rounding: pb::Rounding::Floor as i32,
        basis_bit_offset: 0,
        basis_bit_width: 256,
        basis_signed: false,
        implementation: market.implementation.clone().unwrap_or_default(),
        implementation_slot: market.implementation_slot.map(|s| s.to_vec()).unwrap_or_default(),
        activation_block: market.activation_block,
        activation_ordinal: market.activation_ordinal,
        // The ERC-20 amount is the share count of the cToken itself; the
        // underlying claim is a separate, dependency-bound conversion.
        balance_asset: market.ctoken.clone(),
        balance_decimals: market.ctoken_decimals,
        // Share storage persists across implementation upgrades; a rate-model
        // replacement starts a new epoch whose IRM rows do not carry.
        basis_carryover: true,
        global_carryover: false,
        scope: pb::Scope::Epoch as i32,
        ..Default::default()
    }
}
fn invalidation(config: &Config, market: &Market, reason: pb::InvalidationReason, r: &Change) -> pb::ModelEpoch {
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
fn code_invalidation(config: &Config, market: &Market, reason: pb::InvalidationReason, c: &CodeChanged) -> pb::ModelEpoch {
    pb::ModelEpoch {
        reason: reason as i32,
        scope: scope_of(c.scope) as i32,
        ordinal: c.ordinal,
        transaction_index: c.tx_index,
        transaction_hash: c.tx_hash.clone(),
        call_index: c.call_index,
        evidence_contract: c.address.clone(),
        evidence_code_hash: c.new_hash.clone(),
        ..epoch_row(config, market, pb::EpochEventKind::Invalidated)
    }
}
fn dependency(config: &Config, market: &Market, kind: pb::EpochEventKind, role: pb::DependencyRole, contract: &[u8], pin: &str) -> pb::Dependency {
    pb::Dependency {
        chain_id: config.chain_id,
        market: market.ctoken.clone(),
        epoch: market.epoch,
        kind: kind as i32,
        role: role as i32,
        contract: contract.to_vec(),
        depth: 1,
        binding: pb::BindingKind::Declared as i32,
        activation_block: market.activation_block,
        source_pin: pin.to_string(),
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
        require(collected.errors == 0, "malformed persisted change")?;
        let mut preimages = BTreeMap::new();
        for call in block.system_calls.iter().chain(block.transaction_traces.iter().flat_map(|tx| &tx.calls)) {
            if !active
                .iter()
                .any(|m| call.address == m.ctoken || call.storage_changes.iter().any(|c| c.address == m.ctoken))
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
        // Each epoch owns only the effects at or after its activation position,
        // so writes are selected per market before reduction.
        for market in &active {
            let owns = |w: &Change| market.watches(&w.address) && market.active_at(block.number, w.ordinal);
            let mut writes: Vec<Change> = collected.writes.iter().filter(|w| owns(w)).cloned().collect();
            writes.extend(
                collected
                    .noops
                    .iter()
                    .filter(|w| owns(w) && market.pointer_reason(&w.address, &w.key).is_some())
                    .cloned(),
            );
            // Validate continuity first, then evidence every pointer transition
            // individually: an in-block excursion X->Z->X must not be concealed.
            let reduced = reduce(writes.clone(), "storage")?;
            for w in &writes {
                if let Some(reason) = market.pointer_reason(&w.address, &w.key) {
                    events.epochs.push(invalidation(config, market, reason, w));
                }
            }
            for r in reduced {
                if market.pointer_reason(&r.address, &r.key).is_some() {
                    // Every pointer write was invalidated above.
                } else if r.address == market.ctoken {
                    if let Some((_, field, scale)) = market.scalars.iter().find(|(k, _, _)| *k == r.key) {
                        events.global_state.push(global_row(config, market, &r, *field, scale, Vec::new()));
                    } else if let Some(holder) = preimages
                        .get(&r.key)
                        .filter(|p| p.len() == 64 && p[..12] == [0; 12] && p[32..] == market.account_tokens_slot)
                        .map(|p| p[12..32].to_vec())
                    {
                        events.holder_basis.push(pb::HolderBasis {
                            chain_id: config.chain_id,
                            market: market.ctoken.clone(),
                            holder,
                            epoch: market.epoch,
                            basis_kind: pb::BasisKind::Shares as i32,
                            value: unsigned(&r.new),
                            previous_value: unsigned(&r.old),
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
                            bit_width: 256,
                            signed: false,
                        });
                    } else if market.other_slots.contains(&r.key) || market.other_mapping_slots.iter().any(|base| mapping_member(r.key, &preimages, base)) {
                        // Reviewed non-balance storage: reentrancy flag, metadata,
                        // admin, comptroller, allowances, borrow snapshots.
                    } else {
                        return Err(Error::msg(format!(
                            "unresolved storage for cToken 0x{} at key 0x{}; refusing incomplete balance state",
                            hex::encode(&r.address),
                            hex::encode(r.key)
                        )));
                    }
                } else if r.address == market.rate_model {
                    if let Some((_, field)) = market.rate_model_slots.iter().find(|(k, _)| *k == r.key) {
                        events.global_state.push(global_row(config, market, &r, *field, EXP_SCALE, Vec::new()));
                    }
                    // Other rate-model storage (owner) is not a balance input.
                } else if market.underlying.as_deref() == Some(r.address.as_slice()) {
                    if let Cash::Erc20Mapping { balances_slot, value_bits, .. } = &market.cash {
                        if r.key == mapping_key(&market.ctoken, balances_slot) {
                            // Only the low `value_bits` are the balance (FiatToken V2.2 keeps
                            // the blacklist flag in bit 255 and `_balanceOf` masks it).
                            let mut row = global_row(config, market, &r, pb::StateField::CompoundV2TotalCash, "1", market.ctoken.clone());
                            row.value = bits(&r.new, 0, *value_bits).to_string();
                            row.previous_value = bits(&r.old, 0, *value_bits).to_string();
                            row.bit_width = *value_bits;
                            events.global_state.push(row);
                        }
                        // Every other underlying write (other holders, supply,
                        // allowances) is that token's own state.
                    }
                }
            }
        }
        let balances: Vec<Change> = collected
            .balances
            .into_iter()
            .filter(|b| {
                active
                    .iter()
                    .any(|m| m.cash == Cash::Native && b.address == m.ctoken && m.active_at(block.number, b.ordinal))
            })
            .collect();
        for r in reduce(balances, "native balance")? {
            let market = active.iter().find(|m| m.ctoken == r.address).unwrap();
            events
                .global_state
                .push(global_row(config, market, &r, pb::StateField::CompoundV2TotalCash, "1", market.ctoken.clone()));
        }
        for c in &collected.codes {
            for market in active.iter().filter(|m| m.active_at(block.number, c.ordinal)) {
                let reason = if c.address == market.ctoken || market.implementation.as_deref() == Some(c.address.as_slice()) {
                    pb::InvalidationReason::CodeChange
                } else if c.address == market.rate_model {
                    pb::InvalidationReason::RateModelChange
                } else if market.underlying.as_deref() == Some(c.address.as_slice()) {
                    pb::InvalidationReason::DependencyCodeChange
                } else {
                    continue;
                };
                events.epochs.push(code_invalidation(config, market, reason, c));
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
            if let (Some(implementation), Some(slot)) = (&market.implementation, market.implementation_slot) {
                events.dependencies.push(pb::Dependency {
                    binding: pb::BindingKind::StoragePointer as i32,
                    pointer_contract: market.ctoken.clone(),
                    pointer_slot: slot.to_vec(),
                    pointer_value: word(implementation)?.to_vec(),
                    ..dependency(config, market, kind, pb::DependencyRole::Implementation, implementation, &market.source_pin)
                });
            }
            // The cToken resolves its rate model through `interestRateModel`.
            events.dependencies.push(pb::Dependency {
                binding: pb::BindingKind::StoragePointer as i32,
                pointer_contract: market.ctoken.clone(),
                pointer_slot: market.rate_model_slot.to_vec(),
                pointer_value: word(&market.rate_model)?.to_vec(),
                ..dependency(
                    config,
                    market,
                    kind,
                    pb::DependencyRole::InterestRateModel,
                    &market.rate_model,
                    &market.rate_model_pin,
                )
            });
            if let Some(underlying) = &market.underlying {
                events.dependencies.push(dependency(
                    config,
                    market,
                    kind,
                    pb::DependencyRole::Underlying,
                    underlying,
                    &market.underlying_pin,
                ));
            }
            for (field, value, scale) in &market.rate_model_constants {
                events.global_state.push(pb::GlobalState {
                    chain_id: config.chain_id,
                    market: market.ctoken.clone(),
                    epoch: market.epoch,
                    field: *field as i32,
                    value: value.clone(),
                    scale: (*scale).into(),
                    observation: pb::Observation::QualifiedConstant as i32,
                    boundary: pb::Boundary::Declaration as i32,
                    scope: pb::Scope::Epoch as i32,
                    storage_contract: market.rate_model.clone(),
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
