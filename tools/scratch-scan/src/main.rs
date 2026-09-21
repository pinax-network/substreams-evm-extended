use prost::Message;
use serde_json::json;
use std::{collections::BTreeMap, fs, path::PathBuf};
use substreams_ethereum::pb::eth::v2 as eth;

const WBNB: &str = "bb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c";
const DEPOSIT: &str = "e1fffcc4923d04b559f4d29a8bfc6cda04eb5b0d3c460751c2402c5c5cc9109c";
const WITHDRAWAL: &str = "7fcf532c15f0a6db0bd6d0e038bea71d30d808c7d98cb3bf7268a95bf5081b65";
const TRANSFER: &str = "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

fn main() {
    let dir = std::env::args().nth(1).expect("blocks dir");
    let mode = std::env::args().nth(2).unwrap_or_else(|| "scan".into());
    let wbnb = hex::decode(WBNB).unwrap();
    let mut files: Vec<PathBuf> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|x| x == "pb")).collect();
    files.sort();
    if mode == "trim" {
        // trim <block> <tx_index> <out.pb>: keep header/identity and one transaction.
        let height: u64 = std::env::args().nth(3).unwrap().parse().unwrap();
        let tx_index: u32 = std::env::args().nth(4).unwrap().parse().unwrap();
        let out = std::env::args().nth(5).unwrap();
        let path = PathBuf::from(&dir).join(format!("{height}.pb"));
        let block = eth::Block::decode(fs::read(&path).unwrap().as_slice()).unwrap();
        let mut trimmed = eth::Block { transaction_traces: vec![], balance_changes: vec![], code_changes: vec![], system_calls: vec![], ..block.clone() };
        trimmed.transaction_traces = block.transaction_traces.iter().filter(|t| t.index == tx_index).cloned().collect();
        assert_eq!(trimmed.transaction_traces.len(), 1);
        fs::write(&out, trimmed.encode_to_vec()).unwrap();
        println!("{}", json!({"block": height, "hash": format!("0x{}", hex::encode(&block.hash)), "tx": format!("0x{}", hex::encode(&trimmed.transaction_traces[0].hash)), "source_sha256_note": "compute externally", "out": out}));
        return;
    }
    let mut summary: BTreeMap<&str, u64> = BTreeMap::new();
    let mut examples: Vec<serde_json::Value> = vec![];
    for path in &files {
        let block = eth::Block::decode(fs::read(path).unwrap().as_slice()).unwrap();
        for tx in &block.transaction_traces {
            let mut has_deposit = false;
            let mut has_withdrawal = false;
            let mut wbnb_transfers = 0;
            let mut reverted_wbnb_write = false;
            let mut delegate_wbnb_write = false;
            let mut contract_depositor = false;
            let mut zero_final = false;
            for call in &tx.calls {
                for log in &call.logs {
                    if log.address == wbnb && !log.topics.is_empty() {
                        let t0 = hex::encode(&log.topics[0]);
                        if t0 == DEPOSIT { has_deposit = true; if call.caller != tx.from && call.address == wbnb && call.caller.len() == 20 && call.depth > 0 { contract_depositor = true; } }
                        if t0 == WITHDRAWAL { has_withdrawal = true; }
                        if t0 == TRANSFER { wbnb_transfers += 1; }
                    }
                }
                let wbnb_writes = call.storage_changes.iter().filter(|c| c.address == wbnb).count();
                if wbnb_writes > 0 && call.state_reverted { reverted_wbnb_write = true; }
                if wbnb_writes > 0 && call.call_type == eth::CallType::Delegate as i32 { delegate_wbnb_write = true; }
                for c in call.storage_changes.iter().filter(|c| c.address == wbnb) {
                    if c.new_value.iter().all(|b| *b == 0) && !c.old_value.iter().all(|b| *b == 0) { zero_final = true; }
                }
            }
            if !(has_deposit || has_withdrawal) && !reverted_wbnb_write && !delegate_wbnb_write { continue; }
            if has_deposit { *summary.entry("deposit_txs").or_default() += 1; }
            if has_withdrawal { *summary.entry("withdrawal_txs").or_default() += 1; }
            if has_deposit && wbnb_transfers == 0 { *summary.entry("deposit_without_any_wbnb_transfer").or_default() += 1; }
            if has_withdrawal && wbnb_transfers == 0 { *summary.entry("withdrawal_without_any_wbnb_transfer").or_default() += 1; }
            if reverted_wbnb_write { *summary.entry("reverted_wbnb_write_txs").or_default() += 1; }
            if delegate_wbnb_write { *summary.entry("delegatecall_wbnb_write_txs").or_default() += 1; }
            if contract_depositor { *summary.entry("deposit_by_nested_contract").or_default() += 1; }
            if zero_final { *summary.entry("wbnb_zero_final_write_txs").or_default() += 1; }
            let interesting = (has_deposit && wbnb_transfers == 0) || (has_withdrawal && wbnb_transfers == 0) || reverted_wbnb_write || delegate_wbnb_write;
            if interesting && examples.len() < 60 {
                examples.push(json!({"block": block.number, "tx_index": tx.index, "tx": format!("0x{}", hex::encode(&tx.hash)), "status": tx.status, "calls": tx.calls.len(),
                    "deposit": has_deposit, "withdrawal": has_withdrawal, "wbnb_transfers": wbnb_transfers, "reverted_wbnb_write": reverted_wbnb_write, "delegate_wbnb_write": delegate_wbnb_write, "contract_depositor": contract_depositor, "zero_final": zero_final}));
            }
        }
    }
    println!("{}", serde_json::to_string_pretty(&json!({"files": files.len(), "summary": summary, "examples": examples})).unwrap());
}
