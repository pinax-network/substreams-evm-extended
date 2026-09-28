#![cfg(not(target_arch = "wasm32"))]
//! Source-derived shapes only: no transaction execution or candidate promotion.
use erc20_balances::{hash, layout, project};
use std::collections::HashMap;
use substreams_ethereum::pb::eth::v2 as eth;

const CANDIDATE: &str = include_str!("fixtures/tagger-role-candidate/layouts.json");
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
fn arbitrary_admin_and_membership_shapes_preserve_simultaneous_balances() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    let baseline = layout::parse(BASELINE).unwrap();
    let original = baseline.iter().find(|p| p.contract == candidate[0].contract).unwrap();
    let member = keys().1;
    // Constructor-known roles, default role and arbitrary outer role. The
    // public onlyOwner setter is not restricted to constructor constants.
    for role in [[0; 32], hash(b"ROLE_DEPLOYER"), hash(b"ROLE_OPERATOR"), [0xff; 32]] {
        let (outer, outer_hints) = nested(6, &[role]);
        let (membership, membership_hints) = nested(6, &[role, member]);
        for (key, hints) in [(plus(outer), outer_hints), (membership, membership_hints)] {
            // Grant/revoke/renounce/admin-change/no-op shapes; this does not
            // execute the source authorization checks or validate bool values.
            for (old, new) in [(0, 1), (1, 0), (0, 0), (1, 1)] {
                let b = block(key, hints.clone(), old, new);
                assert!(project(&b, &candidate).unwrap().balances.is_empty());
                assert_eq!(project(&b, &candidate).unwrap(), project(&b, std::slice::from_ref(original)).unwrap());
            }
        }
        // Both metadata shapes and a balance write must coexist in one call.
        let (balance, balance_hints) = nested(0, &[member]);
        let (_, mut hints) = nested(6, &[role]);
        hints.extend(nested(6, &[role, member]).1);
        hints.extend(balance_hints);
        let mut b = block(plus(outer), hints, 0, 1);
        let template = b.transaction_traces[0].calls[0].storage_changes[0].clone();
        // Admin values are arbitrary bytes32, not booleans/fixed role labels.
        b.transaction_traces[0].calls[0].storage_changes[0].new_value = vec![0xff; 32];
        for (key, ordinal, amount) in [(membership, 2, 1), (balance, 3, 9)] {
            let mut c = template.clone();
            c.key = key.to_vec();
            c.ordinal = ordinal;
            c.new_value = vec![amount];
            b.transaction_traces[0].calls[0].storage_changes.push(c);
        }
        let got = project(&b, &candidate).unwrap();
        assert_eq!(got, project(&b, std::slice::from_ref(original)).unwrap());
        assert_eq!(got.balances.len(), 1);
        assert_eq!(got.balances[0].amount, "9");
    }
}

#[test]
fn outer_anchor_adjacency_shifted_parent_and_invalid_paths_are_refused() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    let baseline = layout::parse(BASELINE).unwrap();
    let original = baseline.iter().find(|p| p.contract == candidate[0].contract).unwrap();
    let (role, member) = keys();
    let (outer, outer_hints) = nested(6, &[role]);
    let (membership, member_hints) = nested(6, &[role, member]);
    let mut padded = member;
    padded[0] = 1;
    let shifted_image = [member.as_slice(), plus(outer).as_slice()].concat();
    let shifted = hash(&shifted_image);
    let mut shifted_hints = outer_hints.clone();
    shifted_hints.insert(hex::encode(shifted), hex::encode(shifted_image));
    for (name, (key, hints)) in [
        ("outer anchor", (outer, outer_hints.clone())),
        ("outer plus2", (plus(plus(outer)), outer_hints.clone())),
        ("member plus1", (plus(membership), member_hints.clone())),
        ("member plus2", (plus(plus(membership)), member_hints.clone())),
        ("mapping based on admin word", (shifted, shifted_hints)),
        ("extra depth", nested(6, &[role, member, member])),
        ("wrong root", nested(99, &[role, member])),
        ("address padding", nested(6, &[role, padded])),
        ("wrong order", nested(6, &[member, role])),
        ("missing membership hints", (membership, HashMap::new())),
        ("missing admin hints", (plus(outer), HashMap::new())),
    ] {
        assert!(project(&block(key, hints, 0, 1), &candidate).is_err(), "{name}");
    }
    let mut missing = member_hints.clone();
    missing.remove(&hex::encode(outer));
    assert!(project(&block(membership, missing, 0, 1), &candidate).is_err());
    for key in [outer, membership] {
        let mut corrupt = member_hints.clone();
        corrupt.insert(hex::encode(key), hex::encode([0; 64]));
        assert!(project(&block(membership, corrupt, 0, 1), &candidate).is_err());
    }
    assert!(
        project(&block(plus(membership), member_hints, 0, 1), std::slice::from_ref(original)).is_ok(),
        "old width2 over-admission remains reproducible"
    );
    assert!(
        project(&block(outer, outer_hints, 0, 1), std::slice::from_ref(original)).is_ok(),
        "old mapping-anchor over-admission remains reproducible"
    );
}

#[test]
fn failed_reverted_and_noop_writes_keep_existing_persistence_filtering() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    let (outer, hints) = nested(6, &[keys().0]);
    for (key, hints) in [(plus(outer), hints), nested(6, &[keys().0, keys().1]), (word(999), HashMap::new())] {
        let mut b = block(key, hints.clone(), 0, 1);
        b.transaction_traces[0].calls[0].state_reverted = true;
        assert!(project(&b, &candidate).unwrap().balances.is_empty());
        b.transaction_traces[0].calls[0].state_reverted = false;
        b.transaction_traces[0].status = eth::TransactionTraceStatus::Failed as i32;
        assert!(project(&b, &candidate).unwrap().balances.is_empty());
        assert!(project(&block(key, hints, 1, 1), &candidate).unwrap().balances.is_empty());
    }
}

#[test]
fn malformed_words_and_runtime_changes_still_fail_closed() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    let (outer, hints) = nested(6, &[keys().0]);
    for (key, hints) in [(plus(outer), hints), nested(6, &[keys().0, keys().1])] {
        for field in ["key", "old", "new"] {
            let mut b = block(key, hints.clone(), 0, 1);
            let change = &mut b.transaction_traces[0].calls[0].storage_changes[0];
            match field {
                "key" => change.key = vec![0; 33],
                "old" => change.old_value = vec![0; 33],
                _ => change.new_value = vec![0; 33],
            }
            assert!(project(&b, &candidate).is_err(), "malformed {field}");
        }
        let mut b = block(key, hints, 0, 1);
        b.transaction_traces[0].calls[0].code_changes.push(eth::CodeChange {
            address: candidate[0].contract.clone(),
            old_code: vec![0],
            new_code: vec![1],
            old_hash: hash(&[0]).to_vec(),
            new_hash: hash(&[1]).to_vec(),
            ordinal: 2,
        });
        assert!(project(&b, &candidate).is_err());
    }
}

#[test]
fn owner_signer_status_mode_claim_fee_and_allowance_metadata_are_preserved() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    let baseline = layout::parse(BASELINE).unwrap();
    let original = baseline.iter().find(|p| p.contract == candidate[0].contract).unwrap();
    let member = keys().1;
    let mut paths: Vec<_> = [2, 3, 4, 5, 7, 8, 9, 10, 11, 14].into_iter().map(|n| (word(n), HashMap::new())).collect();
    paths.extend([nested(12, &[member]), nested(13, &[member]), nested(1, &[member, member])]);
    for (key, hints) in paths {
        for (old, new) in [(0, 1), (1, 0)] {
            let b = block(key, hints.clone(), old, new);
            let got = project(&b, &candidate).unwrap();
            assert!(got.balances.is_empty());
            assert_eq!(got, project(&b, std::slice::from_ref(original)).unwrap());
        }
    }
    // Adding the legacy broad root back alongside the typed rules is refused.
    let mut raw: serde_json::Value = serde_json::from_str(CANDIDATE).unwrap();
    raw[0]["other_mapping_words"][format!("0x{:064x}", 6)] = serde_json::json!(2);
    assert!(layout::parse(&raw.to_string()).is_err());
}
