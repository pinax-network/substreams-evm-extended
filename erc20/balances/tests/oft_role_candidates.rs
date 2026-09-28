#![cfg(not(target_arch = "wasm32"))]
//! Source-derived Extended-record shapes, not executed role transactions or promotion.
//! Membership/admin permissions constrain locations, not authorization, values or deployment.
use erc20_balances::{hash, layout, project};
use std::collections::{BTreeSet, HashMap};
use substreams_ethereum::pb::eth::v2 as eth;

const CANDIDATES: &str = include_str!("fixtures/oft-role-candidates/layouts.json");
const BASELINE: &str = include_str!("fixtures/bsc-refined450-layouts.json");
const DEEP_ROLE_ROOT: &str = "02dd7bc7dec4dceedda775e58dd541e08a116c6c53815c0bd028192f7b626800";
// Captured OAppOptionsType3Upgradeable.sol, OAPP_OPTIONS_TYPE_3_STORAGE_LOCATION.
const DEEP_OPTIONS_ROOT: &str = "8d2bda5d9f6ffb5796910376005392955773acee5548d0fcdb10e7c264ea0000";
const DEEP_ADMINS: [(&str, &str); 3] = [
    ("ADMIN_ROLE", "b16e88c42fd4e48df2dd6a2eabd6bc9aec654ec170056b470819f8892cc6431d"),
    ("GUARDIAN_ROLE", "18476f5b3d6d00091ddd56161ac5e9ba807d29b59f48f8df98938ee352a7cf24"),
    ("BANLIST_OPERATOR_ROLE", "cc685bd430dd10cd13db9f52a68d39fc6e8fa7409ed69c8c1ed8aecaff3c8832"),
];

fn word(n: u64) -> [u8; 32] {
    let mut w = [0; 32];
    w[24..].copy_from_slice(&n.to_be_bytes());
    w
}
fn hex_word(s: &str) -> [u8; 32] {
    hex::decode(s.trim_start_matches("0x")).unwrap().try_into().unwrap()
}
fn member() -> [u8; 32] {
    let mut m = [0; 32];
    m[12..].fill(0x33);
    m
}
fn role_root(profile: usize) -> [u8; 32] {
    if profile == 0 {
        word(10)
    } else {
        hex_word(DEEP_ROLE_ROOT)
    }
}
fn nested(root: [u8; 32], keys: &[[u8; 32]]) -> ([u8; 32], HashMap<String, String>) {
    let mut slot = root;
    let mut hints = HashMap::new();
    for key in keys {
        let image = [key.as_slice(), slot.as_slice()].concat();
        slot = hash(&image);
        hints.insert(hex::encode(slot), hex::encode(image));
    }
    (slot, hints)
}
fn plus(mut key: [u8; 32]) -> [u8; 32] {
    for byte in key.iter_mut().rev() {
        let (value, carry) = byte.overflowing_add(1);
        *byte = value;
        if !carry {
            break;
        }
    }
    key
}
fn block(profile: usize, key: [u8; 32], hints: HashMap<String, String>, old: u8, new: u8) -> eth::Block {
    let contract = layout::parse(CANDIDATES).unwrap()[profile].contract.clone();
    eth::Block {
        ver: 5,
        number: 122288010,
        hash: vec![7; 32],
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        header: Some(eth::BlockHeader {
            number: 122288010,
            parent_hash: vec![6; 32],
            state_root: vec![8; 32],
            ..Default::default()
        }),
        transaction_traces: vec![eth::TransactionTrace {
            status: eth::TransactionTraceStatus::Succeeded as i32,
            calls: vec![eth::Call {
                address: contract.clone(),
                keccak_preimages: hints,
                storage_changes: vec![eth::StorageChange {
                    address: contract,
                    key: key.to_vec(),
                    old_value: vec![old],
                    new_value: vec![new],
                    ordinal: 1,
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn append(b: &mut eth::Block, key: [u8; 32], old: u8, new: u8, ordinal: u64) {
    let call = &mut b.transaction_traces[0].calls[0];
    call.storage_changes.push(eth::StorageChange {
        address: call.storage_changes[0].address.clone(),
        key: key.to_vec(),
        old_value: vec![old],
        new_value: vec![new],
        ordinal,
    });
}

#[test]
fn candidate_changes_only_membership_shape_and_three_deep_admin_locations() {
    let candidates: serde_json::Value = serde_json::from_str(CANDIDATES).unwrap();
    let baseline: serde_json::Value = serde_json::from_str(BASELINE).unwrap();
    assert_eq!(candidates.as_array().unwrap().len(), 2);
    for (profile, address) in ["0xf3d5b4c34ed623478cc5141861776e6cf7ae3a1e", "0x9b6a1d4fa5d90e5f2d34130053978d14cd301d58"]
        .into_iter()
        .enumerate()
    {
        let mut restored = candidates[profile].clone();
        assert_eq!(restored["contract"], address);
        let original = baseline.as_array().unwrap().iter().find(|p| p["contract"] == address).unwrap();
        assert_eq!(
            restored.as_object_mut().unwrap().remove("other_mapping_paths").unwrap(),
            serde_json::json!([{
                "root": format!("0x{}", hex::encode(role_root(profile))),
                "key_types": ["bytes32", "address"], "offset": 0, "words": 1
            }])
        );
        assert!(restored["other_mapping_words"].as_object().is_none_or(|words| words.is_empty()));
        restored["other_mapping_words"] = original["other_mapping_words"].clone();
        let old_slots: BTreeSet<_> = original["other_slots"].as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect();
        let slots: Vec<_> = restored["other_slots"].as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect();
        let new_slots: BTreeSet<_> = slots.iter().copied().collect();
        assert_eq!(slots.len(), new_slots.len(), "duplicate fixed slots");
        assert!(old_slots.is_subset(&new_slots));
        let additions: BTreeSet<_> = new_slots.difference(&old_slots).map(|s| (*s).to_owned()).collect();
        let expected = if profile == 0 {
            BTreeSet::new()
        } else {
            DEEP_ADMINS.iter().map(|(_, slot)| format!("0x{slot}")).collect()
        };
        assert_eq!(additions, expected);
        restored["other_slots"] = original["other_slots"].clone();
        assert_eq!(&restored, original, "all unrelated fields must restore exactly");
    }
    for candidate in layout::parse(CANDIDATES).unwrap() {
        assert!(candidate.deployment.is_none());
        assert!(candidate.enumerable_address_sets.is_empty());
        assert!(candidate.address_lists.is_empty());
    }
}

#[test]
fn membership_grant_revoke_noop_and_width_valid_metadata_preserve_balances() {
    let candidates = layout::parse(CANDIDATES).unwrap();
    let baseline = layout::parse(BASELINE).unwrap();
    for (profile, candidate) in candidates.iter().enumerate() {
        let original = baseline.iter().find(|p| p.contract == candidate.contract).unwrap();
        for role in [hash(b"ADMIN_ROLE"), hash(b"NEW_ROLE"), [0; 32], [0xff; 32]] {
            let (key, hints) = nested(role_root(profile), &[role, member()]);
            for (old, new) in [(0, 1), (1, 0), (0, 0), (1, 1), (2, 3)] {
                let b = block(profile, key, hints.clone(), old, new);
                let got = project(&b, &candidates).unwrap();
                assert!(got.balances.is_empty());
                assert_eq!(got, project(&b, std::slice::from_ref(original)).unwrap());
            }
            let mut b = block(profile, key, hints, 0, 1);
            b.transaction_traces[0].calls[0].storage_changes[0].new_value = vec![0xff; 32];
            assert!(project(&b, &candidates).unwrap().balances.is_empty(), "not a boolean-value validator");
            let (balance, balance_hints) = nested(candidate.balance_slot, &[member()]);
            b.transaction_traces[0].calls[0].keccak_preimages.extend(balance_hints);
            append(&mut b, balance, 0, 9, 2);
            let got = project(&b, &candidates).unwrap();
            assert_eq!(got, project(&b, std::slice::from_ref(original)).unwrap());
            assert_eq!(got.balances.len(), 1);
            assert_eq!(got.balances[0].amount, "9");
            assert_eq!(got.balances[0].address, member()[12..]);
            assert_eq!(got.balances[0].contract.as_deref(), Some(candidate.contract.as_slice()));
        }
    }
}

#[test]
fn deep_fixed_admins_are_exact_locations_without_value_or_initializer_checks() {
    let candidates = layout::parse(CANDIDATES).unwrap();
    let baseline = layout::parse(BASELINE).unwrap();
    let original = baseline.iter().find(|p| p.contract == candidates[1].contract).unwrap();
    for (role, fixed) in DEEP_ADMINS {
        let admin = hex_word(fixed);
        assert_eq!(plus(nested(role_root(1), &[hash(role.as_bytes())]).0), admin);
        let mut b = block(1, admin, HashMap::new(), 0, 1);
        b.transaction_traces[0].calls[0].storage_changes[0].new_value = hash(b"ARBITRARY_ADMIN_VALUE").to_vec();
        assert!(
            project(&b, std::slice::from_ref(original)).is_err(),
            "legacy baseline has no hint-free fixed admin permission"
        );
        assert!(project(&b, &candidates).unwrap().balances.is_empty());
        let (membership, hints) = nested(role_root(1), &[hash(role.as_bytes()), member()]);
        let (balance, balance_hints) = nested(candidates[1].balance_slot, &[member()]);
        b.transaction_traces[0].calls[0].keccak_preimages.extend(hints);
        b.transaction_traces[0].calls[0].keccak_preimages.extend(balance_hints);
        append(&mut b, membership, 0, 1, 2);
        append(&mut b, balance, 0, 9, 3);
        let got = project(&b, &candidates).unwrap();
        assert_eq!(got.balances.len(), 1);
        assert_eq!(got.balances[0].amount, "9");
        for (old, new) in [(0, 0), (1, 1), (1, 0), (2, 3)] {
            assert!(project(&block(1, admin, HashMap::new(), old, new), &candidates).unwrap().balances.is_empty());
        }
        assert!(project(&block(1, plus(admin), HashMap::new(), 0, 1), &candidates).is_err());
    }
}

#[test]
fn arbitrary_admin_anchor_adjacent_depth_order_padding_and_preimage_errors_refuse() {
    let candidates = layout::parse(CANDIDATES).unwrap();
    let baseline = layout::parse(BASELINE).unwrap();
    for (profile, candidate) in candidates.iter().enumerate() {
        let root = role_root(profile);
        let role = hash(b"UNLISTED_ROLE");
        let (key, hints) = nested(root, &[role, member()]);
        let (outer, outer_hints) = nested(root, &[role]);
        let mut padded = member();
        padded[0] = 1;
        for (name, (key, hints)) in [
            ("outer anchor / missing depth", (outer, outer_hints.clone())),
            ("arbitrary role admin", (plus(outer), outer_hints)),
            ("default admin", {
                let (key, hints) = nested(root, &[[0; 32]]);
                (plus(key), hints)
            }),
            ("member+1", (plus(key), hints.clone())),
            ("extra depth", nested(root, &[role, member(), member()])),
            ("wrong root", nested(word(999), &[role, member()])),
            ("key order", nested(root, &[member(), role])),
            ("address padding", nested(root, &[role, padded])),
            ("missing all hints", (key, HashMap::new())),
        ] {
            assert!(project(&block(profile, key, hints, 0, 1), &candidates).is_err(), "profile {profile}: {name}");
        }
        for missing_key in [key, outer] {
            let mut missing = hints.clone();
            missing.remove(&hex::encode(missing_key));
            assert!(project(&block(profile, key, missing, 0, 1), &candidates).is_err());
        }
        for image in [vec![0; 64], vec![0; 65]] {
            let mut corrupt = hints.clone();
            corrupt.insert(hex::encode(key), hex::encode(image));
            assert!(project(&block(profile, key, corrupt, 0, 1), &candidates).is_err());
        }
        // Even a correctly hashed 65-byte preimage is not a two-word mapping path.
        let image = [member().as_slice(), outer.as_slice(), &[0]].concat();
        let oversized_key = hash(&image);
        let mut oversized = hints.clone();
        oversized.insert(hex::encode(oversized_key), hex::encode(image));
        assert!(project(&block(profile, oversized_key, oversized, 0, 1), &candidates).is_err());
        let original = baseline.iter().find(|p| p.contract == candidate.contract).unwrap();
        assert!(
            project(&block(profile, plus(key), hints, 0, 1), std::slice::from_ref(original)).is_ok(),
            "preserved historical width-two over-admission"
        );
        if profile == 0 {
            for role in ["ADMIN_ROLE", "GUARDIAN_ROLE", "BANLIST_OPERATOR_ROLE"] {
                let (key, hints) = nested(root, &[hash(role.as_bytes())]);
                assert!(
                    project(&block(profile, plus(key), hints, 0, 1), &candidates).is_err(),
                    "Kgen admits no admin words"
                );
            }
        }
    }
}

#[test]
fn every_existing_scalar_and_non_role_mapping_keeps_baseline_treatment() {
    let candidates = layout::parse(CANDIDATES).unwrap();
    let baseline = layout::parse(BASELINE).unwrap();
    for (profile, candidate) in candidates.iter().enumerate() {
        let original = baseline.iter().find(|p| p.contract == candidate.contract).unwrap();
        for (key, hints) in original.other_slots.iter().map(|key| (*key, HashMap::new())).chain(
            original
                .other_mapping_slots
                .iter()
                .flat_map(|root| [nested(*root, &[member()]), nested(*root, &[member(), word(4)])]),
        ) {
            let b = block(profile, key, hints, 0, 1);
            let got = project(&b, &candidates).unwrap();
            assert!(got.balances.is_empty());
            assert_eq!(got, project(&b, std::slice::from_ref(original)).unwrap());
        }
        let mut empty = block(profile, word(0), HashMap::new(), 0, 0);
        empty.transaction_traces.clear();
        assert!(project(&empty, &candidates).unwrap().balances.is_empty(), "no deployment or holder seed");
    }
}

#[test]
fn forwarder_array_elements_and_long_option_bytes_remain_unsupported() {
    let candidates = layout::parse(CANDIDATES).unwrap();
    let baseline: Vec<_> = layout::parse(BASELINE)
        .unwrap()
        .into_iter()
        .filter(|profile| candidates.iter().any(|candidate| candidate.contract == profile.contract))
        .collect();
    let (position, mut hints) = nested(word(16), &[member()]);
    let array = hash(&word(15));
    hints.insert(hex::encode(array), hex::encode(word(15)));
    let mut b = block(0, word(15), hints, 0, 1);
    append(&mut b, position, 0, 1, 2);
    assert!(
        project(&b, &candidates).unwrap().balances.is_empty(),
        "existing length/index permissions remain"
    );
    for key in [array, plus(array)] {
        let mut unsupported = b.clone();
        append(&mut unsupported, key, 0, 1, 3);
        assert!(project(&unsupported, &candidates).is_err(), "forwarder array elements were not admitted");
        assert!(project(&unsupported, &baseline).is_err());
    }
    for (profile, root) in [(0, word(3)), (1, hex_word(DEEP_OPTIONS_ROOT))] {
        let (anchor, mut hints) = nested(root, &[word(30101), word(1)]);
        let data = hash(&anchor);
        hints.insert(hex::encode(data), hex::encode(anchor));
        let b = block(profile, anchor, hints, 0, 65); // Solidity long bytes: length 32 encoded as 2*n+1.
        assert!(
            project(&b, &candidates).unwrap().balances.is_empty(),
            "ordinary nested option anchor remains metadata"
        );
        assert_eq!(project(&b, &candidates).unwrap(), project(&b, &baseline).unwrap());
        for key in [data, plus(data)] {
            let mut unsupported = b.clone();
            append(&mut unsupported, key, 0, 1, 2);
            assert!(project(&unsupported, &candidates).is_err(), "long option data remains unqualified");
            assert!(project(&unsupported, &baseline).is_err());
        }
    }
}

#[test]
fn failed_reverted_and_ordinary_noop_records_keep_persisted_effect_rules() {
    let candidates = layout::parse(CANDIDATES).unwrap();
    for profile in 0..candidates.len() {
        for key in [word(999), nested(role_root(profile), &[hash(b"ADMIN_ROLE"), member()]).0] {
            let mut reverted = block(profile, key, HashMap::new(), 0, 1);
            reverted.transaction_traces[0].calls[0].state_reverted = true;
            assert!(project(&reverted, &candidates).unwrap().balances.is_empty());
            let mut failed = block(profile, key, HashMap::new(), 0, 1);
            failed.transaction_traces[0].status = eth::TransactionTraceStatus::Failed as i32;
            assert!(project(&failed, &candidates).unwrap().balances.is_empty());
            assert!(project(&block(profile, key, HashMap::new(), 1, 1), &candidates).unwrap().balances.is_empty());
        }
    }
}

#[test]
fn malformed_mapping_scalar_and_noop_words_never_become_valid_metadata() {
    let candidates = layout::parse(CANDIDATES).unwrap();
    for (profile, candidate) in candidates.iter().enumerate() {
        let membership = nested(role_root(profile), &[hash(b"ADMIN_ROLE"), member()]);
        let mut locations = vec![membership, (*candidate.other_slots.iter().next().unwrap(), HashMap::new())];
        if profile == 1 {
            locations.push((hex_word(DEEP_ADMINS[0].1), HashMap::new()));
        }
        for (key, hints) in locations {
            for field in ["old", "new", "key", "noop-values", "noop-key"] {
                let mut b = block(profile, key, hints.clone(), 0, 1);
                let change = &mut b.transaction_traces[0].calls[0].storage_changes[0];
                match field {
                    "old" => change.old_value = vec![0; 33],
                    "new" => change.new_value = vec![0; 33],
                    "key" => change.key = vec![0; 33],
                    "noop-values" => {
                        change.old_value = vec![0; 33];
                        change.new_value = vec![0; 33];
                    }
                    _ => {
                        change.key = vec![0; 33];
                        change.old_value = vec![1];
                        change.new_value = vec![1];
                    }
                }
                assert!(project(&b, &candidates).is_err(), "profile {profile}: malformed {field}");
            }
        }
    }
}

#[test]
fn deep_pointer_changes_and_restoration_still_refuse() {
    let candidates = layout::parse(CANDIDATES).unwrap();
    let proxy = candidates[1].proxy.as_ref().unwrap();
    for restoration in [false, true] {
        let mut b = block(1, proxy.implementation_slot, HashMap::new(), 0, 1);
        let first = &mut b.transaction_traces[0].calls[0].storage_changes[0];
        first.old_value = proxy.implementation.clone();
        first.new_value = vec![0x44; 20];
        if restoration {
            let mut restored = first.clone();
            restored.old_value = first.new_value.clone();
            restored.new_value = proxy.implementation.clone();
            restored.ordinal = 2;
            b.transaction_traces[0].calls[0].storage_changes.push(restored);
        }
        assert!(project(&b, &candidates).is_err());
        let mut reverted = b.clone();
        reverted.transaction_traces[0].calls[0].state_reverted = true;
        assert!(project(&reverted, &candidates).unwrap().balances.is_empty());
        b.transaction_traces[0].status = eth::TransactionTraceStatus::Failed as i32;
        assert!(project(&b, &candidates).unwrap().balances.is_empty());
    }
    let mut initialization = block(1, proxy.implementation_slot, HashMap::new(), 0, 1);
    initialization.transaction_traces[0].calls[0].storage_changes[0].new_value = proxy.implementation.clone();
    assert!(
        project(&initialization, &candidates).is_err(),
        "captured implementation identity does not qualify pointer initialization"
    );
}

#[test]
fn all_three_captured_creations_and_changed_runtimes_remain_unqualified() {
    let candidates = layout::parse(CANDIDATES).unwrap();
    let proxy = candidates[1].proxy.as_ref().unwrap();
    for (profile, address, expected_hash, fixture, height) in [
        (
            0,
            &candidates[0].contract,
            candidates[0].code_hash,
            include_str!("fixtures/oft-role-candidates/kgen.json"),
            59625960,
        ),
        (
            1,
            &proxy.implementation,
            proxy.code_hash,
            include_str!("fixtures/oft-role-candidates/deep.json"),
            73719448,
        ),
        (
            1,
            &candidates[1].contract,
            candidates[1].code_hash,
            include_str!("fixtures/oft-role-candidates/proxy.json"),
            73700424,
        ),
    ] {
        let capture: serde_json::Value = serde_json::from_str(fixture).unwrap();
        let runtime = hex::decode(capture["runtimeBytecode"]["onchainBytecode"].as_str().unwrap().trim_start_matches("0x")).unwrap();
        assert_eq!(hash(&runtime), expected_hash);
        for (old_code, new_code) in [(runtime.clone(), vec![1]), (Vec::new(), runtime)] {
            let (key, hints) = nested(role_root(profile), &[hash(b"ADMIN_ROLE"), member()]);
            let mut b = block(profile, key, hints, 0, 1);
            b.number = height;
            b.header.as_mut().unwrap().number = height;
            let call = &mut b.transaction_traces[0].calls[0];
            call.address = address.clone();
            call.call_type = if old_code.is_empty() { eth::CallType::Create } else { eth::CallType::Call } as i32;
            call.code_changes.push(eth::CodeChange {
                address: address.clone(),
                old_hash: hash(&old_code).to_vec(),
                new_hash: hash(&new_code).to_vec(),
                old_code,
                new_code,
                ordinal: 2,
            });
            assert!(project(&b, &candidates).is_err(), "profile {profile}: creation/runtime remains guarded");
            let mut reverted = b.clone();
            reverted.transaction_traces[0].calls[0].state_reverted = true;
            assert!(project(&reverted, &candidates).unwrap().balances.is_empty());
            b.transaction_traces[0].status = eth::TransactionTraceStatus::Failed as i32;
            assert!(project(&b, &candidates).unwrap().balances.is_empty());
        }
    }
}
