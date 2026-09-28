#![cfg(not(target_arch = "wasm32"))]
//! Independent public-API adversarial checks using frozen actual-runtime traces.
//! Translated Extended records and synthetic collision controls are not producer
//! observations, authorization checks, or runtime/deployment qualification.
use erc20_balances::{hash, layout, project};
use serde_json::{json, Value};
use substreams_ethereum::pb::eth::v2 as eth;

type Word = [u8; 32];
const ZERO: Word = [0; 32];
const TRANSCRIPTS: &str = include_str!("../docs/evidence/ptoken-operation-proof-20260928-transcripts.json");
const CAPTURE: &str = include_str!("fixtures/ptoken-operation-proof/capture.json");

fn n(value: u64) -> Word {
    let mut word = ZERO;
    word[24..].copy_from_slice(&value.to_be_bytes());
    word
}
fn bytes(value: &str) -> Vec<u8> {
    hex::decode(value.trim_start_matches("0x")).unwrap()
}
fn w(value: &Value) -> Word {
    bytes(value.as_str().unwrap()).try_into().unwrap()
}
fn h(value: Word) -> String {
    format!("0x{}", hex::encode(value))
}
fn account() -> Vec<u8> {
    bytes("c63961bf9a7d6bb5844524851d04fbd76dbe915b")
}
fn pair(a: Word, b: Word) -> Vec<u8> {
    [a.as_slice(), b.as_slice()].concat()
}
fn leaf(a: Word, b: Word) -> Word {
    hash(&pair(a, b))
}
fn plus(a: Word, b: Word) -> Word {
    let mut result = ZERO;
    let mut carry = 0u16;
    for i in (0..32).rev() {
        carry += u16::from(a[i]) + u16::from(b[i]);
        result[i] = carry as u8;
        carry >>= 8;
    }
    result
}
fn minus(a: Word, b: Word) -> Word {
    plus(plus(a, b.map(|byte| !byte)), n(1))
}
fn config() -> Value {
    json!({
        "contract": format!("0x{}", hex::encode(account())),
        "code_hash": "0x60f53552d1ab18923098e47b0d7952ae7a4a48d8bece60b20d3d41734832c15c",
        "balance_slot": h(ZERO), "other_mapping_slots": [h(n(1))],
        "other_slots": [h(n(2)), h(n(3)), h(n(4)), h(n(7))],
        "enumerable_address_sets": [{
            "root": h(n(6)), "membership_root": h(n(5)), "key_types": ["bytes32"],
            "semantics": layout::PTOKEN_ENUMERABLE_SEMANTICS
        }]
    })
}
fn parse(config: Value) -> Vec<layout::VerifiedLayout> {
    layout::parse(&json!([config]).to_string()).unwrap()
}
fn case(name: &str) -> Value {
    serde_json::from_str::<Vec<Value>>(TRANSCRIPTS)
        .unwrap()
        .into_iter()
        .find(|c| c["name"] == name)
        .unwrap()
}
fn role(name: &str) -> Word {
    bytes(case(name)["calldata"].as_str().unwrap())[4..36].try_into().unwrap()
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
fn preimage(call: &mut eth::Call, image: Vec<u8>) {
    call.keccak_preimages.insert(hex::encode(hash(&image)), hex::encode(image));
}
fn saved(name: &str, keep_equal: bool) -> eth::Call {
    let case = case(name);
    let mut call = eth::Call {
        address: account(),
        begin_ordinal: 1,
        end_ordinal: 2000,
        ..Default::default()
    };
    for entry in case["execution"]["keccaks"].as_array().unwrap() {
        let raw = bytes(entry["input"].as_str().unwrap());
        assert_eq!(h(hash(&raw)), entry["output"]);
        preimage(&mut call, raw);
    }
    for store in case["execution"]["writes"].as_array().unwrap() {
        if keep_equal || store["old"] != store["new"] {
            call.storage_changes
                .push(row(w(&store["key"]), w(&store["old"]), w(&store["new"]), store["step"].as_u64().unwrap() + 10));
        }
    }
    call
}
fn block(mut calls: Vec<eth::Call>) -> eth::Block {
    for (index, call) in calls.iter_mut().enumerate() {
        let shift = 2000 * index as u64;
        call.begin_ordinal += shift;
        call.end_ordinal += shift;
        for row in &mut call.storage_changes {
            row.ordinal += shift;
        }
        for change in &mut call.code_changes {
            change.ordinal += shift;
        }
    }
    // A separate observed balance makes rejection atomic and keeps role-only
    // acceptance from masquerading as a deployment/holder seed.
    let first = &mut calls[0];
    preimage(first, pair(n(44), ZERO));
    first.storage_changes.push(row(leaf(n(44), ZERO), ZERO, n(9), 1900));
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
fn good(b: &eth::Block, configuration: Value) {
    let result = project(b, &parse(configuration)).unwrap();
    assert_eq!(result.balances.len(), 1);
    assert_eq!(result.balances[0].amount, "9");
}
fn bad(b: &eth::Block, configuration: Value) {
    assert!(
        project(b, &parse(configuration)).is_err(),
        "malformed metadata must reject the whole block, including its valid balance"
    );
}
// This only constructs a mathematical address-collision boundary. Phase A did
// not execute sets with these huge lengths; no admission claim is inferred.
fn synthetic_zero_add(role: Word, target: Word, keep_equal: bool) -> eth::Call {
    let head = leaf(role, n(6));
    let base = leaf(role, n(5));
    let length = minus(target, hash(&head));
    assert_ne!(length, [255; 32]);
    let next = plus(length, n(1));
    let mut call = eth::Call {
        address: account(),
        begin_ordinal: 1,
        end_ordinal: 2000,
        ..Default::default()
    };
    for image in [pair(role, n(5)), pair(role, n(6)), pair(ZERO, base), pair(ZERO, plus(head, n(1)))] {
        preimage(&mut call, image);
    }
    call.storage_changes = vec![row(leaf(ZERO, base), ZERO, n(1), 100), row(head, length, next, 200)];
    if keep_equal {
        call.storage_changes.push(row(target, ZERO, ZERO, 300));
    }
    call.storage_changes.push(row(leaf(ZERO, plus(head, n(1))), ZERO, next, 400));
    call
}
fn proxy(slot: Word) -> Value {
    json!({"implementation_slot":h(slot), "implementation":format!("0x{}", hex::encode([8;20])), "code_hash":h([9;32])})
}
fn beacon(slot: Word) -> Value {
    json!({
        "beacon_slot":h(slot), "beacon":format!("0x{}", hex::encode([8;20])), "beacon_code_hash":h([9;32]),
        "implementation_slot":h(n(81)), "implementation":format!("0x{}", hex::encode([10;20])), "implementation_code_hash":h([11;32])
    })
}

#[test]
fn source_roles_and_members_cannot_exchange_either_side_of_the_link() {
    let names = [
        "grant_empty",
        "grant_default_admin",
        "grant_pauser",
        "grant_minter",
        "grant_arbitrary",
        "grant_zero_empty",
    ];
    for name in names {
        good(&block(vec![saved(name, true)]), config());
    }
    for left in names {
        for right in names.into_iter().filter(|right| *right != left) {
            let left_call = saved(left, true);
            let right_call = saved(right, true);
            assert_ne!(left_call.storage_changes[0].key, right_call.storage_changes[0].key);
            for bool_from_right in [false, true] {
                let mut mixed = if bool_from_right { left_call.clone() } else { right_call.clone() };
                let donor = if bool_from_right { &right_call } else { &left_call };
                let ordinal = mixed.storage_changes[0].ordinal;
                mixed.storage_changes[0] = donor.storage_changes[0].clone();
                mixed.storage_changes[0].ordinal = ordinal;
                mixed.keccak_preimages.extend(donor.keccak_preimages.clone());
                bad(&block(vec![mixed]), config());
            }
        }
    }
}

#[test]
fn omitted_zero_element_still_constrains_the_next_complete_operation() {
    for keep_equal in [false, true] {
        good(
            &block(vec![saved("grant_zero_empty", keep_equal), saved("remove_only_zero", keep_equal)]),
            config(),
        );
        good(&block(vec![saved("grant_empty", true), saved("remove_only", true)]), config());
        // Both traces are individually valid. Length 1->0 agrees and the removed
        // member's bool/index have no prior facts, but array[0] was inferred zero.
        let b = block(vec![saved("grant_zero_empty", keep_equal), saved("remove_only", true)]);
        let error = project(&b, &parse(config())).unwrap_err().to_string();
        assert!(error.contains("discontinuous"), "must exercise inferred array continuity: {error}");
    }
}

#[test]
fn moved_tail_boolean_is_a_constraint_without_a_preimage_or_write_permission() {
    let call = saved("remove_first", true);
    let moved_member = n(4);
    let moved_bool = leaf(moved_member, leaf(role("remove_first"), n(5)));
    assert!(!call.keccak_preimages.contains_key(&hex::encode(moved_bool)), "exact runtime did not hash M(t)");
    good(&block(vec![call.clone()]), config());
    for with_hint in [false, true] {
        let mut extra = call.clone();
        extra.storage_changes.push(row(moved_bool, n(1), n(1), 1000));
        if with_hint {
            preimage(&mut extra, pair(moved_member, leaf(role("remove_first"), n(5))));
        }
        bad(&block(vec![extra]), config());
    }
    // These schemas are syntactically valid but contradict the derived source
    // invariant. The constraint is protected even without a moved-bool preimage.
    for field in ["other_slots", "proxy", "balance_divisor"] {
        let mut c = config();
        match field {
            "other_slots" => c[field].as_array_mut().unwrap().push(json!(h(moved_bool))),
            "proxy" => c[field] = proxy(moved_bool),
            _ => c[field] = json!({"value":h(n(1)), "storage_slot":h(moved_bool)}),
        }
        let b = block(vec![call.clone()]);
        let error = project(&b, &parse(c)).unwrap_err().to_string();
        assert!(error.contains("coherence key aliases protected"), "{error}");
    }
}

#[test]
fn earlier_or_later_omitted_array_stages_cannot_alias_another_roles_derived_boolean() {
    let remove = saved("remove_first", true);
    let moved_bool = leaf(n(4), leaf(role("remove_first"), n(5)));
    for keep_equal in [false, true] {
        let add = synthetic_zero_add(n(12345), moved_bool, keep_equal);
        good(&block(vec![add.clone()]), config());
        for reversed in [false, true] {
            let calls = if reversed {
                vec![remove.clone(), add.clone()]
            } else {
                vec![add.clone(), remove.clone()]
            };
            let error = project(&block(calls), &parse(config())).unwrap_err().to_string();
            assert!(error.contains("alias"), "must reject cross-role coherence alias: {error}");
        }
    }
}

#[test]
fn coupled_roots_are_exclusive_against_every_dependency_and_checkpoint_kind() {
    for root in [n(5), n(6)] {
        for kind in [
            "proxy",
            "beacon_proxy",
            "zero_balance",
            "balance_divisor",
            "address_hash_balance",
            "checkpoint_direct",
            "checkpoint_mapping",
        ] {
            for (slot, valid) in [(n(99), true), (root, false)] {
                let mut c = config();
                match kind {
                    "proxy" => c[kind] = proxy(slot),
                    "beacon_proxy" => c[kind] = beacon(slot),
                    "zero_balance" => c[kind] = json!({"value":h(n(1)), "storage_slot":h(slot)}),
                    "balance_divisor" => c[kind] = json!({"value":h(n(1)), "storage_slot":h(slot)}),
                    "address_hash_balance" => {
                        c[kind] = json!({
                            "modulus":h(n(1)), "offset":h(ZERO), "multiplier":h(n(1)),
                            "stored_addresses":[{"slot":h(slot), "address":format!("0x{}", hex::encode([8;20]))}]
                        })
                    }
                    _ => {
                        let field = if kind == "checkpoint_direct" { "slots" } else { "mapping_slots" };
                        c["voting_checkpoints"] = json!({"clock":"block_number", field:[h(slot)]});
                    }
                }
                assert_eq!(layout::parse(&json!([c]).to_string()).is_ok(), valid, "{kind} at {}", h(slot));
            }
        }
        for before in [false, true] {
            let mut c = config();
            let legacy = json!({"root":h(root), "key_types":["bytes32"], "semantics":"oz_3_4_2"});
            let rules = c["enumerable_address_sets"].as_array_mut().unwrap();
            if before {
                rules.insert(0, legacy);
            } else {
                rules.push(legacy);
            }
            assert!(layout::parse(&json!([c]).to_string()).is_err(), "root ownership cannot depend on rule order");
        }
    }
    let mut duplicate = config();
    let rule = duplicate["enumerable_address_sets"][0].clone();
    duplicate["enumerable_address_sets"].as_array_mut().unwrap().push(rule);
    assert!(layout::parse(&json!([duplicate]).to_string()).is_err());
}

#[test]
fn optional_zero_stage_cannot_hide_foreign_equality_or_exact_boundary_barriers() {
    let call = saved("grant_zero_empty", false);
    let first_ordinal = call.storage_changes.first().unwrap().ordinal;
    let last_ordinal = call.storage_changes.last().unwrap().ordinal;
    let omitted_ordinal = case("grant_zero_empty")["execution"]["writes"][2]["step"].as_u64().unwrap() + 10;
    for ordinal in [first_ordinal, omitted_ordinal, last_ordinal] {
        let mut b = block(vec![call.clone()]);
        let mut foreign = row(n(70), ZERO, ZERO, ordinal);
        foreign.address = vec![8; 20];
        b.transaction_traces[0].calls[0].storage_changes.push(foreign);
        bad(&b, config());
        let mut b = block(vec![call.clone()]);
        b.transaction_traces[0].calls.push(eth::Call {
            address: vec![8; 20],
            begin_ordinal: ordinal,
            end_ordinal: ordinal + 1,
            state_reverted: true,
            ..Default::default()
        });
        bad(&b, config());
    }
    let mut outside = block(vec![call]);
    let mut foreign = row(n(70), ZERO, ZERO, 1500);
    foreign.address = vec![8; 20];
    outside.transaction_traces[0].calls[0].storage_changes.push(foreign);
    good(&outside, config());
}

#[test]
fn identical_physical_keys_in_distinct_storage_accounts_do_not_share_coherence() {
    let first = saved("grant_zero_empty", false);
    let mut second = saved("remove_only", true);
    let other = vec![8; 20];
    second.address = other.clone();
    for row in &mut second.storage_changes {
        row.address = other.clone();
    }
    let mut other_config = config();
    other_config["contract"] = json!(format!("0x{}", hex::encode(other)));
    let layouts = layout::parse(&json!([config(), other_config]).to_string()).unwrap();
    let result = project(&block(vec![first, second]), &layouts).unwrap();
    assert_eq!(result.balances.len(), 1);
    assert_eq!(result.balances[0].amount, "9");
}

#[test]
fn source_matched_runtime_does_not_admit_creation_and_dependencies_still_fail_closed() {
    let capture: Value = serde_json::from_str(CAPTURE).unwrap();
    let runtime = bytes(capture["runtimeBytecode"]["onchainBytecode"].as_str().unwrap());
    assert_eq!(h(hash(&runtime)), config()["code_hash"]);
    for (old_code, new_code) in [(runtime.clone(), vec![1]), (Vec::new(), runtime)] {
        let mut b = block(vec![saved("grant_empty", true)]);
        if old_code.is_empty() {
            b.number = capture["deployment"]["blockNumber"].as_str().unwrap().parse().unwrap();
            b.header.as_mut().unwrap().number = b.number;
            b.transaction_traces[0].calls[0].call_type = eth::CallType::Create as i32;
        }
        b.transaction_traces[0].calls[0].code_changes.push(eth::CodeChange {
            address: account(),
            old_hash: hash(&old_code).to_vec(),
            new_hash: hash(&new_code).to_vec(),
            old_code,
            new_code,
            ordinal: 1000,
        });
        bad(&b, config());
    }
    // Synthetic dependency configuration only: this does not claim PToken is a proxy.
    let mut c = config();
    c["proxy"] = proxy(n(99));
    let mut b = block(vec![saved("grant_empty", true)]);
    good(&b, c.clone());
    b.transaction_traces[0].calls[0]
        .storage_changes
        .extend([row(n(99), n(8), n(9), 1000), row(n(99), n(9), n(8), 1100)]);
    bad(&b, c.clone());
    let mut b = block(vec![saved("grant_empty", true)]);
    b.transaction_traces[0].calls[0].code_changes.push(eth::CodeChange {
        address: vec![8; 20],
        old_hash: vec![9; 32],
        new_hash: hash(&[1]).to_vec(),
        old_code: vec![],
        new_code: vec![1],
        ordinal: 1000,
    });
    bad(&b, c);
}

#[test]
fn selecting_coupled_semantics_refuses_v3_even_with_no_role_witnesses() {
    let mut b = block(vec![eth::Call {
        address: account(),
        begin_ordinal: 1,
        end_ordinal: 2000,
        ..Default::default()
    }]);
    for version in [4, 5] {
        b.ver = version;
        good(&b, config());
    }
    b.ver = 3;
    bad(&b, config());
    // Unselected ordinary layouts retain their existing v3 behavior.
    let mut ordinary = config();
    ordinary.as_object_mut().unwrap().remove("enumerable_address_sets");
    good(&b, ordinary);
}
