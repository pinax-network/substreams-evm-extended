#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::securities_role as bound;
use serde_json::{json, Value};
const BASELINE: &[u8] = include_bytes!("../../tests/fixtures/bsc-refined450-layouts.json");
const CANDIDATE: &str = include_str!("../../tests/fixtures/securities-coupled-role-candidate/layouts.json");
fn inputs() -> [&'static [u8]; 6] {
    [
        include_bytes!("../../tests/fixtures/securities-operation-proof/capture.json"),
        include_bytes!("../../tests/fixtures/securities-operation-proof/compiler-output.json"),
        include_bytes!("../../docs/evidence/securities-operation-proof-20260928-transcripts.json"),
        include_bytes!("../../docs/evidence/securities-operation-proof-20260928.json"),
        include_bytes!("../../tests/fixtures/securities-operation-proof/primary-sources.json"),
        include_bytes!("../../docs/evidence/securities-operation-proof-20260928-compiler.json"),
    ]
}
#[test]
fn securities_seventeen_candidates_restore_every_baseline_guard_exactly() {
    let candidates: Value = serde_json::from_str(CANDIDATE).unwrap();
    bound::verify_candidate(BASELINE, &candidates).unwrap();
    assert_eq!(candidates.as_array().unwrap().len(), 17);
    let baseline: Value = serde_json::from_slice(BASELINE).unwrap();
    for c in candidates.as_array().unwrap() {
        let original = baseline.as_array().unwrap().iter().find(|v| v["contract"] == c["contract"]).unwrap();
        let mut restored = c.clone();
        restored.as_object_mut().unwrap().remove("enumerable_address_sets");
        assert_eq!(restored["other_slots"].as_array_mut().unwrap().pop(), Some(json!(bound::ADMIN_SLOT)));
        restored["other_mapping_words"][bound::MEMBERSHIP_ROOT] = json!(2);
        assert_eq!(&restored, original);
        assert!(c.get("other_mapping_paths").is_none());
    }
    let i = inputs();
    assert_eq!(
        bound::review(i[0], i[1], i[2], i[3], i[4], i[5]).unwrap(),
        serde_json::from_str::<Value>(include_str!("../../tests/fixtures/securities-coupled-role-candidate/source-review.json")).unwrap()
    );
    assert_eq!(format!("0x{}", hex::encode(bound::issuer_admin_slot())), bound::ADMIN_SLOT);
}
#[test]
fn securities_candidate_and_complete_frozen_proof_tampering_refuses() {
    let original: Value = serde_json::from_str(CANDIDATE).unwrap();
    for field in [
        "contract",
        "balance_slot",
        "code_hash",
        "beacon_proxy",
        "other_mapping_slots",
        "other_mapping_words",
        "other_slots",
        "enumerable_address_sets",
    ] {
        let mut c = original.clone();
        c[0][field] = json!(null);
        assert!(bound::verify_candidate(BASELINE, &c).is_err(), "{field}");
    }
    let mut c = original.clone();
    c.as_array_mut().unwrap().pop();
    assert!(bound::verify_candidate(BASELINE, &c).is_err());
    let mut c = original.clone();
    c[0]["other_mapping_words"][bound::MEMBERSHIP_ROOT] = json!(2);
    assert!(bound::verify_candidate(BASELINE, &c).is_err());
    let mut c = original;
    c[0]["other_mapping_paths"] = json!([{"root":bound::MEMBERSHIP_ROOT,"key_types":["bytes32","address"],"words":1}]);
    assert!(bound::verify_candidate(BASELINE, &c).is_err());
    let mut changed = BASELINE.to_vec();
    changed.push(b' ');
    assert!(bound::candidate(&changed).is_err());
    let i = inputs();
    for n in 0..6 {
        let mut changed = i[n].to_vec();
        changed.push(b' ');
        let mut supplied = i;
        supplied[n] = &changed;
        assert!(
            bound::review(supplied[0], supplied[1], supplied[2], supplied[3], supplied[4], supplied[5]).is_err(),
            "bound input {n}"
        );
    }
}
