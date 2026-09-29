#![cfg(not(target_arch = "wasm32"))]
//! Selected source-derived storage operations. Synthetic Extended records are
//! not historical producer/runtime qualification.
use erc20_balances::{hash, layout, project};
use prost::Message;
use serde_json::{json, Value};
use substreams_ethereum::pb::eth::v2 as eth;
type Word = [u8; 32];
const ZERO: Word = [0; 32];
const TOKEN: &str = "cdf52c0b13c24f32f1d8d4ec6356203a1ef0826a";
fn n(v: u64) -> Word {
    let mut w = ZERO;
    w[24..].copy_from_slice(&v.to_be_bytes());
    w
}
fn add(a: Word, b: Word) -> Word {
    let mut out = ZERO;
    let mut carry = 0u16;
    for i in (0..32).rev() {
        carry += a[i] as u16 + b[i] as u16;
        out[i] = carry as u8;
        carry >>= 8;
    }
    out
}
fn account() -> Vec<u8> {
    hex::decode(TOKEN).unwrap()
}
fn leaf(a: Word, root: Word) -> Word {
    hash(&[a.as_slice(), root.as_slice()].concat())
}
fn element(owner: Word, i: usize, f: usize) -> Word {
    add(hash(&leaf(owner, n(31))), n((i * 3 + f) as u64))
}
fn original() -> Value {
    serde_json::from_str::<Vec<Value>>(include_str!("fixtures/bsc-refined450-layouts.json"))
        .unwrap()
        .into_iter()
        .find(|v| v["contract"] == format!("0x{TOKEN}"))
        .unwrap()
}
fn config() -> Value {
    let mut v = original();
    v["other_mapping_slots"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r != &format!("0x{}", hex::encode(n(32))));
    v["lpinfo_array"] = json!(layout::TOPS_LPINFO_SEMANTICS);
    v
}
fn params(v: Value) -> Vec<layout::VerifiedLayout> {
    layout::parse(&json!([v]).to_string()).unwrap()
}
fn image(call: &mut eth::Call, bytes: Vec<u8>) {
    call.keccak_preimages.insert(hex::encode(hash(&bytes)), hex::encode(bytes));
}
fn row(key: Word, old: Word, new: Word, ordinal: u64) -> eth::StorageChange {
    eth::StorageChange {
        address: account(),
        key: key.to_vec(),
        old_value: old.to_vec(),
        new_value: new.to_vec(),
        ordinal,
    }
}
fn input(signature: &[u8], words: &[Word]) -> Vec<u8> {
    [hash(signature)[..4].to_vec(), words.iter().flatten().copied().collect()].concat()
}
fn call(owner: Word) -> eth::Call {
    let mut c = eth::Call {
        address: account(),
        caller: n(11)[12..].to_vec(),
        input: input(b"transfer(address,uint256)", &[n(22), n(7)]),
        begin_ordinal: 1,
        end_ordinal: 10_000,
        ..Default::default()
    };
    for root in [31, 32] {
        image(&mut c, [owner.as_slice(), n(root).as_slice()].concat());
    }
    image(&mut c, leaf(owner, n(31)).to_vec());
    c
}
fn block(c: eth::Call, owner: Word, now: i64) -> eth::Block {
    eth::Block {
        ver: 5,
        number: 100,
        hash: vec![1; 32],
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        header: Some(eth::BlockHeader {
            number: 100,
            parent_hash: vec![2; 32],
            state_root: vec![3; 32],
            timestamp: Some(prost_types::Timestamp { seconds: now, nanos: 0 }),
            ..Default::default()
        }),
        transaction_traces: vec![eth::TransactionTrace {
            from: owner[12..].to_vec(),
            status: 1,
            begin_ordinal: 1,
            end_ordinal: 10_000,
            calls: vec![c],
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn append(owner: Word, length: usize, amount: Word, now: u64) -> eth::Block {
    let mut c = call(owner);
    c.input = input(b"createLPInfo(address,uint256)", &[owner, amount]);
    c.storage_changes.push(row(leaf(owner, n(31)), n(length as u64), n(length as u64 + 1), 10));
    for (f, value) in [amount, n(now), add(n(now), n(8_640_000))].into_iter().enumerate() {
        c.storage_changes.push(row(element(owner, length, f), ZERO, value, 11 + f as u64));
    }
    block(c, owner, now as i64)
}
fn cleanup(owner: Word, records: &[[Word; 3]], removed: usize, credit: Word, now: i64) -> eth::Block {
    let mut c = call(owner);
    let remaining = records.len() - removed;
    let mut ordinal = 10;
    let mut push = |key, old, new| {
        c.storage_changes.push(row(key, old, new, ordinal));
        ordinal += 1;
    };
    for i in 0..remaining {
        for f in 0..3 {
            push(element(owner, i, f), records[i][f], records[i + removed][f]);
        }
    }
    for i in (remaining..records.len()).rev() {
        for f in 0..3 {
            push(element(owner, i, f), records[i][f], ZERO);
        }
        push(leaf(owner, n(31)), n(i as u64 + 1), n(i as u64));
    }
    let sum = records[..removed].iter().fold(ZERO, |sum, r| add(sum, r[0]));
    push(leaf(owner, n(32)), credit, add(credit, sum));
    block(c, owner, now)
}
fn output(b: &eth::Block) -> Result<proto::pb::evm::balances::v1::Events, substreams::errors::Error> {
    project(b, &params(config()))
}
fn first(b: &mut eth::Block) -> &mut eth::Call {
    &mut b.transaction_traces[0].calls[0]
}
#[test]
fn deleted_changed_tail_word_is_not_an_independent_zero_anchor() {
    let mut b = cleanup(n(55), &[[n(9), n(50), n(100)]], 1, n(3), 100);
    assert!(output(&b).is_ok());
    first(&mut b).storage_changes.remove(1);
    assert!(output(&b).is_err(), "missing createdAt50→0 must not manufacture old zero");
}
#[test]
fn preserved_original_five_pop_refusal_before_candidate() {
    let b = eth::Block::decode(include_bytes!("fixtures/bsc-exclusions-20260928/tops-123561227-tx63.pb").as_slice()).unwrap();
    assert!(project(&b, &params(original())).is_err());
    output(&b).unwrap();
}
#[test]
fn omitted_zero_timestamp_append_requires_an_independent_old_zero() {
    let mut b = append(n(55), 0, n(9), 0);
    assert!(output(&b).is_ok());
    first(&mut b).storage_changes.remove(2);
    assert!(output(&b).is_err(), "constant new zero is not an old-word witness");
}
fn sequence(mut a: eth::Block, mut b: eth::Block) -> eth::Block {
    for tx in &mut b.transaction_traces {
        tx.begin_ordinal += 10_000;
        tx.end_ordinal += 10_000;
        for c in &mut tx.calls {
            c.begin_ordinal += 10_000;
            c.end_ordinal += 10_000;
            for s in &mut c.storage_changes {
                s.ordinal += 10_000;
            }
        }
    }
    a.transaction_traces.extend(b.transaction_traces);
    a
}
#[test]
fn prior_accepted_facts_ground_omitted_zero_append_stage() {
    let cleared = cleanup(n(55), &[[n(9), ZERO, ZERO]], 1, n(3), 0);
    let mut next = append(n(55), 0, n(9), 0);
    first(&mut next).storage_changes.remove(2);
    assert!(output(&next).is_err());
    output(&sequence(cleared, next)).unwrap();
}
#[test]
fn observed_copy_sources_ground_omitted_tail_equalities_without_guessing() {
    let mut b = cleanup(n(55), &[[n(9), n(50), n(100)], [ZERO, ZERO, n(200)]], 1, n(3), 100);
    output(&b).unwrap();
    first(&mut b).storage_changes.retain(|r| r.old_value != r.new_value);
    output(&b).unwrap();
    let mut b = cleanup(n(55), &[[n(9), n(50), n(100)], [n(9), n(50), n(200)]], 1, n(3), 100);
    first(&mut b).storage_changes.retain(|r| r.old_value != r.new_value);
    assert!(output(&b).is_err(), "tail anchor alone must not invent omitted destination old value");
}
#[test]
fn complete_append_and_all_bounded_cleanup_prefixes_match_both_producers() {
    for version in [4, 5] {
        for length in 0..6 {
            let mut b = append(n(55), length, n(9), 100);
            b.ver = version;
            output(&b).unwrap();
        }
        for length in 1..=6 {
            for removed in 1..=length {
                let records: Vec<_> = (0..length)
                    .map(|i| [n(i as u64 + 1), n(i as u64 + 30), n(if i < removed { 100 } else { 200 })])
                    .collect();
                let mut b = cleanup(n(55), &records, removed, n(3), 100);
                b.ver = version;
                output(&b).unwrap();
                for i in 0..first(&mut b).storage_changes.len() {
                    let stage = &b.transaction_traces[0].calls[0].storage_changes[i];
                    if stage.old_value == stage.new_value {
                        continue;
                    }
                    let mut missing = b.clone();
                    first(&mut missing).storage_changes.remove(i);
                    assert!(output(&missing).is_err(), "changed stage {i}, length{length}, removed{removed}, v{version}");
                }
            }
        }
    }
}
#[test]
fn profile_parser_and_six_record_policy_are_exact() {
    let mut v = config();
    v.as_object_mut().unwrap().remove("lpinfo_array");
    v["other_mapping_slots"]
        .as_array_mut()
        .unwrap()
        .push(json!(format!("0x{}", hex::encode(n(32)))));
    assert_eq!(v, original());
    for (key, value) in [
        ("lpinfo_array", Value::Null),
        ("lpinfo_array", json!("generic")),
        ("balance_slot", json!(format!("0x{}", hex::encode(n(6))))),
        ("code_hash", json!(format!("0x{}", "11".repeat(32)))),
        ("contract", json!(format!("0x{}", "11".repeat(20)))),
    ] {
        let mut v = config();
        v[key] = value;
        assert!(layout::parse(&json!([v]).to_string()).is_err());
    }
    for key in ["other_slots", "other_mapping_slots"] {
        for root in [31, 32] {
            let mut v = config();
            v[key].as_array_mut().unwrap().push(json!(format!("0x{}", hex::encode(n(root)))));
            assert!(layout::parse(&json!([v]).to_string()).is_err());
        }
    }
    for (key, value) in [
        ("other_slots", json!(format!("0x{}", hex::encode(element(n(55), 6, 0))))),
        ("other_mapping_slots", json!(format!("0x{}", hex::encode(n(777))))),
        ("address_lists", json!(format!("0x{}", hex::encode(n(777))))),
    ] {
        let mut v = config();
        v[key].as_array_mut().unwrap().push(value);
        assert!(layout::parse(&json!([v]).to_string()).is_err());
    }
    for key in ["other_slots", "other_mapping_slots", "address_lists"] {
        let mut v = config();
        v[key].as_array_mut().unwrap().pop();
        assert!(layout::parse(&json!([v]).to_string()).is_err());
    }
    let mut v = config();
    v["other_mapping_words"] = json!({format!("0x{}",hex::encode(n(777))):2});
    assert!(layout::parse(&json!([v]).to_string()).is_err());
    let mut v = config();
    v["other_mapping_paths"] = json!([{"root":format!("0x{}",hex::encode(n(777))),"key_types":["address"],"value_offset":0,"value_words":1}]);
    assert!(layout::parse(&json!([v]).to_string()).is_err());
    assert!(output(&append(n(55), 6, n(9), 100)).is_err());
    assert!(output(&cleanup(n(55), &vec![[n(9), n(50), n(100)]; 7], 7, n(3), 100)).is_err());
    for version in [0, 3, 6] {
        let mut b = append(n(55), 0, n(9), 100);
        b.ver = version;
        assert!(output(&b).is_err());
    }
}
#[test]
fn append_value_time_and_abi_constraints_are_source_specific() {
    for b in [append(ZERO, 0, n(9), 100), append(n(55), 0, ZERO, 100)] {
        assert!(output(&b).is_err());
    }
    for field in [1, 2, 3] {
        let mut b = append(n(55), 0, n(9), 100);
        first(&mut b).storage_changes[field].new_value = n(77).to_vec();
        assert!(output(&b).is_err());
    }
    let mut b = append(n(55), 0, n(9), 100);
    first(&mut b).storage_changes[2].old_value = n(50).to_vec();
    assert!(output(&b).is_err());
    let mut b = append(n(55), 0, n(9), 100);
    first(&mut b).input[4] = 1;
    assert!(output(&b).is_err());
    let mut b = append(n(55), 0, n(9), 100);
    first(&mut b).input.extend([9, 9, 9]);
    output(&b).unwrap();
    for nanos in [-1, 1] {
        let mut b = append(n(55), 0, n(9), 100);
        b.header.as_mut().unwrap().timestamp.as_mut().unwrap().nanos = nanos;
        assert!(output(&b).is_err());
    }
}
#[test]
fn cleanup_uses_actual_clock_prefix_and_modular_credit() {
    let max = [255; 32];
    output(&cleanup(n(55), &[[n(2), n(50), n(100)], [max, n(50), n(100)]], 2, max, 100)).unwrap();
    assert!(output(&cleanup(n(55), &[[n(1), n(50), n(100)], [max, n(50), n(100)]], 2, n(3), 100)).is_err());
    assert!(output(&cleanup(n(55), &[[n(9), n(50), n(101)]], 1, n(3), 100)).is_err());
    assert!(output(&cleanup(
        n(55),
        &[[n(9), n(50), n(100)], [n(8), n(50), n(200)], [n(7), n(50), n(100)]],
        3,
        n(3),
        100
    ))
    .is_err());
    output(&cleanup(
        n(55),
        &[[n(9), n(50), n(100)], [n(8), n(50), n(200)], [n(7), n(50), n(100)]],
        1,
        n(3),
        100,
    ))
    .unwrap();
    let mut b = cleanup(n(55), &[[n(9), n(50), n(100)]], 1, n(3), 100);
    b.transaction_traces[0].from = n(99)[12..].to_vec();
    assert!(output(&b).is_err());
    for signature in [b"transfer(address,uint256)".as_slice(), b"transferFrom(address,address,uint256)".as_slice()] {
        let args = if signature.starts_with(b"transferFrom") {
            vec![n(11), n(22), n(7)]
        } else {
            vec![n(22), n(7)]
        };
        let mut b = cleanup(n(55), &[[n(9), n(50), n(100)]], 1, n(3), 100);
        first(&mut b).input = input(signature, &args);
        output(&b).unwrap();
        for offset in if args.len() == 3 { vec![4, 36] } else { vec![4] } {
            let mut bad = b.clone();
            first(&mut bad).input[offset] = 1;
            assert!(output(&bad).is_err());
        }
    }
}
#[test]
fn root32_independent_extra_and_equal_stores_never_gain_permission() {
    for (old, new) in [(n(3), n(4)), (n(3), n(2)), (n(3), n(3)), (ZERO, ZERO)] {
        let mut c = call(n(55));
        c.storage_changes.push(row(leaf(n(55), n(32)), old, new, 10));
        assert!(output(&block(c, n(55), 100)).is_err());
        let mut b = cleanup(n(55), &[[n(9), n(50), n(100)]], 1, n(3), 100);
        first(&mut b).storage_changes.push(row(leaf(n(55), n(32)), n(12), new, 90));
        assert!(output(&b).is_err());
    }
}
#[test]
fn selected_noops_are_classified_without_balance_initialization() {
    for key in [n(777), element(n(55), 6, 0), element(n(55), 6, 1), n(31), n(32)] {
        let mut c = call(n(55));
        c.storage_changes.push(row(key, ZERO, ZERO, 10));
        let b = block(c, n(55), 100);
        assert!(output(&b).is_err(), "unknown equal key{}", hex::encode(key));
    }
    let mut c = call(n(55));
    image(&mut c, [n(44).as_slice(), n(5).as_slice()].concat());
    c.storage_changes.push(row(leaf(n(44), n(5)), n(7), n(7), 10));
    c.storage_changes.push(row(n(0), n(4), n(4), 20));
    assert!(output(&block(c, n(55), 100)).unwrap().balances.is_empty());
    let mut c = call(n(55));
    c.storage_changes.push(row(n(777), ZERO, ZERO, 10));
    assert!(project(&block(c, n(55), 100), &params(original())).unwrap().balances.is_empty());
}
#[test]
fn exact_head_preimage_width_and_no_fallback_are_required() {
    let b = cleanup(n(55), &[[n(9), n(50), n(100)]], 1, n(3), 100);
    let mut bad = b.clone();
    first(&mut bad).keccak_preimages.remove(&hex::encode(leaf(n(55), n(31))));
    assert!(output(&bad).is_err());
    let mut bad = b.clone();
    first(&mut bad).keccak_preimages.clear();
    assert!(output(&bad).is_err());
    let mut bad = b.clone();
    let head = hex::encode(leaf(n(55), n(31)));
    first(&mut bad).keccak_preimages.get_mut(&head).unwrap().push_str("00");
    assert!(output(&bad).is_err());
    for field in [0, 1, 2] {
        let mut bad = b.clone();
        let r = &mut first(&mut bad).storage_changes[0];
        match field {
            0 => r.key.push(0),
            1 => r.old_value.push(0),
            _ => r.new_value.push(0),
        };
        assert!(output(&bad).is_err());
    }
    let mut bad = b.clone();
    first(&mut bad).storage_changes[3].old_value = n(2).to_vec();
    assert!(output(&bad).is_err());
}
#[test]
fn order_frames_foreign_stores_and_cross_group_ties_are_barriers() {
    let b = cleanup(n(55), &[[n(9), n(50), n(100)]], 1, n(3), 100);
    for ordinal in [0, 10, 10_000] {
        let mut bad = b.clone();
        first(&mut bad).storage_changes[1].ordinal = ordinal;
        assert!(output(&bad).is_err());
    }
    let mut bad = b.clone();
    first(&mut bad).storage_changes.swap(0, 1);
    first(&mut bad).storage_changes[0].ordinal = 10;
    first(&mut bad).storage_changes[1].ordinal = 11;
    assert!(output(&bad).is_err());
    for at in [10, 12, 14] {
        let mut bad = b.clone();
        let mut foreign = eth::Call {
            address: vec![8; 20],
            begin_ordinal: 1000,
            end_ordinal: 2000,
            ..Default::default()
        };
        let mut r = row(n(777), ZERO, n(1), at);
        r.address = vec![8; 20];
        foreign.storage_changes.push(r);
        bad.transaction_traces.push(eth::TransactionTrace {
            status: 1,
            calls: vec![foreign],
            ..Default::default()
        });
        assert!(output(&bad).is_err(), "foreign inclusive tie at {at}");
    }
    let mut bad = b.clone();
    bad.transaction_traces[0].calls.push(eth::Call {
        address: vec![8; 20],
        begin_ordinal: 11,
        end_ordinal: 12,
        ..Default::default()
    });
    assert!(output(&bad).is_err());
    let mut bad = b.clone();
    let suffix = first(&mut bad).storage_changes.split_off(2);
    let mut c = call(n(55));
    c.begin_ordinal = 11;
    c.end_ordinal = 15;
    c.storage_changes = suffix;
    bad.transaction_traces[0].calls.push(c);
    assert!(output(&bad).is_err());
}
#[test]
fn failed_calls_are_filtered_and_block_local_continuity_is_atomic() {
    let mut b = cleanup(n(55), &[[n(9), n(50), n(100)]], 1, n(3), 100);
    first(&mut b).storage_changes[0].new_value = n(99).to_vec();
    let mut reverted = b.clone();
    first(&mut reverted).state_reverted = true;
    assert!(output(&reverted).unwrap().balances.is_empty());
    b.transaction_traces[0].status = 2;
    assert!(output(&b).unwrap().balances.is_empty());
    let a = append(n(55), 0, n(9), 100);
    let b = append(n(55), 1, n(8), 100);
    output(&sequence(a.clone(), b)).unwrap();
    assert!(output(&sequence(a.clone(), a)).is_err());
}

#[test]
fn exactly_one_lpinfo_operation_per_selected_source_invocation() {
    let a = append(n(55), 0, n(9), 100);
    let b = append(n(55), 1, n(9), 100);
    output(&sequence(a.clone(), b.clone())).unwrap();
    let mut combined = a;
    for mut row in b.transaction_traces[0].calls[0].storage_changes.clone() {
        row.ordinal += 10;
        first(&mut combined).storage_changes.push(row);
    }
    assert!(output(&combined).is_err(), "one createLPInfo invocation cannot append twice");
}

#[test]
fn inherited_fields_balance_continuity_and_runtime_guards_are_preserved() {
    let mut b = append(n(55), 0, n(9), 100);
    let c = first(&mut b);
    image(c, [n(44).as_slice(), n(5).as_slice()].concat());
    let allowance = leaf(n(44), n(6));
    image(c, [n(44).as_slice(), n(6).as_slice()].concat());
    image(c, [n(22).as_slice(), allowance.as_slice()].concat());
    c.storage_changes.extend([
        row(leaf(n(44), n(5)), n(7), n(8), 100),
        row(leaf(n(44), n(5)), n(8), n(8), 101),
        row(leaf(n(22), allowance), n(4), n(5), 102),
        row(n(0), n(4), n(5), 103),
        row(n(28), ZERO, n(1), 104),
        row(hash(&n(28)), ZERO, n(22), 105),
    ]);
    let events = output(&b).unwrap();
    assert_eq!(events.balances.len(), 1);
    let mut bad = b.clone();
    first(&mut bad).storage_changes[5].old_value = n(7).to_vec();
    first(&mut bad).storage_changes[5].new_value = n(7).to_vec();
    assert!(output(&bad).is_err());
    let capture: Value = serde_json::from_str(include_str!("fixtures/tops-operation-proof/capture.json")).unwrap();
    let runtime = hex::decode(capture["runtimeBytecode"]["onchainBytecode"].as_str().unwrap().trim_start_matches("0x")).unwrap();
    assert_eq!(config()["code_hash"], format!("0x{}", hex::encode(hash(&runtime))));
    for creation in [false, true] {
        let mut bad = b.clone();
        if creation {
            first(&mut bad).call_type = eth::CallType::Create as i32;
        }
        first(&mut bad).code_changes.push(eth::CodeChange {
            address: account(),
            old_code: if creation { vec![] } else { runtime.clone() },
            new_code: if creation { runtime.clone() } else { vec![1] },
            old_hash: if creation { hash(&[]).to_vec() } else { hash(&runtime).to_vec() },
            new_hash: if creation { hash(&runtime).to_vec() } else { hash(&[1]).to_vec() },
            ordinal: 9000,
        });
        assert!(output(&bad).is_err());
    }
}
