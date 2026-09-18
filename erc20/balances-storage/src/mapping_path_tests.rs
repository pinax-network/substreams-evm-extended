//! Offline shape regressions. Synthetic path configurations are not live qualifications.
use super::*;
use prost::Message;
use serde_json::{json, Value};
use std::collections::HashMap;

fn hex_word(n: u64) -> String {
    format!("0x{n:064x}")
}

fn base() -> Value {
    json!({"contract":format!("0x{}", "11".repeat(20)),"code_hash":format!("0x{}", "22".repeat(32)),"balance_slot":hex_word(0)})
}

fn path(root: u64, types: &[&str], offset: u8, words: u8) -> Value {
    json!({"root":hex_word(root),"key_types":types,"offset":offset,"words":words})
}

fn configuration(paths: Vec<Value>) -> Value {
    let mut value = base();
    value["other_mapping_paths"] = json!(paths);
    value
}

fn parse(value: &Value) -> Result<Vec<VerifiedLayout>, Error> {
    layout::parse(&json!([value]).to_string())
}

fn configured(paths: Vec<Value>) -> VerifiedLayout {
    parse(&configuration(paths)).unwrap().remove(0)
}

fn plus(mut key: [u8; 32], offset: u8) -> [u8; 32] {
    let mut carry = offset as u16;
    for byte in key.iter_mut().rev() {
        let sum = u16::from(*byte) + carry;
        *byte = sum as u8;
        carry = sum >> 8;
    }
    key
}

fn address_key(byte: u8) -> [u8; 32] {
    let mut key = [0; 32];
    key[12..].fill(byte);
    key
}

fn nested(root: u64, keys: &[[u8; 32]]) -> ([u8; 32], HashMap<String, String>) {
    let mut slot = word(&root.to_be_bytes()).unwrap();
    let mut preimages = HashMap::new();
    for key in keys {
        let mut preimage = Vec::from(*key);
        preimage.extend_from_slice(&slot);
        slot = hash(&preimage);
        preimages.insert(hex::encode(slot), hex::encode(preimage));
    }
    (slot, preimages)
}

fn write(layout: &VerifiedLayout, key: [u8; 32], old: u8, new: u8, ordinal: u64) -> eth::StorageChange {
    eth::StorageChange {
        address: layout.contract.clone(),
        key: key.to_vec(),
        old_value: vec![old],
        new_value: vec![new],
        ordinal,
    }
}

fn block(layout: &VerifiedLayout, key: [u8; 32], preimages: HashMap<String, String>) -> eth::Block {
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
                address: layout.contract.clone(),
                keccak_preimages: preimages,
                storage_changes: vec![write(layout, key, 0, 1, 1)],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn accepted(layout: &VerifiedLayout, key: [u8; 32], preimages: HashMap<String, String>) {
    assert!(project(&block(layout, key, preimages), std::slice::from_ref(layout))
        .unwrap()
        .balances
        .is_empty());
}

fn rejected(layout: &VerifiedLayout, key: [u8; 32], preimages: HashMap<String, String>) {
    assert!(project(&block(layout, key, preimages), std::slice::from_ref(layout)).is_err());
}

#[test]
fn mapping_paths_validate_root_type_depth_and_terminal_range() {
    let valid = path(6, &["bytes32", "address"], 0, 1);
    assert!(parse(&configuration(vec![valid.clone()])).is_ok());
    let mut defaulted = valid.clone();
    defaulted.as_object_mut().unwrap().remove("offset");
    defaulted.as_object_mut().unwrap().remove("words");
    let layout = parse(&configuration(vec![defaulted])).unwrap().remove(0);
    let (key, preimages) = nested(6, &[[0xff; 32], address_key(0x33)]);
    accepted(&layout, key, preimages.clone());
    rejected(&layout, plus(key, 1), preimages);

    for root in [
        "0x06".to_owned(),
        "00".repeat(32),
        format!("0x{}", "00".repeat(33)),
        format!("0x{}g0", "00".repeat(31)),
    ] {
        let mut bad = valid.clone();
        bad["root"] = json!(root);
        assert!(parse(&configuration(vec![bad])).is_err(), "root {root}");
    }
    for key_type in ["uint", "uint0", "uint7", "uint264", "int256", "bytes20", "bool", "Address", "uint032"] {
        assert!(parse(&configuration(vec![path(6, &[key_type], 0, 1)])).is_err(), "type {key_type}");
    }
    assert!(parse(&configuration(vec![path(6, &[], 0, 1)])).is_err());
    assert!(parse(&configuration(vec![path(6, &["bytes32"; 9], 0, 1)])).is_err());
    assert!(parse(&configuration(vec![path(6, &["bytes32"; 8], 0, 1)])).is_ok());
    for (offset, words) in [(0, 0), (0, 33), (255, 2), (225, 32)] {
        assert!(parse(&configuration(vec![path(6, &["bytes32"], offset, words)])).is_err());
    }
    for (offset, words) in [(255, 1), (224, 32)] {
        assert!(parse(&configuration(vec![path(6, &["bytes32"], offset, words)])).is_ok());
    }
    for (field, value) in [("offset", json!(-1)), ("offset", json!(256)), ("words", json!(256)), ("surprise", json!(true))] {
        let mut bad = valid.clone();
        bad[field] = value;
        assert!(parse(&configuration(vec![bad])).is_err());
    }
}

#[test]
fn mapping_paths_same_root_shapes_must_be_unambiguous() {
    let member = path(6, &["bytes32", "address"], 0, 1);
    let admin = path(6, &["bytes32"], 1, 1);
    assert!(parse(&configuration(vec![member.clone(), admin.clone()])).is_ok());
    for conflict in [
        member.clone(),
        path(6, &["bytes32", "address"], 0, 2),
        path(6, &["bytes32", "uint160"], 2, 1),
        path(6, &["address"], 1, 1),
        path(6, &["bytes32"], 0, 1),
        path(6, &["bytes32"], 0, 2),
    ] {
        for ordered in [vec![member.clone(), conflict.clone()], vec![conflict, member.clone()]] {
            assert!(parse(&configuration(ordered)).is_err());
        }
    }
    assert!(parse(&configuration(vec![path(6, &["bytes32"], 1, 2), path(6, &["bytes32"], 2, 2)])).is_err());
    assert!(parse(&configuration(vec![path(6, &["bytes32"], 1, 2), path(6, &["bytes32"], 3, 1)])).is_ok());
    assert!(parse(&configuration(vec![member, path(7, &["uint24"], 0, 1)])).is_ok());
}

#[test]
fn mapping_paths_cannot_hide_balance_metadata_or_protected_dependencies() {
    let mut profiles = Vec::new();
    for (field, value) in [
        ("balance_slot", json!(hex_word(6))),
        ("other_slots", json!([hex_word(6)])),
        ("other_mapping_slots", json!([hex_word(6)])),
        ("other_mapping_words", json!({hex_word(6): 2})),
        ("zero_balance", json!({"value":hex_word(1),"storage_slot":hex_word(6)})),
        ("balance_divisor", json!({"value":hex_word(2),"storage_slot":hex_word(6)})),
        (
            "proxy",
            json!({"implementation_slot":hex_word(6),"implementation":format!("0x{}","33".repeat(20)),"code_hash":hex_word(2)}),
        ),
        (
            "beacon_proxy",
            json!({"beacon_slot":hex_word(6),"beacon":format!("0x{}","33".repeat(20)),"beacon_code_hash":hex_word(2),"implementation_slot":hex_word(9),"implementation":format!("0x{}","44".repeat(20)),"implementation_code_hash":hex_word(3)}),
        ),
        ("voting_checkpoints", json!({"clock":"block_number","slots":[hex_word(6)]})),
        ("voting_checkpoints", json!({"clock":"timestamp","mapping_slots":[hex_word(6)]})),
        ("address_lists", json!([hex_word(6)])),
        (
            "address_hash_balance",
            json!({"modulus":hex_word(9),"offset":hex_word(1),"multiplier":hex_word(2),"stored_addresses":[{"slot":hex_word(6),"address":format!("0x{}","55".repeat(20))}]}),
        ),
    ] {
        let mut profile = base();
        profile[field] = value;
        assert!(parse(&profile).is_ok(), "invalid independent fixture for {field}");
        profiles.push((field, profile));
    }
    for (field, mut profile) in profiles {
        profile["other_mapping_paths"] = json!([path(6, &["bytes32", "address"], 0, 1)]);
        assert!(parse(&profile).is_err(), "path hides {field}");
    }
}

#[test]
fn mapping_paths_role_admin_and_membership_offsets_are_terminal_only() {
    let layout = configured(vec![path(6, &["bytes32"], 1, 1), path(6, &["bytes32", "address"], 0, 1)]);
    let role = [0xfe; 32];
    let holder = address_key(0x42);
    let (outer, outer_preimages) = nested(6, &[role]);
    let (member, member_preimages) = nested(6, &[role, holder]);
    accepted(&layout, plus(outer, 1), outer_preimages.clone());
    accepted(&layout, member, member_preimages.clone());
    rejected(&layout, outer, outer_preimages.clone());
    rejected(&layout, plus(outer, 2), outer_preimages);
    rejected(&layout, plus(member, 1), member_preimages.clone());
    rejected(&layout, plus(member, 2), member_preimages);
    let (too_deep, hints) = nested(6, &[role, holder, address_key(0x17)]);
    rejected(&layout, too_deep, hints);
    let (swapped, hints) = nested(6, &[holder, role]);
    rejected(&layout, swapped, hints);
    let (wrong_root, hints) = nested(7, &[role, holder]);
    rejected(&layout, wrong_root, hints);

    let (_, mut hints) = nested(6, &[role]);
    let mut inner = holder.to_vec();
    inner.extend_from_slice(&plus(outer, 1));
    let shifted_parent = hash(&inner);
    hints.insert(hex::encode(shifted_parent), hex::encode(inner));
    rejected(&layout, shifted_parent, hints);
}

#[test]
fn mapping_paths_require_the_complete_uncorrupted_hash_chain() {
    let layout = configured(vec![path(6, &["bytes32", "address"], 0, 1)]);
    let role = [0xff; 32];
    let (outer, _) = nested(6, &[role]);
    let (member, hints) = nested(6, &[role, address_key(0xab)]);
    accepted(&layout, member, hints.clone());
    for missing in [outer, member] {
        let mut absent = hints.clone();
        assert!(absent.remove(&hex::encode(missing)).is_some());
        rejected(&layout, member, absent);
    }
    let mut corrupt = hints;
    corrupt.get_mut(&hex::encode(outer)).unwrap().replace_range(0..2, "00");
    rejected(&layout, member, corrupt);

    let root = word(&[6]).unwrap();
    let array = hash(&root);
    rejected(&layout, array, HashMap::from([(hex::encode(array), hex::encode(root))]));
}

#[test]
fn mapping_paths_typed_keys_enforce_canonical_padding_at_zero_and_maximum() {
    for kind in ["bytes32", "address", "uint24", "uint32", "uint256"] {
        let layout = configured(vec![path(6, &[kind], 0, 1)]);
        let width = match kind {
            "address" => 20,
            "uint24" => 3,
            "uint32" => 4,
            _ => 32,
        };
        let mut maximum = [0; 32];
        maximum[32 - width..].fill(0xff);
        for key in [[0; 32], maximum] {
            let (slot, hints) = nested(6, &[key]);
            accepted(&layout, slot, hints);
        }
        if width < 32 {
            maximum[31 - width] = 1;
            let (slot, hints) = nested(6, &[maximum]);
            rejected(&layout, slot, hints);
        }
    }
    for bits in (8..=256).step_by(8) {
        let kind = format!("uint{bits}");
        let layout = configured(vec![path(6, &[&kind], 0, 1)]);
        let mut maximum = [0; 32];
        maximum[32 - bits / 8..].fill(0xff);
        let (slot, hints) = nested(6, &[maximum]);
        accepted(&layout, slot, hints);
    }
}

#[test]
fn mapping_paths_rate_limit_word_four_is_outside_the_reviewed_record() {
    let layout = configured(vec![path(11, &["uint32"], 0, 4)]);
    for eid in [0u32, 30101, u32::MAX] {
        let (base, hints) = nested(11, &[word(&eid.to_be_bytes()).unwrap()]);
        for offset in 0..4 {
            accepted(&layout, plus(base, offset), hints.clone());
        }
        rejected(&layout, plus(base, 4), hints.clone());
        rejected(&layout, plus(base, 3), HashMap::new());
        let (nested_record, hints) = nested(11, &[word(&eid.to_be_bytes()).unwrap(), address_key(0x37)]);
        rejected(&layout, plus(nested_record, 3), hints);
    }
}

#[test]
fn mapping_paths_maximum_depth_and_terminal_offset_remain_exact() {
    let layout = configured(vec![path(6, &["uint8"; 8], 255, 1)]);
    let keys = (0u8..8).map(|v| word(&[v]).unwrap()).collect::<Vec<_>>();
    let (slot, hints) = nested(6, &keys);
    accepted(&layout, plus(slot, 255), hints.clone());
    rejected(&layout, plus(slot, 254), hints);
    let (shorter, hints) = nested(6, &keys[..7]);
    rejected(&layout, plus(shorter, 255), hints);
    let mut longer = keys;
    longer.push(word(&[8]).unwrap());
    let (deeper, hints) = nested(6, &longer);
    rejected(&layout, plus(deeper, 255), hints);
}

#[test]
fn mapping_paths_metadata_writes_restores_and_reverts_respect_persistence() {
    let layout = configured(vec![path(6, &["bytes32", "address"], 0, 1)]);
    let (member, hints) = nested(6, &[[0xfe; 32], address_key(0x42)]);
    let mut valid = block(&layout, member, hints.clone());
    valid.transaction_traces[0].calls[0].storage_changes.push(write(&layout, member, 1, 0, 2));
    assert!(project(&valid, std::slice::from_ref(&layout)).unwrap().balances.is_empty());

    let invalid_key = plus(member, 1);
    let mut invalid = block(&layout, invalid_key, hints);
    invalid.transaction_traces[0].calls[0]
        .storage_changes
        .push(write(&layout, invalid_key, 1, 0, 2));
    assert!(
        project(&invalid, std::slice::from_ref(&layout)).is_err(),
        "restoration cannot hide a persisted unsupported write"
    );
    invalid.transaction_traces[0].calls[0].state_reverted = true;
    assert!(project(&invalid, std::slice::from_ref(&layout)).unwrap().balances.is_empty());
    invalid.transaction_traces[0].calls[0].state_reverted = false;
    invalid.transaction_traces[0].status = eth::TransactionTraceStatus::Failed as i32;
    assert!(project(&invalid, std::slice::from_ref(&layout)).unwrap().balances.is_empty());

    let mut system = valid.clone();
    system.system_calls = system.transaction_traces.remove(0).calls;
    assert!(project(&system, std::slice::from_ref(&layout)).unwrap().balances.is_empty());
    system.system_calls[0].storage_changes = vec![write(&layout, invalid_key, 0, 1, 1)];
    assert!(project(&system, std::slice::from_ref(&layout)).is_err());
    system.system_calls[0].state_reverted = true;
    assert!(project(&system, std::slice::from_ref(&layout)).unwrap().balances.is_empty());
}

#[test]
fn mapping_paths_reverted_preimages_are_hints_not_authorization_for_wrong_depth() {
    let layout = configured(vec![path(6, &["bytes32", "address"], 0, 1)]);
    let (member, hints) = nested(6, &[[0xfc; 32], address_key(0x32)]);
    let mut sample = block(&layout, member, HashMap::new());
    sample.transaction_traces[0].calls.push(eth::Call {
        address: layout.contract.clone(),
        keccak_preimages: hints,
        state_reverted: true,
        ..Default::default()
    });
    assert!(project(&sample, std::slice::from_ref(&layout)).unwrap().balances.is_empty());
    sample.transaction_traces[0].calls[0].storage_changes[0].key = plus(member, 1).to_vec();
    assert!(project(&sample, std::slice::from_ref(&layout)).is_err());
}

#[test]
fn mapping_paths_captured_role_and_rate_fixtures_keep_exact_rpc_events() {
    let mut total = 0;
    for (directory, layout_text, cases_text) in [
        (
            "role-width",
            include_str!("../tests/fixtures/role-width/layouts.json"),
            include_str!("../tests/fixtures/role-width/cases.json"),
        ),
        (
            "bridge450",
            include_str!("../tests/fixtures/bridge450/layouts.json"),
            include_str!("../tests/fixtures/bridge450/cases.json"),
        ),
    ] {
        let original = layout::parse(layout_text).unwrap();
        let mut configs: Value = serde_json::from_str(layout_text).unwrap();
        for config in configs.as_array_mut().unwrap() {
            if directory == "role-width" {
                let root = if config["balance_slot"] == hex_word(1) { 0 } else { 6 };
                config["other_mapping_slots"].as_array_mut().unwrap().retain(|v| v != &hex_word(root));
                config["other_mapping_paths"] = json!([path(root, &["bytes32", "address"], 0, 1)]);
            } else if config["other_mapping_words"].is_object() {
                assert_eq!(config["other_mapping_words"].as_object_mut().unwrap().remove(&hex_word(11)), Some(json!(4)));
                config["other_mapping_paths"] = json!([path(11, &["uint32"], 0, 4)]);
            }
        }
        let narrowed = layout::parse(&configs.to_string()).unwrap();
        let cases: Value = serde_json::from_str(cases_text).unwrap();
        for case in cases.as_array().unwrap() {
            let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(directory)
                .join(case["fixture"].as_str().unwrap());
            let block = eth::Block::decode(std::fs::read(file).unwrap().as_slice()).unwrap();
            let contract = hex_bytes(case["contract"].as_str().unwrap()).unwrap();
            let old = original.iter().find(|l| l.contract == contract).unwrap();
            let new = narrowed.iter().find(|l| l.contract == contract).unwrap();
            let actual = project(&block, std::slice::from_ref(new)).unwrap();
            assert_eq!(actual, project(&block, std::slice::from_ref(old)).unwrap());
            let expected = case["balances"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| (hex_bytes(v["address"].as_str().unwrap()).unwrap(), v["rpc"].as_str().unwrap().to_owned()))
                .collect::<BTreeMap<_, _>>();
            assert_eq!(actual.balances.len(), expected.len());
            assert_eq!(actual.balances.into_iter().map(|b| (b.address, b.amount)).collect::<BTreeMap<_, _>>(), expected);
            total += expected.len();
        }
    }
    assert_eq!(total, 11);
}
