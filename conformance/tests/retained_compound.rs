#![cfg(not(target_arch = "wasm32"))]
//! Actual projectors fed synthetic Extended blocks. All initialized inputs,
//! runtime attestations and checkpoint evidence here are synthetic; the
//! committed profiles contain placeholder deployment bindings.
use compound_v2_balance_state as v2;
use compound_v3_balance_state as v3;
use conformance::retained::{
    holder_storage_slot, HolderBinding, InputBinding, Metric, MetricUnits, MetricValue, QualifiedModel, ReferenceModel, RuntimeQualification,
};
use conformance::Unknown;
use evm_retention::protocol::{Checkpoint, ProtocolLedger};
use evm_retention::{Origin, Stream};
use num_bigint::{BigInt, BigUint};
use proto::pb::evm::balance_state::v1 as pb;
use std::collections::BTreeMap;
use substreams_ethereum::pb::eth::v2 as eth;

const HOLDER: [u8; 20] = [9; 20];
const BORROWER: [u8; 20] = [8; 20];
const ZERO: [u8; 20] = [7; 20];
const START: u64 = 1_789_689_600;
const V2: &str = include_str!("../../compound-v2/balance-state/tests/fixtures/mainnet-ctoken-epochs.json");
const V3: &str = include_str!("../../compound-v3/balance-state/tests/fixtures/mainnet-cusdcv3-epoch.json");

fn word(value: u128) -> [u8; 32] {
    let mut out = [0; 32];
    out[16..].copy_from_slice(&value.to_be_bytes());
    out
}
fn packed(fields: &[(u32, u32, u128)]) -> [u8; 32] {
    let mut out = [0; 32];
    for &(offset, width, value) in fields {
        for bit in 0..width {
            if (value >> bit) & 1 == 1 {
                let target = offset + bit;
                out[31 - (target / 8) as usize] |= 1 << (target % 8);
            }
        }
    }
    out
}
fn principal(value: i128) -> [u8; 32] {
    packed(&[(0, 104, if value < 0 { ((1i128 << 104) + value) as u128 } else { value as u128 })])
}
fn block(number: u64, timestamp: u64) -> eth::Block {
    eth::Block {
        ver: 5,
        number,
        hash: vec![number as u8; 32],
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        header: Some(eth::BlockHeader {
            number,
            parent_hash: vec![(number - 1) as u8; 32],
            state_root: vec![3; 32],
            timestamp: Some(prost_types::Timestamp {
                seconds: timestamp as i64,
                nanos: 0,
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn write(address: &[u8], key: &[u8], old: [u8; 32], new: [u8; 32], ordinal: u64) -> eth::StorageChange {
    eth::StorageChange {
        address: address.to_vec(),
        key: key.to_vec(),
        old_value: old.to_vec(),
        new_value: new.to_vec(),
        ordinal,
    }
}
fn balance(address: &[u8], old: u128, new: u128, ordinal: u64) -> eth::BalanceChange {
    eth::BalanceChange {
        address: address.to_vec(),
        old_value: Some(eth::BigInt { bytes: word(old).to_vec() }),
        new_value: Some(eth::BigInt { bytes: word(new).to_vec() }),
        ordinal,
        reason: eth::balance_change::Reason::Transfer as i32,
    }
}
fn preimage(holder: &[u8], root: &[u8]) -> (String, String) {
    let mut preimage = vec![0; 12];
    preimage.extend(holder);
    preimage.extend(root);
    (hex::encode(holder_storage_slot(holder, root).unwrap()), hex::encode(preimage))
}
fn traces(block: &mut eth::Block, writes: Vec<eth::StorageChange>, balances: Vec<eth::BalanceChange>, preimages: BTreeMap<String, String>) {
    let mut calls: BTreeMap<Vec<u8>, eth::Call> = BTreeMap::new();
    for row in writes {
        calls.entry(row.address.clone()).or_default().storage_changes.push(row);
    }
    for row in balances {
        calls.entry(row.address.clone()).or_default().balance_changes.push(row);
    }
    block.transaction_traces = vec![eth::TransactionTrace {
        hash: vec![7; 32],
        status: eth::TransactionTraceStatus::Succeeded as i32,
        calls: calls
            .into_iter()
            .enumerate()
            .map(|(index, (address, mut call))| {
                call.address = address;
                call.index = index as u32;
                call.keccak_preimages = preimages.clone().into_iter().collect();
                call
            })
            .collect(),
        ..Default::default()
    }];
}
fn counts(events: &mut pb::Events) {
    let c = &mut events.clocks[0];
    c.holder_basis_count = events.holder_basis.len() as u32;
    c.global_state_count = events.global_state.len() as u32;
    c.epoch_count = events.epochs.len() as u32;
    c.dependency_count = events.dependencies.len() as u32;
}
fn qualify(events: &pb::Events, model: ReferenceModel, mapping_slot: &[u8]) -> QualifiedModel {
    let c = &events.clocks[0];
    let epoch = &events.epochs[0];
    QualifiedModel {
        stream: Stream {
            chain_id: c.chain_id,
            package: c.package.clone(),
            package_version: c.package_version.clone(),
            spec_revision: c.spec_revision,
            parameters_sha256: c.parameters_sha256.clone(),
        },
        epoch: epoch.clone(),
        dependencies: events.dependencies.clone(),
        model,
        evidence: "SYNTHETIC profile and initialized state; no deployed qualification".into(),
        holder_binding: HolderBinding {
            storage_contract: epoch.market.clone(),
            mapping_slot: mapping_slot.to_vec(),
        },
        inputs: events
            .global_state
            .iter()
            .map(|row| InputBinding {
                field: pb::StateField::try_from(row.field).unwrap(),
                key: row.key.clone(),
                observation: pb::Observation::try_from(row.observation).unwrap(),
                storage_contract: row.storage_contract.clone(),
                storage_slot: row.storage_slot.clone(),
                bit_offset: row.bit_offset,
                bit_width: row.bit_width,
            })
            .collect(),
        runtime: Some(RuntimeQualification {
            at: c.clone(),
            market_code_hash: vec![5; 32],
            implementation_code_hash: if epoch.implementation.is_empty() { vec![] } else { vec![6; 32] },
            dependency_code_hashes: events.dependencies.iter().map(|d| (d.contract.clone(), vec![6; 32])).collect(),
            evidence: "SYNTHETIC hashes only; not runtime evidence".into(),
        }),
    }
}
fn value(q: &QualifiedModel, ledger: &ProtocolLedger, holder: &[u8], metric: Metric) -> String {
    q.evaluate(ledger, holder, metric).unwrap().value.to_string()
}
fn ledger(events: &pb::Events) -> ProtocolLedger {
    let mut ledger = ProtocolLedger::new(8);
    ledger.apply(events).unwrap();
    ledger
}

fn v2_json(native: bool) -> serde_json::Value {
    let mut json: serde_json::Value = serde_json::from_str(V2).unwrap();
    let mut market = json["markets"][usize::from(native)].clone();
    market["activation_block"] = 10.into();
    json["markets"] = serde_json::json!([market]);
    json
}
fn v2_initial(native: bool) -> (v2::Config, pb::Events, QualifiedModel) {
    let config = v2::parse(&v2_json(native).to_string()).unwrap();
    let market = &config.markets[0];
    let mut b = block(10, START);
    let mut writes = vec![];
    for (slot, field, _) in &market.scalars {
        use pb::StateField as F;
        let value = match field {
            F::CompoundV2InitialExchangeRateMantissa | F::CompoundV2BorrowIndex => 1_000_000_000_000_000_000,
            F::CompoundV2ReserveFactorMantissa => 100_000_000_000_000_000,
            F::CompoundV2AccrualBlockNumber => 10,
            F::CompoundV2TotalBorrows => 1_000_000_000_000,
            F::CompoundV2TotalSupply => 1_000_000_000,
            F::CompoundV2TotalReserves => 0,
            _ => unreachable!(),
        };
        writes.push(write(&market.ctoken, slot, word(u128::from(value == 0)), word(value), 20 + writes.len() as u64));
    }
    for (slot, field) in &market.rate_model_slots {
        let value = match field {
            pb::StateField::CompoundV2IrmBaseRatePerBlock => 1_000_000_000_000,
            pb::StateField::CompoundV2IrmKink => 800_000_000_000_000_000,
            _ => 0,
        };
        writes.push(write(
            &market.rate_model,
            slot,
            word(u128::from(value == 0)),
            word(value),
            20 + writes.len() as u64,
        ));
    }
    let mut preimages = BTreeMap::new();
    for (holder, value) in [(HOLDER, 100_000_000), (ZERO, 0)] {
        let key = holder_storage_slot(&holder, &market.account_tokens_slot).unwrap();
        preimages.insert(
            preimage(&holder, &market.account_tokens_slot).0,
            preimage(&holder, &market.account_tokens_slot).1,
        );
        writes.push(write(&market.ctoken, &key, word(u128::from(value == 0)), word(value), 20 + writes.len() as u64));
    }
    let balances = if let v2::Cash::Erc20Mapping { balances_slot, .. } = &market.cash {
        writes.push(write(
            market.underlying.as_ref().unwrap(),
            &holder_storage_slot(&market.ctoken, balances_slot).unwrap(),
            word(0),
            word(1_000_000_000_000),
            80,
        ));
        vec![]
    } else {
        vec![balance(&market.ctoken, 0, 1_000_000_000_000, 80)]
    };
    traces(&mut b, writes, balances, preimages);
    let events = v2::project(&b, &config).unwrap();
    let q = qualify(
        &events,
        if native {
            ReferenceModel::CompoundV2Ceth2019
        } else {
            ReferenceModel::CompoundV2Cusdc2019
        },
        &market.account_tokens_slot,
    );
    (config, events, q)
}
fn donation(config: &v2::Config, number: u64, cash: u128) -> pb::Events {
    let market = &config.markets[0];
    let mut b = block(number, START + number - 10);
    let (writes, balances) = if let v2::Cash::Erc20Mapping { balances_slot, .. } = &market.cash {
        (
            vec![write(
                market.underlying.as_ref().unwrap(),
                &holder_storage_slot(&market.ctoken, balances_slot).unwrap(),
                word(1_000_000_000_000),
                word(cash),
                20,
            )],
            vec![],
        )
    } else {
        (vec![], vec![balance(&market.ctoken, 1_000_000_000_000, cash, 20)])
    };
    traces(&mut b, writes, balances, BTreeMap::new());
    v2::project(&b, config).unwrap()
}

#[test]
fn cusdc_donation_idle_accrual_and_replacement_keep_shares_and_conversions_distinct() {
    let (config, events, q) = v2_initial(false);
    let mut state = ledger(&events);
    let shares = q.evaluate(&state, &HOLDER, Metric::HolderBasis).unwrap();
    assert_eq!(shares.value.to_string(), "100000000");
    assert_eq!(shares.units, MetricUnits::RawBasis(pb::BasisKind::Shares));
    assert_eq!(value(&q, &state, &HOLDER, Metric::CompoundV2StoredUnderlying), "200000000000");
    assert_eq!(value(&q, &state, &HOLDER, Metric::CompoundV2ProjectedUnderlying), "200000000000");
    let donated = donation(&config, 11, 1_000_001_000_000);
    assert!(donated.holder_basis.is_empty());
    assert_eq!(donated.global_state.len(), 1);
    state.apply(&donated).unwrap();
    assert_eq!(value(&q, &state, &HOLDER, Metric::CompoundV2StoredUnderlying), "200000100000");
    assert_eq!(value(&q, &state, &HOLDER, Metric::CompoundV2ProjectedUnderlying), "200000190000");
    let idle = v2::project(&block(12, START + 3600), &config).unwrap();
    assert!(idle.global_state.is_empty());
    state.apply(&idle).unwrap();
    let projected = q.evaluate(&state, &HOLDER, Metric::CompoundV2ProjectedUnderlying).unwrap();
    assert_eq!(projected.value.to_string(), "200000280000");
    assert_eq!(
        projected.units,
        MetricUnits::Asset {
            contract: q.epoch.balance_asset.clone(),
            decimals: 6
        }
    );
    assert_eq!(projected.holder.unwrap().observed_at.number, 10);
    assert_eq!(value(&q, &state, &[], Metric::CompoundV2ExchangeRateStored), "2000001000000000000000");
    assert!(q.evaluate(&state, &BORROWER, Metric::CompoundV2StoredUnderlying).is_err());
    state.undo(10).unwrap();
    assert_eq!(q.evaluate(&state, &HOLDER, Metric::HolderBasis).unwrap(), shares);
    let mut replacement = donation(&config, 11, 1_000_002_000_000);
    replacement.clocks[0].hash = vec![99; 32];
    state.apply(&replacement).unwrap();
    assert_eq!(value(&q, &state, &HOLDER, Metric::CompoundV2StoredUnderlying), "200000200000");
}

#[test]
fn native_cash_has_no_slot_and_whitepaper_constants_drive_the_selected_ceth_model() {
    let (config, events, q) = v2_initial(true);
    let mut state = ledger(&events);
    let cash = events
        .global_state
        .iter()
        .find(|r| r.field == pb::StateField::CompoundV2TotalCash as i32)
        .unwrap();
    assert!(cash.storage_slot.is_empty());
    assert_eq!(cash.storage_contract, q.epoch.market);
    assert_eq!(cash.key, q.epoch.market);
    let update = donation(&config, 11, 1_000_001_000_000);
    state.apply(&update).unwrap();
    assert_eq!(value(&q, &state, &HOLDER, Metric::CompoundV2StoredUnderlying), "200000100000");
    let projected = q.evaluate(&state, &HOLDER, Metric::CompoundV2ProjectedUnderlying).unwrap();
    assert_ne!(projected.value.to_string(), "200000100000");
    assert_eq!(
        projected.units,
        MetricUnits::Asset {
            contract: vec![],
            decimals: 18
        }
    );
    assert!(projected
        .globals
        .iter()
        .any(|r| r.row.field == pb::StateField::CompoundV2IrmMultiplierPerYear as i32 && r.row.observation == pb::Observation::QualifiedConstant as i32));
    let mut bad = q.clone();
    bad.inputs
        .iter_mut()
        .find(|r| r.field == pb::StateField::CompoundV2TotalCash)
        .unwrap()
        .storage_slot = vec![0; 32];
    assert!(bad.evaluate(&state, &HOLDER, Metric::CompoundV2StoredUnderlying).is_err());
    bad = q.clone();
    bad.model = ReferenceModel::CompoundV2Cusdc2019;
    assert!(bad.evaluate(&state, &HOLDER, Metric::HolderBasis).is_err());
}

#[test]
fn missing_cash_or_rate_state_and_wrong_provenance_never_initialize_a_known_zero_getter() {
    let (_, events, q) = v2_initial(false);
    for field in [pb::StateField::CompoundV2TotalCash, pb::StateField::CompoundV2IrmBaseRatePerBlock] {
        let mut partial = events.clone();
        partial.global_state.retain(|r| r.field != field as i32);
        counts(&mut partial);
        let state = ledger(&partial);
        assert_eq!(value(&q, &state, &ZERO, Metric::HolderBasis), "0");
        assert!(matches!(
            q.evaluate(&state, &ZERO, Metric::CompoundV2ProjectedUnderlying),
            Err(Unknown::MissingInput(_))
        ));
    }
    for change in 0..5 {
        let mut bad = events.clone();
        let cash = bad
            .global_state
            .iter_mut()
            .find(|r| r.field == pb::StateField::CompoundV2TotalCash as i32)
            .unwrap();
        match change {
            0 => cash.storage_contract = vec![99; 20],
            1 => cash.storage_slot = vec![99; 32],
            2 => cash.key.clear(),
            3 => cash.scale = "1000000".into(),
            _ => cash.observation = pb::Observation::Derived as i32,
        }
        assert!(q.evaluate(&ledger(&bad), &HOLDER, Metric::CompoundV2StoredUnderlying).is_err());
    }
    let mut changed = q.clone();
    changed
        .inputs
        .iter_mut()
        .find(|i| i.field == pb::StateField::CompoundV2TotalCash)
        .unwrap()
        .bit_width = 256;
    assert!(changed.evaluate(&ledger(&events), &HOLDER, Metric::HolderBasis).is_err());
}

fn comet_json() -> serde_json::Value {
    let mut json: serde_json::Value = serde_json::from_str(V3).unwrap();
    json["markets"][0]["activation_block"] = 10.into();
    json
}
fn comet_initial(json: &serde_json::Value) -> (v3::Config, pb::Events, QualifiedModel) {
    comet_with_supply_index(json, 1_058_123_456_789_012)
}
fn comet_with_supply_index(json: &serde_json::Value, supply_index: u128) -> (v3::Config, pb::Events, QualifiedModel) {
    let config = v3::parse(&json.to_string()).unwrap();
    let m = &config.markets[0];
    let mut b = block(10, START);
    let indices = packed(&[(0, 64, supply_index), (64, 64, 1_074_987_654_321_098)]);
    let totals = packed(&[(0, 104, 1_234_567_890_123_456), (104, 104, 987_654_321_098_765), (208, 40, START as u128)]);
    let mut writes = vec![
        write(&m.comet, &m.indices_slot, word(0), indices, 20),
        write(&m.comet, &m.totals_slot, word(0), totals, 21),
    ];
    let mut preimages = BTreeMap::new();
    for (holder, p) in [(HOLDER, 5_000_000), (BORROWER, -5_000_000), (ZERO, 0)] {
        let key = holder_storage_slot(&holder, &m.user_basic_slot).unwrap();
        let (key_hex, encoded) = preimage(&holder, &m.user_basic_slot);
        preimages.insert(key_hex, encoded);
        writes.push(write(&m.comet, &key, principal(i128::from(p == 0)), principal(p), 30 + writes.len() as u64));
    }
    traces(&mut b, writes, vec![], preimages);
    let events = v3::project(&b, &config).unwrap();
    let q = qualify(&events, ReferenceModel::CometUsdc, &m.user_basic_slot);
    (config, events, q)
}

#[test]
fn comet_signed_principals_idle_time_global_only_updates_and_fork_undo_use_actual_projector_rows() {
    let (config, events, q) = comet_initial(&comet_json());
    let mut state = ledger(&events);
    let borrower = q.evaluate(&state, &BORROWER, Metric::CometPrincipal).unwrap();
    assert_eq!(borrower.value, MetricValue::SignedPrincipal(BigInt::from(-5_000_000)));
    assert_eq!(borrower.units, MetricUnits::RawBasis(pb::BasisKind::SignedPrincipal));
    assert_eq!(value(&q, &state, &HOLDER, Metric::CometBalanceOf), "5290617");
    for holder in [ZERO, BORROWER] {
        assert_eq!(value(&q, &state, &holder, Metric::CometBalanceOf), "0");
    }
    assert!(q.evaluate(&state, &[6; 20], Metric::CometBalanceOf).is_err());
    assert!(q.evaluate(&state, &BORROWER, Metric::HolderBasis).is_err());
    let idle = v3::project(&block(11, START + 3600), &config).unwrap();
    assert!(idle.global_state.is_empty());
    state.apply(&idle).unwrap();
    assert_eq!(value(&q, &state, &[], Metric::CometStoredSupplyIndex), "1058123456789012");
    assert_eq!(value(&q, &state, &[], Metric::CometProjectedSupplyIndex), "1058127213382182");
    assert_eq!(value(&q, &state, &HOLDER, Metric::CometBalanceOf), "5290636");
    let m = &config.markets[0];
    let mut update = block(12, START + 3600);
    let old = packed(&[(0, 64, 1_058_123_456_789_012), (64, 64, 1_074_987_654_321_098)]);
    let new = packed(&[(0, 64, 2_000_000_000_000_000), (64, 64, 1_074_987_654_321_098)]);
    traces(&mut update, vec![write(&m.comet, &m.indices_slot, old, new, 20)], vec![], BTreeMap::new());
    let output = v3::project(&update, &config).unwrap();
    assert!(output.holder_basis.is_empty());
    state.apply(&output).unwrap();
    assert_eq!(value(&q, &state, &[], Metric::CometStoredSupplyIndex), "2000000000000000");
    assert_eq!(
        q.evaluate(&state, &HOLDER, Metric::CometBalanceOf).unwrap().holder.unwrap().observed_at.number,
        10
    );
    state.undo(11).unwrap();
    assert_eq!(value(&q, &state, &HOLDER, Metric::CometBalanceOf), "5290636");
    let mut replacement = block(12, START + 3601);
    replacement.hash = vec![99; 32];
    let key = holder_storage_slot(&HOLDER, &m.user_basic_slot).unwrap();
    traces(
        &mut replacement,
        vec![write(&m.comet, &key, principal(5_000_000), principal(-1), 30)],
        vec![],
        [preimage(&HOLDER, &m.user_basic_slot)].into(),
    );
    state.apply(&v3::project(&replacement, &config).unwrap()).unwrap();
    assert_eq!(value(&q, &state, &HOLDER, Metric::CometPrincipal), "-1");
    assert_eq!(value(&q, &state, &HOLDER, Metric::CometBalanceOf), "0");
}

#[test]
fn comet_checks_both_indices_before_a_nonpositive_principal_and_rejects_uint40_time() {
    let mut json = comet_json();
    json["markets"][0]["immutables"]["borrow_rate_base"] = u64::MAX.to_string().into();
    json["markets"][0]["immutables"]["borrow_rate_slope_low"] = "0".into();
    json["markets"][0]["immutables"]["borrow_rate_slope_high"] = "0".into();
    let (config, events, q) = comet_initial(&json);
    let mut state = ledger(&events);
    assert_eq!(value(&q, &state, &ZERO, Metric::CometBalanceOf), "0"); // equal timestamp skips rate evaluation
    state.apply(&v3::project(&block(11, START + 2000), &config).unwrap()).unwrap();
    for holder in [HOLDER, ZERO, BORROWER] {
        assert!(q.evaluate(&state, &holder, Metric::CometBalanceOf).is_err());
    }
    let (config, events, q) = comet_initial(&comet_json());
    let mut state = ledger(&events);
    state.apply(&v3::project(&block(11, 1 << 40), &config).unwrap()).unwrap();
    assert_eq!(
        q.evaluate(&state, &ZERO, Metric::CometBalanceOf),
        Err(Unknown::Invalid("Comet timestamp exceeds uint40"))
    );
    assert_eq!(value(&q, &state, &ZERO, Metric::CometPrincipal), "0"); // raw storage metric does not project time
}

#[test]
fn comet_missing_globals_and_wrong_scales_sign_widths_or_constants_fail_closed() {
    let (_, events, q) = comet_initial(&comet_json());
    for field in [
        pb::StateField::CometBaseBorrowIndex,
        pb::StateField::CometBorrowRateBase,
        pb::StateField::CometFactorScale,
    ] {
        let mut partial = events.clone();
        partial.global_state.retain(|r| r.field != field as i32);
        counts(&mut partial);
        assert!(matches!(
            q.evaluate(&ledger(&partial), &ZERO, Metric::CometBalanceOf),
            Err(Unknown::MissingInput(_))
        ));
    }
    for change in 0..4 {
        let mut bad = events.clone();
        let global = bad
            .global_state
            .iter_mut()
            .find(|r| r.field == pb::StateField::CometBaseSupplyIndex as i32)
            .unwrap();
        match change {
            0 => global.scale = "1000000000000000000".into(),
            1 => global.bit_offset = 64,
            2 => global.value = (BigUint::from(1u8) << 64u32).to_string(),
            _ => global.storage_contract = vec![1; 20],
        }
        assert!(q.evaluate(&ledger(&bad), &HOLDER, Metric::CometBalanceOf).is_err());
    }
    let mut bad = events.clone();
    bad.holder_basis.iter_mut().find(|r| r.holder == HOLDER).unwrap().value = (BigInt::from(1u8) << 103u32).to_string();
    assert!(q.evaluate(&ledger(&bad), &HOLDER, Metric::CometPrincipal).is_err());
    let mut bad = events.clone();
    bad.global_state
        .iter_mut()
        .find(|r| r.field == pb::StateField::CometBaseScale as i32)
        .unwrap()
        .value = "1000000000000000000".into();
    assert!(q.evaluate(&ledger(&bad), &HOLDER, Metric::CometBalanceOf).is_err());
    let mut bad = q.clone();
    bad.runtime.as_mut().unwrap().dependency_code_hashes.clear();
    assert!(bad.evaluate(&ledger(&events), &HOLDER, Metric::CometPrincipal).is_err());
}

#[test]
fn an_observed_zero_comet_index_is_distinct_from_a_missing_index() {
    let (config, events, q) = comet_with_supply_index(&comet_json(), 0);
    let mut state = ledger(&events);
    for holder in [HOLDER, ZERO, BORROWER] {
        assert_eq!(value(&q, &state, &holder, Metric::CometBalanceOf), "0");
    }
    state.apply(&v3::project(&block(11, START + 3600), &config).unwrap()).unwrap();
    assert_eq!(value(&q, &state, &HOLDER, Metric::CometBalanceOf), "0");
    let mut missing = events.clone();
    missing.global_state.retain(|r| r.field != pb::StateField::CometBaseSupplyIndex as i32);
    counts(&mut missing);
    assert!(matches!(
        q.evaluate(&ledger(&missing), &ZERO, Metric::CometBalanceOf),
        Err(Unknown::MissingInput(_))
    ));
}

#[test]
fn actual_projector_invalidation_suspends_evaluation_and_undo_restores_exact_facts() {
    let (config, events, q) = v2_initial(false);
    let mut state = ledger(&events);
    let before = q.evaluate(&state, &HOLDER, Metric::CompoundV2ProjectedUnderlying).unwrap();
    let m = &config.markets[0];
    let mut b = block(11, START + 1);
    let old = {
        let mut v = [0; 32];
        v[12..].copy_from_slice(&m.rate_model);
        v
    };
    traces(&mut b, vec![write(&m.ctoken, &m.rate_model_slot, old, word(99), 30)], vec![], BTreeMap::new());
    let output = v2::project(&b, &config).unwrap();
    assert_eq!(output.epochs.len(), 1);
    state.apply(&output).unwrap();
    assert!(q.evaluate(&state, &HOLDER, Metric::HolderBasis).is_err());
    state.undo(10).unwrap();
    assert_eq!(q.evaluate(&state, &HOLDER, Metric::CompoundV2ProjectedUnderlying).unwrap(), before);

    let (config, events, q) = comet_initial(&comet_json());
    let mut state = ledger(&events);
    let before = q.evaluate(&state, &HOLDER, Metric::CometBalanceOf).unwrap();
    let m = &config.markets[0];
    let mut b = block(11, START + 1);
    let mut old = [0; 32];
    old[12..].copy_from_slice(&m.implementation);
    traces(
        &mut b,
        vec![write(&m.comet, &m.implementation_slot, old, word(99), 30)],
        vec![],
        BTreeMap::new(),
    );
    state.apply(&v3::project(&b, &config).unwrap()).unwrap();
    assert!(q.evaluate(&state, &HOLDER, Metric::CometBalanceOf).is_err());
    state.undo(10).unwrap();
    assert_eq!(q.evaluate(&state, &HOLDER, Metric::CometBalanceOf).unwrap(), before);
}

#[test]
fn parameter_changes_require_a_separate_qualified_checkpoint_and_cannot_bypass_stream_identity() {
    let (_, events, q) = comet_initial(&comet_json());
    let mut state = ledger(&events);
    let before = state.report();
    let mut changed = comet_json();
    changed["heartbeat_blocks"] = 1001.into();
    let (config, mut descriptors, mut new_q) = comet_initial(&changed);
    let next = v3::project(&block(11, START + 1), &config).unwrap();
    assert!(state.apply(&next).is_err());
    assert_eq!(state.report(), before);
    assert!(new_q.evaluate(&state, &HOLDER, Metric::CometBalanceOf).is_err());
    // A separately attested initialized snapshot under the new parameter
    // identity, not rows claimed to have been emitted by the clock-only block.
    descriptors.clocks = next.clocks.clone();
    counts(&mut descriptors);
    new_q.runtime.as_mut().unwrap().at = descriptors.clocks[0].clone();
    let mut rebound = ProtocolLedger::from_checkpoint(
        4,
        Checkpoint {
            events: descriptors,
            evidence: "SYNTHETIC independently initialized new-stream snapshot".into(),
        },
    )
    .unwrap();
    let result = new_q.evaluate(&rebound, &HOLDER, Metric::CometBalanceOf).unwrap();
    assert!(matches!(result.holder.unwrap().origin, Origin::Checkpoint { .. }));
    assert!(q.evaluate(&rebound, &HOLDER, Metric::CometBalanceOf).is_err());
    assert!(rebound.undo(10).is_err());
    rebound.apply(&v3::project(&block(12, START + 2), &config).unwrap()).unwrap();
    let mut broken = v3::project(&block(13, START + 3), &config).unwrap();
    broken.clocks[0].parent_hash = vec![99; 32];
    let before = rebound.report();
    assert!(rebound.apply(&broken).is_err());
    assert_eq!(rebound.report(), before);
}

#[test]
fn ctoken_zero_supply_uses_initial_rate_and_legacy_same_block_accrual_still_checks_rate_cap() {
    let (config, events, q) = v2_initial(false);
    let mut state = ledger(&events);
    let m = &config.markets[0];
    let scalar = |field| m.scalars.iter().find(|(_, f, _)| *f == field).unwrap().0;
    let mut b = block(11, START + 1);
    traces(
        &mut b,
        vec![write(
            &m.ctoken,
            &scalar(pb::StateField::CompoundV2TotalSupply),
            word(1_000_000_000),
            word(0),
            20,
        )],
        vec![],
        BTreeMap::new(),
    );
    state.apply(&v2::project(&b, &config).unwrap()).unwrap();
    assert_eq!(value(&q, &state, &[], Metric::CompoundV2ExchangeRateStored), "1000000000000000000");
    assert_eq!(value(&q, &state, &HOLDER, Metric::CompoundV2StoredUnderlying), "100000000");
    assert_eq!(value(&q, &state, &ZERO, Metric::CompoundV2StoredUnderlying), "0");
    state.undo(10).unwrap();
    let rate_slot = m
        .rate_model_slots
        .iter()
        .find(|(_, f)| *f == pb::StateField::CompoundV2IrmBaseRatePerBlock)
        .unwrap()
        .0;
    traces(
        &mut b,
        vec![
            write(&m.rate_model, &rate_slot, word(1_000_000_000_000), word(500_000_000_000_001), 20),
            write(&m.ctoken, &scalar(pb::StateField::CompoundV2AccrualBlockNumber), word(10), word(11), 21),
        ],
        vec![],
        BTreeMap::new(),
    );
    state.apply(&v2::project(&b, &config).unwrap()).unwrap();
    assert_eq!(
        q.evaluate(&state, &ZERO, Metric::CompoundV2ProjectedUnderlying),
        Err(Unknown::Invalid("borrow rate is absurdly high"))
    );
    assert_eq!(value(&q, &state, &HOLDER, Metric::CompoundV2StoredUnderlying), "200000000000");
}

#[test]
fn reverted_donations_and_partial_delivery_preserve_initialized_values_and_reports() {
    let (config, events, q) = v2_initial(false);
    let mut state = ledger(&events);
    let m = &config.markets[0];
    let v2::Cash::Erc20Mapping { balances_slot, .. } = &m.cash else {
        unreachable!()
    };
    let mut b = block(11, START + 1);
    traces(
        &mut b,
        vec![write(
            m.underlying.as_ref().unwrap(),
            &holder_storage_slot(&m.ctoken, balances_slot).unwrap(),
            word(1_000_000_000_000),
            word(9_000_000_000_000),
            20,
        )],
        vec![],
        BTreeMap::new(),
    );
    b.transaction_traces[0].calls[0].state_reverted = true;
    let output = v2::project(&b, &config).unwrap();
    assert!(output.global_state.is_empty());
    state.apply(&output).unwrap();
    assert_eq!(value(&q, &state, &HOLDER, Metric::CompoundV2StoredUnderlying), "200000000000");
    let before = state.report();
    let mut partial = donation(&config, 12, 1_000_001_000_000);
    partial.global_state.clear(); // declared row count deliberately unchanged
    assert!(state.apply(&partial).is_err());
    assert_eq!(state.report(), before);
    assert_eq!(value(&q, &state, &HOLDER, Metric::CompoundV2StoredUnderlying), "200000000000");
}
