//! One RPC-free `map_events` emitting `evm.executions.v1.Events`: transaction
//! identity and status, the structural call tree, emitted logs, code changes
//! and EIP-7702 authorizations of an Extended block, with attempted execution
//! kept separate from persisted effects.
//!
//! Persistence follows the shared [`evm_persist`] rules: a frame's effects
//! persisted when its transaction succeeded and the frame was not reverted, or
//! when it is a non-reverted system call or a block-level record; accepted
//! SetCode authorizations persist even when the transaction fails. Reverted
//! frames and their logs are still emitted with `persisted = false`, so a
//! consumer never mistakes an attempt for a completed action.
//!
//! The package emits facts, not interpretation: no wallet-relative direction,
//! no labels, no economic outcome inferred from a selector.
use evm_persist as persist;
use proto::pb::evm::executions::v1 as pb;
use serde::Deserialize;
use sha2::Digest;
use std::collections::BTreeSet;
use substreams::{errors::Error, scalar::BigInt};
use substreams_ethereum::pb::eth::v2 as eth;
use tiny_keccak::{Hasher, Keccak};

pub const PACKAGE: &str = "evm_executions";
pub const SPEC_REVISION: u32 = 1;
/// EIP-7702 delegation indicator prefix.
const DELEGATION_PREFIX: [u8; 3] = [0xef, 0x01, 0x00];

fn require(ok: bool, message: &str) -> Result<(), Error> {
    if ok {
        Ok(())
    } else {
        Err(Error::msg(message.to_string()))
    }
}
fn keccak(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0; 32];
    let mut hasher = Keccak::v256();
    hasher.update(bytes);
    hasher.finalize(&mut out);
    out
}
fn amount(value: Option<&eth::BigInt>) -> String {
    value.map(|v| BigInt::from_unsigned_bytes_be(&v.bytes).to_string()).unwrap_or_default()
}
fn selector(input: &[u8]) -> Vec<u8> {
    if input.len() >= 4 {
        input[..4].to_vec()
    } else {
        Vec::new()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub chain_id: u64,
    pub producer_versions: Vec<i32>,
    /// Carry full call and transaction input bytes (default: selector only).
    #[serde(default)]
    pub include_input: bool,
    /// Carry return data bytes (default: sizes only).
    #[serde(default)]
    pub include_return_data: bool,
    /// Emit Call rows (default true).
    #[serde(default = "yes")]
    pub include_calls: bool,
    /// Emit Log rows (default true).
    #[serde(default = "yes")]
    pub include_logs: bool,
}
fn yes() -> bool {
    true
}
#[derive(Clone, Debug)]
pub struct Config {
    pub params: Params,
    pub parameters_sha256: String,
}
pub fn parse(params: &str) -> Result<Config, Error> {
    let parsed: Params = serde_json::from_str(params).map_err(|e| Error::msg(format!("invalid executions params: {e}")))?;
    require(parsed.chain_id > 0, "chain_id required")?;
    require(
        !parsed.producer_versions.is_empty() && parsed.producer_versions.iter().all(|v| *v > 0),
        "qualified producer versions required",
    )?;
    Ok(Config {
        params: parsed,
        parameters_sha256: hex::encode(sha2::Sha256::digest(params.as_bytes())),
    })
}

pub fn validate_block(block: &eth::Block, config: &Config) -> Result<u64, Error> {
    require(
        block.detail_level == eth::block::DetailLevel::DetaillevelExtended as i32,
        "Extended blocks required",
    )?;
    require(config.params.producer_versions.contains(&block.ver), "Extended producer version not qualified")?;
    let header = block.header.as_ref().ok_or_else(|| Error::msg("missing header"))?;
    require(
        block.hash.len() == 32 && header.parent_hash.len() == 32 && header.state_root.len() == 32,
        "invalid block identity",
    )?;
    require(header.number == block.number, "header number mismatch")?;
    let timestamp = header.timestamp.as_ref().ok_or_else(|| Error::msg("missing timestamp"))?;
    require(timestamp.seconds >= 0, "negative timestamp")?;
    for tx in &block.transaction_traces {
        require((1..=3).contains(&tx.status) && !tx.calls.is_empty(), "incomplete transaction persistence data")?;
        require(tx.hash.len() == 32, "invalid transaction hash")?;
    }
    Ok(timestamp.seconds as u64)
}

/// Persisted code-change identities collected through the shared rules.
#[derive(Default)]
struct PersistedCodes {
    keys: BTreeSet<(Vec<u8>, u64)>,
}
impl persist::Sink for PersistedCodes {
    fn storage(&mut self, _: &eth::StorageChange, _: persist::Ctx) {}
    fn balance(&mut self, _: &eth::BalanceChange, _: persist::Ctx) {}
    fn nonce(&mut self, _: &eth::NonceChange, _: persist::Ctx) {}
    fn code(&mut self, c: &eth::CodeChange, _: persist::Ctx) {
        self.keys.insert((c.address.clone(), c.ordinal));
    }
}

fn transaction_type(raw: i32) -> pb::TransactionType {
    use eth::transaction_trace::Type;
    match Type::try_from(raw) {
        Ok(Type::TrxTypeLegacy) => pb::TransactionType::Legacy,
        Ok(Type::TrxTypeAccessList) => pb::TransactionType::AccessList,
        Ok(Type::TrxTypeDynamicFee) => pb::TransactionType::DynamicFee,
        Ok(Type::TrxTypeBlob) => pb::TransactionType::Blob,
        Ok(Type::TrxTypeSetCode) => pb::TransactionType::SetCode,
        Ok(Type::TrxTypeOptimismDeposit) => pb::TransactionType::OptimismDeposit,
        Ok(_) if (100..=120).contains(&raw) => pb::TransactionType::Arbitrum,
        _ => pb::TransactionType::Other,
    }
}
fn call_type(raw: i32) -> pb::CallType {
    match eth::CallType::try_from(raw) {
        Ok(eth::CallType::Call) => pb::CallType::Call,
        Ok(eth::CallType::Callcode) => pb::CallType::Callcode,
        Ok(eth::CallType::Delegate) => pb::CallType::Delegate,
        Ok(eth::CallType::Static) => pb::CallType::Static,
        Ok(eth::CallType::Create) => pb::CallType::Create,
        _ => pb::CallType::Unspecified,
    }
}
fn is_empty_code(hash: &[u8], code: &[u8]) -> bool {
    code.is_empty() && (hash.is_empty() || hash.iter().all(|b| *b == 0) || hash == keccak(&[]).as_slice())
}
fn is_delegation(code: &[u8]) -> bool {
    code.len() == 23 && code[..3] == DELEGATION_PREFIX
}
fn code_change_kind(c: &eth::CodeChange) -> (pb::CodeChangeKind, Vec<u8>) {
    let old_empty = is_empty_code(&c.old_hash, &c.old_code);
    let new_empty = is_empty_code(&c.new_hash, &c.new_code);
    if is_delegation(&c.new_code) {
        return (pb::CodeChangeKind::DelegationSet, c.new_code[3..].to_vec());
    }
    if is_delegation(&c.old_code) && new_empty {
        return (pb::CodeChangeKind::DelegationCleared, Vec::new());
    }
    match (old_empty, new_empty) {
        (true, false) => (pb::CodeChangeKind::Created, Vec::new()),
        (false, true) => (pb::CodeChangeKind::Cleared, Vec::new()),
        _ => (pb::CodeChangeKind::Replaced, Vec::new()),
    }
}

struct Frame<'a> {
    scope: pb::Scope,
    tx_hash: &'a [u8],
    tx_index: u32,
    persisted_tx: bool,
}

fn push_call(events: &mut pb::Events, config: &Config, frame: &Frame, call: &eth::Call, persisted_codes: &PersistedCodes) {
    let persisted = frame.persisted_tx && !call.state_reverted;
    if config.params.include_calls {
        events.calls.push(pb::Call {
            chain_id: config.params.chain_id,
            scope: frame.scope as i32,
            transaction_hash: frame.tx_hash.to_vec(),
            transaction_index: frame.tx_index,
            index: call.index,
            parent_index: call.parent_index,
            depth: call.depth,
            call_type: call_type(call.call_type) as i32,
            caller: call.caller.clone(),
            address: call.address.clone(),
            address_delegates_to: call.address_delegates_to.clone().unwrap_or_default(),
            value: amount(call.value.as_ref()),
            gas_limit: call.gas_limit,
            gas_consumed: call.gas_consumed,
            input_selector: selector(&call.input),
            input: if config.params.include_input { call.input.clone() } else { Vec::new() },
            input_size: call.input.len() as u32,
            return_data: if config.params.include_return_data {
                call.return_data.clone()
            } else {
                Vec::new()
            },
            return_data_size: call.return_data.len() as u32,
            executed_code: call.executed_code,
            suicide: call.suicide,
            status_failed: call.status_failed,
            status_reverted: call.status_reverted,
            failure_reason: call.failure_reason.clone(),
            state_reverted: call.state_reverted,
            persisted,
            begin_ordinal: call.begin_ordinal,
            end_ordinal: call.end_ordinal,
            storage_change_count: call.storage_changes.len() as u32,
            balance_change_count: call.balance_changes.len() as u32,
            nonce_change_count: call.nonce_changes.len() as u32,
            code_change_count: call.code_changes.len() as u32,
            log_count: call.logs.len() as u32,
        });
    }
    if config.params.include_logs {
        for log in &call.logs {
            let topic = |i: usize| log.topics.get(i).cloned().unwrap_or_default();
            events.logs.push(pb::Log {
                chain_id: config.params.chain_id,
                scope: frame.scope as i32,
                transaction_hash: frame.tx_hash.to_vec(),
                transaction_index: frame.tx_index,
                call_index: call.index,
                receipt_index: log.index,
                block_index: log.block_index,
                ordinal: log.ordinal,
                address: log.address.clone(),
                topic_count: log.topics.len() as u32,
                topic0: topic(0),
                topic1: topic(1),
                topic2: topic(2),
                topic3: topic(3),
                data: log.data.clone(),
                data_size: log.data.len() as u32,
                persisted,
            });
        }
    }
    for c in &call.code_changes {
        let (kind, delegation_target) = code_change_kind(c);
        events.code_changes.push(pb::CodeChange {
            chain_id: config.params.chain_id,
            scope: frame.scope as i32,
            transaction_hash: frame.tx_hash.to_vec(),
            transaction_index: frame.tx_index,
            call_index: call.index,
            ordinal: c.ordinal,
            address: c.address.clone(),
            old_code_hash: c.old_hash.clone(),
            new_code_hash: c.new_hash.clone(),
            old_code_size: c.old_code.len() as u32,
            new_code_size: c.new_code.len() as u32,
            kind: kind as i32,
            delegation_target,
            persisted: persisted_codes.keys.contains(&(c.address.clone(), c.ordinal)),
        });
    }
}

pub fn project(block: &eth::Block, config: &Config) -> Result<pb::Events, Error> {
    let timestamp = validate_block(block, config)?;
    let header = block.header.as_ref().unwrap();
    let mut persisted_codes = PersistedCodes::default();
    persist::collect_block(block, &mut persisted_codes)?;
    let mut events = pb::Events::default();

    for tx in &block.transaction_traces {
        let succeeded = tx.status == eth::TransactionTraceStatus::Succeeded as i32;
        let frame = Frame {
            scope: pb::Scope::Transaction,
            tx_hash: &tx.hash,
            tx_index: tx.index,
            persisted_tx: succeeded,
        };
        let mut persisted_logs = 0u32;
        for call in &tx.calls {
            if succeeded && !call.state_reverted {
                persisted_logs += call.logs.len() as u32;
            }
            push_call(&mut events, config, &frame, call, &persisted_codes);
        }
        let receipt_logs = tx.receipt.as_ref().map(|r| r.logs.len() as u32).unwrap_or(0);
        require(
            persisted_logs == receipt_logs,
            "receipt logs disagree with the logs of persisted frames; refusing inconsistent execution data",
        )?;
        let root = &tx.calls[0];
        let created_contract = if succeeded
            && root.call_type == eth::CallType::Create as i32
            && !root.state_reverted
            && root.code_changes.iter().any(|c| c.address == root.address)
        {
            root.address.clone()
        } else {
            Vec::new()
        };
        for (position, auth) in tx.set_code_authorizations.iter().enumerate() {
            events.set_code_authorizations.push(pb::SetCodeAuthorization {
                chain_id: config.params.chain_id,
                transaction_hash: tx.hash.clone(),
                transaction_index: tx.index,
                position: position as u32,
                authority: auth.authority.clone().unwrap_or_default(),
                address: auth.address.clone(),
                nonce: auth.nonce,
                authorization_chain_id: BigInt::from_unsigned_bytes_be(&auth.chain_id).to_string(),
                discarded: auth.discarded,
                applied: !auth.discarded,
            });
        }
        events.transactions.push(pb::Transaction {
            chain_id: config.params.chain_id,
            hash: tx.hash.clone(),
            index: tx.index,
            r#type: transaction_type(tx.r#type) as i32,
            type_raw: tx.r#type as u32,
            status: tx.status,
            from: tx.from.clone(),
            to: tx.to.clone(),
            nonce: tx.nonce,
            value: amount(tx.value.as_ref()),
            gas_limit: tx.gas_limit,
            gas_used: tx.gas_used,
            gas_price: amount(tx.gas_price.as_ref()),
            max_fee_per_gas: amount(tx.max_fee_per_gas.as_ref()),
            max_priority_fee_per_gas: amount(tx.max_priority_fee_per_gas.as_ref()),
            input_selector: selector(&tx.input),
            input: if config.params.include_input { tx.input.clone() } else { Vec::new() },
            input_size: tx.input.len() as u32,
            return_data: if config.params.include_return_data {
                tx.return_data.clone()
            } else {
                Vec::new()
            },
            begin_ordinal: tx.begin_ordinal,
            end_ordinal: tx.end_ordinal,
            created_contract,
            call_count: tx.calls.len() as u32,
            receipt_log_count: receipt_logs,
            reverted_call_count: tx.calls.iter().filter(|c| c.state_reverted).count() as u32,
            set_code_authorization_count: tx.set_code_authorizations.len() as u32,
            blob_gas: tx.blob_gas.unwrap_or(0),
            blob_count: tx.blob_hashes.len() as u32,
        });
    }
    for call in &block.system_calls {
        let frame = Frame {
            scope: pb::Scope::SystemCall,
            tx_hash: &[],
            tx_index: 0,
            persisted_tx: true,
        };
        push_call(&mut events, config, &frame, call, &persisted_codes);
    }
    for c in &block.code_changes {
        let (kind, delegation_target) = code_change_kind(c);
        events.code_changes.push(pb::CodeChange {
            chain_id: config.params.chain_id,
            scope: pb::Scope::Block as i32,
            ordinal: c.ordinal,
            address: c.address.clone(),
            old_code_hash: c.old_hash.clone(),
            new_code_hash: c.new_hash.clone(),
            old_code_size: c.old_code.len() as u32,
            new_code_size: c.new_code.len() as u32,
            kind: kind as i32,
            delegation_target,
            persisted: c.old_hash != c.new_hash,
            ..Default::default()
        });
    }
    events.transactions.sort_by_key(|t| t.index);
    events.calls.sort_by_key(|c| (c.scope, c.transaction_index, c.index));
    events
        .logs
        .sort_by_key(|l| (l.scope, l.transaction_index, l.ordinal, l.call_index, l.receipt_index));
    events
        .code_changes
        .sort_by_key(|c| (c.scope, c.transaction_index, c.ordinal, c.address.clone()));
    events.set_code_authorizations.sort_by_key(|a| (a.transaction_index, a.position));
    events.clocks.push(pb::BlockClock {
        chain_id: config.params.chain_id,
        number: block.number,
        hash: block.hash.clone(),
        parent_hash: header.parent_hash.clone(),
        timestamp,
        state_root: header.state_root.clone(),
        coinbase: header.coinbase.clone(),
        gas_used: header.gas_used,
        gas_limit: header.gas_limit,
        base_fee_per_gas: amount(header.base_fee_per_gas.as_ref()),
        producer_version: block.ver as u32,
        spec_revision: SPEC_REVISION,
        package: PACKAGE.into(),
        package_version: env!("CARGO_PKG_VERSION").into(),
        parameters_sha256: config.parameters_sha256.clone(),
        transaction_count: events.transactions.len() as u32,
        call_count: events.calls.len() as u32,
        log_count: events.logs.len() as u32,
        code_change_count: events.code_changes.len() as u32,
        set_code_authorization_count: events.set_code_authorizations.len() as u32,
        system_call_count: block.system_calls.len() as u32,
    });
    Ok(events)
}

// The SDK macro generates raw-pointer parameter decoding and discards function
// attributes; the export exists only in the WASM build so host crates can link
// this crate next to other packages.
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
