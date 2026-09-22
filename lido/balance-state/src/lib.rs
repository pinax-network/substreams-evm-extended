//! One RPC-free `map_events` for one explicitly qualified Lido stETH
//! contract-version epoch, emitting `evm.balance_state.v1.Events`.
//!
//! stETH `balanceOf(holder)` under contract version 4 is
//! `shares[holder] * internalEther / internalShares`, where
//! `internalEther = bufferedEther + clValidatorsBalance + clPendingBalance +
//! depositedPostReport` and `internalShares = totalShares - externalShares`.
//! The map emits the holder shares (`HolderBasis` SHARES), the three packed
//! words those global inputs live in, the contract version, the derived total
//! pooled ether when every input was written in-block, and the `TokenRebased`
//! report evidence. It never computes a balance; the consumer evaluates with
//! `conformance::lido` at a canonical clock. A report changes every holder's
//! balance without any holder write; a holder without a row is unknown.
//!
//! Ownership comes from verified Keccak preimages of the `shares` mapping in
//! the stETH proxy's storage (delegate-call context: the storage address is
//! the proxy). Writes to the contract-version slot, code changes on the proxy
//! or implementation, and unresolved stETH writes are handled fail-closed:
//! the first two emit INVALIDATED epoch rows with evidence, the last fails
//! the block. Slots are caller-qualified; tests check each committed slot
//! against `keccak256` of its pinned name.
use evm_persist as persist;
use proto::pb::evm::balance_state::v1 as pb;
use serde::Deserialize;
use sha2::Digest;
use std::collections::{BTreeMap, BTreeSet};
use substreams::{errors::Error, scalar::BigInt};
use substreams_ethereum::pb::eth::v2 as eth;
use tiny_keccak::{Hasher, Keccak};

pub const PACKAGE: &str = "lido_balance_state";
pub const SPEC_REVISION: u32 = 1;
/// Producer versions whose execution ordinals are qualified (version 3 has
/// broken system-call ordinals and is refused by the contract).
pub const QUALIFIED_PRODUCER_VERSIONS: [i32; 2] = [4, 5];
/// `TokenRebased(uint256 indexed reportTimestamp, uint256 timeElapsed, uint256
/// preTotalShares, uint256 preTotalEther, uint256 postTotalShares, uint256
/// postTotalEther, uint256 sharesMintedAsFees)`.
pub const TOKEN_REBASED_TOPIC0: [u8; 32] = [
    0xff, 0x08, 0xc3, 0xef, 0x60, 0x6d, 0x19, 0x8e, 0x31, 0x6e, 0xf5, 0xb8, 0x22, 0x19, 0x3c, 0x48, 0x99, 0x65, 0x89, 0x9e, 0xb4, 0xe3, 0xc2, 0x48, 0xce, 0xa1,
    0xa4, 0x62, 0x6c, 0x3e, 0xda, 0x50,
];

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
fn unsigned(word: &[u8; 32]) -> BigInt {
    BigInt::from_unsigned_bytes_be(word)
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
    pub epochs: Vec<EpochConfig>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EpochConfig {
    pub steth: String,
    pub epoch: u32,
    /// Value of `lido.Versioned.contractVersion` this epoch is qualified for.
    pub contract_version: u64,
    pub model_id: String,
    pub source_pin: String,
    pub activation_block: u64,
    pub implementation: String,
    pub shares_slot: String,
    #[serde(default)]
    pub other_mapping_slots: Vec<String>,
    pub total_and_external_shares_slot: String,
    pub buffered_ether_and_deposited_post_report_slot: String,
    pub cl_validators_balance_and_cl_pending_balance_slot: String,
    pub contract_version_slot: String,
    #[serde(default)]
    pub other_slots: Vec<String>,
    /// Reviewed unstructured-storage names; the map stores `keccak256(name)`.
    #[serde(default)]
    pub other_slot_names: Vec<String>,
    #[serde(default)]
    pub accounting: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Epoch {
    pub steth: Vec<u8>,
    pub epoch: u32,
    pub contract_version: u64,
    pub model_id: String,
    pub source_pin: String,
    pub activation_block: u64,
    pub implementation: Vec<u8>,
    pub shares_slot: [u8; 32],
    pub other_mapping_slots: Vec<[u8; 32]>,
    pub total_and_external_shares_slot: [u8; 32],
    pub buffered_slot: [u8; 32],
    pub cl_slot: [u8; 32],
    pub contract_version_slot: [u8; 32],
    pub other_slots: Vec<[u8; 32]>,
    pub accounting: Option<Vec<u8>>,
}
#[derive(Clone, Debug)]
pub struct Config {
    pub chain_id: u64,
    pub producer_versions: Vec<i32>,
    pub heartbeat_blocks: u64,
    pub epochs: Vec<Epoch>,
    pub parameters_sha256: String,
}

pub fn parse(params: &str) -> Result<Config, Error> {
    let raw: Params = serde_json::from_str(params).map_err(|e| Error::msg(format!("invalid lido balance-state params: {e}")))?;
    require(raw.chain_id > 0, "chain_id required")?;
    require(
        !raw.producer_versions.is_empty() && raw.producer_versions.iter().all(|v| QUALIFIED_PRODUCER_VERSIONS.contains(v)),
        "producer_versions must be a non-empty subset of the qualified Extended versions 4 and 5",
    )?;
    let mut epochs: Vec<Epoch> = Vec::new();
    for e in &raw.epochs {
        require(
            e.epoch > 0 && e.activation_block > 0 && e.contract_version > 0,
            "epoch, contract_version and activation_block must be positive",
        )?;
        require(!e.model_id.is_empty() && !e.source_pin.is_empty(), "model_id and source_pin required")?;
        let mut other_slots: Vec<[u8; 32]> = e.other_slots.iter().map(|s| slot(s, "other_slots")).collect::<Result<_, _>>()?;
        for name in &e.other_slot_names {
            require(!name.is_empty(), "empty slot name")?;
            other_slots.push(keccak(name.as_bytes()));
        }
        let epoch = Epoch {
            steth: hex_bytes(&e.steth, 20, "steth")?,
            epoch: e.epoch,
            contract_version: e.contract_version,
            model_id: e.model_id.clone(),
            source_pin: e.source_pin.clone(),
            activation_block: e.activation_block,
            implementation: hex_bytes(&e.implementation, 20, "implementation")?,
            shares_slot: slot(&e.shares_slot, "shares_slot")?,
            other_mapping_slots: e.other_mapping_slots.iter().map(|s| slot(s, "other_mapping_slots")).collect::<Result<_, _>>()?,
            total_and_external_shares_slot: slot(&e.total_and_external_shares_slot, "total_and_external_shares_slot")?,
            buffered_slot: slot(
                &e.buffered_ether_and_deposited_post_report_slot,
                "buffered_ether_and_deposited_post_report_slot",
            )?,
            cl_slot: slot(
                &e.cl_validators_balance_and_cl_pending_balance_slot,
                "cl_validators_balance_and_cl_pending_balance_slot",
            )?,
            contract_version_slot: slot(&e.contract_version_slot, "contract_version_slot")?,
            other_slots,
            accounting: e.accounting.as_deref().map(|a| hex_bytes(a, 20, "accounting")).transpose()?,
        };
        let mut all = vec![
            epoch.shares_slot,
            epoch.total_and_external_shares_slot,
            epoch.buffered_slot,
            epoch.cl_slot,
            epoch.contract_version_slot,
        ];
        all.extend(epoch.other_mapping_slots.iter().copied());
        all.extend(epoch.other_slots.iter().copied());
        require(all.iter().collect::<BTreeSet<_>>().len() == all.len(), "epoch slots overlap")?;
        require(epochs.iter().all(|o| o.steth != epoch.steth), "duplicate stETH epoch")?;
        epochs.push(epoch);
    }
    Ok(Config {
        chain_id: raw.chain_id,
        producer_versions: raw.producer_versions,
        heartbeat_blocks: raw.heartbeat_blocks,
        epochs,
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

fn base_row(config: &Config, epoch: &Epoch, field: pb::StateField) -> pb::GlobalState {
    pb::GlobalState {
        chain_id: config.chain_id,
        market: epoch.steth.clone(),
        epoch: epoch.epoch,
        field: field as i32,
        scale: "1".into(),
        boundary: pb::Boundary::EndOfBlock as i32,
        ..Default::default()
    }
}
fn packed_row(config: &Config, epoch: &Epoch, r: &Reduced, field: pb::StateField, offset: u32, width: u32) -> pb::GlobalState {
    pb::GlobalState {
        value: bits(&r.new, offset, width).to_string(),
        previous_value: bits(&r.old, offset, width).to_string(),
        observation: pb::Observation::ObservedWrite as i32,
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
        bit_offset: offset,
        bit_width: width,
        ..base_row(config, epoch, field)
    }
}
fn epoch_row(config: &Config, epoch: &Epoch, kind: pb::EpochEventKind) -> pb::ModelEpoch {
    pb::ModelEpoch {
        chain_id: config.chain_id,
        market: epoch.steth.clone(),
        epoch: epoch.epoch,
        kind: kind as i32,
        family: pb::ModelFamily::LidoSteth as i32,
        model_id: epoch.model_id.clone(),
        source_pin: epoch.source_pin.clone(),
        implementation_revision: epoch.contract_version.to_string(),
        basis_kind: pb::BasisKind::Shares as i32,
        basis_scale: "1".into(),
        balance_rounding: pb::Rounding::Floor as i32,
        basis_bit_offset: 0,
        basis_bit_width: 256,
        basis_signed: false,
        implementation: epoch.implementation.clone(),
        activation_block: epoch.activation_block,
        balance_asset: epoch.steth.clone(),
        balance_decimals: 18,
        basis_carryover: true,
        global_carryover: false,
        scope: pb::Scope::Epoch as i32,
        ..Default::default()
    }
}

pub fn project(block: &eth::Block, config: &Config) -> Result<pb::Events, Error> {
    let timestamp = validate_block(block, config)?;
    let header = block.header.as_ref().unwrap();
    let active: Vec<&Epoch> = config.epochs.iter().filter(|e| e.activation_block <= block.number).collect();
    let mut events = pb::Events::default();
    if !active.is_empty() {
        let mut collected = Collected::default();
        persist::collect_block(block, &mut collected)?;
        require(collected.errors == 0, "malformed persisted storage change")?;
        let mut preimages = BTreeMap::new();
        for call in block.system_calls.iter().chain(block.transaction_traces.iter().flat_map(|tx| &tx.calls)) {
            if !active
                .iter()
                .any(|e| call.address == e.steth || call.storage_changes.iter().any(|c| c.address == e.steth))
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
        let relevant: Vec<Change> = collected.writes.into_iter().filter(|w| active.iter().any(|e| w.address == e.steth)).collect();
        let mut words: BTreeMap<(Vec<u8>, [u8; 32]), Reduced> = BTreeMap::new();
        for r in reduce(relevant)? {
            let epoch = active.iter().find(|e| e.steth == r.address).unwrap();
            if r.key == epoch.total_and_external_shares_slot {
                for (field, offset) in [(pb::StateField::LidoTotalShares, 0), (pb::StateField::LidoExternalShares, 128)] {
                    events.global_state.push(packed_row(config, epoch, &r, field, offset, 128));
                }
                words.insert((r.address.clone(), r.key), r);
            } else if r.key == epoch.buffered_slot {
                for (field, offset) in [(pb::StateField::LidoBufferedEther, 0), (pb::StateField::LidoDepositedPostReport, 128)] {
                    events.global_state.push(packed_row(config, epoch, &r, field, offset, 128));
                }
                words.insert((r.address.clone(), r.key), r);
            } else if r.key == epoch.cl_slot {
                for (field, offset) in [(pb::StateField::LidoClValidatorsBalance, 0), (pb::StateField::LidoClPendingBalance, 128)] {
                    events.global_state.push(packed_row(config, epoch, &r, field, offset, 128));
                }
                words.insert((r.address.clone(), r.key), r);
            } else if r.key == epoch.contract_version_slot {
                events
                    .global_state
                    .push(packed_row(config, epoch, &r, pb::StateField::LidoContractVersion, 0, 256));
                if unsigned(&r.new) != epoch.contract_version {
                    events.epochs.push(pb::ModelEpoch {
                        reason: pb::InvalidationReason::ContractVersionSet as i32,
                        scope: scope_of(r.scope) as i32,
                        ordinal: r.ordinal,
                        transaction_index: r.tx_index,
                        transaction_hash: r.tx_hash.clone(),
                        call_index: r.call_index,
                        evidence_contract: r.address.clone(),
                        evidence_slot: r.key.to_vec(),
                        evidence_previous_word: r.old.to_vec(),
                        evidence_word: r.new.to_vec(),
                        ..epoch_row(config, epoch, pb::EpochEventKind::Invalidated)
                    });
                }
            } else if let Some(holder) = preimages
                .get(&r.key)
                .filter(|p| p.len() == 64 && p[..12] == [0; 12] && p[32..] == epoch.shares_slot)
                .map(|p| p[12..32].to_vec())
            {
                events.holder_basis.push(pb::HolderBasis {
                    chain_id: config.chain_id,
                    market: epoch.steth.clone(),
                    holder,
                    epoch: epoch.epoch,
                    basis_kind: pb::BasisKind::Shares as i32,
                    value: unsigned(&r.new).to_string(),
                    previous_value: unsigned(&r.old).to_string(),
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
            } else if epoch.other_slots.contains(&r.key) || epoch.other_mapping_slots.iter().any(|base| mapping_has_base(r.key, &preimages, base)) {
                // Reviewed non-balance storage: allowances, permit nonces,
                // locator, stake limit, deposit bookkeeping, Aragon app state.
            } else {
                return Err(Error::msg(format!(
                    "unresolved storage for stETH 0x{} at key 0x{}; refusing incomplete balance state",
                    hex::encode(&r.address),
                    hex::encode(r.key)
                )));
            }
        }
        // Derived total pooled ether: only when the three input words were all
        // written in this block; a map holds no state across blocks.
        for epoch in &active {
            let (Some(shares), Some(buffered), Some(cl)) = (
                words.get(&(epoch.steth.clone(), epoch.total_and_external_shares_slot)),
                words.get(&(epoch.steth.clone(), epoch.buffered_slot)),
                words.get(&(epoch.steth.clone(), epoch.cl_slot)),
            ) else {
                continue;
            };
            let total_shares = bits(&shares.new, 0, 128);
            let external_shares = bits(&shares.new, 128, 128);
            let internal_shares = total_shares.clone() - external_shares.clone();
            if internal_shares <= BigInt::zero() {
                continue; // division by zero in the getter; not derivable
            }
            let internal_ether = bits(&buffered.new, 0, 128) + bits(&buffered.new, 128, 128) + bits(&cl.new, 0, 128) + bits(&cl.new, 128, 128);
            let external_ether = external_shares * internal_ether.clone() / internal_shares;
            let last = [shares, buffered, cl].into_iter().max_by_key(|r| r.ordinal).unwrap();
            events.global_state.push(pb::GlobalState {
                value: (internal_ether + external_ether).to_string(),
                observation: pb::Observation::Derived as i32,
                scope: scope_of(last.scope) as i32,
                ordinal: last.ordinal,
                first_ordinal: [shares, buffered, cl].iter().map(|r| r.first_ordinal).min().unwrap(),
                change_count: shares.count + buffered.count + cl.count,
                transaction_index: last.tx_index,
                transaction_hash: last.tx_hash.clone(),
                call_index: last.call_index,
                storage_contract: epoch.steth.clone(),
                ..base_row(config, epoch, pb::StateField::LidoTotalPooledEther)
            });
        }
        // TokenRebased report evidence from receipts of succeeded transactions.
        for tx in block
            .transaction_traces
            .iter()
            .filter(|tx| tx.status == eth::TransactionTraceStatus::Succeeded as i32)
        {
            let Some(receipt) = &tx.receipt else { continue };
            for log in &receipt.logs {
                let Some(epoch) = active.iter().find(|e| e.steth == log.address) else {
                    continue;
                };
                if log.topics.first().map(|t| t.as_slice()) != Some(&TOKEN_REBASED_TOPIC0[..]) {
                    continue;
                }
                require(log.topics.len() == 2 && log.data.len() == 6 * 32, "malformed TokenRebased log")?;
                let field = |i: usize| unsigned(&word(&log.data[i * 32..(i + 1) * 32]).unwrap()).to_string();
                for (state_field, value) in [
                    (pb::StateField::LidoReportTimestamp, unsigned(&word(&log.topics[1])?).to_string()),
                    (pb::StateField::LidoReportPostTotalShares, field(3)),
                    (pb::StateField::LidoReportPostTotalEther, field(4)),
                    (pb::StateField::LidoReportSharesMintedAsFees, field(5)),
                ] {
                    events.global_state.push(pb::GlobalState {
                        value,
                        observation: pb::Observation::ObservedLog as i32,
                        boundary: pb::Boundary::Change as i32,
                        scope: pb::Scope::Transaction as i32,
                        ordinal: log.ordinal,
                        first_ordinal: log.ordinal,
                        change_count: 1,
                        transaction_index: tx.index,
                        transaction_hash: tx.hash.clone(),
                        log_index: log.index,
                        storage_contract: log.address.clone(),
                        ..base_row(config, epoch, state_field)
                    });
                }
            }
        }
        for c in &collected.codes {
            for epoch in &active {
                if c.address == epoch.steth || c.address == epoch.implementation {
                    events.epochs.push(pb::ModelEpoch {
                        reason: pb::InvalidationReason::CodeChange as i32,
                        scope: scope_of(c.scope) as i32,
                        ordinal: c.ordinal,
                        transaction_index: c.tx_index,
                        transaction_hash: c.tx_hash.clone(),
                        call_index: c.call_index,
                        evidence_contract: c.address.clone(),
                        evidence_code_hash: c.new_hash.clone(),
                        ..epoch_row(config, epoch, pb::EpochEventKind::Invalidated)
                    });
                }
            }
        }
        for epoch in &active {
            let kind = if block.number == epoch.activation_block {
                pb::EpochEventKind::Bound
            } else if config.heartbeat_blocks > 0 && (block.number - epoch.activation_block) % config.heartbeat_blocks == 0 {
                pb::EpochEventKind::Reaffirmed
            } else {
                continue;
            };
            events.epochs.push(epoch_row(config, epoch, kind));
            events.dependencies.push(pb::Dependency {
                chain_id: config.chain_id,
                market: epoch.steth.clone(),
                epoch: epoch.epoch,
                kind: kind as i32,
                role: pb::DependencyRole::Implementation as i32,
                contract: epoch.implementation.clone(),
                depth: 1,
                // The Aragon app proxy resolves its base through the Kernel;
                // the binding here is the declared implementation, checked by
                // persisted code changes, not a proxy pointer slot.
                binding: pb::BindingKind::Declared as i32,
                activation_block: epoch.activation_block,
                source_pin: epoch.source_pin.clone(),
                ..Default::default()
            });
            if let Some(accounting) = &epoch.accounting {
                events.dependencies.push(pb::Dependency {
                    chain_id: config.chain_id,
                    market: epoch.steth.clone(),
                    epoch: epoch.epoch,
                    kind: kind as i32,
                    role: pb::DependencyRole::Accounting as i32,
                    contract: accounting.clone(),
                    depth: 1,
                    binding: pb::BindingKind::Declared as i32,
                    activation_block: epoch.activation_block,
                    source_pin: epoch.source_pin.clone(),
                    ..Default::default()
                });
            }
            events.global_state.push(pb::GlobalState {
                value: epoch.contract_version.to_string(),
                observation: pb::Observation::QualifiedConstant as i32,
                boundary: pb::Boundary::Declaration as i32,
                scope: pb::Scope::Epoch as i32,
                storage_contract: epoch.steth.clone(),
                storage_slot: epoch.contract_version_slot.to_vec(),
                ..base_row(config, epoch, pb::StateField::LidoContractVersion)
            });
        }
    }
    events
        .holder_basis
        .sort_by(|a, b| (&a.market, &a.holder, a.ordinal).cmp(&(&b.market, &b.holder, b.ordinal)));
    events
        .global_state
        .sort_by(|a, b| (&a.market, a.field, a.ordinal, a.log_index).cmp(&(&b.market, b.field, b.ordinal, b.log_index)));
    events
        .epochs
        .sort_by(|a, b| (&a.market, a.epoch, a.ordinal, a.kind).cmp(&(&b.market, b.epoch, b.ordinal, b.kind)));
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
