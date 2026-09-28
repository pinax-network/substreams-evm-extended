#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::{ptoken_proof as proof, ptoken_role as bound};
use serde_json::{json, Value};
const BASELINE: &[u8] = include_bytes!("../../tests/fixtures/bsc-refined450-layouts.json");
const CANDIDATE: &str = include_str!("../../tests/fixtures/ptoken-coupled-role-candidate/layouts.json");
fn review() -> Value {
    bound::review(
        include_bytes!("../../tests/fixtures/ptoken-operation-proof/capture.json"),
        include_bytes!("../../tests/fixtures/ptoken-operation-proof/compiler-output.json"),
        include_bytes!("../../docs/evidence/ptoken-operation-proof-20260928-transcripts.json"),
        include_bytes!("../../docs/evidence/ptoken-operation-proof-20260928.json"),
        include_bytes!("../../tests/fixtures/ptoken-operation-proof/primary-sources.json"),
    )
    .unwrap()
}
#[test]
fn ptoken_candidate_removes_only_legacy_membership_and_preserves_every_other_field() {
    let c: Value = serde_json::from_str(CANDIDATE).unwrap();
    bound::verify_candidate(BASELINE, &c).unwrap();
    let original: Value = serde_json::from_slice(BASELINE).unwrap();
    let original = original.as_array().unwrap().iter().find(|v| v["contract"] == bound::CONTRACT).unwrap();
    let mut restored = c[0].clone();
    restored.as_object_mut().unwrap().remove("enumerable_address_sets");
    restored["other_mapping_words"][bound::root(5)] = json!(2);
    assert_eq!(restored, *original);
    assert!(c[0].get("other_mapping_paths").is_none());
    assert_eq!(
        review(),
        serde_json::from_str::<Value>(include_str!("../../tests/fixtures/ptoken-coupled-role-candidate/source-review.json")).unwrap()
    );
}
#[test]
fn ptoken_candidate_rejects_independent_or_changed_fields_and_evidence() {
    let original: Value = serde_json::from_str(CANDIDATE).unwrap();
    for (field, value) in [
        (
            "other_mapping_paths",
            json!([{"root":bound::root(5),"key_types":["bytes32","address"],"words":1}]),
        ),
        ("other_mapping_words", json!({bound::root(5):2})),
        ("other_slots", json!([])),
        ("code_hash", json!(bound::root(99))),
    ] {
        let mut c = original.clone();
        c[0][field] = value;
        assert!(bound::verify_candidate(BASELINE, &c).is_err(), "{field}");
    }
    let mut baseline = BASELINE.to_vec();
    baseline.push(b' ');
    assert!(bound::candidate(&baseline).is_err());
    assert!(bound::review(b"{}", b"{}", b"[]", b"{}", b"{}").is_err());
    let capture = include_bytes!("../../tests/fixtures/ptoken-operation-proof/capture.json");
    let compiled = include_bytes!("../../tests/fixtures/ptoken-operation-proof/compiler-output.json");
    assert_eq!(proof::sha(capture), bound::CAPTURE);
    let primary = include_bytes!("../../tests/fixtures/ptoken-operation-proof/primary-sources.json");
    assert!(bound::review(capture, compiled, b"[]", b"{}", primary).is_err());
    let mut changed = primary.to_vec();
    changed.push(b' ');
    assert!(bound::review(
        capture,
        compiled,
        include_bytes!("../../docs/evidence/ptoken-operation-proof-20260928-transcripts.json"),
        include_bytes!("../../docs/evidence/ptoken-operation-proof-20260928.json"),
        &changed
    )
    .is_err());
}
