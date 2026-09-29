#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::{
    bnbtiger_cookie_candidates as c,
    bnbtiger_cookie_proof::{self as proof, Target},
};
use serde_json::{json, Value};
use std::{fs, path::PathBuf};
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).unwrap().to_owned()
}
#[test]
fn complete_frozen_artifacts_and_source_writers_bind_candidates() {
    let root = root();
    let review = c::review(&root).unwrap();
    assert_eq!(review["phase_a_artifacts"].as_array().unwrap().len(), 29);
    for t in Target::ALL {
        let dir = root.join(proof::FIXTURE);
        let capture = proof::verify_capture(&fs::read(dir.join(format!("{}-capture.json", t.label()))).unwrap(), t).unwrap();
        let writer = proof::writers::review(&capture, t).unwrap();
        let fields = writer["fields"].as_array().unwrap();
        assert_eq!(fields.len(), if t == Target::Bnbtiger { 36 } else { 23 });
        for f in fields {
            assert!(!f["anchors"].as_array().unwrap().is_empty());
        }
        let location = |name: &str| fields.iter().find(|f| f["declaration"]["label"] == name).unwrap()["declaration"].clone();
        if t == Target::Bnbtiger {
            assert_eq!(location("_decimals")["offset"], 0);
            assert_eq!(location("marketingWalletAddress")["offset"], 1);
            assert_eq!(location("checkWalletLimit")["offset"], 23);
            assert_eq!(location("_totalSupply")["slot"], "25");
        } else {
            assert_eq!(location("maxHoldingRate")["offset"], 21);
            assert_eq!(location("_inSwapAndLiquify")["offset"], 20);
            assert_eq!(location("checkpoints")["slot"], "15");
            assert_eq!(location("transferTaxRate")["offset"], 1);
            assert_eq!(location("burnRate")["offset"], 3);
            assert_eq!(location("maxTransferAmountRate")["offset"], 5);
        }
    }
}
#[test]
fn exactly_two_noncohort_additions_restore_every_original_profile() {
    let raw = fs::read(root().join("erc20/balances/tests/fixtures/bsc-refined450-layouts.json")).unwrap();
    let baseline: Value = serde_json::from_slice(&raw).unwrap();
    let selected = c::candidates(&raw).unwrap();
    let all = c::combined(&raw, &selected).unwrap();
    assert_eq!(&all.as_array().unwrap()[..431], baseline.as_array().unwrap());
    assert_eq!(all.as_array().unwrap().len(), 433);
    for index in 0..2 {
        for field in ["contract", "code_hash", "balance_slot", "metadata_semantics"] {
            let mut changed = selected.clone();
            changed[index][field] = json!("changed");
            assert!(c::combined(&raw, &changed).is_err());
        }
        let mut changed = selected.clone();
        changed[index]["other_slots"] = json!([]);
        assert!(c::combined(&raw, &changed).is_err());
    }
    let mut raw = raw;
    raw.push(b' ');
    assert!(c::candidates(&raw).is_err());
}
#[test]
fn every_raw_artifact_tamper_is_refused_before_using_historical_labels() {
    let temp = tempfile::tempdir().unwrap();
    for (path, _) in c::artifact_pins() {
        let dst = temp.path().join(&path);
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        fs::copy(root().join(path), dst).unwrap();
    }
    c::review(temp.path()).unwrap();
    for (path, _) in c::artifact_pins() {
        let dst = temp.path().join(path);
        let raw = fs::read(&dst).unwrap();
        let mut changed = raw.clone();
        changed.push(b' ');
        fs::write(&dst, changed).unwrap();
        assert!(c::review(temp.path()).is_err());
        fs::write(dst, raw).unwrap();
    }
}

#[test]
fn metadata_counter_separates_changed_fields_equal_word_ambiguity_and_reverts() {
    use substreams_ethereum::pb::eth::v2 as eth;
    let review = c::review(&root()).unwrap();
    let mut old = [0u8; 32];
    old[31] = 18;
    let mut new = old;
    new[30] = 1;
    let record = eth::StorageChange {
        address: hex::decode(&Target::Cookie.address()[2..]).unwrap(),
        key: {
            let mut k = vec![0; 32];
            k[31] = 6;
            k
        },
        old_value: old.to_vec(),
        new_value: new.to_vec(),
        ordinal: 2,
    };
    let mut block = eth::Block {
        transaction_traces: vec![eth::TransactionTrace {
            status: eth::TransactionTraceStatus::Succeeded as i32,
            calls: vec![eth::Call {
                storage_changes: vec![record.clone()],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let rows = c::metadata_observations(&block, &review).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["source_fields"], json!(["transferTaxRate"]));
    assert_eq!(rows[0]["equal_record"], false);
    block.transaction_traces[0].calls[0].storage_changes[0].old_value = new.to_vec();
    let rows = c::metadata_observations(&block, &review).unwrap();
    assert_eq!(rows[0]["equal_record"], true);
    assert_eq!(rows[0]["source_fields"].as_array().unwrap().len(), 4);
    assert!(rows[0]["attribution"].as_str().unwrap().contains("writer not determined"));
    block.transaction_traces[0].calls[0].state_reverted = true;
    assert!(c::metadata_observations(&block, &review).unwrap().is_empty());
    block.transaction_traces[0].calls[0].state_reverted = false;
    block.transaction_traces[0].status = eth::TransactionTraceStatus::Failed as i32;
    assert!(c::metadata_observations(&block, &review).unwrap().is_empty());
}
