use proto::pb::evm::balances::v1 as pb;
use substreams::errors::Error;
use substreams_database_change::tables::Tables;

use crate::{address, amount, BlockColumns};

pub fn process_events(tables: &mut Tables, block: &BlockColumns, events: &pb::Events) -> Result<(), Error> {
    for balance in events.balances.iter() {
        let contract_bytes = balance.contract.as_deref().ok_or_else(|| Error::msg("ERC-20 balance without a contract"))?;
        let contract = address(contract_bytes, "ERC-20 contract")?;
        let address = address(&balance.address, "ERC-20 holder")?;
        let key = [("contract", contract.to_string()), ("address", address.to_string())];
        let row = tables
            .create_row("erc20_balances", key)
            .set("contract", &contract)
            .set("address", &address)
            .set("balance", amount(&balance.amount)?);

        block.set(row);
    }
    Ok(())
}
