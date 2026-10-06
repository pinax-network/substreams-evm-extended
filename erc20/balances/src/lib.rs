//! A single RPC-free map with the shared ERC-20 Events output: per-block
//! transfer-guided balances inferred from persisted storage (`src/infer.rs`).
mod infer;
pub mod persist;

use proto::pb::evm::balances::v1 as balances_pb;
use substreams::{errors::Error, scalar::BigInt};
use substreams_ethereum::pb::eth::v2 as eth;
use tiny_keccak::{Hasher, Keccak};

fn require(ok: bool, message: &str) -> Result<(), Error> {
    if ok {
        Ok(())
    } else {
        Err(Error::msg(message.to_string()))
    }
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    let mut result = [0; 32];
    let mut hasher = Keccak::v256();
    hasher.update(bytes);
    hasher.finalize(&mut result);
    result
}
fn mapping(owner: &[u8], position: &[u8; 32]) -> [u8; 32] {
    let mut preimage = [0; 64];
    preimage[12..32].copy_from_slice(owner);
    preimage[32..].copy_from_slice(position);
    hash(&preimage)
}
fn validate_block(block: &eth::Block) -> Result<(), Error> {
    require(block.detail_level == eth::block::DetailLevel::DetaillevelExtended, "Extended blocks required")?;
    require((3..=5).contains(&block.ver), "unsupported Extended producer version")?;
    let header = block.header.as_option().ok_or_else(|| Error::msg("missing header"))?;
    require(
        block.hash.len() == 32 && header.parent_hash.len() == 32 && header.state_root.len() == 32,
        "invalid block identity",
    )?;
    require(header.number == block.number, "header number mismatch")?;
    for tx in &block.transaction_traces {
        require(
            (1..=3).contains(&tx.status.to_i32()) && !tx.calls.is_empty(),
            "incomplete transaction persistence data",
        )?;
    }
    Ok(())
}

/// Discards every record: `run` needs only the persistence rules' verdict.
struct Discard;
impl persist::Sink for Discard {
    fn storage(&mut self, _: &eth::StorageChange, _: persist::Ctx) {}
    fn balance(&mut self, _: &eth::BalanceChange, _: persist::Ctx) {}
    fn nonce(&mut self, _: &eth::NonceChange, _: persist::Ctx) {}
    fn code(&mut self, _: &eth::CodeChange, _: persist::Ctx) {}
}

/// The `map_events` body. A block fails only when it is not a complete
/// Extended block or the persistence rules cannot resolve it. Inference never
/// fails a block: on any doubt it drops that contract's rows for the block.
pub fn run(block: &eth::Block) -> Result<balances_pb::Events, Error> {
    validate_block(block)?;
    persist::collect_block(block, &mut Discard)?;
    let balances = infer::rows(block)
        .into_iter()
        .map(|((contract, address), value)| balances_pb::Balance {
            contract: Some(contract.to_vec()),
            address: address.to_vec(),
            amount: BigInt::from_unsigned_bytes_be(&value).to_string(),
        })
        .collect();
    Ok(balances_pb::Events { balances })
}
// The SDK macro generates raw-pointer parameter decoding and discards function
// attributes. Keep its ABI-specific lint exception scoped to this wrapper. The
// export exists only in the WASM build, as in the other packages.
#[cfg(target_arch = "wasm32")]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
mod handler {
    use super::*;
    #[substreams::handlers::map]
    fn map_events(block: eth::Block) -> Result<balances_pb::Events, Error> {
        run(&block)
    }
}
