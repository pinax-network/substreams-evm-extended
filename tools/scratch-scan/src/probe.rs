use prost::Message;
use std::fs;
use substreams_ethereum::pb::eth::v2 as eth;
const DEPOSIT: &str = "e1fffcc4923d04b559f4d29a8bfc6cda04eb5b0d3c460751c2402c5c5cc9109c";
const WITHDRAWAL: &str = "7fcf532c15f0a6db0bd6d0e038bea71d30d808c7d98cb3bf7268a95bf5081b65";
const TRANSFER: &str = "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";
fn u(b: &[u8]) -> String { substreams::scalar::BigInt::from_unsigned_bytes_be(b).to_string() }
fn main() {
    let layouts = erc20_balances::layout::parse(&fs::read_to_string("../../erc20/balances/tests/fixtures/bsc-reviewed-layouts.json").unwrap()).unwrap();
    let wbnb = layouts.iter().find(|l| hex::encode(&l.contract) == "bb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c").unwrap().clone();
    let params = native_balances::parse_params(r#"{"producer_versions":[5]}"#).unwrap();
    for path in std::env::args().skip(1) {
        let block = eth::Block::decode(fs::read(&path).unwrap().as_slice()).unwrap();
        println!("\n##### {} block {} ver {} txs {}", path, block.number, block.ver, block.transaction_traces.len());
        let tx = &block.transaction_traces[0];
        println!("tx 0x{} status {} from 0x{} to 0x{} value {} calls {}", hex::encode(&tx.hash), tx.status, hex::encode(&tx.from), hex::encode(&tx.to), tx.value.as_ref().map(|v| u(&v.bytes)).unwrap_or_default(), tx.calls.len());
        for call in &tx.calls {
            let wl: Vec<_> = call.logs.iter().filter(|l| l.address == wbnb.contract).collect();
            let ws: Vec<_> = call.storage_changes.iter().filter(|c| c.address == wbnb.contract).collect();
            let wb: Vec<_> = call.balance_changes.iter().filter(|c| c.address == wbnb.contract).collect();
            if wl.is_empty() && ws.is_empty() && wb.is_empty() { continue; }
            println!(" call #{} parent {} depth {} type {} reverted {} caller 0x{} address 0x{} value {}", call.index, call.parent_index, call.depth, call.call_type, call.state_reverted, hex::encode(&call.caller), hex::encode(&call.address), call.value.as_ref().map(|v| u(&v.bytes)).unwrap_or_default());
            for l in wl {
                let t0 = hex::encode(&l.topics[0]);
                let kind = if t0 == DEPOSIT { "Deposit" } else if t0 == WITHDRAWAL { "Withdrawal" } else if t0 == TRANSFER { "Transfer" } else { "other" };
                println!("   log {} ord {} topics {:?} data {}", kind, l.ordinal, l.topics.iter().skip(1).map(|t| format!("0x{}", hex::encode(&t[12..]))).collect::<Vec<_>>(), u(&l.data));
            }
            for c in ws { println!("   sstore key 0x{} old {} new {} ord {}", hex::encode(&c.key), u(&c.old_value), u(&c.new_value), c.ordinal); }
            for b in wb { println!("   wbnb native balance old {} new {} reason {} ord {}", b.old_value.as_ref().map(|v| u(&v.bytes)).unwrap_or_default(), b.new_value.as_ref().map(|v| u(&v.bytes)).unwrap_or_default(), b.reason, b.ordinal); }
        }
        match erc20_balances::changes(&block, std::slice::from_ref(&wbnb)) {
            Ok(rows) => for r in rows { println!(" ERC20 row holder 0x{} old {} new {} ord {}", hex::encode(&r.address), r.old_amount, r.amount, r.ordinal); },
            Err(e) => println!(" ERC20 changes error: {e}"),
        }
        match native_balances::changes(&block, &params) {
            Ok(rows) => for r in rows { println!(" NATIVE row 0x{} old {} new {} records {} ord {}", hex::encode(&r.address), r.old_amount, r.amount, r.records, r.ordinal); },
            Err(e) => println!(" NATIVE changes error: {e}"),
        }
    }
}
