use prost::Message;
use serde_json::json;
use std::{collections::BTreeMap, fs, path::PathBuf};
use substreams_ethereum::pb::eth::v2 as eth;
const POOL: &str = "6807dc923806fe8fd134338eabca509979a7e0cb";
fn main() {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    let pool = hex::decode(POOL).unwrap();
    let topics: BTreeMap<&str, &str> = [
        ("2b627736bca15cd5381dcf80b0bf11fd197d01a037c52b927a881a10fb73ba61", "Supply"),
        ("3115d1449a7b732c986cba18244e897a450f61e1bb8d589cd2e69e6c8924f9f7", "Withdraw"),
        ("b3d084820fb1a9decffb176436bd02558d15fac9b0ddfed8c465bc7359d7dce0", "Borrow"),
        ("a534c8dbe71f871f9f3530e97a74601fea17b426cae02e1c5aee42c96c784051", "Repay"),
        ("e413a321e8681d831f4dbccbca790d2952b56f977908e45be37335533e005286", "LiquidationCall"),
        ("efefaba5e921573100900a3ad9cf29f222d995fb3b6045797eaea7521bd8d6f0", "FlashLoan"),
    ].into_iter().collect();
    let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
    let mut other: BTreeMap<String, u64> = BTreeMap::new();
    let mut examples = vec![];
    for dir in dirs {
        let mut files: Vec<PathBuf> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|x| x == "pb")).collect();
        files.sort();
        for path in files {
            let block = eth::Block::decode(fs::read(&path).unwrap().as_slice()).unwrap();
            for tx in &block.transaction_traces {
                for call in &tx.calls {
                    for log in call.logs.iter().filter(|l| l.address == pool && !l.topics.is_empty()) {
                        let t0 = hex::encode(&log.topics[0]);
                        match topics.get(t0.as_str()) {
                            Some(name) => {
                                *counts.entry(name).or_default() += 1;
                                if examples.len() < 40 { examples.push(json!({"block": block.number, "tx": tx.index, "status": tx.status, "reverted": call.state_reverted, "event": name, "topics": log.topics.iter().skip(1).map(|t| hex::encode(t)).collect::<Vec<_>>(), "data_len": log.data.len(), "ord": log.ordinal, "calls": tx.calls.len()})); }
                            }
                            None => *other.entry(t0[..16].to_string()).or_default() += 1,
                        }
                    }
                }
            }
        }
    }
    println!("{}", serde_json::to_string_pretty(&json!({"counts": counts, "other_pool_topics": other, "examples": examples})).unwrap());
}
