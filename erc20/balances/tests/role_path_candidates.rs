#![cfg(not(target_arch = "wasm32"))]

//! Bounded offline candidates; these checks do not qualify the published layouts.
use erc20_balances::{hash, layout, project};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use substreams_ethereum::pb::eth::v2 as eth;

const CANDIDATES: &str = include_str!("fixtures/role-path-candidates/layouts.json");
const BASELINE: &str = include_str!("fixtures/bsc-refined450-layouts.json");
const REVIEW: &str = include_str!("fixtures/role-path-candidates/source-review.json");

fn word(n: u64) -> [u8; 32] {
    let mut word = [0; 32];
    word[24..].copy_from_slice(&n.to_be_bytes());
    word
}
fn nested(root: [u8; 32], keys: &[[u8; 32]]) -> ([u8; 32], HashMap<String, String>) {
    let mut slot = root;
    let mut preimages = HashMap::new();
    for key in keys {
        let input = [key.as_slice(), slot.as_slice()].concat();
        slot = hash(&input);
        preimages.insert(hex::encode(slot), hex::encode(input));
    }
    (slot, preimages)
}
fn plus_one(mut key: [u8; 32]) -> [u8; 32] {
    for byte in key.iter_mut().rev() {
        let (sum, carry) = byte.overflowing_add(1);
        *byte = sum;
        if !carry {
            break;
        }
    }
    key
}
fn block(contract: &[u8], key: [u8; 32], preimages: HashMap<String, String>) -> eth::Block {
    eth::Block {
        ver: 5,
        number: 122288010,
        hash: vec![7; 32],
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        header: Some(eth::BlockHeader {
            number: 122288010,
            parent_hash: vec![6; 32],
            state_root: vec![8; 32],
            ..Default::default()
        }),
        transaction_traces: vec![eth::TransactionTrace {
            status: eth::TransactionTraceStatus::Succeeded as i32,
            calls: vec![eth::Call {
                address: contract.to_vec(),
                keccak_preimages: preimages,
                storage_changes: vec![eth::StorageChange {
                    address: contract.to_vec(),
                    key: key.to_vec(),
                    old_value: vec![0],
                    new_value: vec![1],
                    ordinal: 1,
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn candidates_change_only_reviewed_role_paths_and_keep_source_runtime_bindings() {
    let candidates: Value = serde_json::from_str(CANDIDATES).unwrap();
    let baseline: Value = serde_json::from_str(BASELINE).unwrap();
    let review: Value = serde_json::from_str(REVIEW).unwrap();
    assert_eq!(review["qualified"], false);
    assert_eq!(review["baseline"]["sha256"], hex::encode(Sha256::digest(BASELINE.as_bytes())));
    assert_eq!(
        review["source_audit"]["sha256"],
        hex::encode(Sha256::digest(include_bytes!("../docs/evidence/role-shape-audit.json")))
    );
    assert_eq!(candidates.as_array().unwrap().len(), 2);
    for (candidate, source) in candidates.as_array().unwrap().iter().zip(review["profiles"].as_array().unwrap()) {
        assert_eq!(candidate["contract"], source["contract"]);
        assert_eq!(source["qualified"], false);
        assert_eq!(source["setter_reachability"], "no_source_callsite");
        let original = baseline.as_array().unwrap().iter().find(|p| p["contract"] == candidate["contract"]).unwrap();
        let root = source["root"].as_str().unwrap();
        let mut restored = candidate.clone();
        assert_eq!(
            restored.as_object_mut().unwrap().remove("other_mapping_paths"),
            Some(json!([{"root":root,"key_types":["bytes32","address"],"offset":0,"words":1}]))
        );
        assert_eq!(restored["other_mapping_words"].as_object_mut().unwrap().insert(root.into(), json!(2)), None);
        assert_eq!(&restored, original, "no unrelated profile changes");
        for (name, content) in source["sources"].as_object().unwrap() {
            assert_eq!(
                format!("0x{}", hex::encode(hash(content["content"].as_str().unwrap().as_bytes()))),
                content["keccak256"],
                "source {name}"
            );
        }
        let code = hex::decode(source["runtime"]["onchainBytecode"].as_str().unwrap().trim_start_matches("0x")).unwrap();
        assert_eq!(format!("0x{}", hex::encode(hash(&code))), candidate["code_hash"]);
        let storage = &source["storage_layout"];
        let field = storage["storage"].as_array().unwrap().iter().find(|f| f["label"] == "_roles").unwrap();
        assert_eq!(format!("0x{:064x}", field["slot"].as_str().unwrap().parse::<u64>().unwrap()), root);
        let mapping = &storage["types"][field["type"].as_str().unwrap()];
        assert_eq!(mapping["key"], "t_bytes32");
        let record = &storage["types"][mapping["value"].as_str().unwrap()];
        let members = record["members"].as_array().unwrap();
        assert_eq!((members[0]["slot"].as_str(), members[0]["offset"].as_u64()), (Some("0"), Some(0)));
        assert_eq!((members[1]["label"].as_str(), members[1]["slot"].as_str()), (Some("adminRole"), Some("1")));
        let membership = &storage["types"][members[0]["type"].as_str().unwrap()];
        assert_eq!((membership["key"].as_str(), membership["value"].as_str()), (Some("t_address"), Some("t_bool")));
    }
}

#[test]
fn candidates_accept_membership_and_refuse_admin_adjacent_padding_depth_and_missing_preimages() {
    let candidates = layout::parse(CANDIDATES).unwrap();
    let baseline = layout::parse(BASELINE).unwrap();
    for (candidate, root) in candidates.iter().zip([8, 6]) {
        let original = baseline.iter().find(|p| p.contract == candidate.contract).unwrap();
        let role = [0xaa; 32];
        let mut holder = [0; 32];
        holder[12..].fill(0x33);
        let (member, preimages) = nested(word(root), &[role, holder]);
        let good = block(&candidate.contract, member, preimages.clone());
        assert_eq!(
            project(&good, std::slice::from_ref(candidate)).unwrap(),
            project(&good, std::slice::from_ref(original)).unwrap()
        );
        let mut revoke = good.clone();
        let write = &mut revoke.transaction_traces[0].calls[0].storage_changes[0];
        write.old_value = vec![1];
        write.new_value = vec![0];
        assert!(project(&revoke, std::slice::from_ref(candidate)).unwrap().balances.is_empty());
        let (role_base, role_preimages) = nested(word(root), &[role]);
        let mut malformed_holder = holder;
        malformed_holder[0] = 1;
        let malformed = nested(word(root), &[role, malformed_holder]);
        let deeper = nested(word(root), &[role, holder, holder]);
        for (key, images) in [
            (plus_one(member), preimages.clone()),
            (plus_one(role_base), role_preimages.clone()),
            (role_base, role_preimages),
            malformed,
            deeper,
            (member, HashMap::new()),
        ] {
            assert!(project(&block(&candidate.contract, key, images), std::slice::from_ref(candidate)).is_err());
        }
        // Reproduce the precise legacy-width over-admission being removed.
        assert!(project(&block(&candidate.contract, plus_one(member), preimages), std::slice::from_ref(original)).is_ok());
    }
}
