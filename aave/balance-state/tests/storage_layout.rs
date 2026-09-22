//! Pins the Aave fixture and the qualified BSC epochs to the compiler storage
//! layout of the pinned aave-v3-origin source
//! (`docs/evidence/storage-layouts/aave-v3-origin@8305565a.json`, solc 0.8.27):
//! `ATokenInstance` and `PoolInstance`. Host-only evidence check.
#![cfg(not(target_arch = "wasm32"))]
use aave_balance_state::{parse, RESERVE_CLOCK_OFFSET, RESERVE_INDEX_RATE_OFFSET, RESERVE_WORDS};
use serde_json::Value;
use std::collections::BTreeSet;

const LAYOUT: &str = include_str!("../../../docs/evidence/storage-layouts/aave-v3-origin@8305565a.json");
const FIXTURE: &str = include_str!("fixtures/bsc-aave-v3-epochs.json");
const QUALIFIED: &str = include_str!("../epochs/bsc-aave-v3.json");

fn layout(contract: &str) -> Value {
    let doc: Value = serde_json::from_str(LAYOUT).unwrap();
    let (_, l) = doc["contracts"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(k, _)| k.ends_with(&format!(":{contract}")))
        .unwrap_or_else(|| panic!("{contract} not in layout"));
    l.clone()
}
fn slot_of(l: &Value, label: &str) -> u64 {
    l["storage"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["label"] == label)
        .unwrap_or_else(|| panic!("{label}"))["slot"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}
fn word(slot: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..].copy_from_slice(&slot.to_be_bytes());
    w
}
fn member(l: &Value, type_prefix: &str, label: &str) -> (u64, u64, String) {
    let (_, t) = l["types"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(k, _)| k.starts_with(type_prefix) && k.ends_with("_storage"))
        .unwrap_or_else(|| panic!("{type_prefix}"));
    let m = t["members"].as_array().unwrap().iter().find(|m| m["label"] == label).unwrap();
    (
        m["slot"].as_str().unwrap().parse().unwrap(),
        m["offset"].as_u64().unwrap(),
        m["type"].as_str().unwrap().to_string(),
    )
}

#[test]
fn atoken_slots_and_the_basis_bits_match_the_compiled_layout() {
    let atoken = layout("ATokenInstance");
    for params in [FIXTURE, QUALIFIED] {
        let cfg = parse(params).unwrap();
        for m in &cfg.markets {
            assert_eq!(m.user_state_slot, word(slot_of(&atoken, "_userState")));
            assert_eq!(m.total_supply_slot, word(slot_of(&atoken, "_totalSupply")));
            // Allowances and permit nonces are written by routine calls
            // (`approve`, `permit` through routers) and must be reviewed.
            let reviewed: BTreeSet<[u8; 32]> = m.other_mapping_slots.iter().copied().collect();
            let expected: BTreeSet<[u8; 32]> = [word(slot_of(&atoken, "_allowances")), word(slot_of(&atoken, "_nonces"))].into();
            assert_eq!(reviewed, expected);
            assert_eq!(m.basis_bits, 120);
        }
    }
    // UserState packs `balance` (uint120) at offset 0; `additionalData` sits above.
    assert_eq!(member(&atoken, "t_struct(UserState)", "balance"), (0, 0, "t_uint120".into()));
    assert_eq!(member(&atoken, "t_struct(UserState)", "additionalData").1, 16);
    // Every mapping of the token is decoded or reviewed; the other words are
    // written only by `initialize`, which runs in an upgrade after the
    // pointer write that ends the epoch.
    let mappings: Vec<String> = atoken["storage"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["type"].as_str().unwrap().starts_with("t_mapping"))
        .map(|s| s["label"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(mappings, vec!["_userState", "_allowances", "_nonces"]);
}

#[test]
fn pool_reserve_words_match_the_compiled_layout() {
    let pool = layout("PoolInstance");
    for params in [FIXTURE, QUALIFIED] {
        assert_eq!(parse(params).unwrap().pool.unwrap().reserves_slot, word(slot_of(&pool, "_reserves")));
    }
    let reserve = |label: &str| member(&pool, "t_struct(ReserveData)", label);
    assert_eq!(reserve("liquidityIndex"), (u64::from(RESERVE_INDEX_RATE_OFFSET), 0, "t_uint128".into()));
    assert_eq!(reserve("currentLiquidityRate"), (u64::from(RESERVE_INDEX_RATE_OFFSET), 16, "t_uint128".into()));
    assert_eq!(reserve("lastUpdateTimestamp"), (u64::from(RESERVE_CLOCK_OFFSET), 16, "t_uint40".into()));
    let (last_word, _, _) = reserve("__deprecatedVirtualUnderlyingBalance");
    assert_eq!(last_word + 1, u64::from(RESERVE_WORDS));
}

#[test]
fn the_qualified_epoch_starts_after_the_pool_upgrade_write() {
    // Pool implementation 0x5e2B…3B6d was installed at block 101087794 by the
    // write at ordinal 3346; the aTokens have run 0x7e19…4134 since 76571348.
    let cfg = parse(QUALIFIED).unwrap();
    assert_eq!(cfg.producer_versions, vec![4, 5]);
    for m in &cfg.markets {
        assert_eq!((m.activation_block, m.activation_ordinal), (101_087_794, 3_347));
        assert_eq!(m.implementation_revision, "5");
    }
}
