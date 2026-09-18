//! Offline, synthetic Extended blocks; these do not establish producer visibility
//! or qualify DSG. Saved SSTORE cases retain their source/runtime bindings in docs.
use super::*;
use serde_json::{json, Value};
use std::collections::HashMap;

type Word = [u8; 32];
const ZERO: Word = [0; 32];
const MAX: Word = [255; 32];

fn n(value: u64) -> Word {
    word(&value.to_be_bytes()).unwrap()
}
fn bytes(value: &Value) -> Vec<u8> {
    hex_bytes(value.as_str().unwrap()).unwrap()
}
fn hexword(value: Word) -> String {
    format!("0x{}", hex::encode(value))
}
fn plus(left: Word, right: Word) -> Word {
    let mut result = ZERO;
    let mut carry = 0u16;
    for i in (0..32).rev() {
        carry += u16::from(left[i]) + u16::from(right[i]);
        result[i] = carry as u8;
        carry >>= 8;
    }
    result
}
fn minus(left: Word, right: Word) -> Word {
    plus(plus(left, right.map(|v| !v)), n(1))
}
fn input(key: Word, root: Word) -> Vec<u8> {
    [key.as_slice(), root.as_slice()].concat()
}
fn leaf(key: Word, root: Word) -> Word {
    hash(&input(key, root))
}
fn evidence() -> Value {
    serde_json::from_str(include_str!("../docs/evidence/dsg-enumerable/runtime-cases.json")).unwrap()
}
fn cases() -> Vec<Value> {
    evidence()["cases"].as_array().unwrap().clone()
}
fn case(name: &str) -> Value {
    cases().into_iter().find(|c| c["name"] == name).unwrap()
}
fn config() -> Value {
    let proof = evidence();
    json!({
        "contract": proof["contract"], "code_hash": proof["runtime_keccak256"],
        "balance_slot": hexword(ZERO),
        "enumerable_address_sets": [{"root": hexword(n(8)), "key_types": ["bytes32"], "semantics": "oz_3_4_2"}]
    })
}
fn parse(value: Value) -> Result<Vec<VerifiedLayout>, Error> {
    layout::parse(&json!([value]).to_string())
}
fn layouts() -> Vec<VerifiedLayout> {
    parse(config()).unwrap()
}
fn preimage(preimages: &mut HashMap<String, String>, value: Vec<u8>) {
    preimages.insert(hex::encode(hash(&value)), hex::encode(value));
}
fn row(account: &[u8], key: Word, old: Word, new: Word, ordinal: u64) -> eth::StorageChange {
    eth::StorageChange {
        address: account.to_vec(),
        key: key.to_vec(),
        old_value: old.to_vec(),
        new_value: new.to_vec(),
        ordinal,
    }
}
fn saved_call(case: &Value, equality_subset: usize) -> eth::Call {
    let account = bytes(&config()["contract"]);
    let role = word(&bytes(&case["synthetic_call"]["role"])).unwrap();
    let head = leaf(role, n(8));
    assert_eq!(head.as_slice(), bytes(&case["role_record"]));
    assert_eq!(hash(&head).as_slice(), bytes(&case["array_base"]));
    let mut preimages = HashMap::new();
    preimage(&mut preimages, input(role, n(8)));
    // Array addressing can be derived from the proven head without this hint.
    for member in case["initial_members"]
        .as_array()
        .unwrap()
        .iter()
        .chain(std::iter::once(&case["synthetic_call"]["member"]))
    {
        preimage(&mut preimages, input(word(&bytes(member)).unwrap(), plus(head, n(1))));
    }
    let mut equality = 0;
    let storage_changes = case["ordered_sstores"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter_map(|(i, store)| {
            assert_eq!(store["equal_value"], store["old"] == store["new"]);
            if store["equal_value"] == true {
                let keep = equality_subset & (1 << equality) != 0;
                equality += 1;
                if !keep {
                    return None;
                }
            }
            Some(row(
                &account,
                word(&bytes(&store["key"])).unwrap(),
                word(&bytes(&store["old"])).unwrap(),
                word(&bytes(&store["new"])).unwrap(),
                10 + 10 * i as u64,
            ))
        })
        .collect();
    eth::Call {
        address: account,
        index: 0,
        begin_ordinal: 1,
        end_ordinal: 100,
        keccak_preimages: preimages,
        storage_changes,
        ..Default::default()
    }
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
            end_ordinal: 100,
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
fn hints(block: &eth::Block) -> BTreeMap<Word, Vec<u8>> {
    block
        .system_calls
        .iter()
        .chain(block.transaction_traces.iter().flat_map(|tx| &tx.calls))
        .flat_map(|call| &call.keccak_preimages)
        .map(|(key, value)| (word(&hex_bytes(key).unwrap()).unwrap(), hex_bytes(value).unwrap()))
        .collect()
}
fn ok(block: &eth::Block) {
    assert!(project(block, &layouts()).unwrap().balances.is_empty());
}
fn bad(block: &eth::Block) {
    assert!(project(block, &layouts()).is_err(), "malformed enumerable witness was accepted");
}
fn shift(call: &mut eth::Call, offset: u64) {
    call.begin_ordinal += offset;
    call.end_ordinal += offset;
    for row in &mut call.storage_changes {
        row.ordinal += offset;
    }
}
fn append(block: &mut eth::Block, mut call: eth::Call) {
    shift(&mut call, 100);
    block.transaction_traces[0].end_ordinal = 200;
    block.transaction_traces[0].calls.push(call);
}

// Deliberately explicit mathematical fixtures for boundaries absent from the
// small saved runtime sample; they are synthetic, not new runtime observations.
fn add_call(role: Word, root: Word, length: Word, member: Word) -> eth::Call {
    let account = bytes(&config()["contract"]);
    let head = leaf(role, root);
    let mut preimages = HashMap::new();
    preimage(&mut preimages, input(role, root));
    preimage(&mut preimages, input(member, plus(head, n(1))));
    eth::Call {
        address: account.clone(),
        begin_ordinal: 1,
        end_ordinal: 100,
        keccak_preimages: preimages,
        storage_changes: vec![
            row(&account, head, length, plus(length, n(1)), 10),
            row(&account, plus(hash(&head), length), ZERO, member, 20),
            row(&account, leaf(member, plus(head, n(1))), ZERO, plus(length, n(1)), 30),
        ],
        ..Default::default()
    }
}
fn remove_tail_call(role: Word, length: Word, member: Word) -> eth::Call {
    let mut call = add_call(role, n(8), ZERO, member);
    let head = leaf(role, n(8));
    let tail = plus(hash(&head), minus(length, n(1)));
    let index = leaf(member, plus(head, n(1)));
    call.storage_changes = vec![
        row(&call.address, tail, member, member, 10),
        row(&call.address, index, length, length, 20),
        row(&call.address, tail, member, ZERO, 30),
        row(&call.address, head, length, minus(length, n(1)), 40),
        row(&call.address, index, length, ZERO, 50),
    ];
    call
}

#[test]
fn enumerable_saved_cases_accept_all_optional_equal_subsets_and_only_observed_events() {
    let proof = evidence();
    assert_eq!(proof["status"], "synthetic_local_evm_not_producer");
    assert_eq!(proof["qualified"], false);
    assert_eq!(proof["compiler"]["compilerVersion"], "0.7.5+commit.eb77ed08");
    assert_eq!(proof["runtime_keccak256"], "0x60cdb82077b195e34bf239223f78f452a414001e509cac48230a865895d86884");
    let mut combinations = 0;
    let mut stored = 0;
    assert_eq!(cases().len(), 11);
    for case in cases() {
        assert_eq!(case["producer_observation"], false);
        let writes = case["ordered_sstores"].as_array().unwrap();
        stored += writes.len();
        let pcs: Vec<_> = writes.iter().map(|w| w["pc"].as_u64().unwrap()).collect();
        if !pcs.is_empty() {
            assert_eq!(
                pcs,
                if case["synthetic_call"]["method"] == "grantRole(bytes32,address)" {
                    vec![6494, 6510, 6529]
                } else {
                    vec![7257, 7277, 7308, 7310, 7335]
                }
            );
        }
        let equal = writes.iter().filter(|w| w["equal_value"] == true).count();
        for subset in 0..(1 << equal) {
            let block = block(saved_call(&case, subset));
            let accepted = enumerable_sets::validate(&block, &layouts(), &hints(&block)).unwrap();
            let expected: BTreeSet<_> = block.transaction_traces[0].calls[0]
                .storage_changes
                .iter()
                .map(|r| (r.address.clone(), word(&r.key).unwrap(), r.ordinal))
                .collect();
            assert_eq!(accepted, expected, "{} subset {subset}", case["name"]);
            ok(&block);
            combinations += 1;
        }
    }
    assert_eq!(combinations, 26);
    assert_eq!(stored, 39);
}

#[test]
fn enumerable_every_changing_store_is_required_and_checked() {
    for case in cases() {
        let valid = block(saved_call(&case, 0));
        for i in 0..valid.transaction_traces[0].calls[0].storage_changes.len() {
            let mut missing = valid.clone();
            first(&mut missing).storage_changes.remove(i);
            bad(&missing);
            for old in [false, true] {
                let mut corrupt = valid.clone();
                let row = &mut first(&mut corrupt).storage_changes[i];
                if old {
                    row.old_value = MAX.to_vec();
                } else {
                    row.new_value = MAX.to_vec();
                }
                bad(&corrupt);
            }
        }
    }
}

#[test]
fn enumerable_source_order_comes_from_unique_ordinals_not_vector_order() {
    for case in cases() {
        let valid = block(saved_call(&case, 0));
        let count = valid.transaction_traces[0].calls[0].storage_changes.len();
        let mut reversed_vector = valid.clone();
        first(&mut reversed_vector).storage_changes.reverse();
        ok(&reversed_vector);
        for i in 0..count {
            let mut zero = valid.clone();
            first(&mut zero).storage_changes[i].ordinal = 0;
            bad(&zero);
            for j in (i + 1)..count {
                let mut reordered = valid.clone();
                let rows = &mut first(&mut reordered).storage_changes;
                let left = rows[i].ordinal;
                rows[i].ordinal = rows[j].ordinal;
                rows[j].ordinal = left;
                bad(&reordered);
                let mut duplicate = valid.clone();
                let rows = &mut first(&mut duplicate).storage_changes;
                rows[j].ordinal = rows[i].ordinal;
                bad(&duplicate);
            }
        }
    }
}

#[test]
fn enumerable_supplied_equal_writes_cannot_be_corrupted_or_duplicated() {
    for case in cases() {
        let valid = block(saved_call(&case, usize::MAX));
        for i in 0..valid.transaction_traces[0].calls[0].storage_changes.len() {
            let row = &valid.transaction_traces[0].calls[0].storage_changes[i];
            if row.old_value != row.new_value {
                continue;
            }
            for old in [false, true] {
                let mut changed = valid.clone();
                let row = &mut first(&mut changed).storage_changes[i];
                if old {
                    row.old_value = MAX.to_vec();
                } else {
                    row.new_value = MAX.to_vec();
                }
                bad(&changed);
            }
            let mut duplicate = valid.clone();
            let mut row = row.clone();
            row.ordinal += 1;
            first(&mut duplicate).storage_changes.push(row);
            bad(&duplicate);
        }
    }
}

#[test]
fn enumerable_valid_permission_does_not_allow_later_changed_or_restored_writes() {
    let valid = saved("add_empty");
    for witness in &valid.transaction_traces[0].calls[0].storage_changes {
        let mut attacked = valid.clone();
        let mut change = witness.clone();
        change.old_value = witness.new_value.clone();
        change.new_value = n(99).to_vec();
        change.ordinal = 70;
        let mut restore = change.clone();
        restore.old_value = change.new_value.clone();
        restore.new_value = change.old_value.clone();
        restore.ordinal = 80;
        first(&mut attacked).storage_changes.extend([change, restore]);
        bad(&attacked);
        // An explicit generic metadata fallback must not mask enum failure.
        let mut cfg = config();
        cfg["other_slots"] = json!([format!("0x{}", hex::encode(&witness.key))]);
        let layouts = parse(cfg).unwrap();
        assert!(project(&attacked, &layouts).is_err());
    }
}

#[test]
fn enumerable_orphan_noops_and_admin_offsets_cannot_borrow_an_operation() {
    let original = saved("add_empty");
    let head = word(&first(&mut original.clone()).storage_changes[0].key).unwrap();
    let index = word(&original.transaction_traces[0].calls[0].storage_changes[2].key).unwrap();
    for key in [head, plus(head, n(1)), plus(head, n(2)), plus(head, n(3)), index] {
        let mut orphan = original.clone();
        let account = first(&mut orphan).address.clone();
        first(&mut orphan).storage_changes = vec![row(&account, key, n(1), n(2), 10)];
        bad(&orphan);
        if key != plus(head, n(3)) {
            first(&mut orphan).storage_changes[0].new_value = n(1).to_vec();
            bad(&orphan);
        }
    }
    for i in 0..3 {
        let mut attacked = original.clone();
        let mut extra = first(&mut attacked).storage_changes[i].clone();
        extra.old_value = extra.new_value.clone();
        extra.ordinal = 70;
        first(&mut attacked).storage_changes.push(extra);
        bad(&attacked);
    }
}

#[test]
fn enumerable_hash_preimages_require_exact_outer_root_depth_and_canonical_address() {
    let valid = saved("add_empty");
    for key in valid.transaction_traces[0].calls[0].keccak_preimages.keys() {
        let mut missing = valid.clone();
        first(&mut missing).keccak_preimages.remove(key);
        bad(&missing);
        let mut corrupt = valid.clone();
        first(&mut corrupt).keccak_preimages.get_mut(key).unwrap().push('0');
        bad(&corrupt);
    }
    let c = case("add_empty");
    let member = word(&bytes(&c["synthetic_call"]["member"])).unwrap();
    let head = word(&bytes(&c["role_record"])).unwrap();
    for offset in [0, 2, 3] {
        let mut wrong = valid.clone();
        let path = input(member, plus(head, n(offset)));
        first(&mut wrong).storage_changes[2].key = hash(&path).to_vec();
        preimage(&mut first(&mut wrong).keccak_preimages, path);
        bad(&wrong);
    }
    let mut dirty = valid.clone();
    let mut dirty_member = member;
    dirty_member[0] = 1;
    let path = input(dirty_member, plus(head, n(1)));
    first(&mut dirty).storage_changes[1].new_value = dirty_member.to_vec();
    first(&mut dirty).storage_changes[2].key = hash(&path).to_vec();
    preimage(&mut first(&mut dirty).keccak_preimages, path);
    bad(&dirty);
    for short in [false, true] {
        let mut wrong = valid.clone();
        let mut path = input(member, plus(head, n(1)));
        if short {
            path.remove(0);
        } else {
            path.insert(0, 0);
        }
        first(&mut wrong).storage_changes[2].key = hash(&path).to_vec();
        preimage(&mut first(&mut wrong).keccak_preimages, path);
        bad(&wrong);
    }
    let mut extra_depth = valid;
    let nested = leaf(member, plus(head, n(1)));
    let path = input(member, nested);
    first(&mut extra_depth).storage_changes[2].key = hash(&path).to_vec();
    preimage(&mut first(&mut extra_depth).keccak_preimages, path);
    bad(&extra_depth);
}

#[test]
fn enumerable_role_keys_are_full_bytes32_and_multiple_roles_remain_independent() {
    for role in [ZERO, MAX] {
        ok(&block(add_call(role, n(8), ZERO, n(17))));
    }
    let mut multiple = block(add_call(ZERO, n(8), ZERO, n(17)));
    append(&mut multiple, add_call(MAX, n(8), ZERO, n(18)));
    ok(&multiple);
    // A second explicit root is independently scoped, not a wildcard mapping.
    let mut cfg = config();
    cfg["enumerable_address_sets"]
        .as_array_mut()
        .unwrap()
        .push(json!({"root":hexword(n(9)),"key_types":["bytes32"],"semantics":"oz_3_4_2"}));
    let mut two_roots = block(add_call(ZERO, n(8), ZERO, n(17)));
    append(&mut two_roots, add_call(ZERO, n(9), ZERO, n(18)));
    assert!(project(&two_roots, &parse(cfg).unwrap()).unwrap().balances.is_empty());
}

#[test]
fn enumerable_structural_identity_blocks_cross_call_and_transaction_witnesses() {
    let valid = saved("add_empty");
    let mut split = valid.clone();
    let mut second = first(&mut split).clone();
    second.storage_changes = first(&mut split).storage_changes.split_off(1);
    // Equal declared indexes must never join two structural records.
    split.transaction_traces[0].calls.push(second);
    bad(&split);
    let mut tx_split = valid.clone();
    let mut second = tx_split.transaction_traces[0].clone();
    second.calls[0].storage_changes = first(&mut tx_split).storage_changes.split_off(1);
    tx_split.transaction_traces.push(second);
    bad(&tx_split);
    let mut system_split = valid;
    let mut system = first(&mut system_split).clone();
    system.storage_changes = first(&mut system_split).storage_changes.split_off(1);
    system_split.system_calls.push(system);
    bad(&system_split);
}

#[test]
fn enumerable_operation_cannot_cross_even_reverted_child_boundaries_or_ambiguous_frames() {
    for (begin, end) in [(15, 16), (5, 15), (25, 120), (1, 100), (0, 0)] {
        let mut attacked = saved("add_empty");
        attacked.transaction_traces[0].calls.push(eth::Call {
            index: 1,
            parent_index: 0,
            depth: 1,
            address: vec![0x77; 20],
            begin_ordinal: begin,
            end_ordinal: end,
            state_reverted: true,
            ..Default::default()
        });
        bad(&attacked);
    }
    for (begin, end) in [(0, 100), (10, 100), (1, 30), (100, 1)] {
        let mut attacked = saved("add_empty");
        first(&mut attacked).begin_ordinal = begin;
        first(&mut attacked).end_ordinal = end;
        bad(&attacked);
    }
    // Properly nested enclosing frames and disjoint child calls do not split it.
    let mut nested = saved("add_empty");
    first(&mut nested).begin_ordinal = 5;
    first(&mut nested).end_ordinal = 90;
    first(&mut nested).index = 1;
    first(&mut nested).depth = 1;
    nested.transaction_traces[0].calls.insert(
        0,
        eth::Call {
            address: vec![0x77; 20],
            begin_ordinal: 1,
            end_ordinal: 100,
            ..Default::default()
        },
    );
    ok(&nested);
}

#[test]
fn enumerable_supported_v3_root_ordinal_uses_transaction_boundary_only() {
    let mut legacy = saved("add_empty");
    legacy.ver = 3;
    first(&mut legacy).begin_ordinal = 0;
    legacy.transaction_traces[0].begin_ordinal = 1;
    ok(&legacy);
    legacy.transaction_traces[0].begin_ordinal = 0;
    bad(&legacy);
    legacy.transaction_traces[0].begin_ordinal = 10;
    bad(&legacy);
    let mut child = saved("add_empty");
    child.ver = 3;
    first(&mut child).begin_ordinal = 0;
    child.transaction_traces[0].calls.insert(
        0,
        eth::Call {
            address: vec![0x77; 20],
            begin_ordinal: 1,
            end_ordinal: 100,
            ..Default::default()
        },
    );
    bad(&child);
}

#[test]
fn enumerable_failed_reverted_and_system_call_persistence_matches_mapper() {
    let mut invalid = saved("add_empty");
    first(&mut invalid).storage_changes.remove(1);
    bad(&invalid);
    for status in [eth::TransactionTraceStatus::Failed, eth::TransactionTraceStatus::Reverted] {
        let mut failed = invalid.clone();
        failed.transaction_traces[0].status = status as i32;
        ok(&failed);
    }
    let mut reverted = invalid.clone();
    first(&mut reverted).state_reverted = true;
    ok(&reverted);
    let mut system = saved("add_empty");
    system.system_calls = system.transaction_traces.remove(0).calls;
    ok(&system);
    system.system_calls[0].storage_changes.remove(1);
    bad(&system);
    system.system_calls[0].state_reverted = true;
    ok(&system);
}

#[test]
fn enumerable_reverted_preimages_are_hints_not_persisted_witnesses() {
    let mut b = saved("add_empty");
    let hints = std::mem::take(&mut first(&mut b).keccak_preimages);
    b.transaction_traces[0].calls.push(eth::Call {
        address: bytes(&config()["contract"]),
        begin_ordinal: 70,
        end_ordinal: 90,
        state_reverted: true,
        keccak_preimages: hints,
        ..Default::default()
    });
    ok(&b);
    let element = first(&mut b).storage_changes.remove(1);
    b.transaction_traces[0].calls[1].storage_changes.push(element);
    bad(&b);
}

#[test]
fn enumerable_multiple_accounts_are_scoped_and_interleaving_is_rejected() {
    let cfg1 = config();
    let mut cfg2 = config();
    let other = vec![0x77; 20];
    cfg2["contract"] = json!(format!("0x{}", hex::encode(&other)));
    let layouts = layout::parse(&json!([cfg1, cfg2]).to_string()).unwrap();
    let mut b = saved("add_empty");
    let mut second = saved_call(&case("add_empty"), usize::MAX);
    second.address = other.clone();
    for row in &mut second.storage_changes {
        row.address = other.clone();
    }
    append(&mut b, second.clone());
    assert!(project(&b, &layouts).unwrap().balances.is_empty());
    let mut interleaved = saved("add_empty");
    second.begin_ordinal = 12;
    second.end_ordinal = 28;
    for (i, row) in second.storage_changes.iter_mut().enumerate() {
        row.ordinal = 13 + i as u64 * 5;
    }
    interleaved.transaction_traces[0].calls.push(second);
    assert!(project(&interleaved, &layouts).is_err());
}

#[test]
fn enumerable_full_uint256_lengths_and_modular_storage_addresses_are_not_truncated() {
    let role = n(7);
    for length in [n(u64::MAX), plus(n(u64::MAX), n(1)), minus(MAX, n(1))] {
        ok(&block(add_call(role, n(8), length, n(17))));
        ok(&block(remove_tail_call(role, plus(length, n(1)), n(17))));
    }
    // E(N) wraps to a small physical word, but N remains a full-width length.
    let array = hash(&leaf(role, n(8)));
    let length = minus(n(3), array);
    let wrapped = add_call(role, n(8), length, n(17));
    assert_eq!(wrapped.storage_changes[1].key, n(3));
    ok(&block(wrapped));
    bad(&block(add_call(role, n(8), MAX, n(17))));
    bad(&block(remove_tail_call(role, ZERO, n(17))));
}

#[test]
fn enumerable_removal_rejects_zero_out_of_range_or_aliased_member_positions() {
    for position in [ZERO, n(4), MAX] {
        let mut wrong = saved("remove_middle");
        first(&mut wrong).storage_changes.last_mut().unwrap().old_value = position.to_vec();
        bad(&wrong);
    }
    let mut wrong = saved("remove_middle");
    let removed = first(&mut wrong).storage_changes.last().unwrap().key.clone();
    first(&mut wrong).storage_changes[1].key = removed;
    bad(&wrong);
}

#[test]
fn enumerable_valid_add_remove_restoration_is_checked_in_execution_order() {
    let mut b = saved("add_empty");
    append(&mut b, saved_call(&case("remove_singleton"), usize::MAX));
    ok(&b);
    let mut third = saved_call(&case("add_empty"), usize::MAX);
    shift(&mut third, 200);
    b.transaction_traces[0].end_ordinal = 300;
    b.transaction_traces[0].calls.push(third);
    ok(&b);
    let mut repeated = saved("add_empty");
    append(&mut repeated, saved_call(&case("add_empty"), usize::MAX));
    bad(&repeated);
}

#[test]
fn enumerable_inferred_zero_constraints_cannot_contradict_prior_touched_state() {
    let role = n(7);
    let mut b = block(add_call(role, n(8), ZERO, n(17)));
    // Claimed removal of zero has the right local head/index equations, but its
    // omitted self-copy/tail-clear contradict the prior observed nonzero element.
    let mut remove = remove_tail_call(role, n(1), ZERO);
    remove.storage_changes.retain(|r| r.old_value != r.new_value);
    append(&mut b, remove);
    bad(&b);
    let mut later = block(add_call(role, n(8), ZERO, ZERO));
    first(&mut later).storage_changes.retain(|r| r.old_value != r.new_value);
    append(&mut later, remove_tail_call(role, n(1), n(17)));
    bad(&later);
}

#[test]
fn enumerable_arbitrary_storage_never_becomes_array_permission() {
    let mut b = saved("add_empty");
    let account = first(&mut b).address.clone();
    first(&mut b).storage_changes.push(row(&account, n(999), ZERO, n(1), 70));
    bad(&b);
    // Unknown no-ops retain the existing mapper policy, but receive no permit.
    first(&mut b).storage_changes.last_mut().unwrap().new_value = ZERO.to_vec();
    ok(&b);
    let accepted = enumerable_sets::validate(&b, &layouts(), &hints(&b)).unwrap();
    assert!(!accepted.contains(&(account, n(999), 70)));
}

#[test]
fn enumerable_balance_output_and_ordinary_balance_noops_are_unchanged() {
    let mut b = saved("add_empty");
    let account = first(&mut b).address.clone();
    let holder = vec![0x45; 20];
    let key = mapping(&holder, &ZERO);
    preimage(&mut first(&mut b).keccak_preimages, input(word(&holder).unwrap(), ZERO));
    first(&mut b).storage_changes.push(row(&account, key, n(8), n(9), 70));
    first(&mut b).storage_changes.push(row(&account, key, n(9), n(9), 80));
    let output = project(&b, &layouts()).unwrap();
    assert_eq!(output.balances.len(), 1);
    assert_eq!(output.balances[0].contract, Some(account));
    assert_eq!(output.balances[0].address, holder);
    assert_eq!(output.balances[0].amount, "9");
    let mut plain = b.clone();
    first(&mut plain).storage_changes.drain(..3);
    let mut cfg = config();
    cfg.as_object_mut().unwrap().remove("enumerable_address_sets");
    assert_eq!(output, project(&plain, &parse(cfg).unwrap()).unwrap());
}

#[test]
fn enumerable_code_and_dependency_change_restoration_still_invalidates_layout() {
    let cfg = config();
    let account = bytes(&cfg["contract"]);
    let mut code = saved("add_empty");
    first(&mut code).code_changes = vec![
        eth::CodeChange {
            address: account.clone(),
            old_hash: vec![1; 32],
            new_hash: vec![2; 32],
            ordinal: 70,
            ..Default::default()
        },
        eth::CodeChange {
            address: account.clone(),
            old_hash: vec![2; 32],
            new_hash: vec![1; 32],
            ordinal: 80,
            ..Default::default()
        },
    ];
    assert!(project(&code, &layouts()).unwrap_err().to_string().contains("code changed"));
    for (field, value) in [
        ("zero_balance", json!({"value":hexword(n(1)),"storage_slot":hexword(n(99))})),
        ("balance_divisor", json!({"value":hexword(n(2)),"storage_slot":hexword(n(99))})),
        (
            "proxy",
            json!({"implementation_slot":hexword(n(99)),"implementation":format!("0x{}","77".repeat(20)),"code_hash":hexword(n(1))}),
        ),
    ] {
        let mut cfg = config();
        cfg[field] = value;
        let layouts = parse(cfg).unwrap();
        let mut b = saved("add_empty");
        first(&mut b)
            .storage_changes
            .extend([row(&account, n(99), n(1), n(2), 70), row(&account, n(99), n(2), n(1), 80)]);
        let error = project(&b, &layouts).unwrap_err().to_string();
        assert!(
            error.contains("dependency changed") || error.contains("divisor changed") || error.contains("implementation slot changed"),
            "{field}: {error}"
        );
    }
}

#[test]
fn enumerable_opt_in_requires_exact_semantics_and_exclusive_root() {
    let mut absent = config();
    absent.as_object_mut().unwrap().remove("enumerable_address_sets");
    let absent = parse(absent).unwrap();
    assert!(absent[0].enumerable_address_sets.is_empty());
    assert!(project(&saved("add_empty"), &absent).is_err());
    for value in [json!("other"), json!(null)] {
        let mut cfg = config();
        cfg["enumerable_address_sets"][0]["semantics"] = value;
        assert!(parse(cfg).is_err());
    }
    for keys in [json!([]), json!(["address"]), json!(["bytes32", "address"])] {
        let mut cfg = config();
        cfg["enumerable_address_sets"][0]["key_types"] = keys;
        assert!(parse(cfg).is_err());
    }
    for root in ["0x08", "0xzz", ""] {
        let mut cfg = config();
        cfg["enumerable_address_sets"][0]["root"] = json!(root);
        assert!(parse(cfg).is_err());
    }
    let mut unknown = config();
    unknown["enumerable_address_sets"][0]["guess"] = json!(true);
    assert!(parse(unknown).is_err());
    let mut duplicate = config();
    let rule = duplicate["enumerable_address_sets"][0].clone();
    duplicate["enumerable_address_sets"].as_array_mut().unwrap().push(rule);
    assert!(parse(duplicate).is_err());
    for (field, value) in [
        ("balance_slot", json!(hexword(n(8)))),
        ("other_slots", json!([hexword(n(8))])),
        ("other_mapping_slots", json!([hexword(n(8))])),
        ("other_mapping_words", json!({hexword(n(8)):3})),
        ("other_mapping_paths", json!([{"root":hexword(n(8)),"key_types":["bytes32"],"offset":2}])),
        ("address_lists", json!([hexword(n(8))])),
        ("voting_checkpoints", json!({"clock":"block_number","slots":[hexword(n(8))]})),
        ("zero_balance", json!({"value":hexword(n(1)),"storage_slot":hexword(n(8))})),
        ("balance_divisor", json!({"value":hexword(n(2)),"storage_slot":hexword(n(8))})),
    ] {
        let mut cfg = config();
        cfg[field] = value;
        assert!(parse(cfg).is_err(), "{field}");
    }
}

#[test]
fn enumerable_absent_or_empty_configuration_preserves_legacy_serialization() {
    let mut absent = config();
    absent.as_object_mut().unwrap().remove("enumerable_address_sets");
    let absent: layout::Layout = serde_json::from_value(absent).unwrap();
    let mut empty = config();
    empty["enumerable_address_sets"] = json!([]);
    let empty: layout::Layout = serde_json::from_value(empty).unwrap();
    let absent = serde_json::to_value(absent).unwrap();
    assert!(absent.get("enumerable_address_sets").is_none());
    assert_eq!(absent, serde_json::to_value(empty).unwrap());
    let configured: layout::Layout = serde_json::from_value(config()).unwrap();
    assert_eq!(
        serde_json::to_value(configured).unwrap()["enumerable_address_sets"],
        config()["enumerable_address_sets"]
    );
}

#[test]
fn enumerable_entire_operation_cannot_be_misattributed_to_parent_or_foreign_transaction() {
    let mut child = saved("add_empty");
    child.transaction_traces[0].calls.push(eth::Call {
        index: 1,
        depth: 1,
        address: vec![0x77; 20],
        begin_ordinal: 5,
        end_ordinal: 40,
        ..Default::default()
    });
    bad(&child);
    let mut foreign = saved("add_empty");
    foreign.transaction_traces.push(eth::TransactionTrace {
        status: eth::TransactionTraceStatus::Succeeded as i32,
        calls: vec![eth::Call {
            address: vec![0x77; 20],
            begin_ordinal: 5,
            end_ordinal: 40,
            ..Default::default()
        }],
        ..Default::default()
    });
    bad(&foreign);
    // An unrelated transaction's malformed empty frame is not used as a proof.
    foreign.transaction_traces[1].calls[0].begin_ordinal = 0;
    foreign.transaction_traces[1].calls[0].end_ordinal = 0;
    ok(&foreign);
}

#[test]
fn enumerable_foreign_account_writes_are_scope_barriers_even_if_unconfigured() {
    for ordinal in [0, 10, 15, 100] {
        let mut b = saved("add_empty");
        first(&mut b).storage_changes.push(row(&[0x77; 20], n(99), ZERO, n(1), ordinal));
        bad(&b);
    }
    let mut outside = saved("add_empty");
    first(&mut outside).storage_changes.push(row(&[0x77; 20], n(99), ZERO, n(1), 70));
    ok(&outside);
}

#[test]
fn enumerable_modular_element_alias_cannot_consume_another_known_role_namespace() {
    let role = n(7);
    let other_role = n(9);
    let other_head = leaf(other_role, n(8));
    let length = minus(other_head, hash(&leaf(role, n(8))));
    for member in [ZERO, n(17)] {
        let mut call = add_call(role, n(8), length, member);
        assert_eq!(call.storage_changes[1].key, other_head);
        preimage(&mut call.keccak_preimages, input(other_role, n(8)));
        if member == ZERO {
            call.storage_changes.retain(|r| r.old_value != r.new_value);
        }
        bad(&block(call));
    }
}

#[test]
fn enumerable_known_balance_leaf_alias_is_rejected_even_when_its_zero_store_is_omitted() {
    let role = n(7);
    let holder = vec![0x45; 20];
    let balance_key = mapping(&holder, &ZERO);
    let length = minus(balance_key, hash(&leaf(role, n(8))));
    for member in [ZERO, n(17)] {
        let mut call = add_call(role, n(8), length, member);
        assert_eq!(call.storage_changes[1].key, balance_key);
        preimage(&mut call.keccak_preimages, input(word(&holder).unwrap(), ZERO));
        if member == ZERO {
            call.storage_changes.retain(|r| r.old_value != r.new_value);
        }
        let b = block(call);
        assert!(enumerable_sets::validate(&b, &layouts(), &hints(&b)).is_err());
        bad(&b);
    }
}

#[test]
fn enumerable_observed_balance_alias_from_candidate_address_never_emits_a_balance() {
    let role = n(7);
    let holder = vec![0x45; 20];
    let balance_key = mapping(&holder, &ZERO);
    let length = minus(balance_key, hash(&leaf(role, n(8))));
    let mut b = block(add_call(role, n(8), length, n(17)));
    // There is deliberately no balance preimage; the normal mapper discovers
    // this holder independently from the transaction's sender candidate.
    assert!(!hints(&b).contains_key(&balance_key));
    b.transaction_traces[0].from = holder;
    bad(&b);
}

#[test]
fn enumerable_inferred_zero_slot_cannot_alias_an_explicit_protected_dependency() {
    let role = n(7);
    let mut variants = vec![(config(), ZERO)]; // The balance mapping root itself.
    for (field, value) in [
        ("zero_balance", json!({"value":hexword(n(1)),"storage_slot":hexword(n(99))})),
        ("balance_divisor", json!({"value":hexword(n(2)),"storage_slot":hexword(n(99))})),
        (
            "proxy",
            json!({"implementation_slot":hexword(n(99)),"implementation":format!("0x{}","77".repeat(20)),"code_hash":hexword(n(1))}),
        ),
    ] {
        let mut cfg = config();
        cfg[field] = value;
        variants.push((cfg, n(99)));
    }
    for (cfg, key) in variants {
        let length = minus(key, hash(&leaf(role, n(8))));
        let mut call = add_call(role, n(8), length, ZERO);
        assert_eq!(call.storage_changes[1].key, key);
        call.storage_changes.retain(|r| r.old_value != r.new_value);
        let b = block(call);
        let layouts = parse(cfg).unwrap();
        assert!(enumerable_sets::validate(&b, &layouts, &hints(&b)).is_err());
        assert!(project(&b, &layouts).is_err());
    }
}

#[test]
fn enumerable_inferred_array_word_cannot_alias_its_own_member_index() {
    let role = n(7);
    let head = leaf(role, n(8));
    let index = leaf(ZERO, plus(head, n(1)));
    let length = minus(index, hash(&head));
    let mut call = add_call(role, n(8), length, ZERO);
    assert_eq!(call.storage_changes[1].key, call.storage_changes[2].key);
    call.storage_changes.retain(|r| r.old_value != r.new_value);
    bad(&block(call));
}
