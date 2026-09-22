//! Pins the fixture's cToken and rate-model slots to the compiler storage
//! layout of the pinned compound-protocol source
//! (`docs/evidence/storage-layouts/compound-v2@a3214f67.json`, `solc 0.8.10`).
//! Host-only evidence check; not part of the map.
#![cfg(not(target_arch = "wasm32"))]
use compound_v2_balance_state::{parse, Market};
use proto::pb::evm::balance_state::v1::StateField;
use serde_json::Value;
use std::collections::BTreeSet;

const LAYOUT: &str = include_str!("../../../docs/evidence/storage-layouts/compound-v2@a3214f67.json");
const FIXTURE: &str = include_str!("fixtures/mainnet-ctoken-epochs.json");
const FIAT_TOKEN: &str = include_str!("../../../docs/evidence/storage-layouts/fiat-token@v2.2.0.json");

fn slots(contract: &str) -> Vec<(String, u64)> {
    slots_in(LAYOUT, contract)
}
fn slots_in(layout: &str, contract: &str) -> Vec<(String, u64)> {
    let doc: Value = serde_json::from_str(layout).unwrap();
    let (_, l) = doc["contracts"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(k, _)| k.ends_with(&format!(":{contract}")))
        .unwrap_or_else(|| panic!("{contract} not in layout"));
    l["storage"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["label"].as_str().unwrap().to_string(), s["slot"].as_str().unwrap().parse().unwrap()))
        .collect()
}
fn word(slot: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..].copy_from_slice(&slot.to_be_bytes());
    w
}
fn slot_of(vars: &[(String, u64)], label: &str) -> [u8; 32] {
    word(vars.iter().find(|(l, _)| l == label).unwrap_or_else(|| panic!("{label} missing")).1)
}
fn check_market(m: &Market, contract: &str) {
    let vars = slots(contract);
    let expect = |field: StateField, label: &str| {
        let (slot, _, _) = m.scalars.iter().find(|(_, f, _)| *f == field).unwrap();
        assert_eq!(*slot, slot_of(&vars, label), "{contract}.{label}");
    };
    expect(StateField::CompoundV2InitialExchangeRateMantissa, "initialExchangeRateMantissa");
    expect(StateField::CompoundV2ReserveFactorMantissa, "reserveFactorMantissa");
    expect(StateField::CompoundV2AccrualBlockNumber, "accrualBlockNumber");
    expect(StateField::CompoundV2BorrowIndex, "borrowIndex");
    expect(StateField::CompoundV2TotalBorrows, "totalBorrows");
    expect(StateField::CompoundV2TotalReserves, "totalReserves");
    expect(StateField::CompoundV2TotalSupply, "totalSupply");
    assert_eq!(m.rate_model_slot, slot_of(&vars, "interestRateModel"));
    assert_eq!(m.account_tokens_slot, slot_of(&vars, "accountTokens"));
    let mappings: BTreeSet<[u8; 32]> = ["transferAllowances", "accountBorrows"].iter().map(|l| slot_of(&vars, l)).collect();
    assert_eq!(mappings, m.other_mapping_slots.iter().copied().collect());
    if let Some(slot) = m.implementation_slot {
        assert_eq!(slot, slot_of(&vars, "implementation"));
    }
    // Completeness: every compiled slot is a configured word, the holder
    // mapping, the pointer, or a reviewed slot / mapping; decimals and admin
    // share slot 3.
    let compiled: BTreeSet<[u8; 32]> = vars.iter().map(|(_, s)| word(*s)).collect();
    let mut covered: BTreeSet<[u8; 32]> = m.scalars.iter().map(|(s, _, _)| *s).collect();
    covered.insert(m.rate_model_slot);
    covered.insert(m.account_tokens_slot);
    covered.extend(m.implementation_slot);
    covered.extend(m.other_slots.iter().copied());
    covered.extend(m.other_mapping_slots.iter().copied());
    assert_eq!(compiled, covered, "{contract} slots not fully covered");
    assert_eq!(
        vars.iter().filter(|(_, s)| *s == 3).map(|(l, _)| l.as_str()).collect::<Vec<_>>(),
        vec!["decimals", "admin"]
    );
}

#[test]
fn ctoken_fixtures_match_the_compiled_layouts() {
    let cfg = parse(FIXTURE).unwrap();
    // cUSDC is a plain CErc20 and cETH a CEther at this pin.
    check_market(&cfg.markets[0], "CErc20");
    check_market(&cfg.markets[1], "CEther");
    // A delegator variant adds `implementation` at slot 18.
    let mut v: Value = serde_json::from_str(FIXTURE).unwrap();
    v["markets"][0]["kind"] = "cerc20_delegator".into();
    v["markets"][0]["implementation_slot"] = "0x0000000000000000000000000000000000000000000000000000000000000012".into();
    v["markets"][0]["implementation"] = "0x99ee778b9a6205657dd03b2b91415c8646d521ec".into();
    let delegator = parse(&v.to_string()).unwrap();
    check_market(&delegator.markets[0], "CErc20Delegator");
}

#[test]
fn jump_rate_model_slots_match_the_compiled_layout() {
    let cfg = parse(FIXTURE).unwrap();
    let vars = slots("JumpRateModelV2");
    let m = &cfg.markets[0];
    for (field, label) in [
        (StateField::CompoundV2IrmMultiplierPerBlock, "multiplierPerBlock"),
        (StateField::CompoundV2IrmBaseRatePerBlock, "baseRatePerBlock"),
        (StateField::CompoundV2IrmJumpMultiplierPerBlock, "jumpMultiplierPerBlock"),
        (StateField::CompoundV2IrmKink, "kink"),
    ] {
        let (slot, _) = m.rate_model_slots.iter().find(|(_, f)| *f == field).unwrap();
        assert_eq!(*slot, slot_of(&vars, label), "{label}");
    }
    // `owner` (slot 0) is the only other storage and is not a balance input.
    assert_eq!(
        vars.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(),
        vec!["owner", "multiplierPerBlock", "baseRatePerBlock", "jumpMultiplierPerBlock", "kink"]
    );
}

#[test]
fn usdc_cash_reads_the_low_255_bits_of_fiat_token_slot_9() {
    use compound_v2_balance_state::Cash;
    let cfg = parse(FIXTURE).unwrap();
    let vars = slots_in(FIAT_TOKEN, "FiatTokenV2_2");
    let Cash::Erc20Mapping { balances_slot, value_bits, .. } = cfg.markets[0].cash.clone() else {
        panic!()
    };
    // FiatTokenV1 declares `balanceAndBlacklistStates` at slot 9; V2.2 packs the
    // blacklist flag into bit 255 and `_balanceOf` masks it.
    assert_eq!(balances_slot, slot_of(&vars, "balanceAndBlacklistStates"));
    assert_eq!(balances_slot, word(9));
    assert_eq!(value_bits, 255);
    assert_eq!(slot_of(&vars, "allowed"), word(10));
    assert_eq!(slot_of(&vars, "totalSupply_"), word(11));
}
