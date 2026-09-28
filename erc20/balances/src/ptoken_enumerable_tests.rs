//! Frozen actual-bytecode operations translated into synthetic Extended records.
//! These test structural admission, not actual PToken producer visibility.
#![cfg(not(target_arch = "wasm32"))]
use super::*;
use serde_json::{json, Value};
type Word = [u8; 32];
const ZERO: Word = [0; 32];
fn n(v: u64) -> Word {
    word(&v.to_be_bytes()).unwrap()
}
fn b(v: &Value) -> Vec<u8> {
    hex_bytes(v.as_str().unwrap()).unwrap()
}
fn w(v: &Value) -> Word {
    word(&b(v)).unwrap()
}
fn h(v: Word) -> String {
    format!("0x{}", hex::encode(v))
}
fn pair(a: Word, b: Word) -> Vec<u8> {
    [a.as_slice(), b.as_slice()].concat()
}
fn leaf(a: Word, b: Word) -> Word {
    hash(&pair(a, b))
}
fn plus(a: Word, b: Word) -> Word {
    let mut out = ZERO;
    let mut c = 0u16;
    for i in (0..32).rev() {
        c += u16::from(a[i]) + u16::from(b[i]);
        out[i] = c as u8;
        c >>= 8;
    }
    out
}
fn minus(a: Word, b: Word) -> Word {
    plus(plus(a, b.map(|v| !v)), n(1))
}
fn account() -> Vec<u8> {
    hex::decode("c63961bf9a7d6bb5844524851d04fbd76dbe915b").unwrap()
}
fn config() -> Value {
    json!({"contract":format!("0x{}",hex::encode(account())),"code_hash":"0x60f53552d1ab18923098e47b0d7952ae7a4a48d8bece60b20d3d41734832c15c","balance_slot":h(ZERO),"other_mapping_slots":[h(n(1))],"other_slots":[h(n(2)),h(n(3)),h(n(4)),h(n(7))],"enumerable_address_sets":[{"root":h(n(6)),"membership_root":h(n(5)),"key_types":["bytes32"],"semantics":layout::PTOKEN_ENUMERABLE_SEMANTICS}]})
}
fn parse(c: Value) -> Result<Vec<VerifiedLayout>, Error> {
    layout::parse(&json!([c]).to_string())
}
fn layouts() -> Vec<VerifiedLayout> {
    parse(config()).unwrap()
}
fn cases() -> Vec<Value> {
    serde_json::from_str::<Value>(include_str!("../docs/evidence/ptoken-operation-proof-20260928-transcripts.json"))
        .unwrap()
        .as_array()
        .unwrap()
        .clone()
}
fn case(name: &str) -> Value {
    cases().into_iter().find(|v| v["name"] == name).unwrap()
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
fn preimage(call: &mut eth::Call, pre: Vec<u8>) {
    call.keccak_preimages.insert(hex::encode(hash(&pre)), hex::encode(pre));
}
fn saved_call(case: &Value, subset: usize) -> eth::Call {
    let mut call = eth::Call {
        address: account(),
        begin_ordinal: 1,
        end_ordinal: 2000,
        ..Default::default()
    };
    for pre in case["execution"]["keccaks"].as_array().unwrap() {
        let raw = hex::decode(pre["input"].as_str().unwrap()).unwrap();
        assert_eq!(h(hash(&raw)), pre["output"]);
        preimage(&mut call, raw);
    }
    let mut equal = 0;
    for store in case["execution"]["writes"].as_array().unwrap() {
        if store["old"] == store["new"] {
            let keep = subset & (1 << equal) != 0;
            equal += 1;
            if !keep {
                continue;
            }
        }
        call.storage_changes
            .push(row(w(&store["key"]), w(&store["old"]), w(&store["new"]), store["step"].as_u64().unwrap() + 10));
    }
    call
}
fn block(call: eth::Call) -> eth::Block {
    eth::Block {
        ver: 5,
        number: 122288046,
        hash: vec![7; 32],
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        header: Some(eth::BlockHeader {
            number: 122288046,
            parent_hash: vec![6; 32],
            state_root: vec![8; 32],
            ..Default::default()
        }),
        transaction_traces: vec![eth::TransactionTrace {
            status: eth::TransactionTraceStatus::Succeeded as i32,
            begin_ordinal: 1,
            end_ordinal: 2000,
            calls: vec![call],
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn saved(name: &str) -> eth::Block {
    block(saved_call(&case(name), usize::MAX))
}
fn first(block: &mut eth::Block) -> &mut eth::Call {
    &mut block.transaction_traces[0].calls[0]
}
fn balance(block: &mut eth::Block) {
    let call = first(block);
    preimage(call, pair(n(44), ZERO));
    call.storage_changes.push(row(leaf(n(44), ZERO), ZERO, n(9), 1900));
}
fn good(mut block: eth::Block) {
    balance(&mut block);
    let out = project(&block, &layouts()).unwrap();
    assert_eq!(out.balances.len(), 1);
    assert_eq!(out.balances[0].amount, "9");
}
fn bad(mut block: eth::Block) {
    balance(&mut block);
    assert!(project(&block, &layouts()).is_err(), "fragment admitted alongside a valid balance");
}
fn hints(block: &eth::Block) -> BTreeMap<Word, Vec<u8>> {
    block
        .transaction_traces
        .iter()
        .flat_map(|t| &t.calls)
        .flat_map(|c| &c.keccak_preimages)
        .map(|(k, v)| (word(&hex_bytes(k).unwrap()).unwrap(), hex_bytes(v).unwrap()))
        .collect()
}
fn shift(call: &mut eth::Call, delta: u64) {
    call.begin_ordinal += delta;
    call.end_ordinal += delta;
    for r in &mut call.storage_changes {
        r.ordinal += delta;
    }
}
fn append(block: &mut eth::Block, mut call: eth::Call) {
    let delta = 2000 * block.transaction_traces[0].calls.len() as u64;
    shift(&mut call, delta);
    block.transaction_traces[0].end_ordinal = call.end_ordinal;
    block.transaction_traces[0].calls.push(call);
}
fn role(case: &Value) -> Word {
    let raw = hex::decode(case["calldata"].as_str().unwrap()).unwrap();
    word(&raw[4..36]).unwrap()
}
#[test]
fn ptoken_compiled_operations_accept_only_source_proven_equality_subsets() {
    let mut count = 0;
    for c in cases().into_iter().filter(|c| {
        c["execution"]["exit"]["kind"] == "return"
            && !c["name"].as_str().unwrap().starts_with("incoherent")
            && c["signature"].as_str().is_some_and(|s| {
                matches!(
                    s,
                    "grantRole(bytes32,address)" | "revokeRole(bytes32,address)" | "renounceRole(bytes32,address)"
                )
            })
    }) {
        let equality = c["execution"]["writes"].as_array().unwrap().iter().filter(|s| s["old"] == s["new"]).count();
        for subset in 0..1 << equality {
            let block = block(saved_call(&c, subset));
            let accepted = enumerable_sets::validate(&block, &layouts(), &hints(&block), &BTreeMap::new()).unwrap();
            assert_eq!(accepted.len(), block.transaction_traces[0].calls[0].storage_changes.len(), "{}", c["name"]);
            good(block);
            count += 1;
        }
    }
    assert_eq!(count, 34);
}
#[test]
fn ptoken_schema_reserves_both_roots_and_legacy_null_is_not_absent() {
    assert!(parse(config()).is_ok());
    for change in [json!(null), json!(h(n(6))), json!(h(n(4))), json!("0x05")] {
        let mut c = config();
        c["enumerable_address_sets"][0]["membership_root"] = change;
        assert!(parse(c).is_err());
    }
    let mut c = config();
    c["enumerable_address_sets"][0].as_object_mut().unwrap().remove("membership_root");
    assert!(parse(c).is_err());
    for field in ["other_slots", "other_mapping_slots", "address_lists"] {
        for root in [5, 6] {
            let mut c = config();
            c[field].as_array_mut().map(|a| a.push(json!(h(n(root))))).unwrap_or_else(|| {
                c[field] = json!([h(n(root))]);
            });
            assert!(parse(c).is_err(), "{field} root{root}");
        }
    }
    for root in [5, 6] {
        let mut c = config();
        c["other_mapping_words"] = json!({h(n(root)):2});
        assert!(parse(c).is_err());
        let mut c = config();
        c["other_mapping_paths"] = json!([{"root":h(n(root)),"key_types":["bytes32","address"],"words":1}]);
        assert!(parse(c).is_err());
        let mut c = config();
        c["balance_slot"] = json!(h(n(root)));
        assert!(parse(c).is_err());
    }
    let legacy = json!({"root":h(n(8)),"key_types":["bytes32"],"semantics":"oz_3_4_2"});
    let parsed: layout::EnumerableAddressSet = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), legacy);
    for value in [json!(null), json!(h(n(5)))] {
        let mut c = config();
        c["enumerable_address_sets"][0] = legacy.clone();
        c["enumerable_address_sets"][0]["membership_root"] = value;
        assert!(parse(c).is_err());
    }
    let mut c = config();
    let duplicate = c["enumerable_address_sets"][0].clone();
    c["enumerable_address_sets"].as_array_mut().unwrap().push(duplicate);
    assert!(parse(c).is_err());
}
#[test]
fn ptoken_missing_corrupt_and_extra_stages_fail_atomically() {
    for name in [
        "grant_empty",
        "grant_zero_empty",
        "remove_first",
        "remove_tail",
        "remove_only_zero",
        "remove_middle_zero_tail",
    ] {
        let original = saved(name);
        let changes = &original.transaction_traces[0].calls[0].storage_changes;
        for (i, change) in changes.iter().enumerate() {
            if change.old_value != change.new_value {
                let mut b = original.clone();
                first(&mut b).storage_changes.remove(i);
                bad(b);
            }
            for field in [0, 1] {
                let mut b = original.clone();
                let r = &mut first(&mut b).storage_changes[i];
                if field == 0 {
                    r.old_value = n(99).to_vec()
                } else {
                    r.new_value = n(99).to_vec()
                }
                bad(b);
            }
        }
        for i in [0, changes.len() - 1] {
            let mut b = original.clone();
            let mut extra = changes[i].clone();
            extra.old_value = extra.new_value.clone();
            extra.ordinal += 1;
            first(&mut b).storage_changes.push(extra);
            bad(b);
        }
        let mut b = original.clone();
        first(&mut b).storage_changes.retain(|r| r == &changes[0]);
        bad(b);
        let mut b = original.clone();
        first(&mut b).storage_changes.remove(0);
        bad(b);
    }
    // DSG tail self-swap equality stages did not execute in selected PToken.
    let mut b = saved("remove_tail");
    let rows = &mut first(&mut b).storage_changes;
    let tail = rows[1].clone();
    let index = rows.last().unwrap().clone();
    for (mut r, ordinal) in [(tail, rows[0].ordinal + 1), (index, rows[0].ordinal + 2)] {
        r.new_value = r.old_value.clone();
        r.ordinal = ordinal;
        rows.push(r);
    }
    bad(b);
}
#[test]
fn ptoken_union_discovery_refuses_boolean_or_set_only_even_when_equal() {
    let original = saved("grant_empty");
    let r = role(&case("grant_empty"));
    let base = leaf(r, n(5));
    let head = leaf(r, n(6));
    for bool_side in [true, false] {
        let mut b = original.clone();
        let call = first(&mut b);
        if bool_side {
            call.storage_changes.truncate(1);
            call.keccak_preimages.retain(|_, v| {
                let x = hex::decode(v).unwrap();
                x.len() != 64 || x[32..] != n(6)
            });
        } else {
            call.storage_changes.remove(0);
            call.keccak_preimages.retain(|_, v| {
                let x = hex::decode(v).unwrap();
                x.len() != 64 || x[32..] != n(5)
            });
        }
        bad(b.clone());
        for row in &mut first(&mut b).storage_changes {
            row.new_value = row.old_value.clone();
        }
        bad(b);
    }
    for anchor in [base, plus(base, n(1)), plus(base, n(2)), plus(head, n(1)), plus(head, n(2))] {
        let mut b = original.clone();
        first(&mut b).storage_changes.push(row(anchor, ZERO, ZERO, 1000));
        bad(b);
    }
}
#[test]
fn ptoken_exact_preimages_dirty_words_and_missing_both_roots_refuse() {
    let original = saved("remove_first");
    for key in first(&mut original.clone()).keccak_preimages.keys() {
        let mut b = original.clone();
        let raw = hex::decode(first(&mut b).keccak_preimages[key].clone()).unwrap();
        let r = role(&case("remove_first"));
        let required = raw.len() == 64
            && ((raw[..32] == r && (raw[32..] == n(5) || raw[32..] == n(6))) || raw[32..] == leaf(r, n(5)) || raw[32..] == plus(leaf(r, n(6)), n(1)));
        if required {
            first(&mut b).keccak_preimages.remove(key);
            bad(b);
        } else {
            first(&mut b).keccak_preimages.remove(key);
            good(b);
        }
    }
    for size in [33, 65] {
        let mut b = original.clone();
        first(&mut b).storage_changes[0].old_value = vec![0; size];
        bad(b);
    }
    let mut b = original.clone();
    first(&mut b).storage_changes[0].new_value = n(2).to_vec();
    bad(b);
    let mut b = original.clone();
    first(&mut b).keccak_preimages.clear();
    bad(b);
    let mut b = original.clone();
    let key = first(&mut b).keccak_preimages.keys().next().unwrap().clone();
    first(&mut b).keccak_preimages.insert(key, hex::encode([0; 64]));
    bad(b);
    // A canonical membership preimage is required; high address bytes cannot be
    // justified by a matching hash or by the root6 address observation.
    let r = role(&case("grant_empty"));
    let mut b = saved("grant_empty");
    let old = leaf(n(2), leaf(r, n(5)));
    first(&mut b).keccak_preimages.remove(&hex::encode(old));
    let mut dirty = n(2);
    dirty[0] = 1;
    let pre = pair(dirty, leaf(r, n(5)));
    first(&mut b).storage_changes[0].key = hash(&pre).to_vec();
    preimage(first(&mut b), pre);
    bad(b);
}
#[test]
fn ptoken_extended_four_five_require_actual_positive_begin_and_v3_is_refused() {
    for ver in [4, 5] {
        let mut b = saved("grant_empty");
        b.ver = ver;
        good(b.clone());
        first(&mut b).begin_ordinal = 0;
        bad(b);
    }
    for ver in [0, 2, 3, 6] {
        let mut b = saved("grant_empty");
        b.ver = ver;
        bad(b.clone());
        first(&mut b).begin_ordinal = 0;
        bad(b);
    }
}
// Mathematical boundary fixtures absent from the bounded compiled small-set
// matrix; they do not establish execution at these synthetic huge lengths.
fn synthetic_add(r: Word, length: Word, m: Word) -> eth::Call {
    let mut c = eth::Call {
        address: account(),
        begin_ordinal: 1,
        end_ordinal: 2000,
        ..Default::default()
    };
    let base = leaf(r, n(5));
    let head = leaf(r, n(6));
    for pre in [pair(r, n(5)), pair(r, n(6)), pair(m, base), pair(m, plus(head, n(1)))] {
        preimage(&mut c, pre);
    }
    c.storage_changes = vec![
        row(leaf(m, base), ZERO, n(1), 100),
        row(head, length, plus(length, n(1)), 200),
        row(plus(hash(&head), length), ZERO, m, 300),
        row(leaf(m, plus(head, n(1))), ZERO, plus(length, n(1)), 400),
    ];
    c
}
fn synthetic_remove(r: Word, length: Word, p: Word, m: Word, tail: Word) -> eth::Call {
    let mut c = synthetic_add(r, length, m);
    let base = leaf(r, n(5));
    let head = leaf(r, n(6));
    let index = plus(head, n(1));
    preimage(&mut c, pair(tail, index));
    c.storage_changes = vec![row(leaf(m, base), n(1), ZERO, 100)];
    if p != length {
        c.storage_changes
            .extend([row(plus(hash(&head), minus(p, n(1))), m, tail, 200), row(leaf(tail, index), length, p, 300)]);
    }
    c.storage_changes.extend([
        row(plus(hash(&head), minus(length, n(1))), tail, ZERO, 400),
        row(head, length, minus(length, n(1)), 500),
        row(leaf(m, index), p, ZERO, 600),
    ]);
    c
}
#[test]
fn ptoken_sequences_and_different_roles_preserve_block_local_continuity() {
    let mut b = saved("sequence_0");
    for i in 1..6 {
        append(&mut b, saved_call(&case(&format!("sequence_{i}")), usize::MAX));
    }
    good(b);
    let mut b = block(synthetic_add(n(77), ZERO, n(2)));
    append(&mut b, synthetic_add(n(88), ZERO, n(2)));
    good(b);
    // After a coherent completed revoke of t, a later fabricated operation
    // cannot resurrect t through a moved-member constraint or position store.
    let mut b = block(synthetic_remove(n(77), n(3), n(2), n(4), n(3)));
    append(&mut b, synthetic_remove(n(77), n(2), n(1), n(2), n(4)));
    bad(b);
    let mut b = saved("sequence_0");
    let mut second = saved_call(&case("sequence_1"), usize::MAX);
    second.storage_changes[1].old_value = n(99).to_vec();
    append(&mut b, second);
    bad(b);
}
#[test]
fn ptoken_cross_frame_account_role_and_root_pair_splicing_refused() {
    let original = saved("grant_empty");
    for index in 1..4 {
        let mut b = original.clone();
        let mut other = first(&mut b).clone();
        other.index = 0;
        other.begin_ordinal = 500;
        other.end_ordinal = 1800;
        other.storage_changes = first(&mut b).storage_changes.split_off(index);
        for row in &mut other.storage_changes {
            row.ordinal += 500;
        }
        b.transaction_traces[0].calls.push(other);
        bad(b);
        let mut b = original.clone();
        let mut tx = b.transaction_traces[0].clone();
        tx.begin_ordinal = 2001;
        tx.end_ordinal = 4000;
        tx.calls[0].storage_changes = first(&mut b).storage_changes.split_off(index);
        shift(&mut tx.calls[0], 2000);
        b.transaction_traces.push(tx);
        bad(b);
        let mut b = original.clone();
        for r in &mut first(&mut b).storage_changes[index..] {
            r.address = vec![8; 20];
        }
        bad(b);
    }
    let mut b = original.clone();
    let other = synthetic_add(n(1234), ZERO, n(2));
    first(&mut b).storage_changes[0] = other.storage_changes[0].clone();
    first(&mut b).keccak_preimages.extend(other.keccak_preimages);
    bad(b);
    let mut b = original.clone();
    let r = role(&case("grant_empty"));
    let pre = pair(n(2), leaf(r, n(4)));
    first(&mut b).storage_changes[0].key = hash(&pre).to_vec();
    preimage(first(&mut b), pre);
    bad(b);
    let mut b = original.clone();
    let rows = &mut first(&mut b).storage_changes;
    let ordinal = rows[0].ordinal;
    rows[0].ordinal = rows[1].ordinal;
    rows[1].ordinal = ordinal;
    bad(b);
    let mut b = original.clone();
    let rows = &mut first(&mut b).storage_changes;
    rows[1].ordinal = rows[0].ordinal;
    bad(b);
    let mut b = original.clone();
    first(&mut b).storage_changes[0].ordinal = 0;
    bad(b);
    // Vector order is not chronology; unique ordinals are the authority.
    let mut b = original;
    first(&mut b).storage_changes.reverse();
    good(b);
}
#[test]
fn ptoken_foreign_stores_and_nested_reverted_boundaries_are_barriers() {
    let original = saved("grant_empty");
    let start = original.transaction_traces[0].calls[0].storage_changes[0].ordinal;
    let end = original.transaction_traces[0].calls[0].storage_changes[1].ordinal;
    let mut b = original.clone();
    let mut foreign = row(n(123), ZERO, n(1), start + 1);
    foreign.address = vec![8; 20];
    first(&mut b).storage_changes.push(foreign);
    bad(b);
    for reverted in [false, true] {
        let mut b = original.clone();
        b.transaction_traces[0].calls.push(eth::Call {
            index: 0,
            address: vec![8; 20],
            begin_ordinal: start + 1,
            end_ordinal: end - 1,
            state_reverted: reverted,
            ..Default::default()
        });
        bad(b);
    }
    // A foreign transaction enclosing all stages is also impossible, even
    // without an interior boundary. Reused declared indices cannot hide it.
    let mut b = original.clone();
    b.transaction_traces.push(eth::TransactionTrace {
        status: eth::TransactionTraceStatus::Succeeded as i32,
        begin_ordinal: 1,
        end_ordinal: 2000,
        calls: vec![eth::Call {
            begin_ordinal: 2,
            end_ordinal: 1800,
            index: 0,
            ..Default::default()
        }],
        ..Default::default()
    });
    bad(b);
    let mut b = original;
    b.system_calls.push(eth::Call {
        begin_ordinal: 2,
        end_ordinal: 1800,
        ..Default::default()
    });
    bad(b);
}
#[test]
fn ptoken_reverted_prefixes_failed_transactions_and_suspended_fragments_do_not_persist() {
    let mut b = block(saved_call(&case("incoherent_empty_set_prefix_revert"), usize::MAX));
    first(&mut b).state_reverted = true;
    // Valid balance is in a separate successful frame, not the reverted call.
    let mut valid = eth::Call {
        address: account(),
        begin_ordinal: 2001,
        end_ordinal: 4000,
        ..Default::default()
    };
    preimage(&mut valid, pair(n(44), ZERO));
    valid.storage_changes.push(row(leaf(n(44), ZERO), ZERO, n(9), 3900));
    b.transaction_traces[0].end_ordinal = 4000;
    b.transaction_traces[0].calls.push(valid);
    assert_eq!(project(&b, &layouts()).unwrap().balances.len(), 1);
    let mut b = saved("grant_empty");
    b.transaction_traces[0].status = eth::TransactionTraceStatus::Failed as i32;
    assert!(project(&b, &layouts()).unwrap().balances.is_empty());
    // A rollback prefix mislabeled persisted cannot grant membership permission.
    bad(block(saved_call(&case("incoherent_empty_set_prefix_revert"), usize::MAX)));
}
#[test]
fn ptoken_logical_overflow_and_protected_physical_aliases_fail_closed() {
    bad(saved("incoherent_max_length_wrap"));
    let r = n(77);
    let head = leaf(r, n(6));
    let array = hash(&head);
    let base = leaf(r, n(5));
    for target in [
        n(0),
        n(1),
        n(2),
        n(5),
        n(6),
        head,
        plus(head, n(1)),
        base,
        plus(base, n(1)),
        leaf(n(2), base),
        leaf(n(44), ZERO),
    ] {
        let length = minus(target, array);
        let c = synthetic_add(r, length, n(2));
        bad(block(c.clone()));
        let mut zero = synthetic_add(r, length, ZERO);
        zero.storage_changes.remove(2);
        preimage(&mut zero, pair(n(2), base));
        bad(block(zero));
    }
    // Array destination aliases moved t's derived boolean key, whose preimage
    // is not executed by the source and must not be invented for permission.
    let tail = n(4);
    let p = plus(minus(leaf(tail, base), array), n(1));
    let length = plus(p, n(1));
    assert!(p < length);
    bad(block(synthetic_remove(r, length, p, n(2), tail)));
    // Valid huge physical arithmetic is modular; logical lengths stay checked.
    let target = n(123456);
    let length = minus(target, array);
    assert!(length != [255; 32]);
    good(block(synthetic_add(r, length, n(2))));
}
#[test]
fn ptoken_root_and_leaf_preimage_depth_padding_and_adjacent_words_refused() {
    let original = saved("grant_empty");
    let r = role(&case("grant_empty"));
    let base = leaf(r, n(5));
    for raw in [pair(base, n(2)), [vec![0], pair(n(2), base)].concat(), pair(n(2), leaf(ZERO, base))] {
        let mut b = original.clone();
        first(&mut b).storage_changes[0].key = hash(&raw).to_vec();
        preimage(first(&mut b), raw);
        bad(b);
    }
    for offset in [1, 2] {
        let mut b = original.clone();
        let old = word(&first(&mut b).storage_changes[0].key).unwrap();
        first(&mut b).storage_changes[0].key = plus(old, n(offset)).to_vec();
        bad(b);
    }
    let mut short = original.clone();
    let mut scalar = row(n(2), ZERO, n(1), 1000);
    scalar.key = vec![2];
    first(&mut short).storage_changes.push(scalar);
    bad(short);
    let mut b = original;
    first(&mut b).storage_changes[0].key = vec![0; 33];
    bad(b);
}
