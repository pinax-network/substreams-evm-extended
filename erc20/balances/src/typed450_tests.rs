//! Historical offline regressions for unqualified APD/DSG candidates.
//! These are saved canonical RPC rows, not fresh eth_call controls or profiles.
use super::*;
use prost::Message;
use serde_json::Value;

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!("../tests/fixtures/typed450-offline/cases.json")).unwrap()
}

fn captured(case: &Value) -> eth::Block {
    assert_eq!(case["qualified"], false);
    assert_eq!(case["fixture_sha256"], case["full_block_sha256"]);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/typed450-offline")
        .join(case["fixture"].as_str().unwrap());
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(format!("0x{}", hex::encode(hash(&bytes))), case["full_block_keccak256"]);
    let block = eth::Block::decode(bytes.as_slice()).unwrap();
    assert_eq!(block.number, case["block"].as_u64().unwrap());
    assert_eq!(block.hash, hex_bytes(case["hash"].as_str().unwrap()).unwrap());
    block
}

// Derive persisted storage records directly from captured execution status,
// independently of the mapper and its persistence collector.
fn persisted(block: &eth::Block) -> Vec<&eth::StorageChange> {
    let calls = block
        .transaction_traces
        .iter()
        .filter(|t| t.status() == eth::TransactionTraceStatus::Succeeded)
        .flat_map(|t| &t.calls)
        .chain(&block.system_calls);
    calls
        .filter(|c| !c.state_reverted)
        .flat_map(|c| &c.storage_changes)
        .filter(|s| BigInt::from_unsigned_bytes_be(&s.old_value) != BigInt::from_unsigned_bytes_be(&s.new_value))
        .collect()
}

fn changed_root_zero_holders(block: &eth::Block, contract: &[u8]) -> BTreeSet<Vec<u8>> {
    let mut preimages = BTreeMap::new();
    for call in block.system_calls.iter().chain(block.transaction_traces.iter().flat_map(|t| &t.calls)) {
        for (key, preimage) in &call.keccak_preimages {
            let key = hex_bytes(key).unwrap();
            let preimage = hex_bytes(preimage).unwrap();
            assert_eq!(hash(&preimage).as_slice(), key);
            if let Some(prior) = preimages.insert(key, preimage.clone()) {
                assert_eq!(prior, preimage);
            }
        }
    }
    persisted(block)
        .into_iter()
        .filter(|s| s.address == contract)
        .filter_map(|s| preimages.get(&s.key))
        .filter(|p| p.len() == 64 && p[..12].iter().all(|b| *b == 0) && p[32..].iter().all(|b| *b == 0))
        .map(|p| p[12..32].to_vec())
        .filter(|holder| holder.iter().any(|b| *b != 0))
        .collect()
}

fn without_allowances(case: &Value) -> Vec<VerifiedLayout> {
    let mut raw = case["layout"].clone();
    let paths = raw["other_mapping_paths"].as_array_mut().unwrap();
    let original = paths.len();
    paths.retain(|p| p["root"] != format!("0x{:064x}", 1));
    assert_eq!(paths.len() + 1, original);
    layout::parse(&serde_json::json!([raw]).to_string()).unwrap()
}

#[test]
fn unqualified_typed450_full_blocks_match_saved_rpc_for_exact_changed_holder_sets() {
    let cases = cases();
    assert_eq!(cases.len(), 2);
    let mut checked = 0;
    let mut gaps = 0;
    let mut nonzero_gaps = 0;
    for case in &cases {
        let block = captured(case);
        let contract = hex_bytes(case["contract"].as_str().unwrap()).unwrap();
        let changed = changed_root_zero_holders(&block, &contract);
        let reference_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/typed450-offline")
            .join(case["provenance"]["expected"].as_str().unwrap());
        let reference: Value = serde_json::from_slice(&std::fs::read(reference_path).unwrap()).unwrap();
        assert_eq!(reference["fresh_eth_call"], false);
        assert_eq!(reference["contract"], case["contract"]);
        assert_eq!(reference["block"], case["block"]);
        let canonical_rows = reference["balances"].as_array().unwrap();
        let expected = case["expected_emitted"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                assert_eq!(row["contract"], case["contract"]);
                assert!(canonical_rows.contains(row));
                (hex_bytes(row["address"].as_str().unwrap()).unwrap(), row["amount"].as_str().unwrap().to_owned())
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(expected.len(), case["expected_emitted"].as_array().unwrap().len());
        assert_eq!(expected.keys().cloned().collect::<BTreeSet<_>>(), changed);
        let layouts = layout::parse(&serde_json::json!([case["layout"]]).to_string()).unwrap();
        let events = project(&block, &layouts).unwrap();
        assert_eq!(events.balances.len(), expected.len());
        assert!(events.balances.iter().all(|r| r.contract.as_ref() == Some(&contract)));
        assert_eq!(events.balances.into_iter().map(|r| (r.address, r.amount)).collect::<BTreeMap<_, _>>(), expected);
        checked += expected.len();
        let mut gap_holders = BTreeSet::new();
        for gap in case["reference_only_gaps"].as_array().unwrap() {
            let row = &gap["reference_row"];
            assert_eq!(row["contract"], case["contract"]);
            assert!(canonical_rows.contains(row));
            let holder = hex_bytes(row["address"].as_str().unwrap()).unwrap();
            assert!(!changed.contains(&holder));
            assert!(gap_holders.insert(holder));
            assert!(!gap["reason"].as_str().unwrap().is_empty());
            gaps += 1;
            nonzero_gaps += usize::from(row["amount"] != "0");
        }
        assert_eq!(expected.len() + case["reference_only_gaps"].as_array().unwrap().len(), canonical_rows.len());
        // Removing precisely the reviewed allowance path must reproduce the
        // original refusal, including when that allowance is later restored.
        assert_eq!(
            project(&block, &without_allowances(case)).unwrap_err().to_string(),
            case["expected_allowance_refusal"]
        );
    }
    assert_eq!(checked, 5);
    assert_eq!(gaps, 6);
    assert_eq!(nonzero_gaps, 2);
}

#[test]
fn dsg_restored_allowance_writes_are_checked_without_any_balance_write() {
    let case = cases().into_iter().find(|c| c["rank"] == 418).unwrap();
    let mut block = captured(&case);
    let contract = hex_bytes(case["contract"].as_str().unwrap()).unwrap();
    let key = hex_bytes(case["allowance_writes"][0]["key"].as_str().unwrap()).unwrap();
    let mut writes = persisted(&block)
        .into_iter()
        .filter(|s| s.address == contract && s.key == key)
        .cloned()
        .collect::<Vec<_>>();
    writes.sort_by_key(|s| s.ordinal);
    assert_eq!(writes.len(), 2);
    assert_ne!(writes[0].old_value, writes[0].new_value);
    assert_eq!(writes[0].new_value, writes[1].old_value);
    assert_eq!(writes[1].new_value, writes[0].old_value);
    for (actual, saved) in writes.iter().zip(case["allowance_writes"].as_array().unwrap()) {
        assert_eq!(actual.old_value, hex_bytes(saved["old"].as_str().unwrap()).unwrap());
        assert_eq!(actual.new_value, hex_bytes(saved["new"].as_str().unwrap()).unwrap());
        assert_eq!(actual.ordinal, saved["ordinal"].as_u64().unwrap());
    }
    // Keep the original complete block call/preimage context; retain only the
    // selected historical allowance writes to isolate metadata-only admission.
    for call in block
        .system_calls
        .iter_mut()
        .chain(block.transaction_traces.iter_mut().flat_map(|t| &mut t.calls))
    {
        call.storage_changes.retain(|s| s.address == contract && s.key == key);
    }
    assert!(changed_root_zero_holders(&block, &contract).is_empty());
    assert_eq!(persisted(&block).len(), 2);
    let layouts = layout::parse(&serde_json::json!([case["layout"]]).to_string()).unwrap();
    assert!(project(&block, &layouts).unwrap().balances.is_empty());
    assert_eq!(
        project(&block, &without_allowances(&case)).unwrap_err().to_string(),
        case["expected_allowance_refusal"]
    );
}
