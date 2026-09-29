#![cfg(not(target_arch = "wasm32"))]
//! Source-derived admission cases, not execution of setters/router/signature paths.
use erc20_balances::{hash, layout, project};
use serde_json::{json, Value};
use substreams_ethereum::pb::eth::v2 as eth;
type W = [u8; 32];
const ADDRESSES: [&str; 2] = ["ac68931b666e086e9de380cfdb0fb5704a35dc2d", "3505bee89d3b4e351dbd4849241a6b0716ea407f"];
const CODES: [&str; 2] = [
    "18f0619e94b94d884d917414ff4248c11e327998aef6a3e7089b9977e6ecef1c",
    "258d1c6c6dcac1bfb46886ed147f9445408e9e5420a2c7a9434cd1b60bcf200f",
];
fn n(n: u64) -> W {
    let mut w = [0; 32];
    w[24..].copy_from_slice(&n.to_be_bytes());
    w
}
fn h(w: W) -> String {
    format!("0x{}", hex::encode(w))
}
fn address(t: usize) -> Vec<u8> {
    hex::decode(ADDRESSES[t]).unwrap()
}
fn config(t: usize) -> Value {
    json!({"contract":format!("0x{}",ADDRESSES[t]),"code_hash":format!("0x{}",CODES[t]),"balance_slot":h(n(if t==0{7}else{1})),"metadata_semantics":if t==0{"bnbtiger_solc_0_8_4"}else{"cookie_solc_0_6_12"}})
}
fn layouts(t: usize) -> Vec<layout::VerifiedLayout> {
    layout::parse(&json!([config(t)]).to_string()).unwrap()
}
fn row(t: usize, key: W, old: W, new: W, ordinal: u64) -> eth::StorageChange {
    eth::StorageChange {
        address: address(t),
        key: key.to_vec(),
        old_value: old.to_vec(),
        new_value: new.to_vec(),
        ordinal,
    }
}
fn leaf(call: &mut eth::Call, root: W, key: W) -> W {
    let raw = [key.as_slice(), root.as_slice()].concat();
    let slot = hash(&raw);
    call.keccak_preimages.insert(hex::encode(slot), hex::encode(raw));
    slot
}
fn call(t: usize) -> eth::Call {
    let mut c = eth::Call {
        address: address(t),
        caller: n(9)[12..].to_vec(),
        begin_ordinal: 1,
        end_ordinal: 1000,
        ..Default::default()
    };
    let balance = leaf(&mut c, n(if t == 0 { 7 } else { 1 }), n(44));
    c.storage_changes.push(row(t, balance, n(0), n(123), 10));
    c
}
fn block(c: eth::Call) -> eth::Block {
    eth::Block {
        number: 100,
        ver: 5,
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        hash: vec![1; 32],
        header: Some(eth::BlockHeader {
            number: 100,
            parent_hash: vec![2; 32],
            state_root: vec![3; 32],
            ..Default::default()
        }),
        transaction_traces: vec![eth::TransactionTrace {
            status: eth::TransactionTraceStatus::Succeeded as i32,
            calls: vec![c],
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn ok(t: usize, c: eth::Call) {
    let out = project(&block(c), &layouts(t)).unwrap();
    assert_eq!(out.balances.len(), 1);
    assert_eq!(out.balances[0].amount, "123");
}
fn bad(t: usize, c: eth::Call) {
    assert!(
        project(&block(c), &layouts(t)).is_err(),
        "invalid metadata must refuse the complete otherwise-valid balance block"
    );
}
fn put(word: &mut W, offset: usize, size: usize, value: u64) {
    word[32 - offset - size..32 - offset].copy_from_slice(&value.to_be_bytes()[8 - size..]);
}
fn scalar(t: usize, slot: u64, old: W, new: W) -> eth::Call {
    let mut c = call(t);
    c.storage_changes.push(row(t, n(slot), old, new, 20));
    c
}
fn packed_base(t: usize, slot: u64) -> W {
    let mut w = [0; 32];
    match (t, slot) {
        (0, 5) => put(&mut w, 0, 1, 9),
        (1, 6) => put(&mut w, 0, 1, 18),
        (1, 11) => put(&mut w, 21, 2, 500),
        _ => {}
    }
    w
}

#[test]
fn identity_schema_and_every_generic_fallback_are_excluded() {
    for t in 0..2 {
        let c = config(t);
        for (field, value) in [
            ("contract", json!(format!("0x{}", ADDRESSES[1 - t]))),
            ("code_hash", json!(format!("0x{}", CODES[1 - t]))),
            ("balance_slot", json!(h(n(88)))),
            ("metadata_semantics", Value::Null),
            ("metadata_semantics", json!("other")),
            ("balance_bits", json!(256)),
            ("other_slots", json!([h(n(5))])),
            ("other_mapping_slots", json!([h(n(15))])),
            ("other_mapping_words", json!({h(n(15)):2})),
            ("other_mapping_paths", json!([{"root":h(n(15)),"key_types":["address","uint32"],"words":2}])),
            ("address_lists", json!([h(n(6))])),
            ("immutable_zero_mapping", json!(true)),
            ("voting_checkpoints", json!({"clock":"block_number","slots":[h(n(1))]})),
            (
                "enumerable_address_sets",
                json!([{"root":h(n(9)),"key_types":["bytes32"],"semantics":"oz_3_4_2"}]),
            ),
            ("proxy", json!({})),
            ("beacon_proxy", json!({})),
            ("minimal_proxy", json!({})),
            ("deployment", json!({})),
            ("zero_balance", json!({})),
            ("balance_divisor", json!({})),
            ("address_hash_balance", json!({})),
            ("guess", json!(true)),
        ] {
            let mut changed = c.clone();
            changed[field] = value;
            assert!(layout::parse(&json!([changed]).to_string()).is_err(), "{field}");
        }
        assert!(layout::parse(&json!([c.clone(), c]).to_string()).is_err());
    }
    let legacy = include_str!("fixtures/bsc-refined450-layouts.json");
    let old: Vec<layout::Layout> = serde_json::from_str(legacy).unwrap();
    assert!(old.iter().all(|l| l.metadata_semantics.is_none()));
    assert!(!serde_json::to_string(&old).unwrap().contains("metadata_semantics"));
}
#[test]
fn packed_runtime_fields_preserve_constants_and_padding() {
    for (t, slot, fields) in [
        (0, 5, vec![(1, 20)]),
        (0, 30, vec![(0, 20), (20, 1), (21, 1), (22, 1), (23, 1)]),
        (1, 6, vec![(1, 2), (3, 2), (5, 2)]),
        (1, 11, vec![(0, 20), (20, 1)]),
    ] {
        let old = packed_base(t, slot);
        for (offset, size) in fields {
            let mut new = old;
            new[31 - offset] = 1;
            ok(t, scalar(t, slot, old, new));
            ok(t, scalar(t, slot, new, new));
            if size == 1 {
                new[31 - offset] = 2;
                bad(t, scalar(t, slot, old, new));
            }
        }
        for byte in 0..32 {
            let is_padding = match (t, slot) {
                (0, 5) => byte < 11,
                (0, 30) => byte < 8,
                (1, 6) => byte < 25,
                (1, 11) => byte < 9,
                _ => false,
            };
            if is_padding {
                let mut dirty = old;
                dirty[byte] = 1;
                bad(t, scalar(t, slot, old, dirty));
                bad(t, scalar(t, slot, dirty, dirty));
                bad(t, scalar(t, slot, dirty, old));
            }
        }
    }
    for (t, slot, offset) in [(0, 5, 0), (1, 6, 0), (1, 11, 21), (1, 11, 22)] {
        let old = packed_base(t, slot);
        let mut bad_constant = old;
        bad_constant[31 - offset] ^= 1;
        for (a, b) in [(old, bad_constant), (bad_constant, old), (bad_constant, bad_constant)] {
            bad(t, scalar(t, slot, a, b));
        }
    }
    for (t, slot, first, second) in [(0, 30, 20, 21), (1, 6, 1, 3), (1, 11, 0, 20)] {
        let old = packed_base(t, slot);
        let mut new = old;
        new[31 - first] = 1;
        new[31 - second] = 1;
        bad(t, scalar(t, slot, old, new));
    }
}
#[test]
fn cookie_rate_limits_and_specific_source_value_constraints() {
    for (offset, limit) in [(1, 1000), (3, 100), (5, 500)] {
        let old = packed_base(1, 6);
        let mut new = old;
        put(&mut new, offset, 2, limit);
        ok(1, scalar(1, 6, old, new));
        put(&mut new, offset, 2, limit + 1);
        bad(1, scalar(1, 6, old, new));
        bad(1, scalar(1, 6, new, new));
    }
    ok(1, scalar(1, 3, n(0), [255; 32]));
    bad(1, scalar(1, 3, n(123), n(122)));
    ok(1, scalar(1, 13, n(1), n(2)));
    bad(1, scalar(1, 13, n(1), n(0)));
    bad(1, scalar(1, 13, n(0), n(0)));
    let zero = packed_base(1, 11);
    let mut pair = zero;
    pair[31] = 1;
    ok(1, scalar(1, 11, zero, pair));
    bad(1, scalar(1, 11, pair, zero));
    let mut locked = zero;
    locked[11] = 1;
    ok(1, scalar(1, 11, zero, locked));
}
#[test]
fn exact_scalar_and_mapping_domains_cover_each_runtime_writer() {
    for t in 0..2 {
        let addresses: &[u64] = if t == 0 { &[0, 1, 6, 29] } else { &[0, 10] };
        for slot in addresses {
            ok(t, scalar(t, *slot, n(0), n(1)));
            let mut dirty = n(1);
            dirty[0] = 1;
            bad(t, scalar(t, *slot, n(0), dirty));
        }
        let full: &[u64] = if t == 0 {
            &[2, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 26, 27, 28]
        } else {
            &[9]
        };
        for slot in full {
            ok(t, scalar(t, *slot, n(0), [255; 32]));
        }
        let roots: &[u64] = if t == 0 { &[9, 10, 11, 12] } else { &[7, 12] };
        for root in roots {
            for holder in [n(0), n(44)] {
                let mut c = call(t);
                let k = leaf(&mut c, n(*root), holder);
                c.storage_changes.push(row(t, k, n(0), n(1), 20));
                ok(t, c.clone());
                c.storage_changes.last_mut().unwrap().new_value = n(2).to_vec();
                bad(t, c);
            }
        }
        let mut c = call(t);
        let a = leaf(&mut c, n(if t == 0 { 8 } else { 2 }), n(1));
        let k = leaf(&mut c, a, n(2));
        c.storage_changes.push(row(t, k, n(0), [255; 32], 20));
        ok(t, c);
        let mut c = call(t);
        let a = leaf(&mut c, n(if t == 0 { 8 } else { 2 }), n(0));
        let k = leaf(&mut c, a, n(2));
        c.storage_changes.push(row(t, k, n(0), n(1), 20));
        bad(t, c);
    }
    ok(1, scalar(1, 8, n(0), n(1)));
    bad(1, scalar(1, 8, n(0), n(2)));
    let mut c = call(1);
    let k = leaf(&mut c, n(14), n(44));
    c.storage_changes.push(row(1, k, n(1), n(0), 20));
    ok(1, c.clone());
    c.storage_changes.last_mut().unwrap().new_value[0] = 1;
    bad(1, c);
}
#[test]
fn checkpoint_two_word_uint32_domain_and_wrapping_counters() {
    for index in [0, u32::MAX as u64] {
        let mut c = call(1);
        let outer = leaf(&mut c, n(15), n(44));
        let k = leaf(&mut c, outer, n(index));
        let mut next = k;
        for b in next.iter_mut().rev() {
            let (x, carry) = b.overflowing_add(1);
            *b = x;
            if !carry {
                break;
            }
        }
        c.storage_changes.push(row(1, k, n(7), n(100), 20));
        c.storage_changes.push(row(1, next, n(1), [255; 32], 20));
        ok(1, c.clone());
        let mut invalid = c.clone();
        invalid.storage_changes[1].new_value = n(99).to_vec();
        bad(1, invalid);
        let mut invalid = c.clone();
        invalid.storage_changes[1].old_value[0] = 1;
        bad(1, invalid);
        let mut invalid = c.clone();
        invalid.storage_changes[1].old_value = n(99).to_vec();
        invalid.storage_changes[1].new_value = n(99).to_vec();
        bad(1, invalid);
        c.storage_changes.remove(1);
        ok(1, c);
    }
    for (root, old, new) in [(16, n(u32::MAX as u64), n(0)), (16, n(0), n(1)), (17, [255; 32], n(0)), (17, n(8), n(9))] {
        let mut c = call(1);
        let k = leaf(&mut c, n(root), n(44));
        c.storage_changes.push(row(1, k, old, new, 20));
        ok(1, c.clone());
        c.storage_changes.last_mut().unwrap().new_value = n(7).to_vec();
        bad(1, c);
    }
    let mut c = call(1);
    let k = leaf(&mut c, n(16), n(44));
    c.storage_changes.push(row(1, k, n(1u64 << 32), n(1u64 << 32), 20));
    bad(1, c);
}
#[test]
fn exact_path_depth_key_padding_and_both_checkpoint_terminal_bounds() {
    for (keys, offset) in [
        (vec![n(44), n(1u64 << 32)], 0),
        (vec![n(44)], 0),
        (vec![n(44), n(1), n(2)], 0),
        (vec![n(44), n(1)], 2),
    ] {
        let mut c = call(1);
        let mut k = n(15);
        for key in keys {
            k = leaf(&mut c, k, key);
        }
        for _ in 0..offset {
            for b in k.iter_mut().rev() {
                let (v, c) = b.overflowing_add(1);
                *b = v;
                if !c {
                    break;
                }
            }
        }
        c.storage_changes.push(row(1, k, n(0), n(100), 20));
        bad(1, c);
    }
    let mut c = call(1);
    let mut dirty = n(44);
    dirty[0] = 1;
    let k = leaf(&mut c, n(14), dirty);
    c.storage_changes.push(row(1, k, n(0), n(1), 20));
    bad(1, c);
    let mut c = call(1);
    let k = leaf(&mut c, n(14), n(44));
    c.storage_changes.push(row(1, k, n(0), n(1), 20));
    let mut missing = c.clone();
    missing.keccak_preimages.remove(&hex::encode(k));
    bad(1, missing);
    c.keccak_preimages.insert(hex::encode(k), hex::encode([0; 64]));
    bad(1, c);
}
#[test]
fn constructor_unknown_and_malformed_noops_refuse_without_changing_legacy_policy() {
    for (t, slots) in [(0, vec![3, 4, 25, 31, 99]), (1, vec![4, 5, 18, 99])] {
        for slot in slots {
            for (old, new) in [(n(0), n(1)), (n(0), n(0)), (n(1), n(1))] {
                bad(t, scalar(t, slot, old, new));
            }
        }
    }
    for t in 0..2 {
        for field in 0..3 {
            let mut c = scalar(t, 2, n(0), n(0));
            let r = c.storage_changes.last_mut().unwrap();
            match field {
                0 => r.key = vec![0; 33],
                1 => r.old_value = vec![0; 33],
                _ => r.new_value = vec![0; 33],
            };
            bad(t, c);
        }
        let mut c = call(t);
        c.storage_changes[0].old_value = n(123).to_vec();
        let out = project(&block(c), &layouts(t)).unwrap();
        assert!(out.balances.is_empty());
    }
    let mut legacy = config(0);
    legacy.as_object_mut().unwrap().remove("metadata_semantics");
    let configured = layout::parse(&json!([legacy]).to_string()).unwrap();
    assert_eq!(project(&block(scalar(0, 99, n(0), n(0))), &configured).unwrap().balances.len(), 1);
}
#[test]
fn exact_same_key_continuity_but_distinct_keys_may_tie_or_commute() {
    for t in 0..2 {
        let mut c = scalar(t, 0, n(0), n(1));
        c.storage_changes.push(row(t, n(0), n(1), n(2), 21));
        ok(t, c.clone());
        for (field, value) in [(0, 20), (0, 0), (1, 99)] {
            let mut bad_c = c.clone();
            let r = bad_c.storage_changes.last_mut().unwrap();
            if field == 0 {
                r.ordinal = value
            } else {
                r.old_value = n(value).to_vec();
            }
            bad(t, bad_c);
        }
        let mut tie = scalar(t, 0, n(0), n(1));
        tie.storage_changes.push(row(t, n(if t == 0 { 6 } else { 10 }), n(0), n(2), 20));
        ok(t, tie.clone());
        tie.storage_changes.swap(1, 2);
        ok(t, tie);
    }
}
#[test]
fn reverted_and_failed_records_filter_but_runtime_creation_and_unknown_system_writes_refuse() {
    for (t, code) in CODES.iter().enumerate() {
        let c = call(t);
        let mut b = block(c.clone());
        let mut child = scalar(t, 99, n(0), n(1));
        child.state_reverted = true;
        child.index = 1;
        child.parent_index = 0;
        b.transaction_traces[0].calls.push(child);
        assert_eq!(project(&b, &layouts(t)).unwrap().balances.len(), 1);
        b.transaction_traces[0].status = eth::TransactionTraceStatus::Reverted as i32;
        assert!(project(&b, &layouts(t)).unwrap().balances.is_empty());
        let mut b = block(c);
        b.code_changes.push(eth::CodeChange {
            address: address(t),
            old_hash: vec![0; 32],
            new_hash: hex::decode(code).unwrap(),
            new_code: vec![1],
            ..Default::default()
        });
        assert!(project(&b, &layouts(t)).is_err());
        let mut b = block(call(t));
        b.system_calls.push(scalar(t, 99, n(0), n(0)));
        assert!(project(&b, &layouts(t)).is_err());
    }
}
#[test]
fn versions_atomic_two_token_scope_and_empty_blocks() {
    for t in 0..2 {
        for ver in [3, 4, 5] {
            let mut b = block(call(t));
            b.ver = ver;
            assert_eq!(project(&b, &layouts(t)).unwrap().balances.len(), 1);
        }
        let mut b = block(call(t));
        b.ver = 6;
        assert!(project(&b, &layouts(t)).is_err());
        let mut b = block(call(t));
        b.transaction_traces.clear();
        assert!(project(&b, &layouts(t)).unwrap().balances.is_empty());
    }
    let mut b = block(call(0));
    b.transaction_traces[0].calls.push(scalar(1, 6, n(0), n(0)));
    let all = layout::parse(&json!([config(0), config(1)]).to_string()).unwrap();
    assert!(project(&b, &all).is_err());
}

#[test]
fn mixed_guarded_and_legacy_noops_keep_their_own_policy_and_atomicity() {
    let mut legacy = config(1);
    legacy.as_object_mut().unwrap().remove("metadata_semantics");
    let legacy_layout = layout::parse(&json!([legacy.clone()]).to_string()).unwrap();
    let both = layout::parse(&json!([config(0), legacy]).to_string()).unwrap();
    let legacy_call = scalar(1, 99, n(7), n(7));
    let original = project(&block(legacy_call.clone()), &legacy_layout).unwrap();
    let mut mixed = block(call(0));
    mixed.transaction_traces[0].calls.push(legacy_call);
    let out = project(&mixed, &both).unwrap();
    assert_eq!(out.balances.len(), 2);
    assert_eq!(
        out.balances.iter().find(|r| r.contract.as_ref() == Some(&address(1))),
        original.balances.first()
    );
    mixed.transaction_traces[0].calls[0].storage_changes.push(row(0, n(99), n(7), n(7), 20));
    assert!(project(&mixed, &both).is_err());
    assert_eq!(project(&mixed, &legacy_layout).unwrap(), original);
}

#[test]
fn uint32_header_boundary_and_zero_spender_are_explicit() {
    let mut c = call(1);
    let outer = leaf(&mut c, n(15), n(44));
    let k = leaf(&mut c, outer, n(0));
    c.storage_changes.push(row(1, k, n(0), n(u32::MAX as u64), 20));
    let mut b = block(c);
    b.number = u32::MAX as u64;
    b.header.as_mut().unwrap().number = b.number;
    assert_eq!(project(&b, &layouts(1)).unwrap().balances.len(), 1);
    b.number += 1;
    b.header.as_mut().unwrap().number = b.number;
    assert!(project(&b, &layouts(1)).is_err());
    let record = b.transaction_traces[0].calls[0].storage_changes.last_mut().unwrap();
    let mut next = k;
    for byte in next.iter_mut().rev() {
        let (v, carry) = byte.overflowing_add(1);
        *byte = v;
        if !carry {
            break;
        }
    }
    record.key = next.to_vec();
    record.new_value = [255; 32].to_vec();
    assert_eq!(
        project(&b, &layouts(1)).unwrap().balances.len(),
        1,
        "votes-only record does not invent a fromBlock write"
    );
    for t in 0..2 {
        let mut c = call(t);
        let outer = leaf(&mut c, n(if t == 0 { 8 } else { 2 }), n(1));
        let k = leaf(&mut c, outer, n(0));
        c.storage_changes.push(row(t, k, n(0), n(1), 20));
        bad(t, c);
    }
}
