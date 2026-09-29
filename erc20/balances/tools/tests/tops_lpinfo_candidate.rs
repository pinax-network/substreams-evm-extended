#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::tops_lpinfo as bound;
use serde_json::{json, Value};
const BASELINE: &[u8] = include_bytes!("../../tests/fixtures/bsc-refined450-layouts.json");

#[test]
fn tops_both_complete_proof_generations_are_bound_with_distinct_limits() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let review = bound::review(root).unwrap();
    assert_eq!(bound::ARTIFACTS.len(), 35);
    assert_eq!(review["frozen_artifacts"].as_array().unwrap().len(), 35);
    assert_eq!(review["record_bound_before_and_after"], 6);
    assert_eq!(review["saved_source_count"], 9);
    assert_eq!(review["exact_primary_dependencies"], 8);
    for key in [
        "qualified",
        "creation_admission",
        "authorization_qualification",
        "external_dependency_qualification",
        "root32_independent_admission",
        "whole_router_path_qualification",
    ] {
        assert_eq!(review[key], false, "{key}");
    }
    assert_eq!(review["source_gap"], erc20_balances_tools::tops_proof::PRIMARY_GAP);
}

#[test]
fn tops_raw_serialization_and_report_link_tampering_refuse() {
    use std::{fs, path::Path};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let settings = fs::read(root.join("tests/fixtures/tops-operation-proof/settings.json")).unwrap();
    bound::verify_compiled_settings(&settings).unwrap();
    let mut whitespace_changed = settings.clone();
    whitespace_changed.push(b' ');
    assert_eq!(
        serde_json::from_slice::<Value>(&settings).unwrap(),
        serde_json::from_slice::<Value>(&whitespace_changed).unwrap()
    );
    assert!(bound::verify_compiled_settings(&whitespace_changed).is_err());
    let temporary = tempfile::tempdir().unwrap();
    for (path, _) in bound::ARTIFACTS {
        let target = temporary.path().join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(root.join(path), target).unwrap();
    }
    bound::verify_artifacts(temporary.path()).unwrap();
    for path in [
        "tests/fixtures/tops-operation-proof/original-input.json",
        "tests/fixtures/tops-operation-proof/full-input.json",
        "tests/fixtures/tops-operation-proof/settings.json",
        "docs/evidence/tops-runtime-cleanup-proof-20260929-preparation.json",
        "docs/evidence/tops-runtime-cleanup-proof-20260929-transcripts.json",
    ] {
        let p = temporary.path().join(path);
        let original = fs::read(&p).unwrap();
        let mut changed = original.clone();
        changed.push(b' ');
        fs::write(&p, changed).unwrap();
        assert!(bound::verify_artifacts(temporary.path()).is_err(), "{path}");
        fs::write(p, original).unwrap();
    }
    let read = |path: &str| serde_json::from_slice::<Value>(&fs::read(root.join(path)).unwrap()).unwrap();
    let compiler = read("docs/evidence/tops-operation-proof-20260929-compiler.json");
    let old = read("docs/evidence/tops-operation-proof-20260929.json");
    let preparation = read("docs/evidence/tops-runtime-cleanup-proof-20260929-preparation.json");
    let report = read("docs/evidence/tops-runtime-cleanup-proof-20260929.json");
    bound::verify_report_links(root, &compiler, &old, &preparation, &report).unwrap();
    for (field, value) in [
        ("source_report_sha256", json!("00")),
        ("source_inventory_sha256", json!("00")),
        ("cases_inventory_sha256", json!("00")),
        ("transcripts_sha256", json!("00")),
        ("historical_compiler_inventory_sha256", json!("00")),
        ("source_artifacts", json!({})),
        ("source_trees_equal", json!(false)),
    ] {
        let mut changed = report.clone();
        changed[field] = value;
        assert!(bound::verify_report_links(root, &compiler, &old, &preparation, &changed).is_err(), "{field}");
    }
    let mut changed = compiler.clone();
    changed["artifacts"][0]["sha256"] = json!("00");
    assert!(bound::verify_report_links(root, &changed, &old, &preparation, &report).is_err());
}

#[test]
fn tops_candidate_restores_only_broad_root32_and_preserves_every_original_field() {
    let candidate = bound::candidate(BASELINE).unwrap();
    bound::verify_candidate(BASELINE, &candidate).unwrap();
    assert_eq!(candidate.as_array().unwrap().len(), 1);
    let rows: Vec<Value> = serde_json::from_slice(BASELINE).unwrap();
    let original = rows.iter().find(|v| v["contract"] == bound::CONTRACT).unwrap();
    let mut restored = candidate[0].clone();
    restored.as_object_mut().unwrap().remove("lpinfo_array");
    restored["other_mapping_slots"].as_array_mut().unwrap().push(json!(bound::root(32)));
    assert_eq!(restored, *original);
    assert_eq!(candidate[0]["address_lists"], original["address_lists"]);
    assert_eq!(candidate[0]["other_slots"], original["other_slots"]);
    assert!(candidate[0].get("deployment").is_none());
}
#[test]
fn tops_candidate_refuses_baseline_drift_and_any_extra_or_missing_permission() {
    let candidate = bound::candidate(BASELINE).unwrap();
    let mut baseline = BASELINE.to_vec();
    baseline.push(b' ');
    assert!(bound::candidate(&baseline).is_err());
    for (field, value) in [
        ("other_mapping_slots", json!([bound::root(6), bound::root(32)])),
        ("other_slots", json!([bound::root(99)])),
        ("other_mapping_words", json!({bound::root(31):3})),
        (
            "other_mapping_paths",
            json!([{"root":bound::root(31),"key_types":["address"],"offset":0,"words":3}]),
        ),
        ("address_lists", json!([bound::root(28), bound::root(31)])),
        ("lpinfo_array", Value::Null),
        ("balance_slot", json!(bound::root(6))),
        ("code_hash", json!(bound::root(1))),
        ("deployment", json!({})),
        ("proxy", json!({})),
        ("immutable_zero_mapping", json!(true)),
    ] {
        let mut bad = candidate.clone();
        bad[0][field] = value;
        assert!(bound::verify_candidate(BASELINE, &bad).is_err(), "{field}");
    }
    let mut bad = candidate.clone();
    bad.as_array_mut().unwrap().push(candidate[0].clone());
    assert!(bound::verify_candidate(BASELINE, &bad).is_err());
}

#[test]
fn tops_counts_only_after_complete_projection_and_ignores_reverted_witnesses() {
    use prost::Message;
    use substreams_ethereum::pb::eth::v2 as eth;
    let b = eth::Block::decode(include_bytes!("../../tests/fixtures/bsc-exclusions-20260928/tops-123561227-tx63.pb").as_slice()).unwrap();
    let layouts = erc20_balances::layout::parse(&bound::candidate(BASELINE).unwrap().to_string()).unwrap();
    let (events, counts) = bound::project_counted(&b, &layouts).unwrap();
    assert_eq!(events.balances.len(), 2);
    assert_eq!(
        counts,
        bound::Counts {
            validated_appends: 0,
            validated_cleanups: 1,
            observed_length_decrements: 5,
            observed_metadata_stores: 21,
            observed_equal_metadata_stores: 0
        }
    );
    let mut discovered = b.clone();
    let mut hints = std::collections::HashMap::new();
    for call in discovered.transaction_traces.iter_mut().flat_map(|tx| &mut tx.calls) {
        hints.extend(std::mem::take(&mut call.keccak_preimages));
    }
    discovered.transaction_traces.push(eth::TransactionTrace {
        status: 2,
        calls: vec![eth::Call {
            address: hex::decode(&bound::CONTRACT[2..]).unwrap(),
            begin_ordinal: 1,
            end_ordinal: 2,
            keccak_preimages: hints,
            ..Default::default()
        }],
        ..Default::default()
    });
    assert_eq!(
        bound::project_counted(&discovered, &layouts).unwrap().1,
        counts,
        "reverted discovery hints do not erase accepted operations"
    );
    let mut missing = b.clone();
    let call = missing
        .transaction_traces
        .iter_mut()
        .flat_map(|tx| &mut tx.calls)
        .find(|c| c.storage_changes.len() == 23)
        .unwrap();
    call.storage_changes.remove(0);
    assert!(bound::project_counted(&missing, &layouts).is_err());
    for reverted in [false, true] {
        let mut ignored = b.clone();
        for tx in &mut ignored.transaction_traces {
            if reverted {
                for call in &mut tx.calls {
                    call.state_reverted = true;
                }
            } else {
                tx.status = 2;
            }
        }
        let (events, counts) = bound::project_counted(&ignored, &layouts).unwrap();
        assert!(events.balances.is_empty());
        assert_eq!(counts, bound::Counts::default());
    }
}
