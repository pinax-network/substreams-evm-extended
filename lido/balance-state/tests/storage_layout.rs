//! Pins the stETH fixture to the AST-derived layout of the pinned Lido source
//! (`docs/evidence/storage-layouts/lido-core@2da0f48f.json`, solc 0.4.24 has
//! no `--storage-layout`): the regular state variables across the linearized
//! Aragon/StETH inheritance chain, and every `*_POSITION` bytes32 constant
//! (unstructured storage) must be a configured or reviewed slot. Host-only
//! evidence check; not part of the map.
#![cfg(not(target_arch = "wasm32"))]
use lido_balance_state::parse;
use serde_json::Value;
use std::collections::BTreeSet;

const LAYOUT: &str = include_str!("../../../docs/evidence/storage-layouts/lido-core@2da0f48f.json");
const FIXTURE: &str = include_str!("fixtures/mainnet-steth-v4-epoch.json");

fn word(slot: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..].copy_from_slice(&slot.to_be_bytes());
    w
}

#[test]
fn regular_storage_of_the_linearized_lido_chain_matches_the_fixture() {
    let e = parse(FIXTURE).unwrap().epochs[0].clone();
    let doc: Value = serde_json::from_str(LAYOUT).unwrap();
    let storage: Vec<(String, u64, String)> = doc["storage"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["label"].as_str().unwrap().to_string(),
                s["slot"].as_str().unwrap().parse().unwrap(),
                s["contract"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    // Only StETH and StETHPermit declare regular state; every Aragon base uses
    // unstructured storage, so `shares` is slot 0.
    assert_eq!(
        storage,
        vec![
            ("shares".to_string(), 0, "StETH".to_string()),
            ("allowances".to_string(), 1, "StETH".to_string()),
            ("noncesByAddress".to_string(), 2, "StETHPermit".to_string()),
        ]
    );
    assert_eq!(e.shares_slot, word(0));
    assert_eq!(e.other_mapping_slots, vec![word(1), word(2)]);
    let chain: Vec<&str> = doc["linearized_base_to_derived"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(chain.first().copied(), Some("Versioned"));
    assert_eq!(chain.last().copied(), Some("Lido"));
    assert!(chain.contains(&"AragonApp") && chain.contains(&"StETH") && chain.contains(&"StETHPermit"));
}

#[test]
fn every_unstructured_position_constant_is_configured_or_reviewed() {
    let e = parse(FIXTURE).unwrap().epochs[0].clone();
    let doc: Value = serde_json::from_str(LAYOUT).unwrap();
    let mut configured: BTreeSet<[u8; 32]> = [
        e.total_and_external_shares_slot,
        e.buffered_slot,
        e.cl_slot,
        e.contract_version_slot,
        e.aragon.kernel_slot,
        e.aragon.app_id_slot,
    ]
    .into();
    configured.extend(e.other_slots.iter().copied());
    let mut positions = 0;
    for c in doc["bytes32_constants"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let value = c["value"].as_str().unwrap();
        if !name.ends_with("_POSITION") || !value.starts_with("0x") {
            continue; // role hashes, type hashes, namespaces and the alias are not storage
        }
        positions += 1;
        let slot: [u8; 32] = hex::decode(&value[2..]).unwrap().try_into().unwrap();
        assert!(configured.contains(&slot), "{name} ({value}) is neither configured nor reviewed");
    }
    assert_eq!(positions, 16);
    // The fixture's named slots hash to the compiled constants (spot checks).
    let by_name = |n: &str| -> [u8; 32] {
        let c = doc["bytes32_constants"].as_array().unwrap().iter().find(|c| c["name"] == n).unwrap();
        hex::decode(&c["value"].as_str().unwrap()[2..]).unwrap().try_into().unwrap()
    };
    assert_eq!(by_name("TOTAL_SHARES_POSITION_LOW128"), e.total_and_external_shares_slot);
    assert_eq!(by_name("CONTRACT_VERSION_POSITION"), e.contract_version_slot);
    assert_eq!(by_name("EIP712_STETH_POSITION"), lido_balance_state::keccak(b"lido.StETHPermit.eip712StETH"));
    assert_eq!(
        by_name("REENTRANCY_MUTEX_POSITION"),
        lido_balance_state::keccak(b"aragonOS.reentrancyGuard.mutex")
    );
}
