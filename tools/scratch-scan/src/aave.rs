use prost::Message;
use serde_json::json;
use std::{collections::BTreeMap, fs, path::PathBuf};
use substreams_ethereum::pb::eth::v2 as eth;
// Aave V3 BNB: Pool proxy, aBnbUSDT, aBnbUSDC, Pool impl, aToken impl.
const POOL: &str = "6807dc923806fe8fd134338eabca509979a7e0cb";
const AUSDT: &str = "a9251ca9de909cb71783723713b21e4233fbf1b1";
const AUSDC: &str = "00901a076785e0906d1028c7d6372d247bec7d61";
const USDT: &str = "55d398326f99059ff775485246999027b3197955";
fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let targets: BTreeMap<Vec<u8>, &str> = [(POOL, "pool"), (AUSDT, "aUSDT"), (AUSDC, "aUSDC")].iter().map(|(a, n)| (hex::decode(a).unwrap(), *n)).collect();
    let mut files: Vec<PathBuf> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|x| x == "pb")).collect();
    files.sort();
    let mut writes: BTreeMap<&str, u64> = BTreeMap::new();
    let mut blocks_with: BTreeMap<&str, u64> = BTreeMap::new();
    let mut slots: BTreeMap<(&str, String), u64> = BTreeMap::new();
    let mut preimage_bases: BTreeMap<String, u64> = BTreeMap::new();
    let mut examples = vec![];
    let usdt = hex::decode(USDT).unwrap();
    for path in &files {
        let block = eth::Block::decode(fs::read(path).unwrap().as_slice()).unwrap();
        let mut seen: BTreeMap<&str, bool> = BTreeMap::new();
        for tx in &block.transaction_traces {
            if tx.status != 1 { continue; }
            for call in &tx.calls {
                if call.state_reverted { continue; }
                for (k, v) in &call.keccak_preimages {
                    let vb = hex::decode(v).unwrap_or_default();
                    if vb.len() == 64 && vb[12..32] == usdt[..] { *preimage_bases.entry(format!("usdt-key base slot {}", hex::encode(&vb[32..]))).or_default() += 1; let _ = k; }
                }
                for sc in &call.storage_changes {
                    if let Some(name) = targets.get(&sc.address) {
                        *writes.entry(name).or_default() += 1;
                        seen.insert(name, true);
                        let key = hex::encode(&sc.key);
                        *slots.entry((name, key.clone())).or_default() += 1;
                        if examples.len() < 40 && *name != "pool" { examples.push(json!({"block": block.number, "tx": tx.index, "contract": name, "key": key, "old": hex::encode(&sc.old_value), "new": hex::encode(&sc.new_value), "ord": sc.ordinal, "preimage": call.keccak_preimages.get(&key).cloned()})); }
                    }
                }
            }
        }
        for (n, _) in seen { *blocks_with.entry(n).or_default() += 1; }
    }
    let mut top: Vec<_> = slots.iter().collect(); top.sort_by(|a, b| b.1.cmp(a.1));
    let top: Vec<_> = top.iter().take(30).map(|((n, k), c)| json!({"contract": n, "key": k, "writes": c})).collect();
    let mut pre: Vec<_> = preimage_bases.into_iter().collect(); pre.sort_by(|a, b| b.1.cmp(&a.1));
    println!("{}", serde_json::to_string_pretty(&json!({"files": files.len(), "writes": writes, "blocks_with_writes": blocks_with, "top_slots": top, "usdt_key_preimage_bases": pre.iter().take(12).collect::<Vec<_>>(), "examples": examples})).unwrap());
}
