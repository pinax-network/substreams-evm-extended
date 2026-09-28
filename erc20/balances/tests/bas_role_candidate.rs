#![cfg(not(target_arch = "wasm32"))]
//! Source-derived storage shapes, not executed role transactions or promotion.
//! Typed membership constrains keys/depth, not Solidity boolean values.
use erc20_balances::{hash, layout, project};
use std::collections::HashMap;
use substreams_ethereum::pb::eth::v2 as eth;

const CANDIDATE: &str = include_str!("fixtures/bas-role-candidate/layouts.json");
const BASELINE: &str = include_str!("fixtures/bsc-refined450-layouts.json");
const ADMIN: &str = "e09f975e15f8f53f24cbbc282b13c40b84df485fcdb8d3997fa103dc5a4ef842";
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
fn admin() -> [u8; 32] {
    hex::decode(ADMIN).unwrap().try_into().unwrap()
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
fn membership_grant_revoke_noop_and_fixed_admin_preserve_simultaneous_balance() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    let baseline = layout::parse(BASELINE).unwrap();
    let original = baseline.iter().find(|p| p.contract == candidate[0].contract).unwrap();
    for role in [hash(b"MINTER_ROLE"), hash(b"PAUSER_ROLE"), [0; 32], [0xff; 32]] {
        let (key, hints) = nested(6, &[role, member()]);
        for (old, new) in [(0, 1), (1, 0), (0, 0), (1, 1), (2, 3)] {
            let b = block(key, hints.clone(), old, new);
            let got = project(&b, &candidate).unwrap();
            assert!(got.balances.is_empty());
            assert_eq!(got, project(&b, std::slice::from_ref(original)).unwrap());
        }
    }
    assert_eq!(plus(nested(6, &[hash(b"PAUSER_ROLE")]).0), admin());
    let mut b = block(admin(), HashMap::new(), 0, 1);
    b.transaction_traces[0].calls[0].storage_changes[0].new_value = hash(b"PAUSER_ROLE").to_vec();
    assert!(
        project(&b, std::slice::from_ref(original)).is_err(),
        "preserved original fixed-admin refusal without hints"
    );
    let (key, hints) = nested(6, &[hash(b"PAUSER_ROLE"), member()]);
    let (balance, balance_hints) = nested(1, &[member()]);
    b.transaction_traces[0].calls[0].keccak_preimages.extend(hints);
    b.transaction_traces[0].calls[0].keccak_preimages.extend(balance_hints);
    for (key, value, ordinal) in [(key, 1, 2), (balance, 9, 3)] {
        let mut change = b.transaction_traces[0].calls[0].storage_changes[0].clone();
        change.key = key.to_vec();
        change.new_value = vec![value];
        change.ordinal = ordinal;
        b.transaction_traces[0].calls[0].storage_changes.push(change);
    }
    let got = project(&b, &candidate).unwrap();
    assert_eq!(got.balances.len(), 1);
    assert_eq!(got.balances[0].amount, "9");
    // Location permission does not validate a role value or constructor context.
    for (old, new) in [(0, 0), (1, 1), (1, 0), (2, 3)] {
        assert!(project(&block(admin(), HashMap::new(), old, new), &candidate).unwrap().balances.is_empty());
    }
}
#[test]
fn arbitrary_admin_outer_adjacent_wrong_depth_order_padding_and_hints_refuse() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    let baseline = layout::parse(BASELINE).unwrap();
    let original = baseline.iter().find(|p| p.contract == candidate[0].contract).unwrap();
    let role = hash(b"MINTER_ROLE");
    let (key, hints) = nested(6, &[role, member()]);
    let (outer, outer_hints) = nested(6, &[role]);
    let mut padded = member();
    padded[0] = 1;
    for (name, (key, hints)) in [
        ("other role admin", (plus(outer), outer_hints)),
        ("default admin", {
            let (k, h) = nested(6, &[[0; 32]]);
            (plus(k), h)
        }),
        ("PAUSER outer anchor", nested(6, &[hash(b"PAUSER_ROLE")])),
        ("membership+1", (plus(key), hints.clone())),
        ("wrong fixed word", (plus(admin()), HashMap::new())),
        ("extra depth", nested(6, &[role, member(), member()])),
        ("wrong root", nested(99, &[role, member()])),
        ("wrong key order", nested(6, &[member(), role])),
        ("address padding", nested(6, &[role, padded])),
        ("missing hints", (key, HashMap::new())),
    ] {
        assert!(project(&block(key, hints, 0, 1), &candidate).is_err(), "{name}");
    }
    let mut missing = hints.clone();
    missing.remove(&hex::encode(outer));
    assert!(project(&block(key, missing, 0, 1), &candidate).is_err());
    let mut corrupt = hints.clone();
    corrupt.insert(hex::encode(key), hex::encode([0; 64]));
    assert!(project(&block(key, corrupt, 0, 1), &candidate).is_err());
    let mut oversized = hints.clone();
    oversized.insert(hex::encode(key), hex::encode([0; 65]));
    assert!(project(&block(key, oversized, 0, 1), &candidate).is_err());
    assert!(
        project(&block(plus(key), hints, 0, 1), std::slice::from_ref(original)).is_ok(),
        "preserved legacy width over-admission"
    );
}
#[test]
fn paused_whitelist_allowance_and_ordinary_scalars_keep_existing_treatment() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    let baseline = layout::parse(BASELINE).unwrap();
    let original = baseline.iter().find(|p| p.contract == candidate[0].contract).unwrap();
    for (key, hints) in [
        (word(0), HashMap::new()),
        (word(3), HashMap::new()),
        (word(4), HashMap::new()),
        (word(5), HashMap::new()),
        nested(7, &[member()]),
        nested(2, &[member(), word(4)]),
    ] {
        let b = block(key, hints, 0, 1);
        let got = project(&b, &candidate).unwrap();
        assert!(got.balances.is_empty());
        assert_eq!(got, project(&b, std::slice::from_ref(original)).unwrap());
    }
}
#[test]
fn failed_reverted_and_noop_records_keep_persisted_effect_boundary() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    for key in [admin(), word(999), nested(6, &[hash(b"PAUSER_ROLE"), member()]).0] {
        let mut reverted = block(key, HashMap::new(), 0, 1);
        reverted.transaction_traces[0].calls[0].state_reverted = true;
        assert!(project(&reverted, &candidate).unwrap().balances.is_empty());
        let mut failed = block(key, HashMap::new(), 0, 1);
        failed.transaction_traces[0].status = eth::TransactionTraceStatus::Failed as i32;
        assert!(project(&failed, &candidate).unwrap().balances.is_empty());
        assert!(project(&block(key, HashMap::new(), 1, 1), &candidate).unwrap().balances.is_empty());
    }
}
#[test]
fn malformed_scalar_mapping_and_noop_storage_never_become_valid_metadata() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    for (key, hints) in [(admin(), HashMap::new()), nested(6, &[hash(b"MINTER_ROLE"), member()])] {
        for field in ["old", "new", "key", "noop"] {
            let mut b = block(key, hints.clone(), 0, 1);
            let change = &mut b.transaction_traces[0].calls[0].storage_changes[0];
            match field {
                "old" => change.old_value = vec![0; 33],
                "new" => change.new_value = vec![0; 33],
                "key" => change.key = vec![0; 33],
                _ => {
                    change.old_value = vec![0; 33];
                    change.new_value = vec![0; 33];
                }
            }
            assert!(project(&b, &candidate).is_err(), "malformed {field}");
        }
    }
}
#[test]
fn captured_creation_and_changed_runtime_still_require_independent_deployment_qualification() {
    let candidate = layout::parse(CANDIDATE).unwrap();
    let source: serde_json::Value = serde_json::from_str(include_str!("fixtures/bas-role-candidate/source-capture.json")).unwrap();
    let runtime = hex::decode(source["runtimeBytecode"]["onchainBytecode"].as_str().unwrap().trim_start_matches("0x")).unwrap();
    for (old_code, new_code) in [(vec![0], vec![1]), (Vec::new(), runtime)] {
        let mut b = block(admin(), HashMap::new(), 0, 1);
        b.number = 53994920;
        b.header.as_mut().unwrap().number = b.number;
        b.transaction_traces[0].calls[0].call_type = eth::CallType::Create as i32;
        b.transaction_traces[0].calls[0].code_changes.push(eth::CodeChange {
            address: candidate[0].contract.clone(),
            old_hash: hash(&old_code).to_vec(),
            new_hash: hash(&new_code).to_vec(),
            old_code,
            new_code,
            ordinal: 2,
        });
        assert!(project(&b, &candidate).is_err());
    }
}
