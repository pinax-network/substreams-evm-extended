#![cfg(not(target_arch = "wasm32"))]
//! Source-derived Extended-record shapes only, not executed role transactions.
use erc20_balances::{hash, layout, project};
use std::collections::HashMap;
use substreams_ethereum::pb::eth::v2 as eth;
const CANDIDATE: &str = include_str!("fixtures/artx-role-candidate/layouts.json");
const BASELINE: &str = include_str!("fixtures/bsc-refined450-layouts.json");
fn word(n: u64) -> [u8; 32] {
    let mut w = [0; 32];
    w[24..].copy_from_slice(&n.to_be_bytes());
    w
}
fn member() -> [u8; 32] {
    let mut m = [0; 32];
    m[12..].fill(0x33);
    m
}
fn nested(root: u64, keys: &[[u8; 32]]) -> ([u8; 32], HashMap<String, String>) {
    let mut slot = word(root);
    let mut hints = HashMap::new();
    for key in keys {
        let image = [key.as_slice(), slot.as_slice()].concat();
        slot = hash(&image);
        hints.insert(hex::encode(slot), hex::encode(image));
    }
    (slot, hints)
}
fn plus(mut key: [u8; 32]) -> [u8; 32] {
    for b in key.iter_mut().rev() {
        let (v, c) = b.overflowing_add(1);
        *b = v;
        if !c {
            break;
        }
    }
    key
}
fn block(key: [u8; 32], hints: HashMap<String, String>, old: u8, new: u8) -> eth::Block {
    let contract = layout::parse(CANDIDATE).unwrap()[0].contract.clone();
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
                address: contract.clone(),
                keccak_preimages: hints,
                storage_changes: vec![eth::StorageChange {
                    address: contract,
                    key: key.to_vec(),
                    old_value: vec![old],
                    new_value: vec![new],
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
fn membership_grant_revoke_noop_shapes_preserve_simultaneous_balances() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    let baseline = layout::parse(BASELINE).unwrap();
    let original = baseline.iter().find(|p| p.contract == candidate[0].contract).unwrap();
    for role in [hash(b"OPERATOR_ROLE"), hash(b"NEW_ROLE"), [0; 32], [0xff; 32]] {
        let (key, hints) = nested(151, &[role, member()]);
        for (old, new) in [(0, 1), (1, 0), (0, 0), (1, 1), (2, 3)] {
            let b = block(key, hints.clone(), old, new);
            let got = project(&b, &candidate).unwrap();
            assert!(got.balances.is_empty());
            assert_eq!(got, project(&b, std::slice::from_ref(original)).unwrap());
        }
    }
    let (key, mut hints) = nested(151, &[hash(b"OPERATOR_ROLE"), member()]);
    let (balance, balance_hints) = nested(51, &[member()]);
    hints.extend(balance_hints);
    let mut b = block(key, hints, 0, 1);
    let mut change = b.transaction_traces[0].calls[0].storage_changes[0].clone();
    change.key = balance.to_vec();
    change.new_value = vec![9];
    change.ordinal = 2;
    b.transaction_traces[0].calls[0].storage_changes.push(change);
    let got = project(&b, &candidate).unwrap();
    assert_eq!(got.balances.len(), 1);
    assert_eq!(got.balances[0].amount, "9");
}
#[test]
fn admin_adjacent_extra_depth_padding_and_preimage_corruption_refuse() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    let baseline = layout::parse(BASELINE).unwrap();
    let original = baseline.iter().find(|p| p.contract == candidate[0].contract).unwrap();
    let role = hash(b"OPERATOR_ROLE");
    let (key, hints) = nested(151, &[role, member()]);
    let (outer, outer_hints) = nested(151, &[role]);
    let mut padded = member();
    padded[0] = 1;
    for (name, (key, hints)) in [
        ("outer anchor", (outer, outer_hints.clone())),
        ("outer admin", (plus(outer), outer_hints)),
        ("member+1", (plus(key), hints.clone())),
        ("extra depth", nested(151, &[role, member(), member()])),
        ("wrong root", nested(999, &[role, member()])),
        ("wrong key order", nested(151, &[member(), role])),
        ("address high padding", nested(151, &[role, padded])),
        ("missing preimages", (key, HashMap::new())),
    ] {
        assert!(project(&block(key, hints, 0, 1), &candidate).is_err(), "{name}");
    }
    let mut missing = hints.clone();
    missing.remove(&hex::encode(outer));
    assert!(project(&block(key, missing, 0, 1), &candidate).is_err());
    for image in [vec![0; 64], vec![0; 65]] {
        let mut corrupt = hints.clone();
        corrupt.insert(hex::encode(key), hex::encode(image));
        assert!(project(&block(key, corrupt, 0, 1), &candidate).is_err());
    }
    assert!(
        project(&block(plus(key), hints, 0, 1), std::slice::from_ref(original)).is_ok(),
        "preserved legacy adjacent-word over-admission"
    );
}
#[test]
fn initializer_owner_counters_allowance_and_gap_boundaries_remain_unchanged() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    let baseline = layout::parse(BASELINE).unwrap();
    let original = baseline.iter().find(|p| p.contract == candidate[0].contract).unwrap();
    for (key, hints) in [
        (word(0), HashMap::new()),
        (word(53), HashMap::new()),
        (word(54), HashMap::new()),
        (word(55), HashMap::new()),
        (word(201), HashMap::new()),
        (word(351), HashMap::new()),
        (word(352), HashMap::new()),
        nested(52, &[member(), word(4)]),
    ] {
        let b = block(key, hints, 0, 1);
        let got = project(&b, &candidate).unwrap();
        assert!(got.balances.is_empty());
        assert_eq!(got, project(&b, std::slice::from_ref(original)).unwrap());
    }
    for gap in [1, 50, 56, 100, 101, 150, 152, 200, 202, 250, 251, 300, 301, 350] {
        assert!(project(&block(word(gap), HashMap::new(), 0, 1), &candidate).is_err(), "unqualified gap {gap}");
    }
    // Captured zero initial supply/empty recipients introduce no deployment seed.
    assert!(candidate[0].deployment.is_none());
    let mut empty = block(word(0), HashMap::new(), 0, 0);
    empty.transaction_traces.clear();
    assert!(project(&empty, &candidate).unwrap().balances.is_empty());
}
#[test]
fn failed_reverted_noop_and_malformed_words_keep_persisted_effect_rules() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    let (key, hints) = nested(151, &[hash(b"OPERATOR_ROLE"), member()]);
    for key in [key, word(999)] {
        let mut reverted = block(key, HashMap::new(), 0, 1);
        reverted.transaction_traces[0].calls[0].state_reverted = true;
        assert!(project(&reverted, &candidate).unwrap().balances.is_empty());
        let mut failed = block(key, HashMap::new(), 0, 1);
        failed.transaction_traces[0].status = eth::TransactionTraceStatus::Failed as i32;
        assert!(project(&failed, &candidate).unwrap().balances.is_empty());
        assert!(project(&block(key, HashMap::new(), 1, 1), &candidate).unwrap().balances.is_empty());
    }
    for field in ["old", "new", "key", "noop"] {
        let mut b = block(key, hints.clone(), 0, 1);
        let c = &mut b.transaction_traces[0].calls[0].storage_changes[0];
        match field {
            "old" => c.old_value = vec![0; 33],
            "new" => c.new_value = vec![0; 33],
            "key" => c.key = vec![0; 33],
            _ => {
                c.old_value = vec![0; 33];
                c.new_value = vec![0; 33];
            }
        }
        assert!(project(&b, &candidate).is_err(), "malformed {field}");
    }
}
#[test]
fn proxy_implementation_pointer_and_captured_creation_guards_remain_fail_closed() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    let proxy = candidate[0].proxy.as_ref().unwrap();
    let (key, hints) = nested(151, &[hash(b"OPERATOR_ROLE"), member()]);
    let mut pointer = block(proxy.implementation_slot, HashMap::new(), 0, 1);
    pointer.transaction_traces[0].calls[0].storage_changes[0].new_value = proxy.implementation.clone();
    assert!(project(&pointer, &candidate).is_err(), "even captured pointer initialization is not qualified");
    for (address, fixture, height) in [
        (candidate[0].contract.clone(), include_str!("fixtures/artx-role-candidate/proxy.json"), 67828331),
        (
            proxy.implementation.clone(),
            include_str!("fixtures/artx-role-candidate/implementation.json"),
            67828310,
        ),
    ] {
        let v: serde_json::Value = serde_json::from_str(fixture).unwrap();
        let runtime = hex::decode(v["runtimeBytecode"]["onchainBytecode"].as_str().unwrap().trim_start_matches("0x")).unwrap();
        for (old_code, new_code) in [(vec![0], vec![1]), (Vec::new(), runtime)] {
            let mut b = block(key, hints.clone(), 0, 1);
            b.number = height;
            b.header.as_mut().unwrap().number = height;
            b.transaction_traces[0].calls[0].call_type = eth::CallType::Create as i32;
            b.transaction_traces[0].calls[0].code_changes.push(eth::CodeChange {
                address: address.clone(),
                old_hash: hash(&old_code).to_vec(),
                new_hash: hash(&new_code).to_vec(),
                old_code,
                new_code,
                ordinal: 2,
            });
            assert!(project(&b, &candidate).is_err());
        }
    }
}
