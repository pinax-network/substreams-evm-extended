#![cfg(not(target_arch = "wasm32"))]
//! Frozen implementation transcripts translated to synthetic Extended proxy storage
//! frames. These check admission structure, not proxy execution/producer visibility.
use erc20_balances::{hash, layout, project};
use serde_json::{json, Value};
use substreams_ethereum::pb::eth::v2 as eth;
type Word = [u8; 32];
const ZERO: Word = [0; 32];
const TRANSCRIPTS: &str = include_str!("../docs/evidence/btr-operation-proof-20260928-transcripts.json");
const CANDIDATE: &str = include_str!("fixtures/btr-coupled-role-candidate/layouts.json");
fn n(value: u64) -> Word {
    let mut v = ZERO;
    v[24..].copy_from_slice(&value.to_be_bytes());
    v
}
fn bytes(v: &str) -> Vec<u8> {
    hex::decode(v.trim_start_matches("0x")).unwrap()
}
fn w(v: &Value) -> Word {
    bytes(v.as_str().unwrap()).try_into().unwrap()
}
fn word(v: &str) -> Word {
    bytes(v).try_into().unwrap()
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
fn config() -> Value {
    serde_json::from_str::<Value>(CANDIDATE).unwrap()[0].clone()
}
fn account() -> Vec<u8> {
    bytes(config()["contract"].as_str().unwrap())
}
fn membership() -> Word {
    n(101)
}
fn set_root() -> Word {
    n(151)
}
fn admin() -> Word {
    word(layout::BTR_PAUSER_ADMIN_SLOT)
}
fn pauser() -> Word {
    hash(b"PAUSER_ROLE")
}
fn case(name: &str) -> Value {
    serde_json::from_str::<Vec<Value>>(TRANSCRIPTS)
        .unwrap()
        .into_iter()
        .find(|v| v["name"] == name)
        .unwrap()
}
fn preimage(call: &mut eth::Call, raw: Vec<u8>) {
    call.keccak_preimages.insert(hex::encode(hash(&raw)), hex::encode(raw));
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
fn saved_subset(name: &str, subset: usize) -> eth::Call {
    let c = case(name);
    let mut call = eth::Call {
        address: account(),
        begin_ordinal: 1,
        end_ordinal: 2000,
        ..Default::default()
    };
    for k in c["execution"]["keccaks"].as_array().unwrap() {
        let raw = bytes(k["input"].as_str().unwrap());
        assert_eq!(h(hash(&raw)), k["output"]);
        preimage(&mut call, raw);
    }
    let mut equality = 0;
    for s in c["execution"]["writes"].as_array().unwrap() {
        if s["old"] == s["new"] {
            let keep = subset & (1 << equality) != 0;
            equality += 1;
            if !keep {
                continue;
            }
        }
        call.storage_changes
            .push(row(w(&s["key"]), w(&s["old"]), w(&s["new"]), s["step"].as_u64().unwrap() + 10));
    }
    call
}
fn saved(name: &str) -> eth::Call {
    saved_subset(name, usize::MAX)
}
fn block(mut calls: Vec<eth::Call>) -> eth::Block {
    for (i, call) in calls.iter_mut().enumerate() {
        let shift = i as u64 * 2000;
        call.begin_ordinal += shift;
        call.end_ordinal += shift;
        for r in &mut call.storage_changes {
            r.ordinal += shift;
        }
    }
    let balance = w(&config()["balance_slot"]);
    preimage(&mut calls[0], pair(n(44), balance));
    calls[0].storage_changes.push(row(leaf(n(44), balance), ZERO, n(9), 1900));
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
            end_ordinal: 2000 * calls.len() as u64,
            calls,
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn project_config(b: &eth::Block, c: Value) -> Result<proto::pb::evm::balances::v1::Events, substreams::errors::Error> {
    project(b, &layout::parse(&json!([c]).to_string()).unwrap())
}
fn good(b: &eth::Block) {
    let r = project_config(b, config()).unwrap();
    assert_eq!(r.balances.len(), 1);
    assert_eq!(r.balances[0].amount, "9");
}
fn bad(b: &eth::Block) {
    assert!(project_config(b, config()).is_err(), "metadata failure must refuse valid balance atomically");
}
#[test]
fn btr_actual_compiled_roles_wrappers_and_all_permitted_equalities() {
    let names = [
        "role_add_empty",
        "role_add_nonempty",
        "role_add_zero_empty",
        "role_add_zero_nonempty",
        "role_duplicate",
        "role_duplicate_zero",
        "role_remove_first",
        "role_remove_middle",
        "role_remove_tail",
        "role_remove_sole",
        "role_remove_zero_tail_swap",
        "role_remove_zero_first",
        "role_remove_zero_middle",
        "role_remove_zero_tail",
        "role_remove_zero_sole",
        "role_absent_empty",
        "role_absent_nonempty",
        "default_admin",
        "pauser",
        "minter",
        "burner",
        "arbitrary",
        "renounce_present",
        "renounce_absent",
        "renounce_zero",
        "renounce_last_owner",
        "wrapper_minter_add",
        "wrapper_minter_duplicate",
        "wrapper_minter_remove",
        "wrapper_minter_absent",
        "wrapper_burner_add",
        "wrapper_burner_duplicate",
        "wrapper_burner_remove",
        "wrapper_burner_absent",
    ];
    let mut variants = 0;
    for name in names {
        let trace = case(name);
        assert_eq!(trace["execution"]["exit"]["kind"], "return");
        let equals = trace["execution"]["writes"].as_array().unwrap().iter().filter(|s| s["old"] == s["new"]).count();
        for subset in 0..1 << equals {
            let b = block(vec![saved_subset(name, subset)]);
            let r = project_config(&b, config()).unwrap_or_else(|e| panic!("{name}/{subset}: {e}"));
            assert_eq!(r.balances[0].amount, "9");
            variants += 1;
        }
    }
    assert_eq!(variants, 40);
    good(&block((0..8).map(|i| saved(&format!("sequence_role_{i}"))).collect()));
}
#[test]
fn btr_inherited_whitelist_permissions_are_explicitly_partial() {
    let original: Value = serde_json::from_str(include_str!("fixtures/bsc-refined450-layouts.json")).unwrap();
    let original = original.as_array().unwrap().iter().find(|p| p["contract"] == config()["contract"]).unwrap();
    for name in [
        "whitelist_add_empty",
        "whitelist_add_nonempty",
        "whitelist_remove_first",
        "whitelist_remove_tail",
        "whitelist_remove_sole",
        "whitelist_remove_zero_first",
        "whitelist_remove_zero_tail_swap",
    ] {
        for subset in [0, usize::MAX] {
            let b = block(vec![saved_subset(name, subset)]);
            bad(&b);
            assert!(
                project_config(&b, original.clone()).is_err(),
                "baseline must expose same partial behavior {name}"
            );
        }
    }
    for name in ["whitelist_add_zero_empty", "whitelist_remove_zero_sole"] {
        for subset in [0, usize::MAX] {
            let b = block(vec![saved_subset(name, subset)]);
            good(&b);
            assert_eq!(project_config(&b, original.clone()).unwrap().balances[0].amount, "9");
        }
    }
    for name in ["whitelist_duplicate", "whitelist_absent_empty"] {
        good(&block(vec![saved(name)]));
    }
    bad(&block(vec![saved("role_add_empty"), saved("whitelist_add_empty")]));
    good(&block(vec![saved("role_add_empty"), saved_subset("whitelist_add_zero_empty", 0)]));
}
#[test]
fn btr_pauser_admin_is_exact_value_only_and_never_a_role_stage() {
    assert_eq!(h(pauser()), layout::BTR_PAUSER_ROLE);
    assert_eq!(admin(), plus(leaf(pauser(), membership()), n(1)));
    let initializer = case("initialize_captured_arguments");
    let expected = initializer["execution"]["writes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| w(&s["key"]) == admin())
        .unwrap();
    assert_eq!(w(&expected["old"]), ZERO);
    assert_eq!(w(&expected["new"]), pauser());
    for old in [ZERO, pauser(), n(7)] {
        for hint in [false, true] {
            let mut c = saved("role_duplicate");
            if hint {
                preimage(&mut c, pair(pauser(), membership()));
            }
            c.storage_changes.push(row(admin(), old, pauser(), 1500));
            good(&block(vec![c]));
        }
    }
    for new in [ZERO, n(1), [255; 32]] {
        let mut c = saved("role_duplicate");
        c.storage_changes.push(row(admin(), pauser(), new, 1500));
        bad(&block(vec![c]));
    }
    for hint in [false, true] {
        for wrong in [ZERO, n(1)] {
            let mut c = saved("role_duplicate");
            if hint {
                preimage(&mut c, pair(pauser(), membership()));
            }
            c.storage_changes.push(row(admin(), wrong, wrong, 1500));
            bad(&block(vec![c]));
        }
        for field in 0..3 {
            let mut c = saved("role_duplicate");
            if hint {
                preimage(&mut c, pair(pauser(), membership()));
            }
            let mut s = row(admin(), ZERO, pauser(), 1500);
            match field {
                0 => s.key.insert(0, 0),
                1 => s.old_value.insert(0, 0),
                _ => s.new_value.insert(0, 0),
            }
            c.storage_changes.push(s);
            bad(&block(vec![c]));
        }
    }
    for role in [ZERO, n(7), [255; 32]] {
        let mut c = saved("role_duplicate");
        preimage(&mut c, pair(role, membership()));
        c.storage_changes.push(row(plus(leaf(role, membership()), n(1)), ZERO, pauser(), 1500));
        bad(&block(vec![c]));
    }
    for key in [leaf(pauser(), membership()), plus(admin(), n(1))] {
        let mut c = saved("role_duplicate");
        preimage(&mut c, pair(pauser(), membership()));
        c.storage_changes.push(row(key, ZERO, ZERO, 1500));
        bad(&block(vec![c]));
    }
    let mut c = saved("role_add_empty");
    let ordinal = c.storage_changes[0].ordinal + 1;
    c.storage_changes.push(row(admin(), ZERO, pauser(), ordinal));
    bad(&block(vec![c]));
    let mut c = saved("role_add_empty");
    c.storage_changes.push(row(admin(), ZERO, pauser(), 1500));
    good(&block(vec![c]));
}
#[test]
fn btr_void_super_one_sided_source_successes_and_growth_are_not_coherent_operations() {
    for name in [
        "incoherent_set_only_grant",
        "incoherent_bool_only_grant",
        "incoherent_bool_only_revoke",
        "incoherent_set_only_revoke",
    ] {
        assert_eq!(case(name)["execution"]["exit"]["kind"], "return");
        bad(&block(vec![saved(name)]));
    }
    assert_eq!(case("incoherent_max_length_push")["execution"]["exit"]["kind"], "return");
    bad(&block(vec![saved("incoherent_max_length_push")]));
    for name in ["empty_set_prefix_revert", "index_out_of_bounds_prefix_revert", "max_index_prefix_revert"] {
        let c = saved(name);
        bad(&block(vec![c.clone()]));
        let mut reverted = c;
        reverted.state_reverted = true;
        good(&block(vec![saved("role_duplicate"), reverted]));
    }
}
#[test]
fn btr_missing_extra_dirty_or_wrong_order_stages_refuse_atomically() {
    for name in [
        "role_add_empty",
        "role_add_zero_empty",
        "role_remove_first",
        "role_remove_tail",
        "role_remove_zero_sole",
        "role_remove_zero_tail_swap",
    ] {
        let original = saved(name);
        for (i, s) in original.storage_changes.iter().enumerate() {
            if s.old_value != s.new_value {
                let mut c = original.clone();
                c.storage_changes.remove(i);
                bad(&block(vec![c]));
            }
            for old in [false, true] {
                let mut c = original.clone();
                if old {
                    c.storage_changes[i].old_value = n(99).to_vec();
                } else {
                    c.storage_changes[i].new_value = n(99).to_vec();
                }
                bad(&block(vec![c]));
            }
        }
        for i in [0, original.storage_changes.len() - 1] {
            let mut c = original.clone();
            let mut extra = c.storage_changes[i].clone();
            extra.new_value = extra.old_value.clone();
            extra.ordinal += 1;
            c.storage_changes.push(extra);
            bad(&block(vec![c]));
        }
        let mut c = original.clone();
        c.storage_changes.truncate(1);
        bad(&block(vec![c]));
        let mut c = original.clone();
        c.storage_changes.remove(0);
        bad(&block(vec![c]));
        let mut c = original.clone();
        let last = c.storage_changes.len() - 1;
        let ordinal = c.storage_changes[0].ordinal;
        c.storage_changes[0].ordinal = c.storage_changes[last].ordinal;
        c.storage_changes[last].ordinal = ordinal;
        bad(&block(vec![c]));
        for i in 0..original.storage_changes.len() {
            for field in 0..3 {
                let mut c = original.clone();
                let s = &mut c.storage_changes[i];
                match field {
                    0 => s.key.insert(0, 0),
                    1 => s.old_value.insert(0, 0),
                    _ => s.new_value.insert(0, 0),
                }
                bad(&block(vec![c]));
            }
        }
    }
}
#[test]
fn btr_schema_reserves_both_roots_and_requires_exact_admin_scalar() {
    let parse = |c: Value| layout::parse(&json!([c]).to_string());
    assert!(parse(config()).is_ok());
    for field in ["root", "membership_root"] {
        for value in [json!(null), json!(h(n(5))), json!(h(n(6))), json!("0x65")] {
            let mut c = config();
            c["enumerable_address_sets"][0][field] = value;
            assert!(parse(c).is_err());
        }
    }
    let mut c = config();
    c["enumerable_address_sets"][0].as_object_mut().unwrap().remove("membership_root");
    assert!(parse(c).is_err());
    let mut c = config();
    c["other_slots"].as_array_mut().unwrap().retain(|v| *v != json!(h(admin())));
    assert!(parse(c).is_err());
    for root in [membership(), set_root()] {
        for field in ["other_slots", "other_mapping_slots", "address_lists"] {
            let mut c = config();
            let v = c.as_object_mut().unwrap().entry(field).or_insert(json!([]));
            v.as_array_mut().unwrap().push(json!(h(root)));
            assert!(parse(c).is_err());
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
    for mode in [layout::PTOKEN_ENUMERABLE_SEMANTICS, layout::SECURITIES_ENUMERABLE_SEMANTICS, "oz_3_4_2"] {
        let mut c = config();
        c["enumerable_address_sets"][0]["semantics"] = json!(mode);
        assert!(parse(c).is_err());
    }
    let mut c = config();
    c["enumerable_address_sets"][0] = json!({"root":h(n(8)),"key_types":["bytes32"],"semantics":"oz_3_4_2","membership_root":null});
    assert!(parse(c).is_err());
}
#[test]
fn btr_preimages_and_both_root_discovery_cannot_be_bypassed() {
    let original = saved("role_add_empty");
    let role: Word = bytes(case("role_add_empty")["calldata"].as_str().unwrap())[4..36].try_into().unwrap();
    for bool_side in [false, true] {
        let mut c = original.clone();
        if bool_side {
            c.storage_changes.truncate(1);
        } else {
            c.storage_changes.remove(0);
        }
        let root = if bool_side { set_root() } else { membership() };
        c.keccak_preimages.retain(|_, v| {
            let raw = bytes(v);
            raw.len() != 64 || raw[32..] != root
        });
        bad(&block(vec![c.clone()]));
        for s in &mut c.storage_changes {
            s.new_value = s.old_value.clone();
        }
        bad(&block(vec![c]));
    }
    for (key, value) in &original.keccak_preimages {
        let raw = bytes(value);
        if raw.len() != 64 {
            continue;
        }
        let outer = raw[..32] == role && (raw[32..] == membership() || raw[32..] == set_root());
        if !outer && !original.storage_changes.iter().any(|s| hex::encode(&s.key) == *key) {
            continue;
        }
        let mut c = original.clone();
        c.keccak_preimages.remove(key);
        bad(&block(vec![c]));
        let mut c = original.clone();
        c.keccak_preimages.insert(key.clone(), "ff".repeat(64));
        bad(&block(vec![c]));
    }
    let mut c = original;
    c.storage_changes[0].new_value[0] = 1;
    bad(&block(vec![c]));
}
#[test]
fn btr_versions_frames_transactions_roles_and_accounts_are_distinct() {
    for version in [4, 5] {
        let mut b = block(vec![saved("role_add_empty")]);
        b.ver = version;
        good(&b);
    }
    for name in ["role_add_empty", "role_duplicate"] {
        let mut b = block(vec![saved(name)]);
        b.ver = 3;
        bad(&b);
        let mut b = block(vec![saved(name)]);
        b.transaction_traces[0].calls[0].begin_ordinal = 0;
        bad(&b);
    }
    let original = saved("role_add_empty");
    let mut left = original.clone();
    let mut right = original.clone();
    right.storage_changes = left.storage_changes.split_off(1);
    bad(&block(vec![left.clone(), right.clone()]));
    let mut b = block(vec![left, right]);
    let call = b.transaction_traces[0].calls.pop().unwrap();
    b.transaction_traces[0].end_ordinal = 2000;
    b.transaction_traces.push(eth::TransactionTrace {
        status: eth::TransactionTraceStatus::Succeeded as i32,
        begin_ordinal: 2001,
        end_ordinal: 4000,
        calls: vec![call],
        ..Default::default()
    });
    bad(&b);
    let mut mixed = original.clone();
    let other_role = saved("arbitrary");
    mixed.keccak_preimages.extend(other_role.keccak_preimages.clone());
    let ordinal = mixed.storage_changes[0].ordinal;
    mixed.storage_changes[0] = other_role.storage_changes[0].clone();
    mixed.storage_changes[0].ordinal = ordinal;
    bad(&block(vec![mixed]));
    let other = vec![8; 20];
    let mut other_config = config();
    other_config["contract"] = json!(format!("0x{}", hex::encode(&other)));
    let layouts = layout::parse(&json!([config(), other_config]).to_string()).unwrap();
    let mut split = original.clone();
    for r in split.storage_changes.iter_mut().skip(1) {
        r.address = other.clone();
    }
    assert!(project(&block(vec![split]), &layouts).is_err());
    let mut independent = saved("role_remove_sole");
    independent.address = other.clone();
    for r in &mut independent.storage_changes {
        r.address = other.clone();
    }
    assert_eq!(
        project(&block(vec![saved_subset("role_add_zero_empty", 0), independent]), &layouts)
            .unwrap()
            .balances
            .len(),
        1
    );
    let mut c = original;
    c.storage_changes[1].ordinal = c.storage_changes[0].ordinal;
    bad(&block(vec![c]));
}
#[test]
fn btr_foreign_and_reverted_boundaries_remain_barriers_even_for_omitted_stages() {
    let call = saved_subset("role_add_zero_empty", 0);
    let omitted = case("role_add_zero_empty")["execution"]["writes"][2]["step"].as_u64().unwrap() + 10;
    for ordinal in [call.storage_changes[0].ordinal, omitted, call.storage_changes.last().unwrap().ordinal] {
        let mut b = block(vec![call.clone()]);
        let mut foreign = row(n(70), ZERO, ZERO, ordinal);
        foreign.address = vec![8; 20];
        b.transaction_traces[0].calls[0].storage_changes.push(foreign);
        bad(&b);
        let mut b = block(vec![call.clone()]);
        b.transaction_traces[0].calls.push(eth::Call {
            address: vec![8; 20],
            index: 1,
            parent_index: 0,
            depth: 1,
            begin_ordinal: ordinal,
            end_ordinal: ordinal + 1,
            state_reverted: true,
            ..Default::default()
        });
        bad(&b);
    }
    let mut b = block(vec![call]);
    let mut foreign = row(n(70), ZERO, ZERO, 1500);
    foreign.address = vec![8; 20];
    b.transaction_traces[0].calls[0].storage_changes.push(foreign);
    good(&b);
}
fn synthetic_zero_add(role: Word, target: Word, keep: bool) -> eth::Call {
    let head = leaf(role, set_root());
    let base = leaf(role, membership());
    let length = minus(target, hash(&head));
    assert_ne!(length, [255; 32]);
    let next = plus(length, n(1));
    let mut c = eth::Call {
        address: account(),
        begin_ordinal: 1,
        end_ordinal: 2000,
        ..Default::default()
    };
    for raw in [pair(role, membership()), pair(role, set_root()), pair(ZERO, base), pair(ZERO, plus(head, n(1)))] {
        preimage(&mut c, raw);
    }
    c.storage_changes = vec![row(leaf(ZERO, base), ZERO, n(1), 100), row(head, length, next, 200)];
    if keep {
        c.storage_changes.push(row(target, ZERO, ZERO, 300));
    }
    c.storage_changes.push(row(leaf(ZERO, plus(head, n(1))), ZERO, next, 400));
    c
}
#[test]
fn btr_optional_zero_stages_keep_continuity_and_protected_admin_aliases() {
    for subset in [0, usize::MAX] {
        good(&block(vec![
            saved_subset("role_add_zero_empty", subset),
            saved_subset("role_remove_zero_sole", subset),
        ]));
        good(&block(vec![saved("role_add_empty"), saved("role_remove_sole")]));
        let b = block(vec![saved_subset("role_add_zero_empty", subset), saved("role_remove_sole")]);
        let error = project_config(&b, config()).unwrap_err().to_string();
        assert!(error.contains("discontinuous"), "{error}");
    }
    bad(&block(vec![saved("role_add_empty"), saved("role_add_empty")]));
    for keep in [false, true] {
        bad(&block(vec![synthetic_zero_add(n(12345), admin(), keep)]));
    }
}
#[test]
fn btr_moved_tail_membership_is_derived_constraint_not_permission_or_required_hint() {
    let c = saved("role_remove_first");
    let role: Word = bytes(case("role_remove_first")["calldata"].as_str().unwrap())[4..36].try_into().unwrap();
    let moved = n(5);
    let moved_bool = leaf(moved, leaf(role, membership()));
    assert!(!c.keccak_preimages.contains_key(&hex::encode(moved_bool)));
    good(&block(vec![c.clone()]));
    for hint in [false, true] {
        let mut extra = c.clone();
        extra.storage_changes.push(row(moved_bool, n(1), n(1), 1500));
        if hint {
            preimage(&mut extra, pair(moved, leaf(role, membership())));
        }
        bad(&block(vec![extra]));
    }
    let mut protected = config();
    protected["other_slots"].as_array_mut().unwrap().push(json!(h(moved_bool)));
    let error = project_config(&block(vec![c.clone()]), protected).unwrap_err().to_string();
    assert!(error.contains("coherence key aliases protected"), "{error}");
    for keep in [false, true] {
        let add = synthetic_zero_add(n(12345), moved_bool, keep);
        good(&block(vec![add.clone()]));
        for calls in [vec![add.clone(), c.clone()], vec![c.clone(), add.clone()]] {
            let error = project_config(&block(calls), config()).unwrap_err().to_string();
            assert!(error.contains("alias"), "{error}");
        }
    }
}
#[test]
fn btr_proxy_runtime_pointer_metadata_and_creation_guards_are_preserved() {
    let mut c = saved("role_add_empty");
    c.storage_changes.push(row(n(554), n(5), n(4), 1500));
    let allowance = leaf(n(44), n(202));
    preimage(&mut c, pair(n(44), n(202)));
    preimage(&mut c, pair(n(45), allowance));
    c.storage_changes.push(row(leaf(n(45), allowance), n(1), n(2), 1550));
    good(&block(vec![c]));
    let cfg = config();
    let implementation = bytes(cfg["proxy"]["implementation"].as_str().unwrap());
    for address in [account(), implementation] {
        let mut b = block(vec![saved("role_add_empty")]);
        b.transaction_traces[0].calls[0].code_changes.push(eth::CodeChange {
            address,
            old_hash: vec![1; 32],
            new_hash: hash(&[1]).to_vec(),
            new_code: vec![1],
            ordinal: 1500,
            ..Default::default()
        });
        bad(&b);
    }
    let mut b = block(vec![saved("role_add_empty")]);
    let slot = w(&cfg["proxy"]["implementation_slot"]);
    b.transaction_traces[0].calls[0]
        .storage_changes
        .extend([row(slot, n(8), n(9), 1500), row(slot, n(9), n(8), 1550)]);
    bad(&b);
    let capture: Value = serde_json::from_str(include_str!("fixtures/btr-operation-proof/proxy-capture.json")).unwrap();
    let runtime = bytes(capture["runtimeBytecode"]["onchainBytecode"].as_str().unwrap());
    assert_eq!(h(hash(&runtime)), cfg["code_hash"]);
    let mut b = block(vec![saved("role_add_empty")]);
    b.transaction_traces[0].calls[0].call_type = eth::CallType::Create as i32;
    b.transaction_traces[0].calls[0].code_changes.push(eth::CodeChange {
        address: account(),
        old_hash: hash(&[]).to_vec(),
        new_hash: hash(&runtime).to_vec(),
        new_code: runtime,
        ordinal: 1500,
        ..Default::default()
    });
    bad(&b);
}
