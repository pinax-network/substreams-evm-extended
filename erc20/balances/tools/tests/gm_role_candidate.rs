#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::{gm_proof as proof, gm_role as bound};
use serde_json::{json, Value};
use std::path::Path;
const BASELINE: &[u8] = include_bytes!("../../tests/fixtures/bsc-refined450-layouts.json");
const CANDIDATE: &str = include_str!("../../tests/fixtures/gm-coupled-role-candidate/layouts.json");
#[test]
fn gm_two_profiles_restore_every_baseline_field_including_finite_long_names() {
    let c: Value = serde_json::from_str(CANDIDATE).unwrap();
    bound::verify_candidate(BASELINE, &c).unwrap();
    assert_eq!(c.as_array().unwrap().len(), 2);
    let baseline: Vec<Value> = serde_json::from_slice(BASELINE).unwrap();
    for profile in c.as_array().unwrap() {
        let original = baseline.iter().find(|v| v["contract"] == profile["contract"]).unwrap();
        let mut restored = profile.clone();
        restored.as_object_mut().unwrap().remove("enumerable_address_sets");
        restored["other_mapping_words"][bound::root(201)] = json!(2);
        assert_eq!(restored, *original);
        assert!(profile.get("other_mapping_paths").is_none());
        for word in bound::long_name_words() {
            assert!(profile["other_slots"].as_array().unwrap().contains(&json!(word)));
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    assert_eq!(
        bound::review(root).unwrap(),
        serde_json::from_str::<Value>(include_str!("../../tests/fixtures/gm-coupled-role-candidate/source-review.json")).unwrap()
    );
}
#[test]
fn gm_candidate_refuses_extra_profile_admin_permission_or_unrelated_metadata_change() {
    let c: Value = serde_json::from_str(CANDIDATE).unwrap();
    for profile in 0..2 {
        for (field, value) in [
            (
                "other_mapping_paths",
                json!([{"root":bound::root(201),"key_types":["bytes32"],"offset":1,"words":1}]),
            ),
            ("other_mapping_words", json!({bound::root(201):2})),
            ("other_slots", json!([])),
            ("code_hash", json!(bound::root(9))),
            ("deployment", json!({})),
            ("beacon_proxy", json!({})),
        ] {
            let mut changed = c.clone();
            changed[profile][field] = value;
            assert!(bound::verify_candidate(BASELINE, &changed).is_err(), "{field}");
        }
    }
    let mut extra = c.clone();
    extra.as_array_mut().unwrap().push(c[0].clone());
    assert!(bound::verify_candidate(BASELINE, &extra).is_err());
    let mut baseline = BASELINE.to_vec();
    baseline.push(b' ');
    assert!(bound::candidate(&baseline).is_err());
}
#[test]
fn gm_artifact_pins_are_exact_and_no_admin_writer_is_reachable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    for (path, expected) in bound::ARTIFACTS {
        let bytes = std::fs::read(root.join(path)).unwrap();
        assert_eq!(proof::sha(&bytes), expected);
        let mut changed = bytes;
        changed.push(b' ');
        assert_ne!(proof::sha(&changed), expected);
    }
    let review = bound::review(root).unwrap();
    assert_eq!(review["admin_write_permission"], false);
    assert_eq!(review["admin_definition_only"].as_array().unwrap().len(), 1);
    assert_eq!(review["admin_definition_only"][0][1], 257);
    assert_eq!(review["qualified"], false);
}
