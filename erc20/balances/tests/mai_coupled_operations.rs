#![cfg(not(target_arch = "wasm32"))]
//! Committed Mai bytecode transcripts translated to synthetic Extended records.
//! Source execution supplies physical order, not actual producer visibility,
//! authorization, deployed initialization or permission for malformed prestates.
use erc20_balances::{hash, layout, project};
use serde_json::{json, Value};
use std::sync::OnceLock;
use substreams_ethereum::pb::eth::v2 as eth;
type Word = [u8; 32];
const ZERO: Word = [0; 32];
const END: u64 = 10000;
const TRANSCRIPTS: &str = include_str!("../docs/evidence/mai-operation-proof-20260929-transcripts.json");
fn n(value: u64) -> Word {
    let mut v = ZERO;
    v[24..].copy_from_slice(&value.to_be_bytes());
    v
}
fn bytes(s: &str) -> Vec<u8> {
    hex::decode(s.trim_start_matches("0x")).unwrap()
}
fn w(v: &Value) -> Word {
    bytes(v.as_str().unwrap()).try_into().unwrap()
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
    let mut v = ZERO;
    let mut carry = 0u16;
    for i in (0..32).rev() {
        carry += u16::from(a[i]) + u16::from(b[i]);
        v[i] = carry as u8;
        carry >>= 8;
    }
    v
}
fn minus(a: Word, b: Word) -> Word {
    plus(plus(a, b.map(|x| !x)), n(1))
}
fn original() -> Value {
    let baseline: Vec<Value> = serde_json::from_str(include_str!("fixtures/bsc-refined450-layouts.json")).unwrap();
    baseline
        .into_iter()
        .find(|v| v["contract"] == "0x35803e77c3163fed8a942536c1c8e0d5bf90f906")
        .unwrap()
}
fn config() -> Value {
    let mut c = original();
    c["other_mapping_slots"] = json!([h(n(3))]);
    c["other_mapping_words"] = json!({});
    c["enumerable_address_sets"] = json!([{"root":h(n(1)),"membership_root":h(ZERO),"key_types":["bytes32"],"semantics":"mai_solc_0_8_9_oz_4_7_0"}]);
    c
}
fn account() -> Vec<u8> {
    bytes("35803e77c3163fed8a942536c1c8e0d5bf90f906")
}
fn records() -> &'static [Value] {
    static ALL: OnceLock<Vec<Value>> = OnceLock::new();
    ALL.get_or_init(|| serde_json::from_str(TRANSCRIPTS).unwrap())
}
fn role_entry(v: &Value) -> bool {
    matches!(
        v["signature"].as_str(),
        Some("grantRole(bytes32,address)" | "revokeRole(bytes32,address)" | "renounceRole(bytes32,address)")
    )
}
fn cases(name: &str) -> Vec<&'static Value> {
    records().iter().filter(|v| v["name"] == name && role_entry(v)).collect()
}
fn case(name: &str) -> &'static Value {
    let found = cases(name);
    assert_eq!(found.len(), 1, "unique role entrypoint for {name}; getters are excluded");
    found[0]
}
fn role(v: &Value) -> Word {
    bytes(v["calldata"].as_str().unwrap())[4..36].try_into().unwrap()
}
fn member(v: &Value) -> Word {
    bytes(v["calldata"].as_str().unwrap())[36..68].try_into().unwrap()
}
fn image(c: &mut eth::Call, raw: Vec<u8>) {
    c.keccak_preimages.insert(hex::encode(hash(&raw)), hex::encode(raw));
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
fn saved_record(v: &Value, subset: usize) -> eth::Call {
    let mut c = eth::Call {
        address: account(),
        begin_ordinal: 1,
        end_ordinal: END,
        ..Default::default()
    };
    for k in v["execution"]["keccaks"].as_array().unwrap() {
        let raw = bytes(k["input"].as_str().unwrap());
        assert_eq!(h(hash(&raw)), k["output"]);
        image(&mut c, raw);
    }
    let mut equality = 0;
    for s in v["execution"]["writes"].as_array().unwrap() {
        if s["old"] == s["new"] {
            let keep = subset & (1 << equality) != 0;
            equality += 1;
            if !keep {
                continue;
            }
        }
        let ordinal = s["step"].as_u64().unwrap() + 10;
        assert!(ordinal < 8000, "source effects fit actual synthetic frame");
        c.storage_changes.push(row(w(&s["key"]), w(&s["old"]), w(&s["new"]), ordinal));
    }
    c
}
fn saved(name: &str) -> eth::Call {
    saved_record(case(name), usize::MAX)
}
fn block(mut calls: Vec<eth::Call>) -> eth::Block {
    let end = END * calls.len() as u64;
    for (i, c) in calls.iter_mut().enumerate() {
        let shift = END * i as u64;
        c.begin_ordinal += shift;
        c.end_ordinal += shift;
        for s in &mut c.storage_changes {
            s.ordinal += shift;
        }
    }
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
            end_ordinal: end,
            calls,
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn first(b: &mut eth::Block) -> &mut eth::Call {
    &mut b.transaction_traces[0].calls[0]
}
fn add_balance(b: &mut eth::Block) {
    image(first(b), pair(n(44), n(2)));
    first(b).storage_changes.push(row(leaf(n(44), n(2)), ZERO, n(9), 9000));
}
fn output(b: &eth::Block, c: Value) -> Result<proto::pb::evm::balances::v1::Events, substreams::errors::Error> {
    project(b, &layout::parse(&json!([c]).to_string()).unwrap())
}
fn good(mut b: eth::Block) {
    assert!(output(&b, config()).unwrap().balances.is_empty());
    add_balance(&mut b);
    let events = output(&b, config()).unwrap();
    assert_eq!(events.balances.len(), 1);
    assert_eq!(events.balances[0].amount, "9");
}
fn bad(b: eth::Block) {
    bad_context(b, "malformed operation");
}
fn bad_context(mut b: eth::Block, context: &str) {
    add_balance(&mut b);
    assert!(
        output(&b, config()).is_err(),
        "{context}: invalid role metadata must refuse the simultaneous valid balance"
    );
}

#[test]
fn mai_exact_selected_template_is_recognized() {
    layout::parse(&json!([config()]).to_string()).unwrap();
    let candidate: Value = serde_json::from_str(include_str!("fixtures/mai-coupled-role-candidate/layouts.json")).unwrap();
    assert_eq!(candidate, json!([config()]));
    let mut restored = config();
    restored["other_mapping_slots"] = json!([h(ZERO), h(n(3))]);
    restored["other_mapping_words"] = json!({h(n(1)):2});
    restored.as_object_mut().unwrap().remove("enumerable_address_sets");
    assert_eq!(restored, original());
    // The preserved broad historical profile cannot classify these real
    // nonzero array stores; a new selector must not rewrite that evidence.
    for name in ["grant_empty", "remove_first"] {
        let mut b = block(vec![saved(name)]);
        add_balance(&mut b);
        assert!(output(&b, original()).is_err(), "historical refusal changed for {name}");
    }
}

#[test]
fn mai_committed_coherent_operations_all_equalities_and_full_role_identities() {
    let names = [
        "grant_empty",
        "grant_nonempty",
        "grant_zero",
        "grant_only_zero",
        "duplicate",
        "duplicate_zero",
        "remove_first",
        "remove_middle",
        "remove_tail",
        "remove_only",
        "remove_zero",
        "remove_zero_tail",
        "remove_only_zero",
        "move_zero_tail",
        "absent_empty",
        "absent_nonempty",
        "full_role_identity",
        "renounce_present",
        "renounce_absent",
        "custom_full_width_admin",
        "last_admin_loss",
    ];
    let mut operations = 0;
    let mut variants = 0;
    for name in names {
        for v in cases(name).into_iter().filter(|v| v["execution"]["exit"]["kind"] == "return") {
            operations += 1;
            let equalities = v["execution"]["writes"].as_array().unwrap().iter().filter(|s| s["old"] == s["new"]).count();
            for subset in 0..1 << equalities {
                for version in [4, 5] {
                    let mut b = block(vec![saved_record(v, subset)]);
                    b.ver = version;
                    good(b);
                }
                variants += 1;
            }
        }
    }
    assert_eq!(operations, 24);
    assert!(variants > operations);
    let identities: Vec<_> = cases("full_role_identity").into_iter().map(role).collect();
    assert_eq!(
        identities,
        [ZERO, hash(b"MINTER_ROLE"), [255; 32], {
            let mut r = ZERO;
            r[0] = 128;
            r
        }]
    );
    good(block((0..8).map(|i| saved(&format!("sequence_{i}"))).collect()));
    // Failure with no persisted stores is silence, not an inferred role prestate.
    for name in ["unauthorized", "wrong_renounce", "last_admin_cannot_regain"] {
        for v in cases(name) {
            assert_eq!(v["execution"]["exit"]["kind"], "revert");
            assert!(v["execution"]["writes"].as_array().unwrap().is_empty());
            good(block(vec![saved_record(v, usize::MAX)]));
        }
    }
}

#[test]
fn mai_one_sided_dirty_words_and_malformed_source_successes_remain_refused() {
    let mut seen = 0;
    for name in [
        "grant_bool_only",
        "grant_set_only",
        "revoke_bool_only",
        "revoke_set_only",
        "packed_boolean_padding",
        "malformed_length_push_source_semantics",
        "dirty_tail_moves_full_bytes32_index",
    ] {
        for v in cases(name) {
            assert_eq!(v["execution"]["exit"]["kind"], "return");
            bad(block(vec![saved_record(v, usize::MAX)]));
            seen += 1;
        }
    }
    assert_eq!(seen, 11);
    for name in ["empty_length_position", "position_past_length"] {
        let v = case(name);
        assert_eq!(v["execution"]["exit"]["kind"], "revert");
        assert_eq!(v["execution"]["writes"].as_array().unwrap().len(), 1);
        bad(block(vec![saved(name)])); // Fabricated persisted failure prefixes supply no permission.
        let mut reverted = saved(name);
        reverted.state_reverted = true;
        good(block(vec![saved("duplicate"), reverted]));
    }
}

#[test]
fn mai_missing_extra_repeated_corrupt_or_reordered_stages_refuse_atomically() {
    for name in [
        "grant_empty",
        "grant_only_zero",
        "remove_first",
        "remove_tail",
        "remove_only_zero",
        "move_zero_tail",
    ] {
        let original = saved(name);
        for (i, s) in original.storage_changes.iter().enumerate() {
            if s.old_value != s.new_value {
                let mut c = original.clone();
                c.storage_changes.remove(i);
                bad(block(vec![c]));
            }
            for old in [false, true] {
                let mut c = original.clone();
                if old {
                    c.storage_changes[i].old_value = n(99).to_vec();
                } else {
                    c.storage_changes[i].new_value = n(99).to_vec();
                }
                bad(block(vec![c]));
            }
            for field in 0..3 {
                let mut c = original.clone();
                match field {
                    0 => c.storage_changes[i].key.insert(0, 0),
                    1 => c.storage_changes[i].old_value.insert(0, 0),
                    _ => c.storage_changes[i].new_value.insert(0, 0),
                }
                bad(block(vec![c]));
            }
        }
        let mut reversed = original.clone();
        reversed.storage_changes.reverse();
        good(block(vec![reversed])); // Physical ordinals, not vector order, own execution order.
        let mut c = original.clone();
        let last = c.storage_changes.len() - 1;
        let ordinal = c.storage_changes[0].ordinal;
        c.storage_changes[0].ordinal = c.storage_changes[last].ordinal;
        c.storage_changes[last].ordinal = ordinal;
        bad(block(vec![c]));
        for i in [0, last] {
            let mut c = original.clone();
            let mut extra = c.storage_changes[i].clone();
            extra.ordinal += 1;
            c.storage_changes.push(extra);
            bad(block(vec![c]));
            let mut c = original.clone();
            let mut noop = c.storage_changes[i].clone();
            noop.new_value = noop.old_value.clone();
            noop.ordinal += 1;
            c.storage_changes.push(noop);
            bad(block(vec![c]));
        }
    }
}

#[test]
fn mai_exact_root_member_and_index_preimages_are_independently_required() {
    let v = case("grant_empty");
    let original = saved("grant_empty");
    let r = role(v);
    let m = member(v);
    let base = leaf(r, ZERO);
    let head = leaf(r, n(1));
    for raw in [pair(r, ZERO), pair(r, n(1)), pair(m, base), pair(m, plus(head, n(1)))] {
        let key = hex::encode(hash(&raw));
        assert_eq!(original.keccak_preimages.get(&key), Some(&hex::encode(&raw)));
        let mut c = original.clone();
        c.keccak_preimages.remove(&key);
        bad(block(vec![c]));
        let mut c = original.clone();
        c.keccak_preimages.insert(key, "ff".repeat(64));
        bad(block(vec![c]));
    }
    for raw in [
        pair(base, m),
        pair(
            {
                let mut dirty = m;
                dirty[0] = 1;
                dirty
            },
            base,
        ),
        {
            let mut p = pair(m, base);
            p.push(0);
            p
        },
        pair(m, leaf(r, n(8))),
    ] {
        let mut c = original.clone();
        c.keccak_preimages.remove(&hex::encode(leaf(m, base)));
        c.storage_changes[0].key = hash(&raw).to_vec();
        image(&mut c, raw);
        bad(block(vec![c]));
    }
    for bool_only in [false, true] {
        let mut c = original.clone();
        if bool_only {
            c.storage_changes.truncate(1);
        } else {
            c.storage_changes.remove(0);
        }
        let hidden_root = if bool_only { n(1) } else { ZERO };
        c.keccak_preimages.retain(|_, v| {
            let raw = bytes(v);
            raw.len() != 64 || raw[32..] != hidden_root
        });
        bad(block(vec![c.clone()]));
        for s in &mut c.storage_changes {
            s.new_value = s.old_value.clone();
        }
        bad(block(vec![c]));
    }
}

#[test]
fn mai_selected_schema_reserves_both_roots_without_admin_or_generic_aliases() {
    let parse = |c: Value| layout::parse(&json!([c]).to_string());
    for field in ["root", "membership_root"] {
        for value in [Value::Null, json!(h(n(8))), json!(h(n(2))), json!("0x00")] {
            let mut c = config();
            c["enumerable_address_sets"][0][field] = value;
            assert!(parse(c).is_err(), "{field}");
        }
    }
    let mut c = config();
    c["enumerable_address_sets"][0].as_object_mut().unwrap().remove("membership_root");
    assert!(parse(c).is_err());
    for types in [json!([]), json!(["address"]), json!(["bytes32", "address"])] {
        let mut c = config();
        c["enumerable_address_sets"][0]["key_types"] = types;
        assert!(parse(c).is_err());
    }
    for root in [ZERO, n(1)] {
        for field in ["other_slots", "other_mapping_slots", "address_lists"] {
            let mut c = config();
            let values = c.as_object_mut().unwrap().entry(field).or_insert(json!([]));
            values.as_array_mut().unwrap().push(json!(h(root)));
            assert!(parse(c).is_err(), "{field}");
        }
        let mut c = config();
        c["balance_slot"] = json!(h(root));
        assert!(parse(c).is_err());
        let mut c = config();
        c["other_mapping_words"] = json!({h(root):2});
        assert!(parse(c).is_err());
        let mut c = config();
        c["other_mapping_paths"] = json!([{"root":h(root),"key_types":["bytes32","address"],"words":1}]);
        assert!(parse(c).is_err());
    }
    let mut c = config();
    let rule = c["enumerable_address_sets"][0].clone();
    c["enumerable_address_sets"].as_array_mut().unwrap().push(rule);
    assert!(parse(c).is_err());
    let r = role(case("grant_empty"));
    for key in [
        leaf(r, ZERO),
        plus(leaf(r, ZERO), n(1)),
        plus(leaf(r, n(1)), n(1)),
        leaf(n(2), leaf(r, ZERO)),
        n(77),
    ] {
        for equal in [false, true] {
            let mut c = saved("duplicate");
            image(&mut c, pair(r, ZERO));
            image(&mut c, pair(r, n(1)));
            c.storage_changes.push(row(key, ZERO, if equal { ZERO } else { n(1) }, 8000));
            if key == n(77) && equal {
                // Outside an operation, ordinary unrelated equal writes retain
                // the existing no-op filter. Role namespace noops stay refused.
                good(block(vec![c]));
            } else {
                bad_context(block(vec![c]), &format!("standalone key={} equal={equal}", h(key)));
            }
        }
    }
}

#[test]
fn mai_roles_members_accounts_calls_and_transactions_cannot_be_joined() {
    let original = saved("grant_empty");
    for v in cases("full_role_identity").into_iter().chain([case("grant_only_zero")]) {
        let mut c = original.clone();
        let other = saved_record(v, usize::MAX);
        c.keccak_preimages.extend(other.keccak_preimages);
        let ordinal = c.storage_changes[0].ordinal;
        c.storage_changes[0] = other.storage_changes[0].clone();
        c.storage_changes[0].ordinal = ordinal;
        bad(block(vec![c]));
    }
    for split in 1..original.storage_changes.len() {
        let mut left = original.clone();
        let mut right = original.clone();
        right.storage_changes = left.storage_changes.split_off(split);
        bad(block(vec![left.clone(), right.clone()]));
        let mut b = block(vec![left, right]);
        let second = b.transaction_traces[0].calls.pop().unwrap();
        b.transaction_traces[0].end_ordinal = END;
        b.transaction_traces.push(eth::TransactionTrace {
            status: eth::TransactionTraceStatus::Succeeded as i32,
            begin_ordinal: END + 1,
            end_ordinal: END * 2,
            calls: vec![second],
            ..Default::default()
        });
        bad(b);
    }
    let other = vec![8; 20];
    let mut other_config = config();
    other_config["contract"] = json!(format!("0x{}", hex::encode(&other)));
    let layouts = layout::parse(&json!([config(), other_config]).to_string()).unwrap();
    let mut split = original;
    split.storage_changes[0].address = other.clone();
    let mut b = block(vec![split]);
    add_balance(&mut b);
    assert!(project(&b, &layouts).is_err());
    let mut independent = saved("remove_only");
    independent.address = other.clone();
    for s in &mut independent.storage_changes {
        s.address = other.clone();
    }
    let mut b = block(vec![saved_record(case("grant_only_zero"), 0), independent]);
    add_balance(&mut b);
    assert_eq!(project(&b, &layouts).unwrap().balances.len(), 1);
}

#[test]
fn mai_versions_actual_frame_bounds_ties_and_failure_filtering() {
    for ver in [0, 2, 3, 6] {
        for name in ["grant_empty", "duplicate"] {
            let mut b = block(vec![saved(name)]);
            b.ver = ver;
            bad(b);
        }
    }
    for ver in [4, 5] {
        for which in 0..7 {
            let mut b = block(vec![saved("grant_empty")]);
            b.ver = ver;
            let c = first(&mut b);
            match which {
                0 => c.begin_ordinal = 0,
                1 => c.end_ordinal = 0,
                2 => c.begin_ordinal = c.end_ordinal + 1,
                3 => c.begin_ordinal = c.storage_changes[0].ordinal,
                4 => c.end_ordinal = c.storage_changes.last().unwrap().ordinal,
                5 => c.storage_changes[1].ordinal = c.storage_changes[0].ordinal,
                _ => c.storage_changes[0].ordinal = 0,
            }
            bad(b);
        }
    }
    let mut b = block(vec![saved("grant_empty")]);
    add_balance(&mut b);
    b.transaction_traces[0].status = eth::TransactionTraceStatus::Failed as i32;
    assert!(output(&b, config()).unwrap().balances.is_empty());
    let mut failed = saved("grant_bool_only");
    failed.state_reverted = true;
    good(block(vec![saved("duplicate"), failed]));
}

#[test]
fn mai_child_sibling_foreign_unknown_and_noop_interruptions_remain_barriers() {
    let v = case("grant_only_zero");
    let c = saved_record(v, 0);
    let omitted = v["execution"]["writes"][2]["step"].as_u64().unwrap() + 10;
    for ordinal in [c.storage_changes[0].ordinal, omitted, c.storage_changes.last().unwrap().ordinal] {
        for foreign in [false, true] {
            let mut call = c.clone();
            let mut s = row(n(77), ZERO, ZERO, ordinal);
            if foreign {
                s.address = vec![8; 20];
            }
            call.storage_changes.push(s);
            bad(block(vec![call]));
        }
        for (depth, reverted) in [(0, false), (1, false), (1, true)] {
            let mut b = block(vec![c.clone()]);
            b.transaction_traces[0].calls.push(eth::Call {
                address: vec![8; 20],
                index: 1,
                parent_index: 0,
                depth,
                begin_ordinal: ordinal,
                end_ordinal: ordinal + 1,
                state_reverted: reverted,
                ..Default::default()
            });
            bad(b);
        }
    }
    let mut call = c;
    let mut outside = row(n(77), ZERO, ZERO, 8000);
    outside.address = vec![8; 20];
    call.storage_changes.push(outside);
    good(block(vec![call]));
}

#[test]
fn mai_optional_zero_stages_and_derived_moved_membership_keep_continuity() {
    for subset in [0, usize::MAX] {
        good(block(vec![
            saved_record(case("grant_only_zero"), subset),
            saved_record(case("remove_only_zero"), subset),
        ]));
        bad(block(vec![saved_record(case("grant_only_zero"), subset), saved("remove_only")]));
    }
    good(block(vec![saved("grant_empty"), saved("remove_only")]));
    bad(block(vec![saved("grant_empty"), saved("grant_empty")]));
    let v = case("remove_first");
    let c = saved("remove_first");
    let moved = w(&v["execution"]["writes"][1]["new"]);
    let moved_bool = leaf(moved, leaf(role(v), ZERO));
    assert!(!c.keccak_preimages.contains_key(&hex::encode(moved_bool)));
    good(block(vec![c.clone()]));
    for hint in [false, true] {
        let mut extra = c.clone();
        extra.storage_changes.push(row(moved_bool, n(1), n(1), 8000));
        if hint {
            image(&mut extra, pair(moved, leaf(role(v), ZERO)));
        }
        bad(block(vec![extra]));
    }
    let mut cfg = config();
    cfg["other_slots"].as_array_mut().unwrap().push(json!(h(moved_bool)));
    assert!(output(&block(vec![c]), cfg).unwrap_err().to_string().contains("coherence key aliases"));
}

fn zero_add_at(r: Word, target: Word, keep: bool) -> eth::Call {
    let head = leaf(r, n(1));
    let base = leaf(r, ZERO);
    let len = minus(target, hash(&head));
    assert_ne!(len, [255; 32]);
    let next = plus(len, n(1));
    let mut c = eth::Call {
        address: account(),
        begin_ordinal: 1,
        end_ordinal: END,
        ..Default::default()
    };
    for raw in [pair(r, ZERO), pair(r, n(1)), pair(ZERO, base), pair(ZERO, plus(head, n(1)))] {
        image(&mut c, raw);
    }
    c.storage_changes = vec![row(leaf(ZERO, base), ZERO, n(1), 100), row(head, len, next, 200)];
    if keep {
        c.storage_changes.push(row(target, ZERO, ZERO, 300));
    }
    c.storage_changes.push(row(leaf(ZERO, plus(head, n(1))), ZERO, next, 400));
    c
}

#[test]
fn mai_observed_and_omitted_array_stages_cannot_alias_protected_storage() {
    let r = n(77);
    let head = leaf(r, n(1));
    let base = leaf(r, ZERO);
    let allowance_base = leaf(n(44), n(3));
    let allowance = leaf(n(55), allowance_base);
    for keep in [false, true] {
        for target in [
            ZERO,
            n(1),
            n(2),
            n(3),
            n(4),
            n(5),
            n(6),
            head,
            base,
            plus(base, n(1)),
            plus(head, n(1)),
            leaf(n(44), n(2)),
            allowance,
        ] {
            let mut c = zero_add_at(r, target, keep);
            image(&mut c, pair(n(44), n(2)));
            image(&mut c, pair(n(44), n(3)));
            image(&mut c, pair(n(55), allowance_base));
            bad_context(block(vec![c]), &format!("array alias target={} keep_equal={keep}", h(target)));
        }
        let remove = saved("remove_first");
        let v = case("remove_first");
        let moved = w(&v["execution"]["writes"][1]["new"]);
        let moved_bool = leaf(moved, leaf(role(v), ZERO));
        let add = zero_add_at(r, moved_bool, keep);
        good(block(vec![add.clone()]));
        bad(block(vec![add.clone(), remove.clone()]));
        bad(block(vec![remove, add]));
        let add = zero_add_at(r, leaf(role(v), n(1)), keep);
        bad(block(vec![add, saved("remove_first")]));
    }
}

#[test]
fn mai_allowance_scalars_exact_runtime_and_saved_creation_guards_are_preserved() {
    let mut c = saved("grant_empty");
    for (i, slot) in [4, 5, 6].into_iter().enumerate() {
        c.storage_changes.push(row(n(slot), n(5), n(4), 8000 + i as u64));
    }
    let allowance_base = leaf(n(44), n(3));
    image(&mut c, pair(n(44), n(3)));
    image(&mut c, pair(n(55), allowance_base));
    c.storage_changes.push(row(leaf(n(55), allowance_base), n(1), n(2), 8100));
    good(block(vec![c]));
    let capture: Value = serde_json::from_str(include_str!("fixtures/mai-operation-proof/capture.json")).unwrap();
    let runtime = bytes(capture["runtimeBytecode"]["onchainBytecode"].as_str().unwrap());
    assert_eq!(h(hash(&runtime)), config()["code_hash"]);
    for creation in [false, true] {
        let mut b = block(vec![saved("grant_empty")]);
        if creation {
            first(&mut b).call_type = eth::CallType::Create as i32;
        }
        first(&mut b).code_changes.push(eth::CodeChange {
            address: account(),
            old_code: if creation { vec![] } else { runtime.clone() },
            new_code: if creation { runtime.clone() } else { vec![1] },
            old_hash: if creation { hash(&[]).to_vec() } else { hash(&runtime).to_vec() },
            new_hash: if creation { hash(&runtime).to_vec() } else { hash(&[1]).to_vec() },
            ordinal: 8000,
        });
        bad(b);
    }
    let constructors: Vec<_> = records().iter().filter(|v| v["signature"] == "constructor()").collect();
    assert_eq!(constructors.len(), 1);
    assert_eq!(constructors[0]["execution"]["exit"]["kind"], "return");
    let mut actual_prefix = saved_record(constructors[0], usize::MAX);
    actual_prefix.call_type = eth::CallType::Create as i32;
    actual_prefix.code_changes.push(eth::CodeChange {
        address: account(),
        old_hash: hash(&[]).to_vec(),
        new_hash: hash(&runtime).to_vec(),
        new_code: runtime,
        ordinal: 8000,
        ..Default::default()
    });
    bad(block(vec![actual_prefix]));
}
