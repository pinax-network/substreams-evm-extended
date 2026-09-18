use super::*;
use prost::Message;
use serde_json::Value;

fn layouts() -> Vec<VerifiedLayout> {
    layout::parse(include_str!("../tests/fixtures/trade450/layouts.json")).unwrap()
}

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!("../tests/fixtures/trade450/cases.json")).unwrap()
}

fn captured(case: &Value) -> eth::Block {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/trade450");
    eth::Block::decode(std::fs::read(root.join(case["fixture"].as_str().unwrap())).unwrap().as_slice()).unwrap()
}

#[test]
fn trade450_captured_blocks_match_independent_rpc() {
    let layouts = layouts();
    let mut tokens = BTreeSet::new();
    let mut checked = 0;
    for case in cases() {
        let contract = hex_bytes(case["contract"].as_str().unwrap()).unwrap();
        let layout = layouts.iter().find(|l| l.contract == contract).unwrap();
        let block = captured(&case);
        assert_eq!(block.number, case["block"].as_u64().unwrap());
        assert_eq!(block.hash, hex_bytes(case["hash"].as_str().unwrap()).unwrap());
        let expected = case["balances"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (hex_bytes(r["address"].as_str().unwrap()).unwrap(), r["rpc"].as_str().unwrap().to_owned()))
            .collect::<BTreeMap<_, _>>();
        let events = project(&block, std::slice::from_ref(layout)).unwrap();
        assert_eq!(events.balances.len(), expected.len());
        assert!(events.balances.iter().all(|r| r.contract.as_ref() == Some(&contract)));
        checked += expected.len();
        assert_eq!(events.balances.into_iter().map(|r| (r.address, r.amount)).collect::<BTreeMap<_, _>>(), expected);
        assert!(tokens.insert(contract));
    }
    assert_eq!(tokens.len(), 3);
    assert_eq!(checked, 9);
}

#[test]
fn trade450_fixed_fields_allowances_and_packed_controls_preserve_the_independent_rpc_balance() {
    let fixture: Value = serde_json::from_str(include_str!("../tests/fixtures/trade450/controls.json")).unwrap();
    let layouts = layouts();
    let mut checked = 0;
    for token in fixture["tokens"].as_array().unwrap() {
        let contract = hex_bytes(token["contract"].as_str().unwrap()).unwrap();
        let holder = hex_bytes(token["holder"].as_str().unwrap()).unwrap();
        let layout = layouts.iter().find(|l| l.contract == contract).unwrap();
        let mut preimage = [0; 64];
        preimage[12..32].copy_from_slice(&holder);
        preimage[32..].copy_from_slice(&layout.balance_slot);
        for control in token["metadata_controls"].as_array().unwrap() {
            let mut preimages: std::collections::HashMap<String, String> = serde_json::from_value(control["preimages"].clone()).unwrap();
            preimages.insert(hex::encode(hash(&preimage)), hex::encode(preimage));
            let changes = control["state_diff"]
                .as_object()
                .unwrap()
                .iter()
                .enumerate()
                .map(|(i, (key, value))| {
                    let new_value = hex_bytes(value.as_str().unwrap()).unwrap();
                    eth::StorageChange {
                        address: contract.clone(),
                        key: hex_bytes(key).unwrap(),
                        old_value: if new_value.iter().all(|b| *b == 0) { vec![1] } else { vec![] },
                        new_value,
                        ordinal: i as u64 + 1,
                    }
                })
                .collect();
            let number = token["block"].as_u64().unwrap();
            let block = eth::Block {
                ver: 5,
                number,
                hash: hex_bytes(token["hash"].as_str().unwrap()).unwrap(),
                detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
                header: Some(eth::BlockHeader {
                    number,
                    parent_hash: vec![2; 32],
                    state_root: vec![3; 32],
                    ..Default::default()
                }),
                transaction_traces: vec![eth::TransactionTrace {
                    status: eth::TransactionTraceStatus::Succeeded as i32,
                    calls: vec![eth::Call {
                        address: contract.clone(),
                        storage_changes: changes,
                        keccak_preimages: preimages,
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            };
            let events = project(&block, std::slice::from_ref(layout)).unwrap();
            assert_eq!(events.balances.len(), 1, "{}", control["name"]);
            assert_eq!(events.balances[0].contract.as_ref(), Some(&contract));
            assert_eq!(events.balances[0].address, holder);
            assert_eq!(events.balances[0].amount, control["balance_of"].as_str().unwrap());
            checked += 1;
        }
    }
    assert_eq!(checked, 164);
}

#[test]
fn trade450_raw_word_controls_match_rpc_including_null_address_filtering() {
    let fixture: Value = serde_json::from_str(include_str!("../tests/fixtures/trade450/controls.json")).unwrap();
    let layouts = layouts();
    let mut checked = 0;
    let mut excluded_null = 0;
    for token in fixture["tokens"].as_array().unwrap() {
        let contract = hex_bytes(token["contract"].as_str().unwrap()).unwrap();
        let layout = layouts.iter().find(|l| l.contract == contract).unwrap();
        for control in token["raw_controls"].as_array().unwrap() {
            let holder = hex_bytes(control["address"].as_str().unwrap()).unwrap();
            let mut preimage = [0; 64];
            preimage[12..32].copy_from_slice(&holder);
            preimage[32..].copy_from_slice(&layout.balance_slot);
            let key = hash(&preimage);
            assert_eq!(key.to_vec(), hex_bytes(control["storage_key"].as_str().unwrap()).unwrap());
            let raw: BigInt = control["raw"].as_str().unwrap().parse().unwrap();
            assert_eq!(control["state_diff"].as_object().unwrap().len(), 1);
            let sent = hex_bytes(control["state_diff"][control["storage_key"].as_str().unwrap()].as_str().unwrap()).unwrap();
            assert_eq!(BigInt::from_unsigned_bytes_be(&sent), raw);
            let number = token["block"].as_u64().unwrap();
            let block = eth::Block {
                ver: 5,
                number,
                hash: hex_bytes(token["hash"].as_str().unwrap()).unwrap(),
                detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
                header: Some(eth::BlockHeader {
                    number,
                    parent_hash: vec![2; 32],
                    state_root: vec![3; 32],
                    ..Default::default()
                }),
                transaction_traces: vec![eth::TransactionTrace {
                    status: eth::TransactionTraceStatus::Succeeded as i32,
                    calls: vec![eth::Call {
                        address: contract.clone(),
                        storage_changes: vec![eth::StorageChange {
                            address: contract.clone(),
                            key: key.to_vec(),
                            old_value: if raw == BigInt::zero() { vec![1] } else { vec![] },
                            new_value: raw.to_bytes_be().1,
                            ordinal: 1,
                        }],
                        keccak_preimages: std::collections::HashMap::from([(hex::encode(key), hex::encode(preimage))]),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            };
            let raw_changes = changes(&block, std::slice::from_ref(layout)).unwrap();
            assert_eq!(raw_changes.len(), 1);
            assert_eq!(raw_changes[0].address, holder);
            assert_eq!(raw_changes[0].amount, control["rpc"].as_str().unwrap());
            let events = project(&block, std::slice::from_ref(layout)).unwrap();
            if holder.iter().all(|b| *b == 0) {
                assert!(events.balances.is_empty());
                excluded_null += 1;
            } else {
                assert_eq!(events.balances.len(), 1);
                assert_eq!(events.balances[0].contract.as_ref(), Some(&contract));
                assert_eq!(events.balances[0].address, holder);
                assert_eq!(events.balances[0].amount, control["rpc"].as_str().unwrap());
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 60);
    assert_eq!(excluded_null, 12);
}

#[test]
fn trade450_unreviewed_mapping_writes_still_stop_processing() {
    let layouts = layouts();
    for case in cases() {
        let contract = hex_bytes(case["contract"].as_str().unwrap()).unwrap();
        let layout = layouts.iter().find(|l| l.contract == contract).unwrap();
        let mut block = captured(&case);
        let mut owner = [0; 64];
        owner[12..32].fill(0xaa);
        owner[63] = 99;
        let mut spender = [0; 64];
        spender[12..32].fill(0xbb);
        spender[32..].copy_from_slice(&hash(&owner));
        let key = hash(&spender);
        block.transaction_traces.push(eth::TransactionTrace {
            status: eth::TransactionTraceStatus::Succeeded as i32,
            calls: vec![eth::Call {
                address: contract.clone(),
                storage_changes: vec![eth::StorageChange {
                    address: contract.clone(),
                    key: key.to_vec(),
                    old_value: vec![],
                    new_value: vec![1],
                    ordinal: 1_000_000,
                }],
                keccak_preimages: std::collections::HashMap::from([(hex::encode(hash(&owner)), hex::encode(owner)), (hex::encode(key), hex::encode(spender))]),
                ..Default::default()
            }],
            ..Default::default()
        });
        assert!(project(&block, std::slice::from_ref(layout))
            .unwrap_err()
            .to_string()
            .contains("unresolved storage"));
    }
}

#[test]
fn trade450_unknown_fixed_fields_remain_guarded() {
    let configured = layouts();
    for case in cases() {
        let contract = hex_bytes(case["contract"].as_str().unwrap()).unwrap();
        let layout = configured.iter().find(|l| l.contract == contract).unwrap();
        let mut block = captured(&case);
        block.transaction_traces.push(eth::TransactionTrace {
            status: eth::TransactionTraceStatus::Succeeded as i32,
            calls: vec![eth::Call {
                address: contract.clone(),
                storage_changes: vec![eth::StorageChange {
                    address: contract.clone(),
                    key: word(&[99]).unwrap().to_vec(),
                    new_value: vec![1],
                    ordinal: 1_000_000,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        });
        assert!(project(&block, std::slice::from_ref(layout))
            .unwrap_err()
            .to_string()
            .contains("unresolved storage"));
    }
}

fn synthetic(contract: &[u8], storage_changes: Vec<eth::StorageChange>, preimages: std::collections::HashMap<String, String>) -> eth::Block {
    eth::Block {
        ver: 5,
        number: 122288793,
        hash: vec![1; 32],
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        header: Some(eth::BlockHeader {
            number: 122288793,
            parent_hash: vec![2; 32],
            state_root: vec![3; 32],
            ..Default::default()
        }),
        transaction_traces: vec![eth::TransactionTrace {
            status: eth::TransactionTraceStatus::Succeeded as i32,
            calls: vec![eth::Call {
                address: contract.to_vec(),
                storage_changes,
                keccak_preimages: preimages,
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn trade450_metadata_without_holder_changes_and_restorations_emit_nothing() {
    let fixture: Value = serde_json::from_str(include_str!("../tests/fixtures/trade450/controls.json")).unwrap();
    let layouts = layouts();
    let mut checked = 0;
    for token in fixture["tokens"].as_array().unwrap() {
        let contract = hex_bytes(token["contract"].as_str().unwrap()).unwrap();
        let l = layouts.iter().find(|l| l.contract == contract).unwrap();
        for control in token["metadata_controls"].as_array().unwrap() {
            let key = hex_bytes(control["storage_key"].as_str().unwrap()).unwrap();
            let new = hex_bytes(control["state_diff"][control["storage_key"].as_str().unwrap()].as_str().unwrap()).unwrap();
            let old = if new.iter().all(|b| *b == 0) { vec![1] } else { vec![] };
            let first = eth::StorageChange {
                address: contract.clone(),
                key: key.clone(),
                old_value: old.clone(),
                new_value: new.clone(),
                ordinal: 1,
            };
            let preimages = serde_json::from_value(control["preimages"].clone()).unwrap();
            let mut b = synthetic(&contract, vec![first], preimages);
            assert!(project(&b, std::slice::from_ref(l)).unwrap().balances.is_empty());
            b.transaction_traces[0].calls[0].storage_changes.push(eth::StorageChange {
                address: contract.clone(),
                key,
                old_value: new,
                new_value: old,
                ordinal: 2,
            });
            assert!(project(&b, std::slice::from_ref(l)).unwrap().balances.is_empty());
            checked += 1;
        }
    }
    assert_eq!(checked, 164);
}

#[test]
fn trade450_labubu_original_reentrancy_refusals_match_rpc_with_explicit_slot6() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("../tests/fixtures/trade450/persisted-cases.json")).unwrap();
    assert_eq!(cases.len(), 2);
    let layouts = layouts();
    let l = layouts
        .iter()
        .find(|l| hex::encode(&l.contract) == "3494dfe19b721dac6c5c8d7470c8f89548177777")
        .unwrap();
    let mut guarded = 0;
    for case in cases {
        let block = captured(&case);
        let expected = case["balances"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (hex_bytes(r["address"].as_str().unwrap()).unwrap(), r["rpc"].as_str().unwrap().to_owned()))
            .collect::<BTreeMap<_, _>>();
        let events = project(&block, std::slice::from_ref(l)).unwrap();
        assert_eq!(events.balances.into_iter().map(|r| (r.address, r.amount)).collect::<BTreeMap<_, _>>(), expected);
        let mut narrow = l.clone();
        narrow.other_slots.retain(|s| *s != word(&[6]).unwrap());
        assert!(project(&block, &[narrow]).unwrap_err().to_string().contains("unresolved storage"));
        let guard = block
            .transaction_traces
            .iter()
            .flat_map(|t| &t.calls)
            .filter(|c| !c.state_reverted)
            .flat_map(|c| &c.storage_changes)
            .filter(|s| s.address == l.contract && word(&s.key).unwrap() == word(&[6]).unwrap())
            .collect::<Vec<_>>();
        assert!(!guard.is_empty());
        for change in guard {
            let old = BigInt::from_unsigned_bytes_be(&change.old_value);
            let new = BigInt::from_unsigned_bytes_be(&change.new_value);
            assert!((old == 1 && new == 2) || (old == 2 && new == 1));
            guarded += 1;
        }
    }
    assert_eq!(guarded, 4);
}

#[test]
fn trade450_fixed_array_words_require_each_explicit_word_and_dynamic_payloads_still_reject() {
    let configured = layouts();
    let l = configured
        .iter()
        .find(|l| hex::encode(&l.contract) == "3494dfe19b721dac6c5c8d7470c8f89548177777")
        .unwrap();
    for slot in 8u8..=25 {
        let key = word(&[slot]).unwrap();
        assert!(l.other_slots.contains(&key));
        let b = synthetic(
            &l.contract,
            vec![eth::StorageChange {
                address: l.contract.clone(),
                key: key.to_vec(),
                new_value: vec![1],
                ordinal: 1,
                ..Default::default()
            }],
            Default::default(),
        );
        assert!(project(&b, std::slice::from_ref(l)).unwrap().balances.is_empty());
        let mut missing = l.clone();
        missing.other_slots.retain(|s| *s != key);
        assert!(project(&b, &[missing]).unwrap_err().to_string().contains("unresolved storage"));
    }
    for l in &configured {
        for head in [3u8, 4] {
            let head = word(&[head]).unwrap();
            let key = hash(&head);
            let b = synthetic(
                &l.contract,
                vec![eth::StorageChange {
                    address: l.contract.clone(),
                    key: key.to_vec(),
                    new_value: vec![1],
                    ordinal: 1,
                    ..Default::default()
                }],
                std::collections::HashMap::from([(hex::encode(key), hex::encode(head))]),
            );
            assert!(project(&b, std::slice::from_ref(l)).unwrap_err().to_string().contains("unresolved storage"));
        }
    }
}

#[test]
fn trade450_mixed_packed_and_fixed_array_field_controls_match_independent_rpc_words() {
    let fixture: Value = serde_json::from_str(include_str!("../tests/fixtures/trade450/controls.json")).unwrap();
    let mut packed = 0;
    let mut arrays = 0;
    let mut fields = 0;
    for token in fixture["tokens"].as_array().unwrap() {
        for c in token["metadata_controls"].as_array().unwrap() {
            if c["kind"] == "packed" {
                packed += 1;
            }
            if c["declarations"][0]["array"] == "hourlyPrices" {
                arrays += 1;
            }
            let raw = hex_bytes(c["state_diff"][c["storage_key"].as_str().unwrap()].as_str().unwrap()).unwrap();
            for f in c["field_calls"].as_array().unwrap() {
                let offset = f["offset"].as_u64().unwrap() as usize;
                let ty = f["output_type"].as_str().unwrap();
                let size = if ty == "address" {
                    20
                } else if ty == "bool" {
                    1
                } else {
                    ty.strip_prefix("uint").unwrap().parse::<usize>().unwrap() / 8
                };
                let mut decoded = BigInt::from_unsigned_bytes_be(&raw[32 - offset - size..32 - offset]);
                if ty == "bool" {
                    decoded = BigInt::from(u32::from(decoded != BigInt::zero()));
                }
                assert_eq!(decoded.to_string(), f["expected"].as_str().unwrap());
                let response = hex_bytes(f["raw_response"].as_str().unwrap()).unwrap();
                let index = f["output_index"].as_u64().unwrap() as usize;
                assert_eq!(BigInt::from_unsigned_bytes_be(&response[index * 32..(index + 1) * 32]), decoded);
                fields += 1;
            }
        }
    }
    assert_eq!(packed, 6);
    assert_eq!(arrays, 36);
    assert_eq!(fields, 150);
}
