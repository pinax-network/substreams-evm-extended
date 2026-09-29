#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::btr_role as bound;
use serde_json::{json, Value};
const BASELINE: &[u8] = include_bytes!("../../tests/fixtures/bsc-refined450-layouts.json");
const CANDIDATE: &str = include_str!("../../tests/fixtures/btr-coupled-role-candidate/layouts.json");
fn inputs() -> [&'static [u8]; 8] {
    [
        include_bytes!("../../tests/fixtures/btr-operation-proof/implementation-capture.json"),
        include_bytes!("../../tests/fixtures/btr-operation-proof/proxy-capture.json"),
        include_bytes!("../../tests/fixtures/btr-operation-proof/implementation-compiler-output.json"),
        include_bytes!("../../tests/fixtures/btr-operation-proof/proxy-compiler-output.json"),
        include_bytes!("../../docs/evidence/btr-operation-proof-20260928-transcripts.json"),
        include_bytes!("../../docs/evidence/btr-operation-proof-20260928.json"),
        include_bytes!("../../tests/fixtures/btr-operation-proof/primary-sources.json"),
        include_bytes!("../../docs/evidence/btr-operation-proof-20260928-compiler.json"),
    ]
}
#[test]
fn btr_role_only_candidate_restores_every_baseline_field_and_whitelist_permission() {
    let c: Value = serde_json::from_str(CANDIDATE).unwrap();
    bound::verify_candidate(BASELINE, &c).unwrap();
    assert_eq!(c.as_array().unwrap().len(), 1);
    let baseline: Value = serde_json::from_slice(BASELINE).unwrap();
    let original = baseline.as_array().unwrap().iter().find(|v| v["contract"] == bound::CONTRACT).unwrap();
    let mut restored = c[0].clone();
    restored.as_object_mut().unwrap().remove("enumerable_address_sets");
    assert_eq!(restored["other_slots"].as_array_mut().unwrap().pop(), Some(json!(bound::ADMIN_SLOT)));
    restored["other_mapping_words"][bound::root(101)] = json!(2);
    assert_eq!(&restored, original);
    assert!(c[0].get("other_mapping_paths").is_none());
    assert!(c[0]["other_slots"].as_array().unwrap().contains(&json!(bound::root(555))));
    assert!(c[0]["other_mapping_slots"].as_array().unwrap().contains(&json!(bound::root(556))));
    let i = inputs();
    assert_eq!(
        bound::review([i[0], i[1]], [i[2], i[3]], i[4], i[5], i[6], i[7]).unwrap(),
        serde_json::from_str::<Value>(include_str!("../../tests/fixtures/btr-coupled-role-candidate/source-review.json")).unwrap()
    );
    assert_eq!(format!("0x{}", hex::encode(bound::pauser_role())), bound::ADMIN_VALUE);
    assert_eq!(format!("0x{}", hex::encode(bound::admin_slot())), bound::ADMIN_SLOT);
}
#[test]
fn btr_rejects_every_candidate_scope_or_frozen_evidence_change() {
    let c: Value = serde_json::from_str(CANDIDATE).unwrap();
    for field in [
        "contract",
        "balance_slot",
        "code_hash",
        "proxy",
        "other_slots",
        "other_mapping_slots",
        "other_mapping_words",
        "enumerable_address_sets",
    ] {
        let mut changed = c.clone();
        changed[0][field] = json!(null);
        assert!(bound::verify_candidate(BASELINE, &changed).is_err(), "{field}");
    }
    let mut changed = c.clone();
    changed[0]["other_mapping_words"][bound::root(101)] = json!(2);
    assert!(bound::verify_candidate(BASELINE, &changed).is_err());
    let mut changed = c;
    changed[0]["other_mapping_paths"] = json!([{"root":bound::root(101),"key_types":["bytes32","address"],"words":1}]);
    assert!(bound::verify_candidate(BASELINE, &changed).is_err());
    let mut changed = BASELINE.to_vec();
    changed.push(b' ');
    assert!(bound::candidate(&changed).is_err());
    let i = inputs();
    for n in 0..8 {
        let mut changed = i[n].to_vec();
        changed.push(b' ');
        let mut supplied = i;
        supplied[n] = &changed;
        assert!(
            bound::review(
                [supplied[0], supplied[1]],
                [supplied[2], supplied[3]],
                supplied[4],
                supplied[5],
                supplied[6],
                supplied[7]
            )
            .is_err(),
            "frozen input {n}"
        );
    }
}
