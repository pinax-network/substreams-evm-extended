//! One `db_out` that writes the balance maps of `native/balances` and
//! `erc20/balances` into the substreams-evm ClickHouse tables.
//!
//! The rows have the format of the upstream `db_out`
//! ([substreams-evm@cb8607f](https://github.com/pinax-network/substreams-evm/tree/cb8607f59a37aa9daf85547162b8871d12568282/db-evm-balances/src)),
//! so a database started from this package can take the RPC-era history by
//! partition copy:
//! - `erc20_balances`, keyed by `(contract, address)`;
//! - `native_balances`, keyed by `address`;
//! - one `blocks` row, keyed by `block_num`, only when the block wrote a
//!   balance row.
//!
//! Every row is a CREATE and carries `block_num`, `block_hash` (`0x` + clock
//! id) and `timestamp` (clock seconds). Addresses are `0x` + lowercase hex and
//! `balance` is `Balance.amount` verbatim. Upstream's `tron_base58` encoding
//! param and its genesis-clock patch for other chains are not ported.
//!
//! Unlike upstream, a row the SQL sink would store wrongly fails the block: a
//! native row with a contract, an ERC-20 row without one, an address that is
//! not 20 bytes, or an amount that is not a decimal `uint256`.
//!
//! The changes are encoded by `substreams-database-change` 5.0.0, where every
//! `Field` also carries `update_op = UPDATE_OP_SET` (field 4). Upstream's 3.0.0
//! has no such field; the SQL sink uses only a CREATE field's name and value,
//! so the stored rows are the same.
mod erc20_balances;
mod native_balances;

use proto::pb::evm::balances::v1 as pb;
use substreams::{errors::Error, pb::substreams::Clock, Hex};
use substreams_database_change::{
    pb::sf::substreams::sink::database::v1::DatabaseChanges,
    tables::{Row, Tables},
};

/// `2^256 - 1`, the largest value of the `UInt256` balance columns.
const MAX_UINT256: &str = "115792089237316195423570985008687907853269984665640564039457584007913129639935";

/// The block columns every row carries.
struct BlockColumns {
    block_num: String,
    block_hash: String,
    timestamp: String,
}

impl BlockColumns {
    fn new(clock: &Clock) -> Result<Self, Error> {
        let timestamp = clock.timestamp.as_option().ok_or_else(|| Error::msg("missing clock timestamp"))?;
        Ok(Self {
            block_num: clock.number.to_string(),
            block_hash: format!("0x{}", clock.id),
            timestamp: timestamp.seconds.to_string(),
        })
    }

    fn set(&self, row: &mut Row) {
        row.set("block_num", &self.block_num)
            .set("block_hash", &self.block_hash)
            .set("timestamp", &self.timestamp);
    }
}

/// `0x` + lowercase hex of a 20-byte address.
fn address(bytes: &[u8], what: &str) -> Result<String, Error> {
    if bytes.len() != 20 {
        return Err(Error::msg(format!("{what} is not a 20-byte address")));
    }
    Ok(format!("0x{}", Hex::encode(bytes)))
}

/// `Balance.amount` verbatim when it is a canonical decimal `uint256`. The SQL
/// sink parses `UInt256` without checking: anything else would be stored as a
/// wrong number or abort the sink.
fn amount(amount: &str) -> Result<&str, Error> {
    let decimal = !amount.is_empty() && amount.bytes().all(|b| b.is_ascii_digit()) && (amount == "0" || !amount.starts_with('0'));
    let in_range = amount.len() < MAX_UINT256.len() || (amount.len() == MAX_UINT256.len() && amount <= MAX_UINT256);
    if decimal && in_range {
        Ok(amount)
    } else {
        Err(Error::msg(format!("balance {amount:?} is not a decimal uint256")))
    }
}

/// The `db_out` body: ERC-20 rows, then native rows, then the `blocks` row.
pub fn database_changes(clock: &Clock, native: &pb::Events, erc20: &pb::Events) -> Result<DatabaseChanges, Error> {
    let block = BlockColumns::new(clock)?;
    let mut tables = Tables::new();

    // -- ERC20 Balances --
    erc20_balances::process_events(&mut tables, &block, erc20)?;

    // -- Native Balances --
    native_balances::process_events(&mut tables, &block, native)?;

    // ONLY include blocks if events are present. Every table `Tables` holds has
    // at least one row, so this is upstream's `!tables.tables.is_empty()`.
    if tables.all_row_count() != 0 {
        block.set(tables.create_row("blocks", [("block_num", block.block_num.clone())]));
    }

    substreams::log::info!("Total rows {}", tables.all_row_count());
    Ok(tables.to_database_changes())
}

// The SDK macro generates raw-pointer parameter decoding and discards function
// attributes. As in the map packages, the export exists only in the WASM build.
#[cfg(target_arch = "wasm32")]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
mod handler {
    use super::*;
    #[substreams::handlers::map]
    fn db_out(clock: Clock, native_balance_events: pb::Events, erc20_balance_events: pb::Events) -> Result<DatabaseChanges, Error> {
        database_changes(&clock, &native_balance_events, &erc20_balance_events)
    }
}
