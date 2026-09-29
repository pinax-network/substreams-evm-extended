#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::wkey_got_role as bound;
use serde_json::{json, Value};
use substreams_ethereum::pb::eth::v2 as eth;
const BASELINE: &[u8] = include_bytes!("../../tests/fixtures/bsc-refined450-layouts.json");
#[test]
fn wkey_got_candidates_restore_every_field_and_both_got_broad_entries() {
    let baseline: Value = serde_json::from_slice(BASELINE).unwrap();
    let selected = bound::candidate(BASELINE).unwrap();
    assert_eq!(
        selected,
        serde_json::from_str::<Value>(include_str!("../../tests/fixtures/wkey-got-enumerable-candidate/layouts.json")).unwrap()
    );
    assert_eq!(selected.as_array().unwrap().len(), 2);
    for (i, mut p) in selected.as_array().unwrap().iter().cloned().enumerate() {
        let address = bound::CONTRACTS[i];
        let r = bound::role_root(address).unwrap();
        assert_eq!(p["contract"], address);
        assert_eq!(
            p["enumerable_address_sets"],
            json!([{"root":bound::root(r),"key_types":["bytes32"],"semantics":"oz_3_4_2"}])
        );
        assert_eq!(p["other_mapping_slots"], json!([bound::root(1), bound::root(7)]));
        assert_eq!(p["other_mapping_words"], json!({}));
        p.as_object_mut().unwrap().remove("enumerable_address_sets");
        assert_eq!(p["other_mapping_words"].as_object_mut().unwrap().insert(bound::root(r), json!(3)), None);
        if i == 1 {
            p["other_mapping_slots"].as_array_mut().unwrap().insert(2, json!(bound::root(8)));
        }
        let originals: Vec<_> = baseline.as_array().unwrap().iter().filter(|v| v["contract"] == address).collect();
        assert_eq!(originals, [&p]);
    }
}
#[test]
fn wkey_got_candidate_mutation_or_either_leftover_permission_refuses() {
    let selected = bound::candidate(BASELINE).unwrap();
    bound::verify_candidate(BASELINE, &selected).unwrap();
    for i in 0..2 {
        let r = bound::role_root(bound::CONTRACTS[i]).unwrap();
        for field in selected[i].as_object().unwrap().keys() {
            let mut c = selected.clone();
            c[i][field] = Value::Null;
            assert!(bound::verify_candidate(BASELINE, &c).is_err());
        }
        for (field, value) in [
            ("membership_root", Value::Null),
            ("membership_root", json!(bound::root(r))),
            ("root", json!(bound::root(17 - r))),
            ("semantics", json!("ptoken_v2_solc_0_8_28_oz_5_4_0")),
        ] {
            let mut c = selected.clone();
            c[i]["enumerable_address_sets"][0][field] = value;
            assert!(bound::verify_candidate(BASELINE, &c).is_err());
        }
        for field in ["other_mapping_words", "other_mapping_slots"] {
            let mut c = selected.clone();
            if field == "other_mapping_words" {
                c[i][field][bound::root(r)] = json!(3);
            } else {
                c[i][field].as_array_mut().unwrap().push(json!(bound::root(r)));
            }
            assert!(bound::verify_candidate(BASELINE, &c).is_err());
            assert!(erc20_balances::layout::parse(&c.to_string()).is_err(), "remaining {field}");
        }
        for (field, value) in [
            (
                "other_mapping_paths",
                json!([{"root":bound::root(r),"key_types":["bytes32","address"],"words":1}]),
            ),
            ("deployment", json!({})),
            ("new_permission", json!(true)),
        ] {
            let mut c = selected.clone();
            c[i][field] = value;
            assert!(bound::verify_candidate(BASELINE, &c).is_err());
        }
    }
    let mut extra = selected.clone();
    extra.as_array_mut().unwrap().push(selected[0].clone());
    assert!(bound::verify_candidate(BASELINE, &extra).is_err());
    let mut swapped = selected.clone();
    swapped.as_array_mut().unwrap().swap(0, 1);
    assert!(bound::verify_candidate(BASELINE, &swapped).is_err());
    let mut raw = BASELINE.to_vec();
    raw.push(b' ');
    assert!(bound::candidate(&raw).is_err());
}
fn image(root: u64) -> Vec<u8> {
    let mut raw = vec![0xa9; 32];
    raw.extend(hex::decode(&bound::root(root)[2..]).unwrap());
    raw
}
fn block(address: &str, r: u64) -> eth::Block {
    let raw = image(r);
    let key = erc20_balances::hash(&raw);
    eth::Block {
        transaction_traces: vec![eth::TransactionTrace {
            status: eth::TransactionTraceStatus::Succeeded as i32,
            calls: vec![eth::Call {
                keccak_preimages: [(hex::encode(key), hex::encode(raw))].into_iter().collect(),
                storage_changes: vec![eth::StorageChange {
                    address: hex::decode(&address[2..]).unwrap(),
                    key: key.to_vec(),
                    old_value: vec![],
                    new_value: vec![1],
                    ordinal: 50,
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    }
}
#[test]
fn wkey_got_length_counter_binds_contract_to_its_own_root_and_persisted_change() {
    assert_eq!(bound::role_root("0x00"), None);
    for address in bound::CONTRACTS {
        let r = bound::role_root(address).unwrap();
        let b = block(address, r);
        let counts = bound::role_length_writes(&b).unwrap();
        assert_eq!(counts[address], 1);
        assert_eq!(counts.values().sum::<u64>(), 1);
        assert_eq!(
            bound::role_length_writes(&block(address, 17 - r)).unwrap()[address],
            0,
            "cross-target root must not count"
        );
        for mode in [
            "account",
            "equal",
            "key33",
            "old33",
            "new33",
            "missing",
            "wrongroot",
            "failed",
            "reverted",
            "admin",
            "index",
        ] {
            let mut c = b.clone();
            let tx = &mut c.transaction_traces[0];
            let call = &mut tx.calls[0];
            let w = &mut call.storage_changes[0];
            match mode {
                "account" => w.address = vec![9; 20],
                "equal" => w.old_value = vec![0, 1],
                "key33" => w.key.push(0),
                "old33" => w.old_value = vec![0; 33],
                "new33" => w.new_value = vec![0; 33],
                "missing" => call.keccak_preimages.clear(),
                "wrongroot" => {
                    let raw = image(17 - r);
                    w.key = erc20_balances::hash(&raw).to_vec();
                    call.keccak_preimages.insert(hex::encode(&w.key), hex::encode(raw));
                }
                "failed" => tx.status = eth::TransactionTraceStatus::Failed as i32,
                "reverted" => call.state_reverted = true,
                "admin" => {
                    let n = primitive_types::U256::from_big_endian(&w.key).overflowing_add(2.into()).0;
                    w.key = vec![0; 32];
                    n.to_big_endian(&mut w.key);
                }
                "index" => {
                    let n = primitive_types::U256::from_big_endian(&w.key).overflowing_add(1.into()).0;
                    let mut parent = [0; 32];
                    n.to_big_endian(&mut parent);
                    let mut raw = vec![0; 32];
                    raw[31] = 3;
                    raw.extend(parent);
                    w.key = erc20_balances::hash(&raw).to_vec();
                    call.keccak_preimages.insert(hex::encode(&w.key), hex::encode(raw));
                }
                _ => unreachable!(),
            }
            assert_eq!(bound::role_length_writes(&c).unwrap().values().sum::<u64>(), 0, "{mode}");
        }
        let mut malformed = b.clone();
        malformed.transaction_traces[0].calls[0]
            .keccak_preimages
            .values_mut()
            .for_each(|v| *v = "ff".repeat(64));
        assert!(bound::role_length_writes(&malformed).is_err());
        let mut removal = b.clone();
        let w = &mut removal.transaction_traces[0].calls[0].storage_changes[0];
        w.old_value = vec![0, 1];
        w.new_value = vec![];
        assert_eq!(bound::role_length_writes(&removal).unwrap()[address], 1);
        let mut system = b.clone();
        system.system_calls = system.transaction_traces.remove(0).calls;
        assert_eq!(bound::role_length_writes(&system).unwrap()[address], 1);
        system.system_calls[0].state_reverted = true;
        assert_eq!(bound::role_length_writes(&system).unwrap()[address], 0);
    }
}

#[test]
fn wkey_got_complete_frozen_proof_and_every_artifact_are_bound_before_review() {
    use std::{fs, path::Path};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let review = bound::review(root).unwrap();
    assert_eq!(review["phase_a"]["calls"], 1263);
    assert_eq!(review["phase_a"]["source_inputs"], 231);
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
    // Cache equality is a separate guard, independent of the copied proof fixtures.
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
fn wkey_got_measured_legacy_operations_have_one_length_witness_without_boolean_stages() {
    use substreams_ethereum::pb::eth::v2 as eth;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    bound::review(root).unwrap();
    let records: Vec<Value> =
        serde_json::from_slice(&std::fs::read(root.join("docs/evidence/wkey-got-operation-proof-20260929-transcripts.json")).unwrap()).unwrap();
    let candidates = bound::candidate(BASELINE).unwrap();
    let layouts = erc20_balances::layout::parse(&candidates.to_string()).unwrap();
    let decode = |v: &Value| hex::decode(v.as_str().unwrap().trim_start_matches("0x")).unwrap();
    for address in bound::CONTRACTS {
        let target = if address == bound::CONTRACTS[0] { "wkeydao" } else { "got" };
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
                .filter(|r| r["target"] == target && r["name"] == name && r["signature"] == sig && r["code_kind"] == "deployedBytecode")
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
