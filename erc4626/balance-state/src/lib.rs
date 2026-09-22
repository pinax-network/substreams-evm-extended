//! One RPC-free `map_events` for explicitly qualified ERC-4626 vault epochs,
//! emitting `evm.balance_state.v1.Events`. The vault's ERC-20 `balanceOf` is
//! the share count (`HolderBasis` SHARES); the underlying claim is a
//! conversion the consumer evaluates with `conformance::erc4626` from the
//! dependency rows of the bound model:
//!
//! | model | conversion inputs carried |
//! | --- | --- |
//! | `aave-static-atoken-lm` | the Aave Pool reserve words of the vault's asset (`AAVE_LIQUIDITY_INDEX`, `AAVE_CURRENT_LIQUIDITY_RATE`, `AAVE_LAST_UPDATE_TIMESTAMP`) |
//! | `maker-savings-dai` | the Maker Pot `dsr`, `chi`, `rho` (`MAKER_POT_*`) |
//! | `oz-virtual-offset` | the asset's balance of the vault (`ERC4626_TOTAL_ASSETS`) and the qualified decimals offset |
//!
//! Every model also carries the vault's `totalSupply` (`ERC4626_TOTAL_SUPPLY`).
//! A standard interface does not imply a standard layout or `totalAssets`;
//! each vault binds to one implementation. Dependency code changes and
//! pointer writes emit INVALIDATED epochs with evidence; unresolved vault
//! writes fail the block. Slots are caller-qualified.
use evm_persist as persist;
use proto::pb::evm::balance_state::v1 as pb;
use serde::Deserialize;
use sha2::Digest;
use std::collections::{BTreeMap, BTreeSet};
use substreams::{errors::Error, scalar::BigInt};
use substreams_ethereum::pb::eth::v2 as eth;
use tiny_keccak::{Hasher, Keccak};

pub const PACKAGE: &str = "erc4626_balance_state";
pub const SPEC_REVISION: u32 = 1;
/// Producer versions whose execution ordinals are qualified (version 3 has
/// broken system-call ordinals and is refused by the contract).
pub const QUALIFIED_PRODUCER_VERSIONS: [i32; 2] = [4, 5];
const RAY: &str = "1000000000000000000000000000";

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
pub fn mapping_key(key: &[u8], base: &[u8; 32]) -> [u8; 32] {
    let mut preimage = [0u8; 64];
    preimage[32 - key.len()..32].copy_from_slice(key);
    preimage[32..].copy_from_slice(base);
    keccak(&preimage)
}
pub fn add_offset(base: &[u8; 32], offset: u8) -> [u8; 32] {
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
    pub vaults: Vec<VaultConfig>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AaveConfig {
    pub pool: String,
    pub reserves_slot: String,
    /// An Aave V3 Pool is always a proxy: its implementation pointer is
    /// required so an upgrade cannot pass unobserved.
    pub implementation_slot: String,
    pub implementation: String,
    pub atoken: String,
    /// Vault slots of `_aToken` and `_aTokenUnderlying`. They select the
    /// reserve `rate()` reads, so a write to either invalidates.
    pub atoken_slot: String,
    pub underlying_slot: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PotConfig {
    pub address: String,
    pub dsr_slot: String,
    pub chi_slot: String,
    pub rho_slot: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OzConfig {
    /// Base slot of the asset's balances mapping; `totalAssets()` is
    /// `asset.balanceOf(vault)` in the OpenZeppelin base.
    pub asset_balances_slot: String,
    pub asset_balance_model: AssetBalanceModel,
    pub asset_source_pin: String,
    /// The asset's proxy pointer. Required unless `asset_not_proxy` states
    /// that the asset has no implementation pointer; omitting both is refused
    /// so an upgradeable asset cannot be bound without its pointer.
    #[serde(default)]
    pub asset_implementation_slot: Option<String>,
    #[serde(default)]
    pub asset_implementation: Option<String>,
    #[serde(default)]
    pub asset_not_proxy: bool,
    /// ERC-7201 `openzeppelin.storage.ERC4626` slot on the vault, holding
    /// `_asset` and `_underlyingDecimals` in one word. A persisted write
    /// rebinds the asset this model converts into, so it invalidates.
    pub erc4626_storage_slot: String,
    pub decimals_offset: u8,
}
/// Only independently reviewed `balanceOf` decoders are admitted. A slot
/// number alone cannot establish the meaning of its packed storage word.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AssetBalanceModel {
    Uint256,
    #[serde(rename = "fiat-token-v2_2-low255")]
    FiatTokenV2_2Low255,
}
impl AssetBalanceModel {
    pub fn value_bits(self) -> u32 {
        match self {
            Self::Uint256 => 256,
            Self::FiatTokenV2_2Low255 => 255,
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VaultConfig {
    pub vault: String,
    pub epoch: u32,
    pub model: String,
    pub model_id: String,
    pub source_pin: String,
    pub activation_block: u64,
    /// First execution ordinal of `activation_block` at which the epoch
    /// applies; earlier writes in that block belong to the previous epoch.
    #[serde(default)]
    pub activation_ordinal: u64,
    pub asset: String,
    pub asset_decimals: u32,
    pub vault_decimals: u32,
    #[serde(default)]
    pub implementation_slot: Option<String>,
    #[serde(default)]
    pub implementation: Option<String>,
    pub balances_slot: String,
    pub total_supply_slot: String,
    #[serde(default)]
    pub other_slots: Vec<String>,
    #[serde(default)]
    pub other_mapping_slots: Vec<String>,
    /// Reviewed dynamic arrays and long strings: the root slot (length) is in
    /// `other_slots`, the data lives at `keccak256(slot) + i` and is accepted
    /// only with that verified 32-byte preimage in the block.
    #[serde(default)]
    pub other_dynamic_slots: Vec<String>,
    /// Protocol-declared revision (StaticATokenLM `STATIC__ATOKEN_LM_REVISION`).
    #[serde(default)]
    pub implementation_revision: String,
    #[serde(default)]
    pub aave: Option<AaveConfig>,
    #[serde(default)]
    pub pot: Option<PotConfig>,
    #[serde(default)]
    pub oz: Option<OzConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Model {
    AaveStaticAToken {
        pool: Vec<u8>,
        reserve_base: [u8; 32],
        implementation_slot: [u8; 32],
        implementation: Vec<u8>,
        atoken: Vec<u8>,
        atoken_slot: [u8; 32],
        underlying_slot: [u8; 32],
    },
    MakerSavingsDai {
        pot: Vec<u8>,
        dsr_slot: [u8; 32],
        chi_slot: [u8; 32],
        rho_slot: [u8; 32],
    },
    OzVirtualOffset {
        asset_balance_key: [u8; 32],
        asset_balance_model: AssetBalanceModel,
        asset_source_pin: String,
        asset_implementation_slot: Option<[u8; 32]>,
        asset_implementation: Option<Vec<u8>>,
        erc4626_storage_slot: [u8; 32],
        decimals_offset: u8,
    },
}
#[derive(Clone, Debug)]
pub struct Vault {
    pub vault: Vec<u8>,
    pub epoch: u32,
    pub model: Model,
    pub model_id: String,
    pub source_pin: String,
    pub activation_block: u64,
    pub activation_ordinal: u64,
    pub asset: Vec<u8>,
    pub asset_decimals: u32,
    pub vault_decimals: u32,
    pub implementation_slot: Option<[u8; 32]>,
    pub implementation: Option<Vec<u8>>,
    pub balances_slot: [u8; 32],
    pub total_supply_slot: [u8; 32],
    pub other_slots: Vec<[u8; 32]>,
    pub other_mapping_slots: Vec<[u8; 32]>,
    pub other_dynamic_slots: Vec<[u8; 32]>,
    pub implementation_revision: String,
}
impl Vault {
    /// Whether this epoch applies to an effect at `(block, ordinal)`.
    pub fn active_at(&self, block: u64, ordinal: u64) -> bool {
        block > self.activation_block || (block == self.activation_block && ordinal >= self.activation_ordinal)
    }
    fn family(&self) -> pb::ModelFamily {
        pb::ModelFamily::Erc4626Vault
    }
    /// All emitted STORAGE_POINTER edges share the same any-write invariant.
    fn pointer_reason(&self, address: &[u8], key: &[u8; 32]) -> Option<pb::InvalidationReason> {
        if address == self.vault && Some(*key) == self.implementation_slot {
            return Some(pb::InvalidationReason::ImplementationPointerWrite);
        }
        let dependency_pointer = match &self.model {
            Model::AaveStaticAToken {
                pool,
                implementation_slot,
                atoken_slot,
                underlying_slot,
                ..
            } => (address == pool && key == implementation_slot) || (address == self.vault && (key == atoken_slot || key == underlying_slot)),
            Model::OzVirtualOffset {
                asset_implementation_slot,
                erc4626_storage_slot,
                ..
            } => {
                (address == self.asset && Some(*key) == *asset_implementation_slot)
                    // `_asset` and `_underlyingDecimals` share this word; a
                    // write rebinds what the vault converts into.
                    || (address == self.vault && key == erc4626_storage_slot)
            }
            Model::MakerSavingsDai { .. } => false,
        };
        dependency_pointer.then_some(pb::InvalidationReason::DependencyPointerWrite)
    }
    /// Denominator constant of the shares -> assets conversion ("" for a
    /// share ratio).
    fn basis_scale(&self) -> &'static str {
        match self.model {
            Model::AaveStaticAToken { .. } | Model::MakerSavingsDai { .. } => RAY,
            Model::OzVirtualOffset { .. } => "",
        }
    }
    /// Contracts whose code changes invalidate the epoch besides the vault.
    fn dependencies(&self) -> Vec<Vec<u8>> {
        let mut out = vec![self.asset.clone()];
        out.extend(self.implementation.clone());
        match &self.model {
            Model::AaveStaticAToken {
                pool, implementation, atoken, ..
            } => {
                out.push(pool.clone());
                out.push(implementation.clone());
                out.push(atoken.clone());
            }
            Model::MakerSavingsDai { pot, .. } => out.push(pot.clone()),
            Model::OzVirtualOffset { asset_implementation, .. } => out.extend(asset_implementation.clone()),
        }
        out
    }
}
#[derive(Clone, Debug)]
pub struct Config {
    pub chain_id: u64,
    pub producer_versions: Vec<i32>,
    pub heartbeat_blocks: u64,
    pub vaults: Vec<Vault>,
    pub parameters_sha256: String,
}

pub fn parse(params: &str) -> Result<Config, Error> {
    let raw: Params = serde_json::from_str(params).map_err(|e| Error::msg(format!("invalid erc4626 balance-state params: {e}")))?;
    require(raw.chain_id > 0, "chain_id required")?;
    require(
        !raw.producer_versions.is_empty() && raw.producer_versions.iter().all(|v| QUALIFIED_PRODUCER_VERSIONS.contains(v)),
        "producer_versions must be a non-empty subset of the qualified Extended versions 4 and 5",
    )?;
    let mut vaults: Vec<Vault> = Vec::new();
    for v in &raw.vaults {
        require(v.epoch > 0 && v.activation_block > 0, "epoch and activation_block must be positive")?;
        require(!v.model_id.is_empty() && !v.source_pin.is_empty(), "model_id and source_pin required")?;
        let vault = hex_bytes(&v.vault, 20, "vault")?;
        let asset = hex_bytes(&v.asset, 20, "asset")?;
        let bound = [v.aave.is_some(), v.pot.is_some(), v.oz.is_some()].iter().filter(|b| **b).count();
        require(bound == 1, "exactly one dependency block (aave, pot or oz) must match the model")?;
        let model = match (v.model.as_str(), &v.aave, &v.pot, &v.oz) {
            ("aave-static-atoken-lm", Some(a), None, None) => Model::AaveStaticAToken {
                pool: hex_bytes(&a.pool, 20, "aave.pool")?,
                reserve_base: mapping_key(&asset, &slot(&a.reserves_slot, "aave.reserves_slot")?),
                implementation_slot: slot(&a.implementation_slot, "aave.implementation_slot")?,
                implementation: hex_bytes(&a.implementation, 20, "aave.implementation")?,
                atoken: hex_bytes(&a.atoken, 20, "aave.atoken")?,
                atoken_slot: slot(&a.atoken_slot, "aave.atoken_slot")?,
                underlying_slot: slot(&a.underlying_slot, "aave.underlying_slot")?,
            },
            ("maker-savings-dai", None, Some(p), None) => Model::MakerSavingsDai {
                pot: hex_bytes(&p.address, 20, "pot.address")?,
                dsr_slot: slot(&p.dsr_slot, "pot.dsr_slot")?,
                chi_slot: slot(&p.chi_slot, "pot.chi_slot")?,
                rho_slot: slot(&p.rho_slot, "pot.rho_slot")?,
            },
            ("oz-virtual-offset", None, None, Some(o)) => Model::OzVirtualOffset {
                asset_balance_key: mapping_key(&vault, &slot(&o.asset_balances_slot, "oz.asset_balances_slot")?),
                asset_balance_model: o.asset_balance_model,
                asset_source_pin: o.asset_source_pin.clone(),
                asset_implementation_slot: o
                    .asset_implementation_slot
                    .as_deref()
                    .map(|s| slot(s, "oz.asset_implementation_slot"))
                    .transpose()?,
                asset_implementation: o
                    .asset_implementation
                    .as_deref()
                    .map(|s| hex_bytes(s, 20, "oz.asset_implementation"))
                    .transpose()?,
                erc4626_storage_slot: slot(&o.erc4626_storage_slot, "oz.erc4626_storage_slot")?,
                decimals_offset: o.decimals_offset,
            },
            (model, ..) => return Err(Error::msg(format!("model `{model}` does not match its dependency block"))),
        };
        if let Model::AaveStaticAToken {
            atoken_slot, underlying_slot, ..
        } = &model
        {
            require(atoken_slot != underlying_slot, "aave vault pointer slots overlap")?;
        }
        if let Model::MakerSavingsDai {
            dsr_slot, chi_slot, rho_slot, ..
        } = &model
        {
            require(dsr_slot != chi_slot && chi_slot != rho_slot && dsr_slot != rho_slot, "pot slots overlap")?;
        }
        if let Model::OzVirtualOffset {
            asset_balance_key,
            asset_source_pin,
            asset_implementation_slot,
            asset_implementation,
            decimals_offset,
            ..
        } = &model
        {
            require(!asset_source_pin.trim().is_empty(), "oz.asset_source_pin required")?;
            require(
                asset_implementation_slot.is_some() == asset_implementation.is_some(),
                "oz asset implementation slot and address go together",
            )?;
            let not_proxy = v.oz.as_ref().is_some_and(|o| o.asset_not_proxy);
            require(
                asset_implementation_slot.is_some() != not_proxy,
                "oz asset needs its implementation pointer, or asset_not_proxy when it has none",
            )?;
            require(
                !(not_proxy && matches!(o_model(&model), Some(AssetBalanceModel::FiatTokenV2_2Low255))),
                "the FiatToken asset model is a proxy and needs its implementation pointer",
            )?;
            require(asset_implementation_slot.as_ref() != Some(asset_balance_key), "oz asset slots overlap")?;
            require(asset != vault, "oz asset must differ from vault")?;
            require(*decimals_offset <= 77, "oz decimals_offset overflows uint256 virtual shares")?;
            require(
                v.asset_decimals <= 255 && v.vault_decimals <= 255 && v.asset_decimals.checked_add(u32::from(*decimals_offset)) == Some(v.vault_decimals),
                "oz vault_decimals must equal asset_decimals plus decimals_offset within uint8",
            )?;
        }
        require(
            v.implementation_slot.is_some() == v.implementation.is_some(),
            "vault implementation slot and address go together",
        )?;
        let out = Vault {
            vault,
            epoch: v.epoch,
            model,
            model_id: v.model_id.clone(),
            source_pin: v.source_pin.clone(),
            activation_block: v.activation_block,
            activation_ordinal: v.activation_ordinal,
            asset,
            asset_decimals: v.asset_decimals,
            vault_decimals: v.vault_decimals,
            implementation_slot: v.implementation_slot.as_deref().map(|s| slot(s, "implementation_slot")).transpose()?,
            implementation: v.implementation.as_deref().map(|s| hex_bytes(s, 20, "implementation")).transpose()?,
            balances_slot: slot(&v.balances_slot, "balances_slot")?,
            total_supply_slot: slot(&v.total_supply_slot, "total_supply_slot")?,
            other_slots: v.other_slots.iter().map(|s| slot(s, "other_slots")).collect::<Result<_, _>>()?,
            other_mapping_slots: v.other_mapping_slots.iter().map(|s| slot(s, "other_mapping_slots")).collect::<Result<_, _>>()?,
            other_dynamic_slots: v.other_dynamic_slots.iter().map(|s| slot(s, "other_dynamic_slots")).collect::<Result<_, _>>()?,
            implementation_revision: v.implementation_revision.clone(),
        };
        // A dynamic area's root (its length or short string) is a reviewed slot.
        require(
            out.other_dynamic_slots.iter().all(|s| out.other_slots.contains(s)),
            "every other_dynamic_slots root must also be listed in other_slots",
        )?;
        let mut all = vec![out.balances_slot, out.total_supply_slot];
        all.extend(out.implementation_slot);
        all.extend(out.other_slots.iter().copied());
        all.extend(out.other_mapping_slots.iter().copied());
        match &out.model {
            Model::OzVirtualOffset { erc4626_storage_slot, .. } => all.push(*erc4626_storage_slot),
            Model::AaveStaticAToken {
                atoken_slot, underlying_slot, ..
            } => all.extend([*atoken_slot, *underlying_slot]),
            Model::MakerSavingsDai { .. } => {}
        }
        require(all.iter().collect::<BTreeSet<_>>().len() == all.len(), "vault slots overlap")?;
        require(vaults.iter().all(|o| o.vault != out.vault), "duplicate vault")?;
        vaults.push(out);
    }
    Ok(Config {
        chain_id: raw.chain_id,
        producer_versions: raw.producer_versions,
        heartbeat_blocks: raw.heartbeat_blocks,
        vaults,
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
    noop_writes: Vec<Change>,
    codes: Vec<CodeChanged>,
    errors: usize,
}
impl Collected {
    fn storage_change(&mut self, c: &eth::StorageChange, ctx: persist::Ctx, noop: bool) {
        match (word(&c.key), word(&c.old_value), word(&c.new_value)) {
            (Ok(key), Ok(old), Ok(new)) => {
                let destination = if noop { &mut self.noop_writes } else { &mut self.writes };
                destination.push(Change {
                    address: c.address.clone(),
                    key,
                    old,
                    new,
                    ordinal: c.ordinal,
                    scope: ctx.scope,
                    tx_hash: ctx.tx_hash.to_vec(),
                    tx_index: ctx.tx_index,
                    call_index: ctx.call_index,
                });
            }
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
fn reduce(mut changes: Vec<Change>) -> Result<Vec<Reduced>, Error> {
    changes.sort_by_key(|w| w.ordinal);
    let mut rows: BTreeMap<(Vec<u8>, [u8; 32]), Reduced> = BTreeMap::new();
    for w in changes {
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

fn o_model(model: &Model) -> Option<AssetBalanceModel> {
    match model {
        Model::OzVirtualOffset { asset_balance_model, .. } => Some(*asset_balance_model),
        _ => None,
    }
}
/// Largest element offset accepted inside a reviewed dynamic area. Arrays and
/// strings of reviewed contracts are far shorter; an unrelated slot landing
/// this close above a Keccak output is not a realistic collision.
const DYNAMIC_AREA_WORDS: u64 = 1 << 32;
/// Whether `key` is `keccak256(base) + i` for a reviewed `base`, proven by a
/// verified 32-byte preimage recorded in the block, with `i` below the bound.
fn dynamic_member(key: &[u8; 32], preimages: &BTreeMap<[u8; 32], Vec<u8>>, bases: &[[u8; 32]]) -> bool {
    preimages.iter().any(|(start, preimage)| {
        preimage.len() == 32 && bases.iter().any(|b| b.as_slice() == preimage.as_slice()) && offset_from(key, start).is_some_and(|i| i < DYNAMIC_AREA_WORDS)
    })
}
/// `key - start` when `key >= start` and the difference fits in 64 bits.
fn offset_from(key: &[u8; 32], start: &[u8; 32]) -> Option<u64> {
    let mut diff = [0u8; 32];
    let mut borrow = 0i16;
    for i in (0..32).rev() {
        let mut d = key[i] as i16 - start[i] as i16 - borrow;
        borrow = i16::from(d < 0);
        if d < 0 {
            d += 256;
        }
        diff[i] = d as u8;
    }
    (borrow == 0 && diff[..24].iter().all(|b| *b == 0)).then(|| u64::from_be_bytes(diff[24..].try_into().unwrap()))
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

struct Field {
    field: pb::StateField,
    offset: u32,
    width: u32,
    scale: &'static str,
    key: Vec<u8>,
}
fn field_row(config: &Config, vault: &Vault, r: &Reduced, f: Field) -> pb::GlobalState {
    pb::GlobalState {
        chain_id: config.chain_id,
        market: vault.vault.clone(),
        epoch: vault.epoch,
        field: f.field as i32,
        key: f.key,
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
fn epoch_row(config: &Config, vault: &Vault, kind: pb::EpochEventKind) -> pb::ModelEpoch {
    pb::ModelEpoch {
        chain_id: config.chain_id,
        market: vault.vault.clone(),
        epoch: vault.epoch,
        kind: kind as i32,
        family: vault.family() as i32,
        model_id: vault.model_id.clone(),
        source_pin: vault.source_pin.clone(),
        implementation_revision: vault.implementation_revision.clone(),
        basis_kind: pb::BasisKind::Shares as i32,
        basis_scale: vault.basis_scale().into(),
        balance_rounding: pb::Rounding::Floor as i32,
        basis_bit_offset: 0,
        basis_bit_width: 256,
        basis_signed: false,
        implementation: vault.implementation.clone().unwrap_or_default(),
        implementation_slot: vault.implementation_slot.map(|s| s.to_vec()).unwrap_or_default(),
        activation_block: vault.activation_block,
        activation_ordinal: vault.activation_ordinal,
        // The basis is the vault's own share count (`basis_kind` SHARES); the
        // evaluated balance is the conversion into the underlying asset.
        balance_asset: vault.asset.clone(),
        balance_decimals: vault.asset_decimals,
        basis_carryover: true,
        global_carryover: true,
        scope: pb::Scope::Epoch as i32,
        ..Default::default()
    }
}
fn invalidation(config: &Config, vault: &Vault, reason: pb::InvalidationReason, r: &Change) -> pb::ModelEpoch {
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
        ..epoch_row(config, vault, pb::EpochEventKind::Invalidated)
    }
}
fn dependency(config: &Config, vault: &Vault, kind: pb::EpochEventKind, role: pb::DependencyRole, contract: &[u8]) -> pb::Dependency {
    pb::Dependency {
        chain_id: config.chain_id,
        market: vault.vault.clone(),
        epoch: vault.epoch,
        kind: kind as i32,
        role: role as i32,
        contract: contract.to_vec(),
        depth: 1,
        binding: pb::BindingKind::Declared as i32,
        activation_block: vault.activation_block,
        source_pin: vault.source_pin.clone(),
        ..Default::default()
    }
}
fn pointer(
    config: &Config,
    vault: &Vault,
    kind: pb::EpochEventKind,
    role: pb::DependencyRole,
    contract: &[u8],
    pointer_contract: &[u8],
    slot: &[u8; 32],
) -> Result<pb::Dependency, Error> {
    Ok(pb::Dependency {
        binding: pb::BindingKind::StoragePointer as i32,
        pointer_contract: pointer_contract.to_vec(),
        pointer_slot: slot.to_vec(),
        pointer_value: word(contract)?.to_vec(),
        ..dependency(config, vault, kind, role, contract)
    })
}

pub fn project(block: &eth::Block, config: &Config) -> Result<pb::Events, Error> {
    let timestamp = validate_block(block, config)?;
    let header = block.header.as_ref().unwrap();
    let active: Vec<&Vault> = config.vaults.iter().filter(|v| v.activation_block <= block.number).collect();
    let mut events = pb::Events::default();
    if !active.is_empty() {
        let mut collected = Collected::default();
        persist::collect_block(block, &mut collected)?;
        require(collected.errors == 0, "malformed persisted storage change")?;
        let mut preimages = BTreeMap::new();
        for call in block.system_calls.iter().chain(block.transaction_traces.iter().flat_map(|tx| &tx.calls)) {
            if !active
                .iter()
                .any(|v| call.address == v.vault || call.storage_changes.iter().any(|c| c.address == v.vault))
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
        // Each epoch owns only the effects at or after its activation position.
        // Dependencies (a Pool, a Pot, an asset) can be shared by vaults with
        // different activation positions, so writes are selected per vault.
        for vault in &active {
            let dependencies = vault.dependencies();
            let owns = |w: &Change| (w.address == vault.vault || dependencies.contains(&w.address)) && vault.active_at(block.number, w.ordinal);
            let mut writes: Vec<Change> = collected.writes.iter().filter(|w| owns(w)).cloned().collect();
            // STORAGE_POINTER contract: any persisted write to a pointer slot
            // invalidates, including a write back to the same value.
            writes.extend(
                collected
                    .noop_writes
                    .iter()
                    .filter(|w| owns(w) && vault.pointer_reason(&w.address, &w.key).is_some())
                    .cloned(),
            );
            let reduced = reduce(writes.clone())?;
            // Keep each implementation transition as evidence, including an
            // upgrade followed by restoration before this block ends. Reducing
            // first would preserve the invalidation but erase the changed target.
            for w in &writes {
                if let Some(reason) = vault.pointer_reason(&w.address, &w.key) {
                    events.epochs.push(invalidation(config, vault, reason, w));
                }
            }
            for r in reduced {
                if r.address == vault.vault {
                    if vault.pointer_reason(&r.address, &r.key).is_some() {
                        // Every pointer write was invalidated before reduction.
                    } else if r.key == vault.total_supply_slot {
                        events.global_state.push(field_row(
                            config,
                            vault,
                            &r,
                            Field {
                                field: pb::StateField::Erc4626TotalSupply,
                                offset: 0,
                                width: 256,
                                scale: "1",
                                key: Vec::new(),
                            },
                        ));
                    } else if let Some(holder) = preimages
                        .get(&r.key)
                        .filter(|p| p.len() == 64 && p[..12] == [0; 12] && p[32..] == vault.balances_slot)
                        .map(|p| p[12..32].to_vec())
                    {
                        events.holder_basis.push(pb::HolderBasis {
                            chain_id: config.chain_id,
                            market: vault.vault.clone(),
                            holder,
                            epoch: vault.epoch,
                            basis_kind: pb::BasisKind::Shares as i32,
                            value: bits(&r.new, 0, 256).to_string(),
                            previous_value: bits(&r.old, 0, 256).to_string(),
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
                    } else if vault.other_slots.contains(&r.key)
                        || vault.other_mapping_slots.iter().any(|base| mapping_has_base(r.key, &preimages, base))
                        || dynamic_member(&r.key, &preimages, &vault.other_dynamic_slots)
                    {
                        // Reviewed non-balance storage: metadata, allowances,
                        // permit nonces, reward bookkeeping, initializer state.
                    } else {
                        return Err(Error::msg(format!(
                            "unresolved storage for vault 0x{} at key 0x{}; refusing incomplete balance state",
                            hex::encode(&r.address),
                            hex::encode(r.key)
                        )));
                    }
                } else {
                    match &vault.model {
                        Model::AaveStaticAToken {
                            pool,
                            reserve_base,
                            implementation_slot,
                            ..
                        } if r.address == *pool => {
                            if r.key == *implementation_slot {
                                // Every pointer write was invalidated before reduction.
                            } else if r.key == add_offset(reserve_base, 1) {
                                for (field, offset, width) in [
                                    (pb::StateField::AaveLiquidityIndex, 0, 128),
                                    (pb::StateField::AaveCurrentLiquidityRate, 128, 128),
                                ] {
                                    events.global_state.push(field_row(
                                        config,
                                        vault,
                                        &r,
                                        Field {
                                            field,
                                            offset,
                                            width,
                                            scale: RAY,
                                            key: vault.asset.clone(),
                                        },
                                    ));
                                }
                            } else if r.key == add_offset(reserve_base, 3) {
                                events.global_state.push(field_row(
                                    config,
                                    vault,
                                    &r,
                                    Field {
                                        field: pb::StateField::AaveLastUpdateTimestamp,
                                        offset: 128,
                                        width: 40,
                                        scale: "1",
                                        key: vault.asset.clone(),
                                    },
                                ));
                            }
                            // Other Pool storage (other reserves, configuration) is not a conversion input.
                        }
                        Model::MakerSavingsDai {
                            pot,
                            dsr_slot,
                            chi_slot,
                            rho_slot,
                        } if r.address == *pot => {
                            let field = if r.key == *dsr_slot {
                                Some((pb::StateField::MakerPotDsr, RAY))
                            } else if r.key == *chi_slot {
                                Some((pb::StateField::MakerPotChi, RAY))
                            } else if r.key == *rho_slot {
                                Some((pb::StateField::MakerPotRho, "1"))
                            } else {
                                None
                            };
                            if let Some((field, scale)) = field {
                                events.global_state.push(field_row(
                                    config,
                                    vault,
                                    &r,
                                    Field {
                                        field,
                                        offset: 0,
                                        width: 256,
                                        scale,
                                        key: Vec::new(),
                                    },
                                ));
                            }
                        }
                        Model::OzVirtualOffset {
                            asset_balance_key,
                            asset_balance_model,
                            asset_implementation_slot,
                            ..
                        } if r.address == vault.asset => {
                            if Some(r.key) == *asset_implementation_slot {
                                // Each transition was invalidated before reduction.
                            } else if r.key == *asset_balance_key {
                                events.global_state.push(field_row(
                                    config,
                                    vault,
                                    &r,
                                    Field {
                                        field: pb::StateField::Erc4626TotalAssets,
                                        offset: 0,
                                        width: asset_balance_model.value_bits(),
                                        scale: "1",
                                        key: vault.vault.clone(),
                                    },
                                ));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        for c in &collected.codes {
            for vault in active.iter().filter(|v| v.active_at(block.number, c.ordinal)) {
                let reason = if c.address == vault.vault || vault.implementation.as_deref() == Some(c.address.as_slice()) {
                    pb::InvalidationReason::CodeChange
                } else if vault.dependencies().contains(&c.address) {
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
                    ..epoch_row(config, vault, pb::EpochEventKind::Invalidated)
                });
            }
        }
        for vault in &active {
            let kind = if block.number == vault.activation_block {
                pb::EpochEventKind::Bound
            } else if config.heartbeat_blocks > 0 && (block.number - vault.activation_block) % config.heartbeat_blocks == 0 {
                pb::EpochEventKind::Reaffirmed
            } else {
                continue;
            };
            events.epochs.push(pb::ModelEpoch {
                // A BOUND row applies from its activation position, after any
                // invalidation of the previous epoch earlier in the block.
                ordinal: if kind == pb::EpochEventKind::Bound { vault.activation_ordinal } else { 0 },
                ..epoch_row(config, vault, kind)
            });
            if let (Some(implementation), Some(slot)) = (&vault.implementation, vault.implementation_slot) {
                events.dependencies.push(pointer(
                    config,
                    vault,
                    kind,
                    pb::DependencyRole::Implementation,
                    implementation,
                    &vault.vault,
                    &slot,
                )?);
            }
            match &vault.model {
                Model::AaveStaticAToken {
                    pool,
                    implementation_slot,
                    implementation,
                    atoken,
                    atoken_slot,
                    underlying_slot,
                    ..
                } => {
                    // `_aTokenUnderlying` and `_aToken` are vault storage pointers.
                    events.dependencies.push(pointer(
                        config,
                        vault,
                        kind,
                        pb::DependencyRole::Underlying,
                        &vault.asset,
                        &vault.vault,
                        underlying_slot,
                    )?);
                    events.dependencies.push(pointer(
                        config,
                        vault,
                        kind,
                        pb::DependencyRole::WrappedAsset,
                        atoken,
                        &vault.vault,
                        atoken_slot,
                    )?);
                    events.dependencies.push(pb::Dependency {
                        depth: 2,
                        parent: pool.clone(),
                        ..pointer(
                            config,
                            vault,
                            kind,
                            pb::DependencyRole::Implementation,
                            implementation,
                            pool,
                            implementation_slot,
                        )?
                    });
                    events.dependencies.push(dependency(config, vault, kind, pb::DependencyRole::Pool, pool));
                }
                Model::MakerSavingsDai { pot, .. } => {
                    // SavingsDai binds `dai` as an immutable: declared, with code-change invalidation.
                    events
                        .dependencies
                        .push(dependency(config, vault, kind, pb::DependencyRole::Underlying, &vault.asset));
                    events
                        .dependencies
                        .push(dependency(config, vault, kind, pb::DependencyRole::RateAccumulator, pot));
                }
                Model::OzVirtualOffset {
                    decimals_offset,
                    asset_source_pin,
                    asset_implementation_slot,
                    asset_implementation,
                    erc4626_storage_slot,
                    ..
                } => {
                    // `ERC4626Storage` packs `_asset` (bits 0..160) and
                    // `_underlyingDecimals` (bits 160..168) in one vault word.
                    let mut expected = word(&vault.asset)?;
                    expected[11] = u8::try_from(vault.asset_decimals).map_err(|_| Error::msg("asset_decimals exceeds uint8"))?;
                    events.dependencies.push(pb::Dependency {
                        binding: pb::BindingKind::StoragePointer as i32,
                        pointer_contract: vault.vault.clone(),
                        pointer_slot: erc4626_storage_slot.to_vec(),
                        pointer_value: expected.to_vec(),
                        source_pin: asset_source_pin.clone(),
                        ..dependency(config, vault, kind, pb::DependencyRole::Underlying, &vault.asset)
                    });
                    if let (Some(implementation), Some(slot)) = (asset_implementation, asset_implementation_slot) {
                        events.dependencies.push(pb::Dependency {
                            depth: 2,
                            parent: vault.asset.clone(),
                            source_pin: asset_source_pin.clone(),
                            ..pointer(config, vault, kind, pb::DependencyRole::Implementation, implementation, &vault.asset, slot)?
                        });
                    }
                    events.global_state.push(pb::GlobalState {
                        chain_id: config.chain_id,
                        market: vault.vault.clone(),
                        epoch: vault.epoch,
                        field: pb::StateField::Erc4626DecimalsOffset as i32,
                        value: decimals_offset.to_string(),
                        scale: "1".into(),
                        observation: pb::Observation::QualifiedConstant as i32,
                        boundary: pb::Boundary::Declaration as i32,
                        scope: pb::Scope::Epoch as i32,
                        // A declaration sits at the epoch boundary and names the
                        // implementation whose code binds the constant.
                        ordinal: if kind == pb::EpochEventKind::Bound { vault.activation_ordinal } else { 0 },
                        storage_contract: vault.implementation.clone().unwrap_or_else(|| vault.vault.clone()),
                        ..Default::default()
                    });
                }
            }
        }
    }
    events
        .holder_basis
        .sort_by(|a, b| (&a.market, &a.holder, a.ordinal).cmp(&(&b.market, &b.holder, b.ordinal)));
    events
        .global_state
        .sort_by(|a, b| (&a.market, a.field, &a.key, a.ordinal).cmp(&(&b.market, b.field, &b.key, b.ordinal)));
    events
        .epochs
        .sort_by(|a, b| (&a.market, a.epoch, a.ordinal, a.kind).cmp(&(&b.market, b.epoch, b.ordinal, b.kind)));
    events
        .dependencies
        .sort_by(|a, b| (&a.market, a.epoch, a.depth, a.role, &a.contract).cmp(&(&b.market, b.epoch, b.depth, b.role, &b.contract)));
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
