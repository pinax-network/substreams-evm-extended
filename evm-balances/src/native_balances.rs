use proto::pb::evm::balances::v1 as pb;
use substreams::errors::Error;
use substreams_database_change::tables::Tables;

use crate::{address, amount, BlockColumns};

pub fn process_events(tables: &mut Tables, block: &BlockColumns, events: &pb::Events) -> Result<(), Error> {
    for balance in events.balances.iter() {
        if balance.contract.is_some() {
            return Err(Error::msg("native balance with a contract"));
        }
        let address = address(&balance.address, "native account")?;
        let row = tables
            .create_row("native_balances", &address)
            .set("address", &address)
            .set("balance", amount(&balance.amount)?);

        block.set(row);
    }
    Ok(())
}
