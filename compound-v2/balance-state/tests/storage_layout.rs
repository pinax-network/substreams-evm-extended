//! Pins the fixture's cToken and rate-model slots to compiler storage
//! layouts. The deployed cUSDC and cETH run the 2019 source
//! (`compound-protocol-2019@f385d719.json`, solc 0.5.17: `ReentrancyGuard`'s
//! `_guardCounter` at slot 0, `uint decimals`), cUSDC's rate model is the
//! 2020 `LegacyJumpRateModelV2` (`compound-protocol-legacy-jump@4caf72a1.json`)
//! and cETH's the 2019 `WhitePaperInterestRateModel`. Delegator markets of the
//! current tree (such as cDAI) use `compound-v2@a3214f67.json` (solc 0.8.10).
//! Host-only evidence check; not part of the map.
#![cfg(not(target_arch = "wasm32"))]
use compound_v2_balance_state::{parse, Cash, Market};
use proto::pb::evm::balance_state::v1::StateField;
use serde_json::Value;
use std::collections::BTreeSet;

const CURRENT: &str = include_str!("../../../docs/evidence/storage-layouts/compound-v2@a3214f67.json");
const LEGACY_2019: &str = include_str!("../../../docs/evidence/storage-layouts/compound-protocol-2019@f385d719.json");
const LEGACY_JUMP: &str = include_str!("../../../docs/evidence/storage-layouts/compound-protocol-legacy-jump@4caf72a1.json");
const FIXTURE: &str = include_str!("fixtures/mainnet-ctoken-epochs.json");
const FIAT_TOKEN: &str = include_str!("../../../docs/evidence/storage-layouts/fiat-token@v2.2.0.json");

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
fn hex_word(slot: u64) -> String {
    format!("0x{}", hex::encode(word(slot)))
}
fn slot_of(vars: &[(String, u64)], label: &str) -> [u8; 32] {
    word(slot_number(vars, label))
}
fn slot_number(vars: &[(String, u64)], label: &str) -> u64 {
    vars.iter().find(|(l, _)| l == label).unwrap_or_else(|| panic!("{label} missing")).1
}
fn check_market(m: &Market, vars: &[(String, u64)], contract: &str) {
    let expect = |field: StateField, label: &str| {
        let (slot, _, _) = m.scalars.iter().find(|(_, f, _)| *f == field).unwrap();
        assert_eq!(*slot, slot_of(vars, label), "{contract}.{label}");
    };
    expect(StateField::CompoundV2InitialExchangeRateMantissa, "initialExchangeRateMantissa");
    expect(StateField::CompoundV2ReserveFactorMantissa, "reserveFactorMantissa");
    expect(StateField::CompoundV2AccrualBlockNumber, "accrualBlockNumber");
    expect(StateField::CompoundV2BorrowIndex, "borrowIndex");
    expect(StateField::CompoundV2TotalBorrows, "totalBorrows");
    expect(StateField::CompoundV2TotalReserves, "totalReserves");
    expect(StateField::CompoundV2TotalSupply, "totalSupply");
    assert_eq!(m.rate_model_slot, slot_of(vars, "interestRateModel"));
    assert_eq!(m.account_tokens_slot, slot_of(vars, "accountTokens"));
    let mappings: BTreeSet<[u8; 32]> = ["transferAllowances", "accountBorrows"].iter().map(|l| slot_of(vars, l)).collect();
    assert_eq!(mappings, m.other_mapping_slots.iter().copied().collect());
    if let Some(slot) = m.implementation_slot {
        assert_eq!(slot, slot_of(vars, "implementation"));
    }
    if let Cash::Erc20Mapping { underlying_slot, .. } = &m.cash {
        assert_eq!(*underlying_slot, slot_of(vars, "underlying"), "{contract}.underlying");
    }
    // Completeness: every compiled slot is a configured word, the holder
    // mapping, a pointer, or a reviewed slot / mapping.
    let compiled: BTreeSet<[u8; 32]> = vars.iter().map(|(_, s)| word(*s)).collect();
    let mut covered: BTreeSet<[u8; 32]> = m.scalars.iter().map(|(s, _, _)| *s).collect();
    covered.insert(m.rate_model_slot);
    covered.insert(m.account_tokens_slot);
    covered.extend(m.implementation_slot);
    if let Cash::Erc20Mapping { underlying_slot, .. } = &m.cash {
        covered.insert(*underlying_slot);
    }
    covered.extend(m.other_slots.iter().copied());
    covered.extend(m.other_mapping_slots.iter().copied());
    assert_eq!(compiled, covered, "{contract} slots not fully covered");
}

#[test]
fn the_deployed_ctokens_match_the_2019_layout_not_the_current_tree() {
    let cfg = parse(FIXTURE).unwrap();
    for (market, contract) in [(&cfg.markets[0], "CErc20"), (&cfg.markets[1], "CEther")] {
        assert!(market.source_pin.contains("f385d71983ae5c5799faae9b2dfea43e5cf75262"));
        check_market(market, &slots_in(LEGACY_2019, contract), contract);
    }
    // `ReentrancyGuard` precedes the cToken state: `_guardCounter` takes slot 0
    // and `decimals` is a full uint256, so every word from `admin` on sits one
    // slot above the current tree (where `decimals` and `admin` share slot 3).
    let legacy = slots_in(LEGACY_2019, "CErc20");
    let current = slots_in(CURRENT, "CErc20");
    assert_eq!((legacy[0].0.as_str(), legacy[0].1), ("_guardCounter", 0));
    for label in ["interestRateModel", "accrualBlockNumber", "totalSupply", "accountTokens", "underlying"] {
        assert_eq!(slot_number(&legacy, label), slot_number(&current, label) + 1, "{label}");
    }
    assert_eq!(
        current.iter().filter(|(_, s)| *s == 3).map(|(l, _)| l.as_str()).collect::<Vec<_>>(),
        vec!["decimals", "admin"]
    );
}

#[test]
fn a_current_tree_delegator_matches_the_0_8_10_layout() {
    // Delegators such as cDAI run the current tree; build a market from its
    // compiled layout and check it the same way.
    let vars = slots_in(CURRENT, "CErc20Delegator");
    let mut v: Value = serde_json::from_str(FIXTURE).unwrap();
    let m = &mut v["markets"][0];
    m["kind"] = "cerc20_delegator".into();
    m["implementation_slot"] = hex_word(slot_number(&vars, "implementation")).into();
    m["implementation"] = "0x99ee778b9a6205657dd03b2b91415c8646d521ec".into();
    for (key, label) in [
        ("interest_rate_model", "interestRateModel"),
        ("initial_exchange_rate_mantissa", "initialExchangeRateMantissa"),
        ("reserve_factor_mantissa", "reserveFactorMantissa"),
        ("accrual_block_number", "accrualBlockNumber"),
        ("borrow_index", "borrowIndex"),
        ("total_borrows", "totalBorrows"),
        ("total_reserves", "totalReserves"),
        ("total_supply", "totalSupply"),
        ("account_tokens", "accountTokens"),
        ("underlying", "underlying"),
    ] {
        m["slots"][key] = hex_word(slot_number(&vars, label)).into();
    }
    // `_notEntered`, name, symbol, decimals|admin, pendingAdmin, comptroller.
    m["other_slots"] = serde_json::json!([hex_word(0), hex_word(1), hex_word(2), hex_word(3), hex_word(4), hex_word(5)]);
    m["other_mapping_slots"] = serde_json::json!([
        hex_word(slot_number(&vars, "transferAllowances")),
        hex_word(slot_number(&vars, "accountBorrows"))
    ]);
    let delegator = parse(&v.to_string()).unwrap();
    check_market(&delegator.markets[0], &vars, "CErc20Delegator");
}

#[test]
fn rate_model_slots_and_constants_match_their_deployed_sources() {
    let cfg = parse(FIXTURE).unwrap();
    // cUSDC: LegacyJumpRateModelV2 shares BaseJumpRateModelV2's storage.
    let m = &cfg.markets[0];
    for layout in [slots_in(LEGACY_JUMP, "LegacyJumpRateModelV2"), slots_in(CURRENT, "JumpRateModelV2")] {
        for (field, label) in [
            (StateField::CompoundV2IrmMultiplierPerBlock, "multiplierPerBlock"),
            (StateField::CompoundV2IrmBaseRatePerBlock, "baseRatePerBlock"),
            (StateField::CompoundV2IrmJumpMultiplierPerBlock, "jumpMultiplierPerBlock"),
            (StateField::CompoundV2IrmKink, "kink"),
        ] {
            let (slot, _) = m.rate_model_slots.iter().find(|(_, f)| *f == field).unwrap();
            assert_eq!(*slot, slot_of(&layout, label), "{label}");
        }
        // `owner` (slot 0) is the only other storage and is not a balance input.
        assert_eq!(
            layout.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(),
            vec!["owner", "multiplierPerBlock", "baseRatePerBlock", "jumpMultiplierPerBlock", "kink"]
        );
    }
    // cETH: the 2019 WhitePaper model keeps per-year `multiplier` and
    // `baseRate`, set only by its constructor, so they are qualified constants.
    let e = &cfg.markets[1];
    assert_eq!(
        slots_in(LEGACY_2019, "WhitePaperInterestRateModel"),
        vec![("multiplier".to_string(), 0), ("baseRate".to_string(), 1)]
    );
    assert!(e.rate_model_slots.is_empty());
    let constants: BTreeSet<StateField> = e.rate_model_constants.iter().map(|(f, _, _)| *f).collect();
    assert_eq!(
        constants,
        [
            StateField::CompoundV2IrmBaseRatePerYear,
            StateField::CompoundV2IrmMultiplierPerYear,
            StateField::CompoundV2IrmBlocksPerYear
        ]
        .into()
    );
}

#[test]
fn usdc_cash_reads_the_low_255_bits_of_fiat_token_slot_9() {
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
