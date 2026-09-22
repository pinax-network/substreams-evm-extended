//! Encode/decode fixtures for `evm.balance_state.v1` (issue #12). The
//! committed byte vectors are the wire-compatibility contract: a change that
//! alters them without a deliberate version bump fails here. `evm.balances.v1`
//! is asserted byte-identical to its historical encoding in the same run.
use crate::pb::evm::balance_state::v1::*;
use crate::pb::evm::balances::v1 as balances;
use prost::Message;

const AAVE_SUPPLY_ENCODED_LEN: usize = 1361;
const CLOCK_ONLY_ENCODED_LEN: usize = 216;

fn addr(byte: u8) -> Vec<u8> {
    vec![byte; 20]
}
fn word(byte: u8) -> Vec<u8> {
    vec![byte; 32]
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn roundtrip<M: Message + Default + PartialEq + std::fmt::Debug>(m: &M) -> Vec<u8> {
    let bytes = m.encode_to_vec();
    assert_eq!(&M::decode(bytes.as_slice()).unwrap(), m);
    bytes
}

fn clock(number: u64, holder_rows: u32, global_rows: u32) -> BlockClock {
    BlockClock {
        chain_id: 56,
        number,
        hash: word(0xaa),
        parent_hash: word(0xa9),
        timestamp: 1789689612,
        state_root: word(0x33),
        producer_version: 5,
        spec_revision: 1,
        package: "aave_balance_state".into(),
        package_version: "v0.1.0".into(),
        parameters_sha256: "cd".repeat(32),
        holder_basis_count: holder_rows,
        global_state_count: global_rows,
        ..Default::default()
    }
}

/// Aave BSC USDT aToken supply: one holder row and the three reserve fields
/// `getNormalizedIncome` reads, all observed writes in one transaction.
fn aave_supply() -> Events {
    let identity = |ordinal| {
        (
            Observation::ObservedWrite as i32,
            Boundary::EndOfBlock as i32,
            Scope::Transaction as i32,
            ordinal,
        )
    };
    let (observation, boundary, scope, ordinal) = identity(15234);
    let holder = HolderBasis {
        chain_id: 56,
        market: addr(0xa9),
        holder: addr(0x11),
        epoch: 1,
        basis_kind: BasisKind::ScaledBalance as i32,
        value: "998472113344556677889".into(),
        previous_value: "998000000000000000000".into(),
        observation,
        boundary,
        scope,
        ordinal,
        first_ordinal: ordinal,
        change_count: 1,
        transaction_index: 42,
        transaction_hash: word(0x7b),
        call_index: 7,
        storage_contract: addr(0xa9),
        storage_slot: word(0x51),
        raw_previous_word: word(0x01),
        raw_word: word(0x02),
        bit_offset: 0,
        bit_width: 120,
        signed: false,
    };
    let reserve = |field: StateField, value: &str, scale: &str, ordinal: u64| GlobalState {
        chain_id: 56,
        market: addr(0xa9),
        epoch: 1,
        field: field as i32,
        key: addr(0x55),
        value: value.into(),
        scale: scale.into(),
        observation: Observation::ObservedWrite as i32,
        boundary: Boundary::EndOfBlock as i32,
        scope: Scope::Transaction as i32,
        ordinal,
        first_ordinal: ordinal,
        change_count: 1,
        transaction_index: 42,
        transaction_hash: word(0x7b),
        call_index: 5,
        storage_contract: addr(0x68),
        storage_slot: word(0x77),
        raw_previous_word: word(0x03),
        raw_word: word(0x04),
        bit_offset: 0,
        bit_width: 128,
        ..Default::default()
    };
    let ray = "1000000000000000000000000000";
    Events {
        clocks: vec![clock(122288010, 1, 3)],
        holder_basis: vec![holder],
        global_state: vec![
            reserve(StateField::AaveLiquidityIndex, "1023456789012345678901234567", ray, 15229),
            GlobalState {
                bit_offset: 128,
                ..reserve(StateField::AaveCurrentLiquidityRate, "18450000000000000000000000", ray, 15229)
            },
            GlobalState {
                bit_offset: 128,
                bit_width: 40,
                ..reserve(StateField::AaveLastUpdateTimestamp, "1789689612", "1", 15230)
            },
        ],
        ..Default::default()
    }
}

#[test]
fn aave_supply_round_trips_and_keeps_its_committed_encoding() {
    let events = aave_supply();
    let bytes = roundtrip(&events);
    assert_eq!(
        hex(&prost::Message::encode_to_vec(&events.holder_basis[0])[..24]),
        "08381214a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9"
    );
    // Structural expectations a consumer relies on.
    assert_eq!(events.clocks.len(), 1);
    assert!(events.global_state.iter().all(|g| g.market == events.holder_basis[0].market));
    println!("aave_supply_bytes={}", bytes.len());
    assert_eq!(bytes.len(), AAVE_SUPPLY_ENCODED_LEN);
}

#[test]
fn comet_global_accrual_emits_no_holder_rows() {
    // Every initialized holder's balanceOf changed; only market rows exist.
    let market = |field: StateField, value: &str, offset: u32, width: u32| GlobalState {
        chain_id: 1,
        market: addr(0xc3),
        epoch: 1,
        field: field as i32,
        value: value.into(),
        scale: if matches!(field, StateField::CometLastAccrualTime) {
            "1"
        } else {
            "1000000000000000"
        }
        .into(),
        observation: Observation::ObservedWrite as i32,
        boundary: Boundary::EndOfBlock as i32,
        scope: Scope::Transaction as i32,
        ordinal: 2044,
        first_ordinal: 2044,
        change_count: 1,
        transaction_index: 11,
        transaction_hash: word(0x3c),
        call_index: 2,
        storage_contract: addr(0xc3),
        storage_slot: word(0x00),
        raw_previous_word: word(0x05),
        raw_word: word(0x06),
        bit_offset: offset,
        bit_width: width,
        ..Default::default()
    };
    let events = Events {
        clocks: vec![BlockClock {
            chain_id: 1,
            global_state_count: 3,
            ..clock(20_000_000, 0, 3)
        }],
        global_state: vec![
            market(StateField::CometBaseSupplyIndex, "1058123456789012", 0, 64),
            market(StateField::CometBaseBorrowIndex, "1074987654321098", 64, 64),
            GlobalState {
                storage_slot: word(0x01),
                ..market(StateField::CometLastAccrualTime, "1789689600", 208, 40)
            },
        ],
        ..Default::default()
    };
    roundtrip(&events);
    assert!(events.holder_basis.is_empty());
    let idx = &events.global_state[0];
    assert_eq!(StateField::try_from(idx.field).unwrap(), StateField::CometBaseSupplyIndex);
}

#[test]
fn signed_principal_keeps_its_sign_and_is_never_an_unsigned_balance() {
    let debt = HolderBasis {
        chain_id: 1,
        market: addr(0xc3),
        holder: addr(0x22),
        epoch: 1,
        basis_kind: BasisKind::SignedPrincipal as i32,
        value: "-10141204801825835211973625643007".into(),         // int104 min
        previous_value: "10141204801825835211973625643007".into(), // int104 max
        observation: Observation::ObservedWrite as i32,
        boundary: Boundary::EndOfBlock as i32,
        scope: Scope::Transaction as i32,
        ordinal: 9,
        first_ordinal: 9,
        change_count: 1,
        bit_width: 104,
        signed: true,
        ..Default::default()
    };
    let decoded = HolderBasis::decode(roundtrip(&debt).as_slice()).unwrap();
    assert!(decoded.value.starts_with('-'));
    assert_eq!(BasisKind::try_from(decoded.basis_kind).unwrap(), BasisKind::SignedPrincipal);
    // An unsigned basis never carries a sign; the invariant is checked by
    // producers, the schema keeps both representable.
    let supply = HolderBasis {
        basis_kind: BasisKind::Shares as i32,
        value: "115792089237316195423570985008687907853269984665640564039457584007913129639935".into(),
        signed: false,
        ..debt.clone()
    };
    assert_eq!(HolderBasis::decode(roundtrip(&supply).as_slice()).unwrap().value.len(), 78);
}

#[test]
fn compound_v2_donation_only_cash_change_is_a_partial_observation() {
    // USDC transferred to cUSDC without mint(): only the cross-contract cash
    // field is observed; no other market field is carried.
    let cash = GlobalState {
        chain_id: 1,
        market: addr(0x39),
        epoch: 1,
        field: StateField::CompoundV2TotalCash as i32,
        key: addr(0x39),
        value: "310457889201347".into(),
        previous_value: "310000000000000".into(),
        scale: "1".into(),
        observation: Observation::ObservedWrite as i32,
        boundary: Boundary::EndOfBlock as i32,
        scope: Scope::Transaction as i32,
        ordinal: 9120,
        first_ordinal: 9120,
        change_count: 1,
        transaction_index: 77,
        transaction_hash: word(0x5d),
        call_index: 1,
        storage_contract: addr(0xa0), // the underlying ERC-20, not the cToken
        storage_slot: word(0x9a),
        raw_previous_word: word(0x07),
        raw_word: word(0x08),
        bit_width: 256,
        ..Default::default()
    };
    let events = Events {
        clocks: vec![BlockClock {
            chain_id: 1,
            ..clock(20_000_001, 0, 1)
        }],
        global_state: vec![cash],
        ..Default::default()
    };
    roundtrip(&events);
    assert_ne!(events.global_state[0].storage_contract, events.global_state[0].market);
    // "" (not carried) and "0" (observed zero) are different on the wire.
    let zero = GlobalState {
        value: "0".into(),
        ..events.global_state[0].clone()
    };
    let absent = GlobalState {
        value: String::new(),
        ..events.global_state[0].clone()
    };
    assert_ne!(zero.encode_to_vec(), absent.encode_to_vec());
}

#[test]
fn lido_rebase_is_a_log_observation_with_no_passive_holder_rows() {
    let report = |field: StateField, value: &str| GlobalState {
        chain_id: 1,
        market: addr(0xae),
        epoch: 4,
        field: field as i32,
        value: value.into(),
        scale: "1".into(),
        observation: Observation::ObservedLog as i32,
        boundary: Boundary::Change as i32,
        scope: Scope::Transaction as i32,
        ordinal: 30880,
        first_ordinal: 30880,
        change_count: 1,
        transaction_index: 5,
        transaction_hash: word(0x9e),
        call_index: 14,
        log_index: 63,
        storage_contract: addr(0xae),
        ..Default::default()
    };
    let events = Events {
        clocks: vec![BlockClock {
            chain_id: 1,
            ..clock(20_000_002, 0, 4)
        }],
        global_state: vec![
            report(StateField::LidoReportTimestamp, "1789689600"),
            report(StateField::LidoReportPostTotalShares, "8214012345678901234567890"),
            report(StateField::LidoReportPostTotalEther, "9783901234567890123456789"),
            report(StateField::LidoReportSharesMintedAsFees, "55567890123456789012"),
        ],
        ..Default::default()
    };
    roundtrip(&events);
    assert!(events.holder_basis.is_empty());
    assert!(events.global_state.iter().all(|g| g.storage_slot.is_empty() && g.log_index == 63));
}

#[test]
fn erc4626_conversion_input_change_lives_in_the_dependency_family() {
    // A Pot drip() writes chi and rho; dsr is governance-only and not carried.
    let pot = |field: StateField, value: &str, scale: &str| GlobalState {
        chain_id: 1,
        market: addr(0x83), // sDAI is the consuming market
        epoch: 1,
        field: field as i32,
        value: value.into(),
        scale: scale.into(),
        observation: Observation::ObservedWrite as i32,
        boundary: Boundary::EndOfBlock as i32,
        scope: Scope::Transaction as i32,
        ordinal: 7701,
        first_ordinal: 7701,
        change_count: 1,
        transaction_index: 18,
        transaction_hash: word(0x2f),
        call_index: 3,
        storage_contract: addr(0x19), // the Maker Pot
        storage_slot: word(0x04),     // chi (compiled Pot: Pie 2, dsr 3, chi 4, rho 7)
        raw_previous_word: word(0x09),
        raw_word: word(0x0a),
        bit_width: 256,
        ..Default::default()
    };
    let epoch = ModelEpoch {
        chain_id: 1,
        market: addr(0x83),
        epoch: 1,
        kind: EpochEventKind::Bound as i32,
        family: ModelFamily::Erc4626Vault as i32,
        model_id: "erc4626/sdai/pot-rpow".into(),
        source_pin: "sky-ecosystem/sdai@665879762f8b5df5d234463f45d1d6a49bd4fbeb".into(),
        basis_kind: BasisKind::Shares as i32,
        // assets = shares * chi / RAY
        basis_scale: "1000000000000000000000000000".into(),
        balance_rounding: Rounding::Floor as i32,
        basis_bit_width: 256,
        market_code_hash: word(0xc0),
        activation_block: 20_000_003,
        balance_asset: addr(0x6b),
        balance_decimals: 18,
        scope: Scope::Epoch as i32,
        ..Default::default()
    };
    let dependency = Dependency {
        chain_id: 1,
        market: addr(0x83),
        epoch: 1,
        kind: EpochEventKind::Bound as i32,
        role: DependencyRole::RateAccumulator as i32,
        contract: addr(0x19),
        depth: 1,
        binding: BindingKind::CodeHash as i32,
        code_hash: word(0xc1),
        activation_block: 20_000_003,
        source_pin: "chainlog MCD_POT".into(),
        ..Default::default()
    };
    let events = Events {
        clocks: vec![BlockClock {
            chain_id: 1,
            epoch_count: 1,
            dependency_count: 1,
            ..clock(20_000_003, 0, 2)
        }],
        epochs: vec![epoch],
        dependencies: vec![dependency],
        global_state: vec![
            pot(StateField::MakerPotChi, "1097654321098765432109876543", "1000000000000000000000000000"),
            GlobalState {
                storage_slot: word(0x07), // rho
                ..pot(StateField::MakerPotRho, "1789689600", "1")
            },
        ],
        ..Default::default()
    };
    roundtrip(&events);
    assert!(events.global_state.iter().all(|g| g.field != StateField::MakerPotDsr as i32));
}

#[test]
fn arc_alias_declares_one_balance_at_two_precisions() {
    let alias = AssetAlias {
        chain_id: 5042,
        kind: AliasKind::PrecisionView as i32,
        event_kind: EpochEventKind::Bound as i32,
        asset: {
            let mut a = vec![0u8; 20];
            a[0] = 0x36;
            a
        },
        alias_of: vec![], // native
        asset_scale: "1000000".into(),
        alias_of_scale: "1000000000000000000".into(),
        asset_rounding: Rounding::Floor as i32,
        log_emitter: vec![0xff; 20].into_iter().enumerate().map(|(i, b)| if i == 19 { 0xfe } else { b }).collect(),
        activation_block: 1,
        source_pin: "docs.arc.io stablecoin-native-model".into(),
    };
    let events = Events {
        clocks: vec![BlockClock {
            chain_id: 5042,
            alias_count: 1,
            ..clock(1, 0, 0)
        }],
        aliases: vec![alias],
        ..Default::default()
    };
    roundtrip(&events);
    assert!(events.aliases[0].alias_of.is_empty());
}

#[test]
fn a_block_without_state_writes_still_carries_exactly_one_clock() {
    let events = Events {
        clocks: vec![clock(122288011, 0, 0)],
        ..Default::default()
    };
    let bytes = roundtrip(&events);
    assert_eq!(Events::decode(bytes.as_slice()).unwrap().clocks.len(), 1);
    println!("clock_only_bytes={}", bytes.len());
    assert_eq!(bytes.len(), CLOCK_ONLY_ENCODED_LEN);
    assert!(Events::default().encode_to_vec().is_empty());
}

#[test]
fn unknown_enum_numbers_survive_decode_as_raw_integers() {
    let row = GlobalState {
        field: 9999,
        observation: 77,
        value: "1".into(),
        ..Default::default()
    };
    let decoded = GlobalState::decode(row.encode_to_vec().as_slice()).unwrap();
    assert_eq!((decoded.field, decoded.observation), (9999, 77));
    assert!(StateField::try_from(decoded.field).is_err());
    // A consumer refuses to evaluate such a row; the schema never coerces it to 0.
    assert_ne!(decoded.field, StateField::Unspecified as i32);
}

#[test]
fn every_enum_starts_at_an_unspecified_zero() {
    assert_eq!(ModelFamily::Unspecified as i32, 0);
    assert_eq!(BasisKind::Unspecified as i32, 0);
    assert_eq!(Observation::Unspecified as i32, 0);
    assert_eq!(Boundary::Unspecified as i32, 0);
    assert_eq!(Scope::Unspecified as i32, 0);
    assert_eq!(EpochEventKind::Unspecified as i32, 0);
    assert_eq!(InvalidationReason::Unspecified as i32, 0);
    assert_eq!(DependencyRole::Unspecified as i32, 0);
    assert_eq!(BindingKind::Unspecified as i32, 0);
    assert_eq!(Rounding::Unspecified as i32, 0);
    assert_eq!(AliasKind::Unspecified as i32, 0);
    assert_eq!(StateField::Unspecified as i32, 0);
    // Family ranges stay disjoint and append-only.
    assert_eq!(StateField::CompoundV2TotalCash as i32, 10);
    assert_eq!(StateField::CometBaseSupplyIndex as i32, 30);
    assert_eq!(StateField::LidoTotalShares as i32, 60);
    assert_eq!(StateField::MakerPotChi as i32, 80);
    assert_eq!(StateField::Erc4626TotalSupply as i32, 90);
}

#[test]
fn canonical_balances_schema_is_untouched() {
    // The historical encoding of a native and an ERC-20 row: field numbers and
    // wire types of evm.balances.v1 must never change.
    let native = balances::Balance {
        contract: None,
        address: addr(0x04),
        amount: "7".into(),
    };
    assert_eq!(hex(&native.encode_to_vec()), format!("1214{}1a0137", "04".repeat(20)));
    let erc20 = balances::Balance {
        contract: Some(addr(0xbb)),
        ..native
    };
    assert_eq!(hex(&erc20.encode_to_vec()), format!("0a14{}1214{}1a0137", "bb".repeat(20), "04".repeat(20)));
    let events = balances::Events { balances: vec![erc20] };
    assert_eq!(events.encode_to_vec()[..2], [0x0a, 47]);
}
