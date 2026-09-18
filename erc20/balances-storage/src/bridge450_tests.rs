use super::*;
use prost::Message;
use serde_json::Value;

fn layouts() -> Vec<VerifiedLayout> {
    layout::parse(include_str!("../tests/fixtures/bridge450/layouts.json")).unwrap()
}

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!("../tests/fixtures/bridge450/cases.json")).unwrap()
}

fn captured(case: &Value) -> eth::Block {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bridge450");
    eth::Block::decode(std::fs::read(root.join(case["fixture"].as_str().unwrap())).unwrap().as_slice()).unwrap()
}

#[test]
fn bridge450_captured_blocks_match_independent_rpc() {
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
    assert_eq!(tokens.len(), 2);
    assert_eq!(checked, 7);
}

#[test]
fn bridge450_fixed_fields_and_allowances_preserve_the_independent_rpc_balance() {
    let fixture: Value = serde_json::from_str(include_str!("../tests/fixtures/bridge450/controls.json")).unwrap();
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
    assert_eq!(checked, 176);
}

#[test]
fn bridge450_raw_word_controls_match_rpc_including_null_address_filtering() {
    let fixture: Value = serde_json::from_str(include_str!("../tests/fixtures/bridge450/controls.json")).unwrap();
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
    assert_eq!(checked, 40);
    assert_eq!(excluded_null, 8);
}

#[test]
fn bridge450_unreviewed_mapping_writes_still_stop_processing() {
    let layouts = layouts();
    for case in cases() {
        let contract = hex_bytes(case["contract"].as_str().unwrap()).unwrap();
        let layout = layouts.iter().find(|l| l.contract == contract).unwrap();
        let mut block = captured(&case);
        let mut owner = [0; 64];
        owner[12..32].fill(0xaa);
        owner[63] = 13;
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
fn bridge450_unknown_fixed_fields_remain_guarded() {
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

fn controls() -> Value {
    serde_json::from_str(include_str!("../tests/fixtures/bridge450/controls.json")).unwrap()
}

fn metadata_block(token: &Value, control: &Value, key: Vec<u8>) -> eth::Block {
    let case = cases().into_iter().find(|c| c["contract"] == token["contract"]).unwrap();
    let mut block = captured(&case);
    let contract = hex_bytes(token["contract"].as_str().unwrap()).unwrap();
    block.system_calls.clear();
    block.transaction_traces = vec![eth::TransactionTrace {
        status: eth::TransactionTraceStatus::Succeeded as i32,
        calls: vec![eth::Call {
            address: contract.clone(),
            keccak_preimages: serde_json::from_value(control["preimages"].clone()).unwrap(),
            storage_changes: vec![eth::StorageChange {
                address: contract,
                key,
                old_value: vec![],
                new_value: vec![1],
                ordinal: 1,
            }],
            ..Default::default()
        }],
        ..Default::default()
    }];
    block
}

#[test]
fn bridge450_metadata_only_writes_emit_no_balances() {
    let fixture = controls();
    let configured = layouts();
    let mut checked = 0;
    for token in fixture["tokens"].as_array().unwrap() {
        let contract = hex_bytes(token["contract"].as_str().unwrap()).unwrap();
        let layout = configured.iter().find(|l| l.contract == contract).unwrap();
        for c in token["metadata_controls"].as_array().unwrap() {
            let block = metadata_block(token, c, hex_bytes(c["storage_key"].as_str().unwrap()).unwrap());
            assert!(project(&block, std::slice::from_ref(layout)).unwrap().balances.is_empty());
            checked += 1;
        }
    }
    assert_eq!(checked, 176);
}

#[test]
fn bridge450_field_getters_bind_each_independent_storage_override() {
    let fixture = controls();
    let mut checked = 0;
    let mut rate = 0;
    for token in fixture["tokens"].as_array().unwrap() {
        for c in token["metadata_controls"].as_array().unwrap().iter().filter(|c| c["field_call"].is_object()) {
            let key = c["storage_key"].as_str().unwrap();
            let bytes = hex_bytes(c["state_diff"][key].as_str().unwrap()).unwrap();
            let expected = if c["is_bool"] == true {
                BigInt::from(u32::from(bytes[31] != 0))
            } else {
                BigInt::from_unsigned_bytes_be(&bytes)
            };
            assert_eq!(expected.to_string(), c["field_value"].as_str().unwrap());
            let returned = hex_bytes(c["field_return"].as_str().unwrap()).unwrap();
            let offset = c["return_word"].as_u64().unwrap() as usize;
            if c["kind"].as_str().unwrap().starts_with("rate-") {
                assert_eq!(returned.len(), 128);
                rate += 1;
            } else {
                assert_eq!(returned.len(), 32);
            }
            assert_eq!(BigInt::from_unsigned_bytes_be(&returned[offset * 32..offset * 32 + 32]), expected);
            checked += 1;
        }
    }
    assert_eq!(checked, 80);
    assert_eq!(rate, 48);
}

#[test]
fn usde_rate_record_offsets_and_preimages_are_required() {
    let fixture = controls();
    let token = fixture["tokens"].as_array().unwrap().iter().find(|t| t["rank"] == 423).unwrap();
    let contract = hex_bytes(token["contract"].as_str().unwrap()).unwrap();
    let layout = layouts().into_iter().find(|l| l.contract == contract).unwrap();
    let root = word(&[11]).unwrap();
    assert_eq!(layout.other_mapping_words[&root], 4);
    let mut checked = BTreeSet::new();
    for c in token["metadata_controls"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"].as_str().unwrap().starts_with("rate-"))
    {
        let kind = c["kind"].as_str().unwrap();
        if !checked.insert(kind.to_owned()) {
            continue;
        }
        let offset = c["return_word"].as_u64().unwrap();
        let key = hex_bytes(c["storage_key"].as_str().unwrap()).unwrap();
        let base = BigInt::from_unsigned_bytes_be(&key) - BigInt::from(offset);
        let block = metadata_block(token, c, key);
        assert!(project(&block, std::slice::from_ref(&layout)).unwrap().balances.is_empty());
        let mut narrower = layout.clone();
        assert_eq!(narrower.other_mapping_words.remove(&root), Some(4));
        assert!(project(&block, &[narrower]).unwrap_err().to_string().contains("unresolved storage"));
        let outside = base + BigInt::from(4);
        let outside = metadata_block(token, c, word(&outside.to_bytes_be().1).unwrap().to_vec());
        assert!(project(&outside, std::slice::from_ref(&layout))
            .unwrap_err()
            .to_string()
            .contains("unresolved storage"));
        let mut absent = block.clone();
        absent.transaction_traces[0].calls[0].keccak_preimages.clear();
        assert!(project(&absent, std::slice::from_ref(&layout))
            .unwrap_err()
            .to_string()
            .contains("unresolved storage"));
        let mut corrupt = block;
        for p in corrupt.transaction_traces[0].calls[0].keccak_preimages.values_mut() {
            p.replace_range(0..2, "ab");
        }
        assert!(project(&corrupt, std::slice::from_ref(&layout)).is_err());
    }
    assert_eq!(checked.len(), 12);
}

#[test]
fn concilium_fixed_array_words_require_explicit_slots() {
    let fixture = controls();
    let token = fixture["tokens"].as_array().unwrap().iter().find(|t| t["rank"] == 440).unwrap();
    let contract = hex_bytes(token["contract"].as_str().unwrap()).unwrap();
    let layout = layouts().into_iter().find(|l| l.contract == contract).unwrap();
    for index in 15u8..=18 {
        let key = format!("0x{index:064x}");
        let c = token["metadata_controls"].as_array().unwrap().iter().find(|c| c["storage_key"] == key).unwrap();
        let slot = word(&[index]).unwrap();
        let block = metadata_block(token, c, slot.to_vec());
        assert!(project(&block, std::slice::from_ref(&layout)).unwrap().balances.is_empty());
        let mut missing = layout.clone();
        assert!(missing.other_slots.remove(&slot));
        assert!(project(&block, &[missing]).unwrap_err().to_string().contains("unresolved storage"));
        let outside = metadata_block(token, c, word(&[19]).unwrap().to_vec());
        assert!(project(&outside, std::slice::from_ref(&layout))
            .unwrap_err()
            .to_string()
            .contains("unresolved storage"));
    }
}

#[test]
fn unreviewed_usde_dynamic_option_payloads_still_stop_processing() {
    let fixture = controls();
    let token = fixture["tokens"].as_array().unwrap().iter().find(|t| t["rank"] == 423).unwrap();
    let contract = hex_bytes(token["contract"].as_str().unwrap()).unwrap();
    let layout = layouts().into_iter().find(|l| l.contract == contract).unwrap();
    let c = token["metadata_controls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["kind"] == "option-byte-head")
        .unwrap();
    let head = hex_bytes(c["storage_key"].as_str().unwrap()).unwrap();
    let payload = hash(&head);
    let mut block = metadata_block(token, c, payload.to_vec());
    block.transaction_traces[0].calls[0]
        .keccak_preimages
        .insert(hex::encode(payload), hex::encode(head));
    assert!(project(&block, &[layout]).unwrap_err().to_string().contains("unresolved storage"));
}
