use prost::Message;
use serde_json::json;
use std::{collections::BTreeMap, fs, path::PathBuf};
use substreams_ethereum::pb::eth::v2 as eth;
// Aave V3 BNB legacy static aTokens (address book): USDT, USDC, WBNB, BTCB, ETH, Cake, FDUSD.
const TARGETS: [(&str, &str); 7] = [
    ("0471d185cc7be61e154277cab2396cd397663da6", "stataUSDT"),
    ("3906cddfb781f02b21f21bd81ed7fd8dc37075e1", "stataUSDC"),
    ("436bacb4c66583de4cb16e13a1a0d9a3075de425", "stataWBNB"),
    ("1f66b530084079d35478a069d9c4424f9c9c320c", "stataBTCB"),
    ("52077433fb7053d747e2846ad0c18ff5015c368e", "stataETH"),
    ("3854354ce3681da1d7f550073061e92a4a7d1b27", "stataCake"),
    ("4d074aaa0821073da827f7bf6a02cf905b394ed0", "stataFDUSD"),
];
fn main() {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    let targets: BTreeMap<Vec<u8>, &str> = TARGETS.iter().map(|(a, n)| (hex::decode(a).unwrap(), *n)).collect();
    let mut files: Vec<PathBuf> = vec![];
    for dir in &dirs {
        files.extend(fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|x| x == "pb")));
    }
    files.sort();
    let mut writes: BTreeMap<&str, u64> = BTreeMap::new();
    let mut calls: BTreeMap<&str, u64> = BTreeMap::new();
    let mut logs: BTreeMap<(&str, String), u64> = BTreeMap::new();
    let mut slots: BTreeMap<(&str, String), u64> = BTreeMap::new();
    let mut bases: BTreeMap<(&str, String), u64> = BTreeMap::new();
    let mut codes = vec![];
    let mut examples = vec![];
    let mut heights = vec![];
    for path in &files {
        let block = eth::Block::decode(fs::read(path).unwrap().as_slice()).unwrap();
        let mut touched = false;
        for tx in &block.transaction_traces {
            for call in &tx.calls {
                if let Some(n) = targets.get(&call.address) { *calls.entry(n).or_default() += 1; }
                for cc in &call.code_changes { if let Some(n) = targets.get(&cc.address) { codes.push(json!({"block": block.number, "tx": tx.index, "contract": n, "old": hex::encode(&cc.old_hash), "new": hex::encode(&cc.new_hash)})); } }
                if tx.status != 1 || call.state_reverted { continue; }
                for sc in &call.storage_changes {
                    if let Some(name) = targets.get(&sc.address) {
                        touched = true;
                        *writes.entry(name).or_default() += 1;
                        let key = hex::encode(&sc.key);
                        let pre = call.keccak_preimages.get(&key).cloned();
                        if let Some(p) = &pre { if p.len() == 128 { *bases.entry((name, p[64..].to_string())).or_default() += 1; } } else { *slots.entry((name, key.clone())).or_default() += 1; }
                        if examples.len() < 60 { examples.push(json!({"block": block.number, "tx": tx.index, "contract": name, "key": key, "old": hex::encode(&sc.old_value), "new": hex::encode(&sc.new_value), "ord": sc.ordinal, "preimage": pre})); }
                    }
                }
            }
            if tx.status == 1 { if let Some(r) = &tx.receipt { for l in &r.logs { if let Some(n) = targets.get(&l.address) { *logs.entry((n, hex::encode(l.topics.first().cloned().unwrap_or_default()))).or_default() += 1; } } } }
        }
        if touched { heights.push(block.number); }
    }
    let slots: Vec<_> = slots.iter().map(|((n, k), c)| json!({"contract": n, "key": k, "writes": c})).collect();
    let bases: Vec<_> = bases.iter().map(|((n, b), c)| json!({"contract": n, "base": b, "writes": c})).collect();
    let logs: Vec<_> = logs.iter().map(|((n, t), c)| json!({"contract": n, "topic0": t, "logs": c})).collect();
    println!("{}", serde_json::to_string_pretty(&json!({"files": files.len(), "calls": calls, "writes": writes, "blocks_with_writes": heights, "scalar_slots": slots, "mapping_bases": bases, "logs": logs, "code_changes": codes, "examples": examples})).unwrap());
}
