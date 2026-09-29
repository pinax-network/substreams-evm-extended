#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::erc20tokenx_role as bound;
use serde_json::{json, Value};
const BASELINE: &[u8] = include_bytes!("../../tests/fixtures/bsc-refined450-layouts.json");
#[test]
fn tokenx_candidates_restore_all_original_fields_without_boolean_or_admin_permission() {
    let baseline: Value = serde_json::from_slice(BASELINE).unwrap();
    let selected = bound::candidate(BASELINE).unwrap();
    assert_eq!(
        selected,
        serde_json::from_str::<Value>(include_str!("../../tests/fixtures/erc20tokenx-enumerable-candidate/layouts.json")).unwrap()
    );
    assert_eq!(selected.as_array().unwrap().len(), 3);
    for (i, mut c) in selected.as_array().unwrap().iter().cloned().enumerate() {
        assert_eq!(c["contract"], bound::CONTRACTS[i]);
        assert_eq!(
            c["enumerable_address_sets"],
            json!([{"root":bound::root(8),"key_types":["bytes32"],"semantics":"oz_3_4_2"}])
        );
        c.as_object_mut().unwrap().remove("enumerable_address_sets");
        assert_eq!(c["other_mapping_words"].as_object_mut().unwrap().insert(bound::root(8), json!(3)), None);
        let originals: Vec<_> = baseline.as_array().unwrap().iter().filter(|p| p["contract"] == c["contract"]).collect();
        assert_eq!(originals, [&c]);
    }
}

#[test]
fn tokenx_length_counter_has_no_boolean_or_admin_interpretation() {
    use substreams_ethereum::pb::eth::v2 as eth;
    let root = hex::decode(&bound::root(8)[2..]).unwrap();
    let mut raw = vec![0x81; 32];
    raw.extend_from_slice(&root);
    let head = erc20_balances::hash(&raw);
    for address in bound::CONTRACTS {
        let account = hex::decode(&address[2..]).unwrap();
        let row = |old: Vec<u8>, new: Vec<u8>| eth::StorageChange {
            address: account.clone(),
            key: head.to_vec(),
            old_value: old,
            new_value: new,
            ordinal: 50,
        };
        let make = |write: eth::StorageChange| eth::Block {
            transaction_traces: vec![eth::TransactionTrace {
                status: eth::TransactionTraceStatus::Succeeded as i32,
                calls: vec![eth::Call {
                    address: account.clone(),
                    keccak_preimages: [(hex::encode(head), hex::encode(&raw))].into_iter().collect(),
                    storage_changes: vec![write],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        for (old, new) in [(vec![], vec![1]), (vec![1], vec![]), (vec![0, 1], vec![2])] {
            let b = make(row(old, new));
            assert_eq!(bound::role_length_writes(&b).unwrap()[address], 1);
        }
        let b = make(row(vec![], vec![1]));
        for mutation in [
            "failed",
            "reverted",
            "account",
            "key",
            "old",
            "new",
            "equal",
            "missing",
            "wrongroot",
            "admin",
            "index",
        ] {
            let mut changed = b.clone();
            let tx = &mut changed.transaction_traces[0];
            let c = &mut tx.calls[0];
            let w = &mut c.storage_changes[0];
            match mutation {
                "failed" => tx.status = eth::TransactionTraceStatus::Failed as i32,
                "reverted" => c.state_reverted = true,
                "account" => w.address = vec![3; 20],
                "key" => w.key = vec![1; 33],
                "old" => w.old_value = vec![1; 33],
                "new" => w.new_value = vec![1; 33],
                "equal" => {
                    w.old_value = vec![0, 1];
                    w.new_value = vec![1];
                }
                "missing" => c.keccak_preimages.clear(),
                "wrongroot" => {
                    let mut wrong = raw.clone();
                    wrong[63] = 9;
                    w.key = erc20_balances::hash(&wrong).to_vec();
                    c.keccak_preimages.clear();
                    c.keccak_preimages.insert(hex::encode(&w.key), hex::encode(wrong));
                }
                "admin" => {
                    let n = primitive_types::U256::from_big_endian(&head).overflowing_add(2.into()).0;
                    let mut key = [0; 32];
                    n.to_big_endian(&mut key);
                    w.key = key.to_vec();
                }
                "index" => {
                    let n = primitive_types::U256::from_big_endian(&head).overflowing_add(1.into()).0;
                    let mut parent = [0; 32];
                    n.to_big_endian(&mut parent);
                    let mut index = vec![0; 32];
                    index[31] = 3;
                    index.extend_from_slice(&parent);
                    w.key = erc20_balances::hash(&index).to_vec();
                    c.keccak_preimages.insert(hex::encode(&w.key), hex::encode(index));
                }
                _ => unreachable!(),
            }
            assert_eq!(bound::role_length_writes(&changed).unwrap()[address], 0, "{mutation}");
        }
        let mut corrupt = b.clone();
        corrupt.transaction_traces[0].calls[0]
            .keccak_preimages
            .insert(hex::encode(head), "ff".repeat(64));
        assert!(bound::role_length_writes(&corrupt).is_err());
    }
}
#[test]
fn tokenx_candidate_tampering_cannot_change_any_profile_or_baseline_field() {
    let selected = bound::candidate(BASELINE).unwrap();
    bound::verify_candidate(BASELINE, &selected).unwrap();
    for i in 0..3 {
        for field in selected[i].as_object().unwrap().keys() {
            let mut changed = selected.clone();
            changed[i][field] = Value::Null;
            assert!(bound::verify_candidate(BASELINE, &changed).is_err(), "{i}/{field}");
        }
        for (field, value) in [
            ("membership_root", Value::Null),
            ("membership_root", json!(bound::root(8))),
            ("semantics", json!("ptoken_v2_solc_0_8_28_oz_5_4_0")),
            ("root", json!(bound::root(9))),
        ] {
            let mut changed = selected.clone();
            changed[i]["enumerable_address_sets"][0][field] = value;
            assert!(bound::verify_candidate(BASELINE, &changed).is_err());
        }
        let mut broad = selected.clone();
        broad[i]["other_mapping_words"][bound::root(8)] = json!(3);
        assert!(bound::verify_candidate(BASELINE, &broad).is_err());
        let mut boolean = selected.clone();
        boolean[i]["other_mapping_paths"] = json!([{"root":bound::root(8),"key_types":["bytes32","address"],"offset":0,"words":1}]);
        assert!(bound::verify_candidate(BASELINE, &boolean).is_err());
    }
    let mut extra = selected.clone();
    extra.as_array_mut().unwrap().push(selected[0].clone());
    assert!(bound::verify_candidate(BASELINE, &extra).is_err());
    let mut raw = BASELINE.to_vec();
    raw.push(b' ');
    assert!(bound::candidate(&raw).is_err());
}

#[test]
fn tokenx_complete_frozen_proof_and_every_artifact_are_bound_before_review() {
    use std::{fs, path::Path};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let review = bound::review(root).unwrap();
    assert_eq!(review["phase_a"]["calls"], 520);
    assert_eq!(review["phase_a"]["source_inputs"], 203);
    assert_eq!(review["admin_callsite_count"], 0);
    let temp = tempfile::tempdir().unwrap();
    for (path, _) in bound::ARTIFACTS {
        let dest = temp.path().join(path);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::copy(root.join(path), dest).unwrap();
    }
    assert_eq!(bound::review(temp.path()).unwrap(), review);
    for (path, _) in bound::ARTIFACTS {
        let dest = temp.path().join(path);
        let original = fs::read(&dest).unwrap();
        let mut changed = original.clone();
        changed.push(b' ');
        fs::write(&dest, &changed).unwrap();
        assert!(bound::review(temp.path()).is_err(), "raw pin {path}");
        fs::write(&dest, original).unwrap();
    }
    // Cache equality is a separate guard, including the original null PHI source.
    let cache = tempfile::tempdir().unwrap();
    for (fixture, path) in bound::CACHE_FILES {
        let dest = cache.path().join(path);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::copy(root.join(bound::PROOF_FIXTURE).join(fixture), dest).unwrap();
    }
    bound::verify_cache(root, cache.path()).unwrap();
    for (_, path) in bound::CACHE_FILES {
        let dest = cache.path().join(path);
        let original = fs::read(&dest).unwrap();
        let mut changed = original.clone();
        changed.push(b' ');
        fs::write(&dest, changed).unwrap();
        assert!(bound::verify_cache(root, cache.path()).is_err(), "original cache {path}");
        fs::write(&dest, original).unwrap();
    }
}

#[test]
fn tokenx_measured_legacy_operations_have_one_length_witness_without_boolean_stages() {
    use substreams_ethereum::pb::eth::v2 as eth;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let records: Vec<Value> =
        serde_json::from_slice(&std::fs::read(root.join("docs/evidence/erc20tokenx-operation-proof-20260929-transcripts.json")).unwrap()).unwrap();
    let candidates = bound::candidate(BASELINE).unwrap();
    let layouts = erc20_balances::layout::parse(&candidates.to_string()).unwrap();
    let decode = |v: &Value| hex::decode(v.as_str().unwrap().trim_start_matches("0x")).unwrap();
    for address in bound::CONTRACTS {
        for (name, sig, count) in [
            ("role_add_empty", "grantRole(bytes32,address)", 1),
            ("role_add_zero_empty", "grantRole(bytes32,address)", 1),
            ("role_remove_first", "revokeRole(bytes32,address)", 1),
            ("role_remove_tail", "revokeRole(bytes32,address)", 1),
            ("role_remove_zero_sole", "revokeRole(bytes32,address)", 1),
            ("role_move_zero_tail", "revokeRole(bytes32,address)", 1),
            ("renounce_self", "renounceRole(bytes32,address)", 1),
            ("role_duplicate", "grantRole(bytes32,address)", 0),
            ("role_absent_empty", "revokeRole(bytes32,address)", 0),
        ] {
            let found: Vec<_> = records
                .iter()
                .filter(|r| r["name"] == name && r["signature"] == sig && r["code_kind"] == "deployedBytecode")
                .collect();
            assert_eq!(found.len(), 1);
            let c = found[0];
            assert_eq!(c["execution"]["exit"]["kind"], "return");
            let equal = c["execution"]["writes"].as_array().unwrap().iter().filter(|w| w["old"] == w["new"]).count();
            for subset in 0..1 << equal {
                let account = hex::decode(&address[2..]).unwrap();
                let mut ix = 0;
                let changes = c["execution"]["writes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|w| {
                        if w["old"] == w["new"] {
                            let keep = subset & (1 << ix) != 0;
                            ix += 1;
                            if !keep {
                                return None;
                            }
                        }
                        Some(eth::StorageChange {
                            address: account.clone(),
                            key: decode(&w["key"]),
                            old_value: decode(&w["old"]),
                            new_value: decode(&w["new"]),
                            ordinal: w["step"].as_u64().unwrap() + 10,
                        })
                    })
                    .collect();
                let hints = c["execution"]["keccaks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|k| {
                        (
                            k["output"].as_str().unwrap().trim_start_matches("0x").to_owned(),
                            k["input"].as_str().unwrap().to_owned(),
                        )
                    })
                    .collect();
                let b = eth::Block {
                    ver: 5,
                    number: 122288046,
                    hash: vec![7; 32],
                    header: Some(eth::BlockHeader {
                        number: 122288046,
                        parent_hash: vec![6; 32],
                        state_root: vec![8; 32],
                        ..Default::default()
                    }),
                    detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
                    transaction_traces: vec![eth::TransactionTrace {
                        status: eth::TransactionTraceStatus::Succeeded as i32,
                        begin_ordinal: 1,
                        end_ordinal: 10000,
                        calls: vec![eth::Call {
                            address: account,
                            begin_ordinal: 1,
                            end_ordinal: 10000,
                            storage_changes: changes,
                            keccak_preimages: hints,
                            ..Default::default()
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                };
                erc20_balances::project(&b, &layouts).unwrap();
                let counts = bound::role_length_writes(&b).unwrap();
                assert_eq!(counts[address], count, "{name}/{subset}");
                assert_eq!(counts.values().sum::<u64>(), count);
            }
        }
    }
}
