//! One RPC-free `map_events` emitting `aave.actions.v1.Events`: Aave V3 Pool
//! `Supply`, `Withdraw`, `Borrow`, `Repay`, `LiquidationCall` and `FlashLoan`
//! facts for explicitly bound Pool epochs. Every row keeps the protocol's own
//! event kind and actor fields (caller, beneficiary, recipient) and a
//! `persisted` flag; nothing here is a wallet label or an economic outcome.
//!
//! The adapter is source-bound: the event shapes are those of `IPool.sol` at
//! the pinned aave-v3-origin release. A Pool log with a bound signature but a
//! different shape, a Pool code change or an implementation-pointer write
//! fails the block instead of guessing, because that is how an unqualified
//! version announces itself.
use evm_persist as persist;
use proto::pb::aave::actions::v1 as pb;
use serde::Deserialize;
use sha2::Digest;
use substreams::{errors::Error, scalar::BigInt};
use substreams_ethereum::pb::eth::v2 as eth;
use tiny_keccak::{Hasher, Keccak};

pub const PACKAGE: &str = "aave_actions";
pub const SPEC_REVISION: u32 = 1;

pub const SUPPLY_TOPIC: &str = "2b627736bca15cd5381dcf80b0bf11fd197d01a037c52b927a881a10fb73ba61";
pub const WITHDRAW_TOPIC: &str = "3115d1449a7b732c986cba18244e897a450f61e1bb8d589cd2e69e6c8924f9f7";
pub const BORROW_TOPIC: &str = "b3d084820fb1a9decffb176436bd02558d15fac9b0ddfed8c465bc7359d7dce0";
pub const REPAY_TOPIC: &str = "a534c8dbe71f871f9f3530e97a74601fea17b426cae02e1c5aee42c96c784051";
pub const LIQUIDATION_CALL_TOPIC: &str = "e413a321e8681d831f4dbccbca790d2952b56f977908e45be37335533e005286";
pub const FLASH_LOAN_TOPIC: &str = "efefaba5e921573100900a3ad9cf29f222d995fb3b6045797eaea7521bd8d6f0";

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
fn hex_bytes(s: &str, len: usize, what: &str) -> Result<Vec<u8>, Error> {
    let bytes = hex::decode(s.strip_prefix("0x").unwrap_or(s)).map_err(|_| Error::msg(format!("invalid hex for {what}")))?;
    require(bytes.len() == len, &format!("{what} must be {len} bytes"))?;
    Ok(bytes)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub chain_id: u64,
    pub producer_versions: Vec<i32>,
    #[serde(default = "yes")]
    pub include_attempted: bool,
    #[serde(default)]
    pub pools: Vec<PoolConfig>,
}
fn yes() -> bool {
    true
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolConfig {
    pub address: String,
    pub epoch: u32,
    pub implementation_slot: String,
    pub implementation: String,
    pub source_pin: String,
}
#[derive(Clone, Debug)]
pub struct Pool {
    pub address: Vec<u8>,
    pub epoch: u32,
    pub implementation_slot: [u8; 32],
    pub implementation: Vec<u8>,
    pub source_pin: String,
}
#[derive(Clone, Debug)]
pub struct Config {
    pub chain_id: u64,
    pub producer_versions: Vec<i32>,
    pub include_attempted: bool,
    pub pools: Vec<Pool>,
    pub parameters_sha256: String,
}
pub fn parse(params: &str) -> Result<Config, Error> {
    let raw: Params = serde_json::from_str(params).map_err(|e| Error::msg(format!("invalid aave actions params: {e}")))?;
    require(raw.chain_id > 0, "chain_id required")?;
    require(
        !raw.producer_versions.is_empty() && raw.producer_versions.iter().all(|v| *v > 0),
        "qualified producer versions required",
    )?;
    let mut pools = Vec::new();
    for p in &raw.pools {
        require(p.epoch > 0 && !p.source_pin.is_empty(), "pool epoch and source_pin required")?;
        let pool = Pool {
            address: hex_bytes(&p.address, 20, "pool address")?,
            epoch: p.epoch,
            implementation_slot: hex_bytes(&p.implementation_slot, 32, "pool implementation_slot")?.try_into().unwrap(),
            implementation: hex_bytes(&p.implementation, 20, "pool implementation")?,
            source_pin: p.source_pin.clone(),
        };
        require(pools.iter().all(|other: &Pool| other.address != pool.address), "duplicate pool")?;
        pools.push(pool);
    }
    Ok(Config {
        chain_id: raw.chain_id,
        producer_versions: raw.producer_versions,
        include_attempted: raw.include_attempted,
        pools,
        parameters_sha256: hex::encode(sha2::Sha256::digest(params.as_bytes())),
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
    require(header.number == block.number, "header number mismatch")?;
    let timestamp = header.timestamp.as_ref().ok_or_else(|| Error::msg("missing timestamp"))?;
    require(timestamp.seconds >= 0, "negative timestamp")?;
    for tx in &block.transaction_traces {
        require((1..=3).contains(&tx.status) && !tx.calls.is_empty(), "incomplete transaction persistence data")?;
        require(tx.hash.len() == 32, "invalid transaction hash")?;
    }
    Ok(timestamp.seconds as u64)
}

/// Guards: Pool code changes and implementation-pointer writes.
#[derive(Default)]
struct Guards {
    codes: Vec<Vec<u8>>,
    pointer_writes: Vec<(Vec<u8>, [u8; 32])>,
}
impl persist::Sink for Guards {
    fn storage(&mut self, c: &eth::StorageChange, _: persist::Ctx) {
        if c.key.len() == 32 {
            self.pointer_writes.push((c.address.clone(), c.key.clone().try_into().unwrap()));
        }
    }
    fn balance(&mut self, _: &eth::BalanceChange, _: persist::Ctx) {}
    fn nonce(&mut self, _: &eth::NonceChange, _: persist::Ctx) {}
    fn code(&mut self, c: &eth::CodeChange, _: persist::Ctx) {
        self.codes.push(c.address.clone());
    }
}

fn topic_address(topic: &[u8]) -> Result<Vec<u8>, Error> {
    require(topic.len() == 32 && topic[..12] == [0; 12], "Pool event topic is not a padded address")?;
    Ok(topic[12..].to_vec())
}
fn topic_u16(topic: &[u8]) -> Result<u32, Error> {
    require(topic.len() == 32 && topic[..30] == [0; 30], "Pool event topic is not a uint16")?;
    Ok(u16::from_be_bytes([topic[30], topic[31]]) as u32)
}
fn word_at(data: &[u8], i: usize) -> Result<&[u8], Error> {
    data.get(i * 32..(i + 1) * 32)
        .ok_or_else(|| Error::msg("Pool event data shorter than its shape"))
}
fn word_address(data: &[u8], i: usize) -> Result<Vec<u8>, Error> {
    topic_address(word_at(data, i)?)
}
fn word_uint(data: &[u8], i: usize) -> Result<String, Error> {
    Ok(BigInt::from_unsigned_bytes_be(word_at(data, i)?).to_string())
}
fn word_u8(data: &[u8], i: usize) -> Result<u32, Error> {
    let w = word_at(data, i)?;
    require(w[..31] == [0; 31], "Pool event uint8 field exceeds one byte")?;
    Ok(w[31] as u32)
}
fn word_bool(data: &[u8], i: usize) -> Result<bool, Error> {
    let w = word_at(data, i)?;
    require(w[..31] == [0; 31] && w[31] <= 1, "Pool event bool field is not 0 or 1")?;
    Ok(w[31] == 1)
}
fn shape(log: &eth::Log, topics: usize, data_words: usize, name: &str) -> Result<(), Error> {
    require(
        log.topics.len() == topics && log.data.len() == data_words * 32,
        &format!("Pool {name} event shape differs from the bound IPool version; requalify the epoch"),
    )
}

/// Decode one Pool log with a bound signature into the kind-specific fields.
pub fn decode(log: &eth::Log) -> Result<Option<pb::Action>, Error> {
    let Some(topic0) = log.topics.first() else { return Ok(None) };
    let t0 = hex::encode(topic0);
    let mut a = pb::Action::default();
    match t0.as_str() {
        SUPPLY_TOPIC => {
            shape(log, 4, 2, "Supply")?;
            a.kind = pb::ActionKind::Supply as i32;
            a.reserve = topic_address(&log.topics[1])?;
            a.beneficiary = topic_address(&log.topics[2])?;
            a.referral_code = topic_u16(&log.topics[3])?;
            a.actor = word_address(&log.data, 0)?;
            a.amount = word_uint(&log.data, 1)?;
        }
        WITHDRAW_TOPIC => {
            shape(log, 4, 1, "Withdraw")?;
            a.kind = pb::ActionKind::Withdraw as i32;
            a.reserve = topic_address(&log.topics[1])?;
            a.actor = topic_address(&log.topics[2])?;
            a.beneficiary = a.actor.clone();
            a.recipient = topic_address(&log.topics[3])?;
            a.amount = word_uint(&log.data, 0)?;
        }
        BORROW_TOPIC => {
            shape(log, 4, 4, "Borrow")?;
            a.kind = pb::ActionKind::Borrow as i32;
            a.reserve = topic_address(&log.topics[1])?;
            a.beneficiary = topic_address(&log.topics[2])?;
            a.referral_code = topic_u16(&log.topics[3])?;
            a.actor = word_address(&log.data, 0)?;
            a.amount = word_uint(&log.data, 1)?;
            a.interest_rate_mode = word_u8(&log.data, 2)?;
            a.borrow_rate = word_uint(&log.data, 3)?;
        }
        REPAY_TOPIC => {
            shape(log, 4, 2, "Repay")?;
            a.kind = pb::ActionKind::Repay as i32;
            a.reserve = topic_address(&log.topics[1])?;
            a.beneficiary = topic_address(&log.topics[2])?;
            a.actor = topic_address(&log.topics[3])?;
            a.amount = word_uint(&log.data, 0)?;
            a.use_atokens = word_bool(&log.data, 1)?;
        }
        LIQUIDATION_CALL_TOPIC => {
            shape(log, 4, 4, "LiquidationCall")?;
            a.kind = pb::ActionKind::LiquidationCall as i32;
            a.collateral_asset = topic_address(&log.topics[1])?;
            a.reserve = topic_address(&log.topics[2])?;
            a.beneficiary = topic_address(&log.topics[3])?;
            a.amount = word_uint(&log.data, 0)?;
            a.liquidated_collateral_amount = word_uint(&log.data, 1)?;
            a.actor = word_address(&log.data, 2)?;
            a.receive_atoken = word_bool(&log.data, 3)?;
        }
        FLASH_LOAN_TOPIC => {
            shape(log, 4, 4, "FlashLoan")?;
            a.kind = pb::ActionKind::FlashLoan as i32;
            a.beneficiary = topic_address(&log.topics[1])?;
            a.reserve = topic_address(&log.topics[2])?;
            a.referral_code = topic_u16(&log.topics[3])?;
            a.actor = word_address(&log.data, 0)?;
            a.amount = word_uint(&log.data, 1)?;
            a.interest_rate_mode = word_u8(&log.data, 2)?;
            a.premium = word_uint(&log.data, 3)?;
        }
        _ => return Ok(None),
    }
    Ok(Some(a))
}

struct Frame<'a> {
    scope: pb::Scope,
    tx_hash: &'a [u8],
    tx_index: u32,
    persisted_tx: bool,
}

pub fn project(block: &eth::Block, config: &Config) -> Result<pb::Events, Error> {
    let timestamp = validate_block(block, config)?;
    let header = block.header.as_ref().unwrap();
    let mut events = pb::Events::default();
    if !config.pools.is_empty() {
        let mut guards = Guards::default();
        persist::collect_block(block, &mut guards)?;
        for pool in &config.pools {
            require(
                !guards.codes.iter().any(|a| *a == pool.address || *a == pool.implementation),
                "Pool or implementation code changed; requalify the epoch",
            )?;
            require(
                !guards.pointer_writes.iter().any(|(a, k)| *a == pool.address && *k == pool.implementation_slot),
                "Pool implementation pointer changed; requalify the epoch",
            )?;
        }
        let mut push = |frame: &Frame, call: &eth::Call| -> Result<(), Error> {
            let persisted = frame.persisted_tx && !call.state_reverted;
            if !persisted && !config.include_attempted {
                return Ok(());
            }
            for log in &call.logs {
                let Some(pool) = config.pools.iter().find(|p| p.address == log.address) else {
                    continue;
                };
                if let Some(action) = decode(log)? {
                    events.actions.push(pb::Action {
                        chain_id: config.chain_id,
                        scope: frame.scope as i32,
                        transaction_hash: frame.tx_hash.to_vec(),
                        transaction_index: frame.tx_index,
                        call_index: call.index,
                        log_index: log.index,
                        block_index: log.block_index,
                        ordinal: log.ordinal,
                        pool: pool.address.clone(),
                        epoch: pool.epoch,
                        persisted,
                        ..action
                    });
                }
            }
            Ok(())
        };
        for tx in &block.transaction_traces {
            let frame = Frame {
                scope: pb::Scope::Transaction,
                tx_hash: &tx.hash,
                tx_index: tx.index,
                persisted_tx: tx.status == eth::TransactionTraceStatus::Succeeded as i32,
            };
            for call in &tx.calls {
                push(&frame, call)?;
            }
        }
        for call in &block.system_calls {
            let frame = Frame {
                scope: pb::Scope::SystemCall,
                tx_hash: &[],
                tx_index: 0,
                persisted_tx: true,
            };
            push(&frame, call)?;
        }
    }
    events
        .actions
        .sort_by_key(|a| (a.scope, a.transaction_index, a.ordinal, a.call_index, a.log_index));
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
        action_count: events.actions.len() as u32,
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
