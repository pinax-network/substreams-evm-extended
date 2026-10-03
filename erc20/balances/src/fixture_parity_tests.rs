use super::*;
use prost::Message;
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};

/// The reviewed core layouts on the complete captured block emit only balances
/// equal to the saved same-block RPC reference, for all four core tokens.
#[test]
fn core_layouts_match_the_rpc_reference_on_the_captured_block() {
    let block = eth::Block::decode(include_bytes!("../tests/fixtures/bsc-122260950.pb").as_slice()).unwrap();
    let layouts = layout::parse(include_str!("../tests/fixtures/bsc-reviewed-layouts.json")).unwrap();
    let reference: Value = serde_json::from_str(include_str!("../tests/fixtures/bsc-122260950-core-reference.json")).unwrap();
    assert_eq!(reference["block"], block.number);
    let expected: BTreeMap<(String, String), String> = reference["balances"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                (row["contract"].as_str().unwrap().to_string(), row["address"].as_str().unwrap().to_string()),
                row["amount"].as_str().unwrap().to_string(),
            )
        })
        .collect();

    let events = project(&block, &layouts).unwrap();
    let mut contracts = std::collections::BTreeSet::new();
    for balance in &events.balances {
        let key = (
            format!("0x{}", hex::encode(balance.contract.as_ref().unwrap())),
            format!("0x{}", hex::encode(&balance.address)),
        );
        assert_eq!(expected.get(&key), Some(&balance.amount), "{key:?}");
        contracts.insert(key.0);
    }
    assert_eq!(contracts.len(), 4);
    assert!(events.balances.len() > 18);
}

/// Every committed layout set (qualified rankings, cohorts and candidates)
/// still parses with the current mapper.
#[test]
fn every_committed_layout_set_parses() {
    fn collect(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(&path, out);
            } else if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("layouts.json") || n.ends_with("-layout.json"))
            {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    collect(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"), &mut files);
    assert!(files.len() > 20, "{files:?}");
    let failed: Vec<String> = files
        .iter()
        .filter_map(|path| {
            layout::parse(&fs::read_to_string(path).unwrap())
                .err()
                .map(|e| format!("{}: {e}", path.display()))
        })
        .collect();
    assert!(failed.is_empty(), "{failed:#?}");
}
