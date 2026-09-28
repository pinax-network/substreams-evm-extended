#![cfg(not(target_arch = "wasm32"))]
//! Source-derived shapes only: no transaction execution or candidate promotion.
use erc20_balances::{hash, layout, project};
use std::collections::HashMap;
use substreams_ethereum::pb::eth::v2 as eth;

const CANDIDATE: &str = include_str!("fixtures/burnmint-role-candidate/layouts.json");
const BASELINE: &str = include_str!("fixtures/bsc-refined450-layouts.json");
fn word(n: u64) -> [u8; 32] {
    let mut w = [0; 32];
    w[24..].copy_from_slice(&n.to_be_bytes());
    w
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
fn keys() -> ([u8; 32], [u8; 32]) {
    let role = hash(b"MINTER_ROLE");
    let mut member = [0; 32];
    member[12..].fill(0x33);
    (role, member)
}

#[test]
fn grant_revoke_and_noop_shapes_preserve_balances() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    let baseline = layout::parse(BASELINE).unwrap();
    let original = baseline.iter().find(|v| v.contract == candidate[0].contract).unwrap();
    let (role, member) = keys();
    for role in [role, hash(b"BURNER_ROLE"), [0; 32], [0xff; 32]] {
        let (key, hints) = nested(5, &[role, member]);
        for (old, new) in [(0, 1), (1, 0), (0, 0), (1, 1)] {
            let b = block(key, hints.clone(), old, new);
            let got = project(&b, &candidate).unwrap();
            assert!(got.balances.is_empty());
            assert_eq!(got, project(&b, std::slice::from_ref(original)).unwrap());
        }
    }
    // Membership metadata must not hide a simultaneous ordinary balance write.
    let (key, mut hints) = nested(5, &[role, member]);
    let (balance_key, balance_hints) = nested(0, &[member]);
    hints.extend(balance_hints);
    let mut b = block(key, hints, 0, 1);
    let mut change = b.transaction_traces[0].calls[0].storage_changes[0].clone();
    change.key = balance_key.to_vec();
    change.new_value = vec![9];
    change.ordinal = 2;
    b.transaction_traces[0].calls[0].storage_changes.push(change);
    let got = project(&b, &candidate).unwrap();
    assert_eq!(got.balances.len(), 1);
    assert_eq!(got.balances[0].amount, "9");
}

#[test]
fn admin_adjacent_extra_depth_wrong_root_padding_and_missing_hints_are_refused() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    let baseline = layout::parse(BASELINE).unwrap();
    let original = baseline.iter().find(|v| v.contract == candidate[0].contract).unwrap();
    let (role, member) = keys();
    let (member_key, member_hints) = nested(5, &[role, member]);
    let (outer, outer_hints) = nested(5, &[role]);
    let mut bad_member = member;
    bad_member[0] = 1;
    for (name, (key, hints)) in [
        ("outer base", (outer, outer_hints.clone())),
        ("admin at outer+1", (plus(outer), outer_hints)),
        ("membership+1", (plus(member_key), member_hints.clone())),
        ("third depth", nested(5, &[role, member, member])),
        ("wrong root", nested(7, &[role, member])),
        ("address high padding", nested(5, &[role, bad_member])),
        ("wrong key order", nested(5, &[member, role])),
        ("missing all hints", (member_key, HashMap::new())),
    ] {
        assert!(project(&block(key, hints, 0, 1), &candidate).is_err(), "{name}");
    }
    let mut missing = member_hints.clone();
    missing.remove(&hex::encode(outer));
    assert!(project(&block(member_key, missing, 0, 1), &candidate).is_err());
    let mut corrupt = member_hints.clone();
    corrupt.insert(hex::encode(member_key), hex::encode([0; 64]));
    assert!(project(&block(member_key, corrupt, 0, 1), &candidate).is_err());
    assert!(
        project(&block(plus(member_key), member_hints, 0, 1), std::slice::from_ref(original)).is_ok(),
        "captured legacy width over-admission remains reproducible"
    );
}

#[test]
fn failed_and_reverted_writes_do_not_become_metadata_or_balance_rows() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    for key in [word(999), nested(5, &[keys().0, keys().1]).0] {
        let mut reverted = block(key, HashMap::new(), 0, 1);
        reverted.transaction_traces[0].calls[0].state_reverted = true;
        assert!(project(&reverted, &candidate).unwrap().balances.is_empty());
        let mut failed = block(key, HashMap::new(), 0, 1);
        failed.transaction_traces[0].status = eth::TransactionTraceStatus::Failed as i32;
        assert!(project(&failed, &candidate).unwrap().balances.is_empty());
        // Existing ordinary-layout no-op filtering is preserved, including
        // unknown keys; this is not evidence that the write is reachable.
        assert!(project(&block(key, HashMap::new(), 1, 1), &candidate).unwrap().balances.is_empty());
    }
}

#[test]
fn malformed_storage_and_unreviewed_runtime_changes_still_fail_closed() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    let (key, hints) = nested(5, &[keys().0, keys().1]);
    for field in ["old", "new", "key"] {
        let mut b = block(key, hints.clone(), 0, 1);
        let change = &mut b.transaction_traces[0].calls[0].storage_changes[0];
        match field {
            "old" => change.old_value = vec![0; 33],
            "new" => change.new_value = vec![0; 33],
            _ => change.key = vec![0; 33],
        }
        assert!(project(&b, &candidate).is_err(), "malformed {field}");
    }
    let mut b = block(key, hints, 0, 1);
    b.transaction_traces[0].calls[0].code_changes.push(eth::CodeChange {
        address: candidate[0].contract.clone(),
        old_code: vec![0x00],
        new_code: vec![0x01],
        old_hash: hash(&[0x00]).to_vec(),
        new_hash: hash(&[0x01]).to_vec(),
        ordinal: 2,
    });
    assert!(project(&b, &candidate).is_err());
}
