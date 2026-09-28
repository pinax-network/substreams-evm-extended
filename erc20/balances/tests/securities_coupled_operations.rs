#![cfg(not(target_arch = "wasm32"))]
//! Frozen implementation transcripts translated to synthetic Extended proxy storage
//! frames. These check admission structure, not proxy execution/producer visibility.
use erc20_balances::{hash, layout, project};
use serde_json::{json, Value};
use substreams_ethereum::pb::eth::v2 as eth;
type Word = [u8; 32];
const ZERO: Word = [0; 32];
const TRANSCRIPTS: &str = include_str!("../docs/evidence/securities-operation-proof-20260928-transcripts.json");
const CANDIDATE: &str = include_str!("fixtures/securities-coupled-role-candidate/layouts.json");
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
    word(layout::SECURITIES_MEMBERSHIP_ROOT)
}
fn set_root() -> Word {
    word(layout::SECURITIES_SET_ROOT)
}
fn admin() -> Word {
    word(layout::SECURITIES_ISSUER_ADMIN_SLOT)
}
fn issuer() -> Word {
    hash(b"ISSUER_ROLE")
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
fn securities_actual_compiled_operations_and_every_permitted_equality_subset() {
    let names = [
        "grant_empty",
        "grant_nonempty",
        "grant_zero_empty",
        "grant_zero_nonempty",
        "grant_duplicate",
        "grant_duplicate_zero",
        "revoke_first",
        "revoke_middle",
        "revoke_tail",
        "revoke_sole",
        "revoke_zero_tail_swap",
        "revoke_zero_first",
        "revoke_zero_middle",
        "revoke_zero_tail",
        "revoke_zero_sole",
        "revoke_absent_empty",
        "revoke_absent",
        "default_admin",
        "issuer_role",
        "full_width_role",
        "renounce_present",
        "renounce_absent",
        "renounce_zero",
        "full_width_admin",
        "full_width_admin_revoke",
        "self_admin",
        "self_admin_revoke",
        "high_bit_admin",
        "high_bit_admin_revoke",
    ];
    let mut checked = 0;
    for name in names {
        let c = case(name);
        let equal = c["execution"]["writes"].as_array().unwrap().iter().filter(|s| s["old"] == s["new"]).count();
        for subset in 0..1 << equal {
            let b = block(vec![saved_subset(name, subset)]);
            let r = project_config(&b, config()).unwrap_or_else(|e| panic!("{name}/{subset}: {e}"));
            assert_eq!(r.balances[0].amount, "9");
            checked += 1;
        }
    }
    assert_eq!(checked, 35);
    // Each sequence's two roles are independently coherent, then repeated across calls.
    let calls = (0..8).flat_map(|i| (0..2).map(move |r| saved(&format!("sequence_{i}_{r}")))).collect();
    good(&block(calls));
}
#[test]
fn securities_missing_corrupt_extra_and_reordered_stages_refuse() {
    for name in [
        "grant_empty",
        "grant_zero_empty",
        "revoke_first",
        "revoke_tail",
        "revoke_zero_sole",
        "revoke_zero_tail_swap",
    ] {
        let original = saved(name);
        for (i, s) in original.storage_changes.iter().enumerate() {
            if s.old_value != s.new_value {
                let mut c = original.clone();
                c.storage_changes.remove(i);
                bad(&block(vec![c]));
            }
            for old in [true, false] {
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
        let mut c = original;
        let last = c.storage_changes.len() - 1;
        let ordinal = c.storage_changes[last].ordinal;
        c.storage_changes[last].ordinal = c.storage_changes[0].ordinal;
        c.storage_changes[0].ordinal = ordinal;
        bad(&block(vec![c]));
    }
}
#[test]
fn securities_fixed_issuer_admin_is_explicit_zero_only_and_a_barrier() {
    assert_eq!(admin(), plus(leaf(issuer(), membership()), n(1)));
    for old in [ZERO, n(1), [255; 32]] {
        for include_preimage in [false, true] {
            let mut c = saved("grant_duplicate");
            c.storage_changes.push(row(admin(), old, ZERO, 1000));
            if include_preimage {
                preimage(&mut c, pair(issuer(), membership()));
            }
            good(&block(vec![c]));
        }
    }
    for (old, new) in [(ZERO, n(1)), (n(1), n(1)), (ZERO, [255; 32])] {
        let mut c = saved("grant_duplicate");
        c.storage_changes.push(row(admin(), old, new, 1000));
        bad(&block(vec![c]));
    }
    for role in [ZERO, n(6), [255; 32]] {
        let mut c = saved("grant_duplicate");
        preimage(&mut c, pair(role, membership()));
        c.storage_changes.push(row(plus(leaf(role, membership()), n(1)), ZERO, ZERO, 1000));
        bad(&block(vec![c]));
    }
    let mut c = saved("grant_empty");
    let ordinal = c.storage_changes[0].ordinal + 1;
    c.storage_changes.push(row(admin(), n(5), ZERO, ordinal));
    bad(&block(vec![c]));
    let mut c = saved("grant_empty");
    c.storage_changes.push(row(admin(), n(5), ZERO, 1500));
    good(&block(vec![c]));
}
#[test]
fn securities_schema_fixed_roots_scalar_and_legacy_semantics_remain_distinct() {
    let parse = |c: Value| layout::parse(&json!([c]).to_string());
    assert!(parse(config()).is_ok());
    for field in ["membership_root", "root"] {
        for value in [json!(null), json!(h(n(5))), json!(h(n(6))), json!("0x01")] {
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
    for semantics in ["oz_3_4_2", layout::PTOKEN_ENUMERABLE_SEMANTICS] {
        let mut c = config();
        c["enumerable_address_sets"][0]["semantics"] = json!(semantics);
        assert!(parse(c).is_err());
    }
    let mut c = config();
    c["enumerable_address_sets"][0] = json!({"root":h(n(8)),"semantics":"oz_3_4_2","key_types":["bytes32"],"membership_root":null});
    assert!(parse(c).is_err());
}
#[test]
fn securities_exact_preimages_boolean_and_set_union_and_dirty_words() {
    let original = saved("grant_empty");
    for bool_side in [true, false] {
        let mut c = original.clone();
        if bool_side {
            c.storage_changes.truncate(1);
        } else {
            c.storage_changes.remove(0);
        }
        let remove = if bool_side { set_root() } else { membership() };
        c.keccak_preimages.retain(|_, v| {
            let raw = bytes(v);
            raw.len() != 64 || raw[32..] != remove
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
        let parent: Word = raw[32..].try_into().unwrap();
        let role = bytes(case("grant_empty")["calldata"].as_str().unwrap())[4..36].to_vec();
        let operation_outer = (parent == membership() || parent == set_root()) && raw[..32] == role;
        if !operation_outer && !original.storage_changes.iter().any(|r| hex::encode(&r.key) == *key) {
            continue;
        }
        let mut c = original.clone();
        c.keccak_preimages.remove(key);
        bad(&block(vec![c]));
        let mut c = original.clone();
        c.keccak_preimages.insert(key.clone(), "ff".repeat(64));
        bad(&block(vec![c]));
    }
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
    let mut c = original;
    let s = &mut c.storage_changes[0];
    s.new_value[0] = 1;
    bad(&block(vec![c]));
}
#[test]
fn securities_versions_real_frames_and_reverted_prefixes() {
    for ver in [4, 5] {
        let mut b = block(vec![saved("grant_empty")]);
        b.ver = ver;
        good(&b);
    }
    for name in ["grant_empty", "grant_duplicate"] {
        let mut b = block(vec![saved(name)]);
        b.ver = 3;
        bad(&b);
        let mut b = block(vec![saved(name)]);
        b.transaction_traces[0].calls[0].begin_ordinal = 0;
        bad(&b);
    }
    for name in ["empty_array_prefix_revert", "out_of_bounds_prefix_revert", "max_index_prefix_revert"] {
        let mut c = saved(name);
        c.state_reverted = true;
        let mut b = block(vec![saved("grant_duplicate"), c]);
        good(&b);
        b.transaction_traces[0].calls[1].state_reverted = false;
        bad(&b);
    }
    let mut c = saved("grant_empty");
    let mut other = c.clone();
    other.storage_changes = c.storage_changes.split_off(1);
    let b = block(vec![c, other]);
    bad(&b);
    let mut b = block(vec![saved("grant_empty")]);
    let rows = &b.transaction_traces[0].calls[0].storage_changes;
    let ordinal = rows[0].ordinal + 1;
    b.transaction_traces[0].calls.push(eth::Call {
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
#[test]
fn securities_journal_aliases_and_conservative_growth() {
    good(&block(vec![saved("sequence_0_0"), saved("sequence_1_0")]));
    bad(&block(vec![saved("grant_empty"), saved("grant_empty")]));
    // Exact compiled source wraps length at MAX. This admission domain refuses it.
    bad(&block(vec![saved("max_length_push_outside_domain")]));
    // A zero-element stage colliding with the fixed admin scalar stays protected
    // whether the producer retains or omits its equal SSTORE.
    for keep in [false, true] {
        let role = n(9);
        let head = leaf(role, set_root());
        let base = leaf(role, membership());
        let length = minus(admin(), hash(&head));
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
            c.storage_changes.push(row(admin(), ZERO, ZERO, 300));
        }
        c.storage_changes.push(row(leaf(ZERO, plus(head, n(1))), ZERO, next, 400));
        bad(&block(vec![c]));
    }
}

#[test]
fn securities_role_account_and_foreign_barriers_cannot_join_fragments() {
    let first = saved("grant_empty");
    let second = saved("full_width_role");
    let mut mixed = first.clone();
    mixed.storage_changes.truncate(1);
    mixed.storage_changes.extend(second.storage_changes.iter().skip(1).cloned());
    mixed.keccak_preimages.extend(second.keccak_preimages.clone());
    bad(&block(vec![mixed]));
    let profiles: Value = serde_json::from_str(CANDIDATE).unwrap();
    let other = bytes(profiles[1]["contract"].as_str().unwrap());
    let mut split = first.clone();
    for s in split.storage_changes.iter_mut().skip(1) {
        s.address = other.clone();
    }
    let b = block(vec![split]);
    assert!(project(&b, &layout::parse(CANDIDATE).unwrap()).is_err());
    let mut independent = saved("revoke_sole");
    independent.address = other.clone();
    for s in &mut independent.storage_changes {
        s.address = other.clone();
    }
    let b = block(vec![saved_subset("grant_zero_empty", 0), independent]);
    assert_eq!(project(&b, &layout::parse(CANDIDATE).unwrap()).unwrap().balances.len(), 1);
    let c = saved_subset("grant_zero_empty", 0);
    let omitted = case("grant_zero_empty")["execution"]["writes"][2]["step"].as_u64().unwrap() + 10;
    for ordinal in [c.storage_changes[0].ordinal, omitted, c.storage_changes.last().unwrap().ordinal] {
        let mut b = block(vec![c.clone()]);
        let mut foreign = row(n(77), ZERO, ZERO, ordinal);
        foreign.address = vec![8; 20];
        b.transaction_traces[0].calls[0].storage_changes.push(foreign);
        bad(&b);
    }
    let mut b = block(vec![c]);
    let mut foreign = row(n(77), ZERO, ZERO, 1500);
    foreign.address = vec![8; 20];
    b.transaction_traces[0].calls[0].storage_changes.push(foreign);
    good(&b);
    let mut c = first;
    c.storage_changes[1].ordinal = c.storage_changes[0].ordinal;
    bad(&block(vec![c]));
}
#[test]
fn securities_preserves_raw_balance_metadata_proxy_beacon_and_creation_guards() {
    // Ordinary reviewed scalar and allowance permissions survive outside role stages.
    let mut c = saved("grant_empty");
    c.storage_changes.push(row(n(50), n(1), n(2), 1500));
    let allowance = w(&config()["other_mapping_slots"][0]);
    let holder = leaf(n(44), allowance);
    preimage(&mut c, pair(n(44), allowance));
    preimage(&mut c, pair(n(45), holder));
    c.storage_changes.push(row(leaf(n(45), holder), n(1), n(2), 1550));
    good(&block(vec![c]));
    // A long-string payload hash remains unknown; allowing the declared scalar
    // does not expand the dynamic payload's namespace.
    let mut c = saved("grant_empty");
    c.storage_changes.push(row(hash(&plus(w(&config()["balance_slot"]), n(3))), ZERO, n(1), 1500));
    bad(&block(vec![c]));
    let cfg = config();
    let beacon = bytes(cfg["beacon_proxy"]["beacon"].as_str().unwrap());
    let implementation = bytes(cfg["beacon_proxy"]["implementation"].as_str().unwrap());
    for address in [account(), beacon.clone(), implementation] {
        let mut b = block(vec![saved("grant_empty")]);
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
    for (address, slot) in [
        (account(), w(&cfg["beacon_proxy"]["beacon_slot"])),
        (beacon, w(&cfg["beacon_proxy"]["implementation_slot"])),
    ] {
        let mut b = block(vec![saved("grant_empty")]);
        for (old, new, ordinal) in [(n(8), n(9), 1500), (n(9), n(8), 1550)] {
            let mut s = row(slot, old, new, ordinal);
            s.address = address.clone();
            b.transaction_traces[0].calls[0].storage_changes.push(s);
        }
        bad(&b);
    }
    let mut b = block(vec![saved("grant_empty")]);
    b.transaction_traces[0].calls[0].call_type = eth::CallType::Create as i32;
    b.transaction_traces[0].calls[0].code_changes.push(eth::CodeChange {
        address: account(),
        old_hash: hash(&[]).to_vec(),
        new_hash: hash(&[1]).to_vec(),
        new_code: vec![1],
        ordinal: 1500,
        ..Default::default()
    });
    bad(&b);
}
#[test]
fn securities_source_malformed_outcomes_do_not_expand_the_coherent_domain() {
    for name in [
        "malformed_false_index_grant",
        "malformed_true_no_index_revoke",
        "dirty_low_bool_revoke",
        "dirty_high_bool_revoke",
    ] {
        bad(&block(vec![saved(name)]));
    }
    // No persisted facts are fabricated from source no-ops; initial consistency
    // and authorization remain qualification inputs, not inferred observations.
    for name in ["malformed_true_no_index_grant", "malformed_false_index_revoke"] {
        assert!(saved(name).storage_changes.is_empty());
        good(&block(vec![saved(name)]));
    }
    // Explicit observed contradictions to omitted zero-array stages are retained
    // across frames, so a later coherent operation cannot silently reset them.
    let mut first = saved_subset("grant_zero_empty", 0);
    let second = saved("revoke_zero_sole");
    let element = w(&case("grant_zero_empty")["execution"]["writes"][2]["key"]);
    // Replacing the zero with a nonzero member without the companion bool/index
    // update is rejected even though that first zero SSTORE was omitted.
    first.storage_changes.push(row(element, ZERO, n(7), 1500));
    bad(&block(vec![first, second]));
}

#[test]
fn securities_omitted_element_has_exact_block_local_continuity() {
    for subset in [0, usize::MAX] {
        good(&block(vec![saved_subset("grant_zero_empty", subset), saved_subset("revoke_zero_sole", subset)]));
        good(&block(vec![saved("grant_empty"), saved("revoke_sole")]));
        let b = block(vec![saved_subset("grant_zero_empty", subset), saved("revoke_sole")]);
        let error = project_config(&b, config()).unwrap_err().to_string();
        assert!(
            error.contains("discontinuous"),
            "inferred zero must constrain later complete operation: {error}"
        );
    }
}

#[test]
fn securities_moved_tail_membership_is_protected_constraint_without_store_or_hint() {
    let c = saved("revoke_first");
    let role: Word = bytes(case("revoke_first")["calldata"].as_str().unwrap())[4..36].try_into().unwrap();
    let moved = n(5);
    let moved_bool = leaf(moved, leaf(role, membership()));
    assert!(!c.keccak_preimages.contains_key(&hex::encode(moved_bool)));
    good(&block(vec![c.clone()]));
    for hinted in [false, true] {
        let mut changed = c.clone();
        changed.storage_changes.push(row(moved_bool, n(1), n(1), 1500));
        if hinted {
            preimage(&mut changed, pair(moved, leaf(role, membership())));
        }
        bad(&block(vec![changed]));
    }
    let mut protected = config();
    protected["other_slots"].as_array_mut().unwrap().push(json!(h(moved_bool)));
    let error = project_config(&block(vec![c.clone()]), protected).unwrap_err().to_string();
    assert!(error.contains("coherence key aliases protected"), "{error}");
    // Synthetic mathematical collision, not a source-executed huge set. The
    // moved-tail constraint remains protected across roles and operation order.
    for keep in [false, true] {
        let r = n(12345);
        let head = leaf(r, set_root());
        let base = leaf(r, membership());
        let length = minus(moved_bool, hash(&head));
        let next = plus(length, n(1));
        let mut add = eth::Call {
            address: account(),
            begin_ordinal: 1,
            end_ordinal: 2000,
            ..Default::default()
        };
        for raw in [pair(r, membership()), pair(r, set_root()), pair(ZERO, base), pair(ZERO, plus(head, n(1)))] {
            preimage(&mut add, raw);
        }
        add.storage_changes = vec![row(leaf(ZERO, base), ZERO, n(1), 100), row(head, length, next, 200)];
        if keep {
            add.storage_changes.push(row(moved_bool, ZERO, ZERO, 300));
        }
        add.storage_changes.push(row(leaf(ZERO, plus(head, n(1))), ZERO, next, 400));
        good(&block(vec![add.clone()]));
        for calls in [vec![add.clone(), c.clone()], vec![c.clone(), add.clone()]] {
            let error = project_config(&block(calls), config()).unwrap_err().to_string();
            assert!(error.contains("alias"), "{error}");
        }
    }
}
