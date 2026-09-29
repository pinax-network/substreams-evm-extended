#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::{mai_proof as proof, mai_role as bound};
use serde_json::{json, Value};
use std::{fs, path::Path};
const BASELINE: &[u8] = include_bytes!("../../tests/fixtures/bsc-refined450-layouts.json");
const CANDIDATE: &str = include_str!("../../tests/fixtures/mai-coupled-role-candidate/layouts.json");

#[test]
fn mai_candidate_restores_exactly_two_legacy_permissions_and_every_other_field() {
    let candidate: Value = serde_json::from_str(CANDIDATE).unwrap();
    bound::verify_candidate(BASELINE, &candidate).unwrap();
    assert_eq!(candidate.as_array().unwrap().len(), 1);
    let baseline: Vec<Value> = serde_json::from_slice(BASELINE).unwrap();
    let original = baseline.iter().find(|v| v["contract"] == bound::ADDRESS).unwrap();
    let mut restored = candidate[0].clone();
    restored.as_object_mut().unwrap().remove("enumerable_address_sets");
    restored["other_mapping_slots"].as_array_mut().unwrap().insert(0, json!(bound::root(0)));
    restored["other_mapping_words"][bound::root(1)] = json!(2);
    assert_eq!(restored, *original);
    assert_eq!(candidate[0]["other_mapping_slots"], json!([bound::root(3)]));
    assert_eq!(candidate[0]["other_mapping_words"], json!({}));
    assert_eq!(candidate[0]["other_slots"], original["other_slots"]);
    assert!(candidate[0].get("other_mapping_paths").is_none());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    assert_eq!(
        bound::review(root).unwrap(),
        serde_json::from_str::<Value>(include_str!("../../tests/fixtures/mai-coupled-role-candidate/source-review.json")).unwrap()
    );
}

#[test]
fn mai_candidate_refuses_extra_broad_admin_creation_or_unrelated_permissions() {
    let candidate: Value = serde_json::from_str(CANDIDATE).unwrap();
    for (field, value) in [
        ("other_mapping_slots", json!([bound::root(0), bound::root(3)])),
        ("other_mapping_words", json!({bound::root(1):2})),
        (
            "other_mapping_paths",
            json!([{"root":bound::root(0),"key_types":["bytes32"],"offset":1,"words":1}]),
        ),
        ("other_slots", json!([bound::root(0), bound::root(4), bound::root(5), bound::root(6)])),
        ("balance_slot", json!(bound::root(3))),
        ("code_hash", json!(bound::root(9))),
        ("deployment", json!({})),
        ("proxy", json!({})),
        ("immutable_zero_mapping", json!(true)),
    ] {
        let mut changed = candidate.clone();
        changed[0][field] = value;
        assert!(bound::verify_candidate(BASELINE, &changed).is_err(), "{field}");
    }
    for (field, value) in [
        ("membership_root", json!(bound::root(1))),
        ("root", json!(bound::root(0))),
        ("semantics", json!("oz_3_4_2")),
    ] {
        let mut changed = candidate.clone();
        changed[0]["enumerable_address_sets"][0][field] = value;
        assert!(bound::verify_candidate(BASELINE, &changed).is_err());
    }
    let mut extra = candidate.clone();
    extra.as_array_mut().unwrap().push(candidate[0].clone());
    assert!(bound::verify_candidate(BASELINE, &extra).is_err());
    let mut raw = BASELINE.to_vec();
    raw.push(b' ');
    assert!(bound::candidate(&raw).is_err());
}

#[test]
fn mai_final_artifacts_and_source_directory_are_bound_with_no_admin_callsite() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    for (path, expected) in bound::ARTIFACTS {
        let mut raw = fs::read(root.join(path)).unwrap();
        assert_eq!(proof::sha(&raw), expected);
        raw.push(b' ');
        assert_ne!(proof::sha(&raw), expected);
    }
    let review = bound::review(root).unwrap();
    assert_eq!(review["qualified"], false);
    assert_eq!(review["admin_write_permission"], false);
    assert_eq!(review["creation_admission"], false);
    assert_eq!(review["optimizer_enabled"], false);
    assert_eq!(
        review["admin_definition_only"],
        json!([[
            "@openzeppelin/contracts/access/AccessControl.sol",
            214,
            "function _setRoleAdmin(bytes32 role, bytes32 adminRole) internal virtual {"
        ]])
    );
    assert_eq!(review["custom_primary_gap"], proof::PRIMARY_GAP);
    assert_eq!(review["frozen_phase_a_artifacts"].as_array().unwrap().len(), 16);
}

#[test]
fn mai_capture_source_settings_creation_and_cap_mutations_refuse() {
    let raw = include_bytes!("../../tests/fixtures/mai-operation-proof/capture.json");
    proof::verify_capture(raw).unwrap();
    let capture: Value = serde_json::from_slice(raw).unwrap();
    for (pointer, value) in [
        ("/address", json!("0x0000000000000000000000000000000000000000")),
        ("/chainId", json!(1)),
        ("/sources", json!({})),
        ("/stdJsonInput/settings/optimizer/enabled", json!(true)),
        ("/runtimeBytecode/onchainBytecode", json!("0x00")),
        ("/creationBytecode/onchainBytecode", json!("0x00")),
    ] {
        let mut changed = capture.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(proof::verify_capture(&serde_json::to_vec(&changed).unwrap()).is_err(), "{pointer}");
    }
    let compiled = include_bytes!("../../tests/fixtures/mai-operation-proof/compiler-output.json");
    proof::verify_compiled(&capture, compiled).unwrap();
    let mut changed: Value = serde_json::from_slice(compiled).unwrap();
    changed["contracts"][proof::SOURCE][proof::NAME]["evm"]["deployedBytecode"]["immutableReferences"] = json!({"1258":[{"start":1768,"length":32}]});
    assert!(proof::verify_compiled(&capture, &serde_json::to_vec(&changed).unwrap()).is_err());
}

#[test]
fn mai_raw_input_serialization_is_bound_to_the_frozen_compiler_report() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut refused = vec![];
    for name in ["compiler-input-original.json", "compiler-input.json"] {
        let temp = tempfile::tempdir().unwrap();
        for (path, _) in bound::ARTIFACTS {
            let target = temp.path().join(path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::copy(root.join(path), target).unwrap();
        }
        let dir = temp.path().join(bound::PROOF_FIXTURE);
        fs::create_dir_all(&dir).unwrap();
        for file in fs::read_dir(root.join(bound::PROOF_FIXTURE)).unwrap() {
            let file = file.unwrap();
            fs::copy(file.path(), dir.join(file.file_name())).unwrap();
        }
        let path = dir.join(name);
        let mut raw = fs::read(&path).unwrap();
        raw.push(b' ');
        fs::write(path, raw).unwrap();
        refused.push(bound::review(temp.path()).is_err());
    }
    assert_eq!(refused, vec![true, true]);
}

#[test]
fn mai_report_artifact_and_same_inventory_links_cannot_be_replaced() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let report: Value = serde_json::from_slice(&fs::read(root.join(bound::ARTIFACTS[0].0)).unwrap()).unwrap();
    let compiler: Value = serde_json::from_slice(&fs::read(root.join(bound::ARTIFACTS[3].0)).unwrap()).unwrap();
    bound::verify_report_links(root, &compiler, &report).unwrap();
    for field in [
        "source_inventory_sha256",
        "compiler_source_inventory_sha256",
        "cases_inventory_sha256",
        "transcripts_sha256",
    ] {
        let mut changed = report.clone();
        changed[field] = json!("00");
        assert!(bound::verify_report_links(root, &compiler, &changed).is_err(), "{field}");
    }
    let mut changed = report.clone();
    changed["source_artifacts"].as_array_mut().unwrap().last_mut().unwrap()["sha256"] = json!("00");
    assert!(bound::verify_report_links(root, &compiler, &changed).is_err());
    let mut changed = compiler.clone();
    changed["source_inventory_sha256"] = json!("00");
    assert!(bound::verify_report_links(root, &changed, &report).is_err());
    for remove in [true, false] {
        let mut changed = compiler.clone();
        if remove {
            changed["artifacts"].as_array_mut().unwrap().remove(4);
        } else {
            changed["artifacts"][4]["sha256"] = json!("00");
        }
        assert!(bound::verify_report_links(root, &changed, &report).is_err());
    }
}
