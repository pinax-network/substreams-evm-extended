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
    #[serde(default)]
    pub implementation_slot: Option<String>,
    #[serde(default)]
    pub implementation: Option<String>,
    pub atoken: String,
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
    pub decimals_offset: u8,
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
        implementation_slot: Option<[u8; 32]>,
        implementation: Option<Vec<u8>>,
        atoken: Vec<u8>,
    },
    MakerSavingsDai {
        pot: Vec<u8>,
        dsr_slot: [u8; 32],
        chi_slot: [u8; 32],
        rho_slot: [u8; 32],
    },
    OzVirtualOffset {
        asset_balance_key: [u8; 32],
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
    pub asset: Vec<u8>,
    pub asset_decimals: u32,
    pub vault_decimals: u32,
    pub implementation_slot: Option<[u8; 32]>,
    pub implementation: Option<Vec<u8>>,
    pub balances_slot: [u8; 32],
    pub total_supply_slot: [u8; 32],
    pub other_slots: Vec<[u8; 32]>,
    pub other_mapping_slots: Vec<[u8; 32]>,
}
impl Vault {
    fn family(&self) -> pb::ModelFamily {
        pb::ModelFamily::Erc4626Vault
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
                out.extend(implementation.clone());
                out.push(atoken.clone());
            }
            Model::MakerSavingsDai { pot, .. } => out.push(pot.clone()),
            Model::OzVirtualOffset { .. } => {}
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
                implementation_slot: a.implementation_slot.as_deref().map(|s| slot(s, "aave.implementation_slot")).transpose()?,
                implementation: a.implementation.as_deref().map(|s| hex_bytes(s, 20, "aave.implementation")).transpose()?,
                atoken: hex_bytes(&a.atoken, 20, "aave.atoken")?,
            },
            ("maker-savings-dai", None, Some(p), None) => Model::MakerSavingsDai {
                pot: hex_bytes(&p.address, 20, "pot.address")?,
                dsr_slot: slot(&p.dsr_slot, "pot.dsr_slot")?,
                chi_slot: slot(&p.chi_slot, "pot.chi_slot")?,
                rho_slot: slot(&p.rho_slot, "pot.rho_slot")?,
            },
            ("oz-virtual-offset", None, None, Some(o)) => Model::OzVirtualOffset {
                asset_balance_key: mapping_key(&vault, &slot(&o.asset_balances_slot, "oz.asset_balances_slot")?),
                decimals_offset: o.decimals_offset,
            },
            (model, ..) => return Err(Error::msg(format!("model `{model}` does not match its dependency block"))),
        };
        if let Model::AaveStaticAToken {
            implementation_slot,
            implementation,
            ..
        } = &model
        {
            require(
                implementation_slot.is_some() == implementation.is_some(),
                "aave pool implementation slot and address go together",
            )?;
        }
        if let Model::MakerSavingsDai {
            dsr_slot, chi_slot, rho_slot, ..
        } = &model
        {
            require(dsr_slot != chi_slot && chi_slot != rho_slot && dsr_slot != rho_slot, "pot slots overlap")?;
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
            asset,
            asset_decimals: v.asset_decimals,
            vault_decimals: v.vault_decimals,
            implementation_slot: v.implementation_slot.as_deref().map(|s| slot(s, "implementation_slot")).transpose()?,
            implementation: v.implementation.as_deref().map(|s| hex_bytes(s, 20, "implementation")).transpose()?,
            balances_slot: slot(&v.balances_slot, "balances_slot")?,
            total_supply_slot: slot(&v.total_supply_slot, "total_supply_slot")?,
            other_slots: v.other_slots.iter().map(|s| slot(s, "other_slots")).collect::<Result<_, _>>()?,
            other_mapping_slots: v.other_mapping_slots.iter().map(|s| slot(s, "other_mapping_slots")).collect::<Result<_, _>>()?,
        };
        let mut all = vec![out.balances_slot, out.total_supply_slot];
        all.extend(out.implementation_slot);
        all.extend(out.other_slots.iter().copied());
        all.extend(out.other_mapping_slots.iter().copied());
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
    codes: Vec<CodeChanged>,
    errors: usize,
}
impl persist::Sink for Collected {
    fn storage(&mut self, c: &eth::StorageChange, ctx: persist::Ctx) {
        match (word(&c.key), word(&c.old_value), word(&c.new_value)) {
            (Ok(key), Ok(old), Ok(new)) => self.writes.push(Change {
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
        basis_kind: pb::BasisKind::Shares as i32,
        basis_scale: "1".into(),
        balance_rounding: pb::Rounding::Floor as i32,
        basis_bit_offset: 0,
        basis_bit_width: 256,
        basis_signed: false,
        implementation: vault.implementation.clone().unwrap_or_default(),
        implementation_slot: vault.implementation_slot.map(|s| s.to_vec()).unwrap_or_default(),
        activation_block: vault.activation_block,
        // The ERC-20 amount is the share count of the vault itself.
        balance_asset: vault.vault.clone(),
        balance_decimals: vault.vault_decimals,
        basis_carryover: true,
        global_carryover: true,
        scope: pb::Scope::Epoch as i32,
        ..Default::default()
    }
}
fn invalidation(config: &Config, vault: &Vault, reason: pb::InvalidationReason, r: &Reduced) -> pb::ModelEpoch {
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
        let watched = |a: &[u8]| active.iter().any(|v| a == v.vault || v.dependencies().iter().any(|d| d == a));
        let writes: Vec<Change> = collected.writes.into_iter().filter(|w| watched(&w.address)).collect();
        for r in reduce(writes)? {
            for vault in active.iter().filter(|v| r.address == v.vault) {
                if Some(r.key) == vault.implementation_slot {
                    events
                        .epochs
                        .push(invalidation(config, vault, pb::InvalidationReason::ImplementationPointerWrite, &r));
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
                } else if vault.other_slots.contains(&r.key) || vault.other_mapping_slots.iter().any(|base| mapping_has_base(r.key, &preimages, base)) {
                    // Reviewed non-balance storage: metadata, allowances,
                    // permit nonces, reward bookkeeping, initializer state.
                } else {
                    return Err(Error::msg(format!(
                        "unresolved storage for vault 0x{} at key 0x{}; refusing incomplete balance state",
                        hex::encode(&r.address),
                        hex::encode(r.key)
                    )));
                }
            }
            for vault in active.iter().filter(|v| r.address != v.vault) {
                match &vault.model {
                    Model::AaveStaticAToken {
                        pool,
                        reserve_base,
                        implementation_slot,
                        ..
                    } if r.address == *pool => {
                        if Some(r.key) == *implementation_slot {
                            events
                                .epochs
                                .push(invalidation(config, vault, pb::InvalidationReason::DependencyPointerWrite, &r));
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
                    Model::OzVirtualOffset { asset_balance_key, .. } if r.address == vault.asset && r.key == *asset_balance_key => {
                        events.global_state.push(field_row(
                            config,
                            vault,
                            &r,
                            Field {
                                field: pb::StateField::Erc4626TotalAssets,
                                offset: 0,
                                width: 256,
                                scale: "1",
                                key: vault.vault.clone(),
                            },
                        ));
                    }
                    _ => {}
                }
            }
        }
        for c in &collected.codes {
            for vault in &active {
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
            events.epochs.push(epoch_row(config, vault, kind));
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
            events
                .dependencies
                .push(dependency(config, vault, kind, pb::DependencyRole::Underlying, &vault.asset));
            match &vault.model {
                Model::AaveStaticAToken {
                    pool,
                    implementation_slot,
                    implementation,
                    atoken,
                    ..
                } => {
                    if let (Some(implementation), Some(slot)) = (implementation, implementation_slot) {
                        events.dependencies.push(pb::Dependency {
                            depth: 2,
                            parent: pool.clone(),
                            ..pointer(config, vault, kind, pb::DependencyRole::Implementation, implementation, pool, slot)?
                        });
                    }
                    events.dependencies.push(dependency(config, vault, kind, pb::DependencyRole::Pool, pool));
                    events
                        .dependencies
                        .push(dependency(config, vault, kind, pb::DependencyRole::WrappedAsset, atoken));
                }
                Model::MakerSavingsDai { pot, .. } => events
                    .dependencies
                    .push(dependency(config, vault, kind, pb::DependencyRole::RateAccumulator, pot)),
                Model::OzVirtualOffset { decimals_offset, .. } => events.global_state.push(pb::GlobalState {
                    chain_id: config.chain_id,
                    market: vault.vault.clone(),
                    epoch: vault.epoch,
                    field: pb::StateField::Erc4626DecimalsOffset as i32,
                    value: decimals_offset.to_string(),
                    scale: "1".into(),
                    observation: pb::Observation::QualifiedConstant as i32,
                    boundary: pb::Boundary::Declaration as i32,
                    scope: pb::Scope::Epoch as i32,
                    storage_contract: vault.vault.clone(),
                    ..Default::default()
                }),
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
