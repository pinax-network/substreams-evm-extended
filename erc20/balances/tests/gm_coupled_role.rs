#![cfg(not(target_arch = "wasm32"))]
//! Frozen runtime traces translated into synthetic Extended records, not producer qualification.
use erc20_balances::{hash, layout, project};
use serde_json::{json, Value};
use substreams_ethereum::pb::eth::v2 as eth;
type Word = [u8; 32];
const ZERO: Word = [0; 32];
fn n(v: u64) -> Word {
    let mut b = ZERO;
    b[24..].copy_from_slice(&v.to_be_bytes());
    b
}
fn bytes(v: &str) -> Vec<u8> {
    hex::decode(v.trim_start_matches("0x")).unwrap()
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
    let mut out = ZERO;
    let mut carry = 0u16;
    for i in (0..32).rev() {
        carry += u16::from(a[i]) + u16::from(b[i]);
        out[i] = carry as u8;
        carry >>= 8;
    }
    out
}
fn minus(a: Word, b: Word) -> Word {
    plus(plus(a, b.map(|v| !v)), n(1))
}
fn configs() -> Vec<Value> {
    serde_json::from_str(include_str!("fixtures/gm-coupled-role-candidate/layouts.json")).unwrap()
}
fn address(profile: usize) -> Vec<u8> {
    bytes(configs()[profile]["contract"].as_str().unwrap())
}
fn cases() -> Vec<Value> {
    serde_json::from_str::<Vec<Value>>(include_str!("../docs/evidence/gm-operation-proof-20260928-transcripts.json"))
        .unwrap()
        .into_iter()
        .filter(|v| v["runtime_variant"] == "captured")
        .collect()
}
fn case(name: &str) -> Value {
    cases().into_iter().find(|v| v["name"] == name).unwrap_or_else(|| panic!("missing {name}"))
}
fn role(name: &str) -> Word {
    bytes(case(name)["calldata"].as_str().unwrap())[4..36].try_into().unwrap()
}
fn image(c: &mut eth::Call, p: Vec<u8>) {
    c.keccak_preimages.insert(hex::encode(hash(&p)), hex::encode(p));
}
fn row(profile: usize, key: Word, old: Word, new: Word, ordinal: u64) -> eth::StorageChange {
    eth::StorageChange {
        address: address(profile),
        key: key.to_vec(),
        old_value: old.to_vec(),
        new_value: new.to_vec(),
        ordinal,
    }
}
fn saved(v: &Value, profile: usize, subset: usize) -> eth::Call {
    let mut c = eth::Call {
        address: address(profile),
        begin_ordinal: 1,
        end_ordinal: 10000,
        ..Default::default()
    };
    for p in v["execution"]["keccaks"].as_array().unwrap() {
        let raw = bytes(p["input"].as_str().unwrap());
        assert_eq!(h(hash(&raw)), p["output"]);
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
        c.storage_changes
            .push(row(profile, w(&s["key"]), w(&s["old"]), w(&s["new"]), s["step"].as_u64().unwrap() + 10));
    }
    c
}
fn block(mut calls: Vec<eth::Call>) -> eth::Block {
    let end = 10000 * calls.len() as u64;
    for (i, c) in calls.iter_mut().enumerate() {
        let shift = 10000 * i as u64;
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
    let c = first(b);
    image(c, pair(n(44), n(51)));
    c.storage_changes.push(row(0, leaf(n(44), n(51)), ZERO, n(9), 9000));
}
fn project_with(b: &eth::Block, configuration: Vec<Value>) -> Result<usize, String> {
    project(b, &layout::parse(&json!(configuration).to_string()).unwrap())
        .map(|r| r.balances.len())
        .map_err(|e| e.to_string())
}
fn good(mut b: eth::Block) {
    assert_eq!(project_with(&b, configs()).unwrap(), 0);
    add_balance(&mut b);
    assert_eq!(project_with(&b, configs()).unwrap(), 1);
}
fn bad(mut b: eth::Block) {
    add_balance(&mut b);
    assert!(project_with(&b, configs()).is_err(), "bad role effects admitted with valid balance");
}
fn saved_block(name: &str) -> eth::Block {
    block(vec![saved(&case(name), 0, usize::MAX)])
}
#[test]
fn gm_frozen_coherent_operations_and_every_equality_subset_both_profiles() {
    let mut operations = 0;
    let mut subsets = 0;
    for v in cases().into_iter().filter(|v| {
        v["execution"]["exit"]["kind"] == "return"
            && v["signature"].as_str().is_some_and(|s| {
                matches!(
                    s,
                    "grantRole(bytes32,address)" | "revokeRole(bytes32,address)" | "renounceRole(bytes32,address)"
                )
            })
            && {
                let name = v["name"].as_str().unwrap();
                name.starts_with("role_")
                    || name.starts_with("sequence_")
                    || name.starts_with("renounce_")
                    || matches!(
                        name,
                        "default_admin" | "minter" | "burner" | "configurer" | "arbitrary" | "max_role" | "custom_admin_correct"
                    )
            }
    }) {
        operations += 1;
        let equality = v["execution"]["writes"].as_array().unwrap().iter().filter(|s| s["old"] == s["new"]).count();
        for p in 0..2 {
            for subset in 0..1 << equality {
                let b = block(vec![saved(&v, p, subset)]);
                for version in [4, 5] {
                    let mut b = b.clone();
                    b.ver = version;
                    good(b);
                }
                subsets += 1;
            }
        }
    }
    assert_eq!(operations, 40);
    assert!(subsets > 80);
}
#[test]
fn gm_one_sided_and_malformed_source_successes_never_supply_permissions() {
    for name in [
        "incoherent_set_only_grant",
        "incoherent_bool_only_grant",
        "incoherent_bool_only_revoke",
        "incoherent_set_only_revoke",
        "empty_set_prefix_revert",
        "index_out_of_bounds_prefix_revert",
        "max_index_prefix_revert",
        "incorrect_array_member",
        "duplicate_array_entries",
        "moved_tail_wrong_index",
        "dirty_bool_two",
        "dirty_bool_high_bits",
        "dirty_bool_zero_byte_grant",
        "incoherent_max_length_push",
    ] {
        bad(saved_block(name));
    }
    // A source no-op emits no facts; admission cannot infer its hidden malformed prestate.
    good(saved_block("dirty_bool_duplicate"));
}
#[test]
fn gm_missing_corrupt_ordered_stages_and_exact_preimages_fail_atomically() {
    for name in [
        "role_add_empty",
        "role_add_zero_empty",
        "role_remove_first",
        "role_remove_tail",
        "role_remove_zero_sole",
    ] {
        let original = saved_block(name);
        let changes = &original.transaction_traces[0].calls[0].storage_changes;
        for (i, s) in changes.iter().enumerate() {
            if s.old_value != s.new_value {
                let mut b = original.clone();
                first(&mut b).storage_changes.remove(i);
                bad(b);
            }
            for old in [true, false] {
                let mut b = original.clone();
                let s = &mut first(&mut b).storage_changes[i];
                if old {
                    s.old_value = n(99).to_vec()
                } else {
                    s.new_value = n(99).to_vec()
                }
                bad(b);
            }
        }
        let mut reversed = original.clone();
        first(&mut reversed).storage_changes.reverse();
        good(reversed); // vectors are not execution order
        let mut b = original.clone();
        let a = first(&mut b).storage_changes[0].ordinal;
        let z = first(&mut b).storage_changes.last().unwrap().ordinal;
        first(&mut b).storage_changes[0].ordinal = z;
        first(&mut b).storage_changes.last_mut().unwrap().ordinal = a;
        bad(b);
    }
    let original = saved_block("role_remove_first");
    let r = role("role_remove_first");
    for (key, value) in &original.transaction_traces[0].calls[0].keccak_preimages {
        let raw = bytes(value);
        let required = raw.len() == 64
            && ((raw[..32] == r && (raw[32..] == n(201) || raw[32..] == n(251))) || raw[32..] == leaf(r, n(201)) || raw[32..] == plus(leaf(r, n(251)), n(1)));
        if required {
            let mut b = original.clone();
            first(&mut b).keccak_preimages.remove(key);
            bad(b);
        }
    }
    let mut b = original.clone();
    first(&mut b).keccak_preimages.clear();
    bad(b);
    let mut b = original.clone();
    let key = first(&mut b).keccak_preimages.keys().next().unwrap().clone();
    first(&mut b).keccak_preimages.insert(key, hex::encode([0; 65]));
    bad(b);
    for field in [0, 1, 2] {
        let mut b = original.clone();
        let s = &mut first(&mut b).storage_changes[0];
        match field {
            0 => s.key.insert(0, 0),
            1 => s.old_value.insert(0, 0),
            _ => s.new_value.insert(0, 0),
        }
        bad(b);
    }
    // Correct hashes do not excuse wrong key order or non-address padding.
    let original = saved_block("role_add_empty");
    let r = role("role_add_empty");
    let base = leaf(r, n(201));
    let old = leaf(n(3), base);
    let mut dirty = n(3);
    dirty[0] = 1;
    for preimage in [pair(dirty, base), pair(base, n(3))] {
        let mut b = original.clone();
        first(&mut b).keccak_preimages.remove(&hex::encode(old));
        first(&mut b).storage_changes[0].key = hash(&preimage).to_vec();
        image(first(&mut b), preimage);
        bad(b);
    }
}
#[test]
fn gm_roles_members_storage_accounts_and_frames_cannot_be_joined() {
    for name in ["role_add_zero_empty", "minter", "burner", "arbitrary", "max_role"] {
        let mut c = saved(&case("role_add_empty"), 0, usize::MAX);
        let other = saved(&case(name), 0, usize::MAX);
        c.keccak_preimages.extend(other.keccak_preimages);
        let ordinal = c.storage_changes[0].ordinal;
        c.storage_changes[0] = other.storage_changes[0].clone();
        c.storage_changes[0].ordinal = ordinal;
        bad(block(vec![c]));
    }
    let original = saved(&case("role_add_empty"), 0, usize::MAX);
    for split in 1..original.storage_changes.len() {
        let mut left = original.clone();
        let mut right = original.clone();
        right.storage_changes = left.storage_changes.split_off(split);
        bad(block(vec![left, right]));
    }
    let mut c = original.clone();
    c.storage_changes[0].address = address(1);
    bad(block(vec![c]));
    // Same roots in two storage accounts have independent continuity journals.
    good(block(vec![
        saved(&case("role_add_zero_empty"), 0, 0),
        saved(&case("role_remove_sole"), 1, usize::MAX),
    ]));
    for ordinal in [
        original.storage_changes[0].ordinal,
        original.storage_changes[1].ordinal,
        original.storage_changes.last().unwrap().ordinal,
    ] {
        let mut c = original.clone();
        let mut foreign = row(0, n(99), ZERO, ZERO, ordinal);
        foreign.address = vec![8; 20];
        c.storage_changes.push(foreign);
        bad(block(vec![c]));
        let mut b = block(vec![original.clone()]);
        b.transaction_traces[0].calls.push(eth::Call {
            address: vec![8; 20],
            begin_ordinal: ordinal,
            end_ordinal: ordinal + 1,
            state_reverted: true,
            ..Default::default()
        });
        bad(b);
    }
}
#[test]
fn gm_physical_continuity_including_omitted_equal_stores_and_moved_member() {
    for keep in [0, usize::MAX] {
        good(block(vec![
            saved(&case("role_add_zero_empty"), 0, keep),
            saved(&case("role_remove_zero_sole"), 0, keep),
        ]));
        bad(block(vec![
            saved(&case("role_add_zero_empty"), 0, keep),
            saved(&case("role_remove_sole"), 0, usize::MAX),
        ]));
    }
    let seq = (0..10).map(|i| saved(&case(&format!("sequence_{i}")), 0, usize::MAX)).collect();
    good(block(seq));
    let remove = saved(&case("role_remove_first"), 0, usize::MAX);
    let r = role("role_remove_first");
    let moved = leaf(n(5), leaf(r, n(201)));
    assert!(!remove.keccak_preimages.contains_key(&hex::encode(moved)));
    for field in ["other_slots", "balance_slot"] {
        let mut c = configs();
        if field == "other_slots" {
            c[0][field].as_array_mut().unwrap().push(json!(h(moved)));
        } else {
            c[0][field] = json!(h(moved));
        }
        assert!(project_with(&block(vec![remove.clone()]), c).unwrap_err().contains("coherence key aliases"));
    }
    for with_hint in [false, true] {
        let mut c = remove.clone();
        c.storage_changes.push(row(0, moved, n(1), n(1), 8000));
        if with_hint {
            image(&mut c, pair(n(5), leaf(r, n(201))));
        }
        bad(block(vec![c]));
    }
}
#[test]
fn gm_schema_is_exact_and_admins_and_unbounded_string_payloads_remain_unknown() {
    for field in ["root", "membership_root"] {
        for value in [Value::Null, json!(h(n(5))), json!(h(n(6))), json!("0xc9")] {
            let mut c = configs();
            c[0]["enumerable_address_sets"][0][field] = value;
            assert!(layout::parse(&json!(c).to_string()).is_err());
        }
    }
    for root in [201, 251] {
        for field in ["other_slots", "other_mapping_slots", "address_lists"] {
            let mut c = configs();
            if c[0][field].is_null() {
                c[0][field] = json!([]);
            }
            c[0][field].as_array_mut().unwrap().push(json!(h(n(root))));
            assert!(layout::parse(&json!(c).to_string()).is_err());
        }
        let mut c = configs();
        c[0]["other_mapping_words"][h(n(root))] = json!(2);
        assert!(layout::parse(&json!(c).to_string()).is_err());
        let mut c = configs();
        c[0]["beacon_proxy"]["beacon_slot"] = json!(h(n(root)));
        assert!(layout::parse(&json!(c).to_string()).is_err());
    }
    let mut c = configs();
    let rule = c[0]["enumerable_address_sets"][0].clone();
    c[0]["enumerable_address_sets"].as_array_mut().unwrap().push(rule);
    assert!(layout::parse(&json!(c).to_string()).is_err());
    let r = role("role_add_empty");
    for slot in [leaf(r, n(201)), plus(leaf(r, n(201)), n(1)), plus(leaf(r, n(251)), n(1))] {
        for equal in [false, true] {
            let mut b = saved_block("role_add_empty");
            first(&mut b).storage_changes.push(row(0, slot, ZERO, if equal { ZERO } else { n(1) }, 8000));
            bad(b);
        }
    }
    for root in [54, 401] {
        for i in [0, 1] {
            let mut b = saved_block("role_add_empty");
            first(&mut b).storage_changes.push(row(0, plus(hash(&n(root)), n(i)), ZERO, n(1), 8000));
            good(b);
        }
        let mut b = saved_block("role_add_empty");
        first(&mut b).storage_changes.push(row(0, plus(hash(&n(root)), n(2)), ZERO, n(1), 8000));
        bad(b);
    }
}
#[test]
fn gm_versions_actual_frame_boundaries_and_persisted_effect_rules() {
    for ver in [0, 2, 3, 6] {
        let mut b = saved_block("role_add_empty");
        b.ver = ver;
        bad(b);
    }
    for begin in [0, first(&mut saved_block("role_add_empty")).storage_changes[0].ordinal] {
        let mut b = saved_block("role_add_empty");
        first(&mut b).begin_ordinal = begin;
        bad(b);
    }
    let mut b = saved_block("role_add_empty");
    b.transaction_traces[0].status = eth::TransactionTraceStatus::Failed as i32;
    assert_eq!(project_with(&b, configs()).unwrap(), 0);
    let mut c = saved(&case("empty_set_prefix_revert"), 0, usize::MAX);
    c.state_reverted = true;
    let valid = eth::Call {
        address: address(0),
        begin_ordinal: 1,
        end_ordinal: 10000,
        ..Default::default()
    };
    let mut b = block(vec![c, valid]);
    let successful = &mut b.transaction_traces[0].calls[1];
    image(successful, pair(n(44), n(51)));
    successful.storage_changes.push(row(0, leaf(n(44), n(51)), ZERO, n(9), 19000));
    assert_eq!(project_with(&b, configs()).unwrap(), 1);
    // Root begin=0 has no synthetic fallback for the newly selected v4/v5 mode.
    for ver in [4, 5] {
        let mut b = saved_block("role_add_empty");
        b.ver = ver;
        first(&mut b).begin_ordinal = 0;
        bad(b);
    }
}
#[test]
fn gm_exact_proxy_implementation_beacon_runtime_and_creation_guards() {
    let fixtures = [
        include_str!("fixtures/gm-operation-proof/proxy-a-capture.json"),
        include_str!("fixtures/gm-operation-proof/proxy-b-capture.json"),
        include_str!("fixtures/gm-operation-proof/implementation-capture.json"),
    ];
    for (i, raw) in fixtures.into_iter().enumerate() {
        let capture: Value = serde_json::from_str(raw).unwrap();
        let runtime = bytes(capture["runtimeBytecode"]["onchainBytecode"].as_str().unwrap());
        let account = bytes(capture["address"].as_str().unwrap());
        for creation in [false, true] {
            let mut b = saved_block("role_add_empty");
            if creation {
                first(&mut b).call_type = eth::CallType::Create as i32;
            }
            first(&mut b).code_changes.push(eth::CodeChange {
                address: account.clone(),
                old_code: if creation { vec![] } else { runtime.clone() },
                new_code: if creation { runtime.clone() } else { vec![1] },
                old_hash: if creation { hash(&[]).to_vec() } else { hash(&runtime).to_vec() },
                new_hash: if creation { hash(&runtime).to_vec() } else { hash(&[1]).to_vec() },
                ordinal: 8000,
            });
            bad(b);
        }
        assert!(i < 3);
    }
    let c = configs();
    let beacon = bytes(c[0]["beacon_proxy"]["beacon"].as_str().unwrap());
    let mut b = saved_block("role_add_empty");
    first(&mut b).code_changes.push(eth::CodeChange {
        address: beacon.clone(),
        old_hash: bytes(c[0]["beacon_proxy"]["beacon_code_hash"].as_str().unwrap()),
        new_hash: hash(&[1]).to_vec(),
        new_code: vec![1],
        ordinal: 8000,
        ..Default::default()
    });
    bad(b);
    for (account, key, old) in [
        (address(0), w(&c[0]["beacon_proxy"]["beacon_slot"]), {
            let mut a = ZERO;
            a[12..].copy_from_slice(&beacon);
            a
        }),
        (beacon, n(1), {
            let mut a = ZERO;
            a[12..].copy_from_slice(&bytes(c[0]["beacon_proxy"]["implementation"].as_str().unwrap()));
            a
        }),
    ] {
        let mut b = saved_block("role_add_empty");
        for (old, new, ordinal) in [(old, n(9), 8000), (n(9), old, 8100)] {
            let mut s = row(0, key, old, new, ordinal);
            s.address = account.clone();
            first(&mut b).storage_changes.push(s);
        }
        bad(b);
    }
}
#[test]
fn gm_omitted_zero_array_stage_cannot_alias_roots_or_known_balance_leaves() {
    let r = n(77);
    let head = leaf(r, n(251));
    let base = leaf(r, n(201));
    let array = hash(&head);
    for target in [n(51), n(52), n(201), n(251), head, base, plus(base, n(1)), leaf(n(44), n(51))] {
        let length = minus(target, array);
        assert_ne!(length, [255; 32]);
        let next = plus(length, n(1));
        let mut c = eth::Call {
            address: address(0),
            begin_ordinal: 1,
            end_ordinal: 10000,
            ..Default::default()
        };
        for p in [pair(r, n(201)), pair(r, n(251)), pair(ZERO, base), pair(ZERO, plus(head, n(1)))] {
            image(&mut c, p);
        }
        c.storage_changes = vec![
            row(0, leaf(ZERO, base), ZERO, n(1), 100),
            row(0, head, length, next, 200),
            row(0, leaf(ZERO, plus(head, n(1))), ZERO, next, 400),
        ];
        bad(block(vec![c]));
    }
}
