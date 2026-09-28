#![cfg(not(target_arch = "wasm32"))]
//! Synthetic lifecycle/qualification controls unless a test explicitly cites
//! a captured fixture. These do not qualify a runtime or a newly built SPKG.
use conformance::aave::Era;
use conformance::retained::{holder_storage_slot, HolderBinding, InputBinding, Metric, QualifiedModel, ReferenceModel, RuntimeQualification};
use conformance::Unknown;
use evm_retention::protocol::{Checkpoint, GlobalKey, ProtocolLedger, Unavailable};
use evm_retention::{Origin, Stream};
use prost::Message;
use proto::pb::evm::balance_state::v1 as pb;
use substreams_ethereum::pb::eth::v2 as eth;

const RAY: &str = "1000000000000000000000000000";
const MARKET: [u8; 20] = [1; 20];
const HOLDER: [u8; 20] = [9; 20];

fn model(reference: ReferenceModel) -> pb::ModelEpoch {
    let (family, id, source, basis, scale, width) = match reference {
        ReferenceModel::AaveAtoken(era) => (
            pb::ModelFamily::AaveV3Atoken,
            if era == Era::Floor {
                "aave-v3/atoken/scaled-floor"
            } else {
                "aave-v3/atoken/scaled-half-up"
            },
            "aave-dao/aave-v3-origin@8305565ae342f1773c42cd2e4593f175fe5968a0",
            pb::BasisKind::ScaledBalance,
            RAY,
            120,
        ),
        ReferenceModel::StaticAToken => (
            pb::ModelFamily::Erc4626Vault,
            "erc4626/aave-static-atoken-lm/ray-mul-round-down",
            "bgd-labs/static-a-token-v3@101f5d977889254ca2d2711b9582b45f832d10a0",
            pb::BasisKind::Shares,
            RAY,
            256,
        ),
        ReferenceModel::SavingsDai => (
            pb::ModelFamily::Erc4626Vault,
            "erc4626/maker-savings-dai/rpow-chi",
            "sky-ecosystem/sdai@665879762f8b5df5d234463f45d1d6a49bd4fbeb",
            pb::BasisKind::Shares,
            RAY,
            256,
        ),
        ReferenceModel::OzVirtualOffset => (
            pb::ModelFamily::Erc4626Vault,
            "erc4626/openzeppelin-v5/virtual-offset",
            "OpenZeppelin/openzeppelin-contracts@932fddf69a699a9a80fd2396fd1a2ab91cdda123",
            pb::BasisKind::Shares,
            "",
            256,
        ),
        ReferenceModel::LidoV4 => (
            pb::ModelFamily::LidoSteth,
            "lido/steth/v4/internal-share-rate",
            "lidofinance/core@2da0f48f1a2a103a394dcf8760810fe9165697fb",
            pb::BasisKind::Shares,
            "1",
            256,
        ),
        ReferenceModel::CompoundV2Cusdc2019 | ReferenceModel::CompoundV2Ceth2019 | ReferenceModel::CometUsdc => panic!("use actual Compound projectors"),
    };
    pb::ModelEpoch {
        chain_id: 56,
        market: MARKET.to_vec(),
        epoch: 1,
        kind: pb::EpochEventKind::Bound as i32,
        family: family as i32,
        model_id: id.into(),
        source_pin: source.into(),
        basis_kind: basis as i32,
        basis_scale: scale.into(),
        basis_bit_width: width,
        implementation_revision: match reference {
            ReferenceModel::AaveAtoken(Era::Floor) => "5",
            ReferenceModel::AaveAtoken(Era::HalfUp) => "3",
            ReferenceModel::StaticAToken => "2",
            ReferenceModel::LidoV4 => "4",
            _ => "",
        }
        .into(),
        balance_rounding: if reference == ReferenceModel::AaveAtoken(Era::HalfUp) {
            pb::Rounding::HalfUp
        } else {
            pb::Rounding::Floor
        } as i32,
        market_code_hash: vec![7; 32],
        activation_block: 10,
        balance_asset: vec![2; 20],
        balance_decimals: 18,
        basis_carryover: true,
        global_carryover: true,
        scope: pb::Scope::Epoch as i32,
        ..Default::default()
    }
}
fn holder(epoch: &pb::ModelEpoch, value: &str) -> pb::HolderBasis {
    pb::HolderBasis {
        chain_id: epoch.chain_id,
        market: epoch.market.clone(),
        holder: HOLDER.to_vec(),
        epoch: epoch.epoch,
        basis_kind: epoch.basis_kind,
        value: value.into(),
        observation: pb::Observation::ObservedWrite as i32,
        boundary: pb::Boundary::EndOfBlock as i32,
        scope: pb::Scope::Transaction as i32,
        ordinal: 20,
        first_ordinal: 20,
        change_count: 1,
        storage_contract: epoch.market.clone(),
        storage_slot: holder_storage_slot(&HOLDER, &[0; 32]).unwrap(),
        raw_word: vec![4; 32],
        bit_offset: epoch.basis_bit_offset,
        bit_width: epoch.basis_bit_width,
        ..Default::default()
    }
}
fn global(epoch: &pb::ModelEpoch, field: pb::StateField, value: &str, scale: &str) -> pb::GlobalState {
    pb::GlobalState {
        chain_id: epoch.chain_id,
        market: epoch.market.clone(),
        epoch: epoch.epoch,
        field: field as i32,
        value: value.into(),
        scale: scale.into(),
        observation: pb::Observation::ObservedWrite as i32,
        boundary: pb::Boundary::EndOfBlock as i32,
        scope: pb::Scope::Transaction as i32,
        ordinal: 21,
        first_ordinal: 21,
        change_count: 1,
        storage_contract: epoch.market.clone(),
        storage_slot: vec![field as u8; 32],
        raw_word: vec![5; 32],
        bit_width: 256,
        ..Default::default()
    }
}
fn events(number: u64, timestamp: u64, epochs: Vec<pb::ModelEpoch>, holder_basis: Vec<pb::HolderBasis>, global_state: Vec<pb::GlobalState>) -> pb::Events {
    pb::Events {
        clocks: vec![pb::BlockClock {
            chain_id: 56,
            number,
            hash: vec![number as u8; 32],
            parent_hash: vec![(number - 1) as u8; 32],
            timestamp,
            state_root: vec![8; 32],
            producer_version: 5,
            package: "synthetic-reference-fixture".into(),
            package_version: "0.1.0".into(),
            spec_revision: 1,
            parameters_sha256: "ab".repeat(32),
            epoch_count: epochs.len() as u32,
            holder_basis_count: holder_basis.len() as u32,
            global_state_count: global_state.len() as u32,
            ..Default::default()
        }],
        epochs,
        holder_basis,
        global_state,
        ..Default::default()
    }
}
fn input(row: &pb::GlobalState) -> InputBinding {
    InputBinding {
        field: pb::StateField::try_from(row.field).unwrap(),
        key: row.key.clone(),
        observation: pb::Observation::try_from(row.observation).unwrap(),
        storage_contract: row.storage_contract.clone(),
        storage_slot: row.storage_slot.clone(),
        bit_offset: row.bit_offset,
        bit_width: row.bit_width,
    }
}
fn qualify(events: &pb::Events, reference: ReferenceModel) -> QualifiedModel {
    let clock = &events.clocks[0];
    QualifiedModel {
        stream: Stream {
            chain_id: clock.chain_id,
            package: clock.package.clone(),
            package_version: clock.package_version.clone(),
            spec_revision: clock.spec_revision,
            parameters_sha256: clock.parameters_sha256.clone(),
        },
        epoch: events.epochs[0].clone(),
        dependencies: events.dependencies.clone(),
        inputs: events.global_state.iter().map(input).collect(),
        model: reference,
        holder_binding: HolderBinding {
            storage_contract: events.epochs[0].market.clone(),
            mapping_slot: vec![0; 32],
        },
        evidence: "synthetic qualification for lifecycle tests only".into(),
        runtime: Some(RuntimeQualification {
            at: clock.clone(),
            market_code_hash: if events.epochs[0].market_code_hash.is_empty() {
                vec![7; 32]
            } else {
                events.epochs[0].market_code_hash.clone()
            },
            implementation_code_hash: if events.epochs[0].implementation.is_empty() { vec![] } else { vec![8; 32] },
            dependency_code_hashes: events
                .dependencies
                .iter()
                .map(|d| {
                    (
                        d.contract.clone(),
                        if !d.code_hash.is_empty() {
                            d.code_hash.clone()
                        } else if d.contract == events.epochs[0].implementation {
                            vec![8; 32]
                        } else if d.contract == events.epochs[0].market {
                            vec![7; 32]
                        } else {
                            vec![9; 32]
                        },
                    )
                })
                .collect(),
            evidence: "synthetic runtime attestation; does not qualify a deployment".into(),
        }),
    }
}
fn seed(events: pb::Events) -> ProtocolLedger {
    ProtocolLedger::from_checkpoint(
        4,
        Checkpoint {
            events,
            evidence: "synthetic exact checkpoint; no deployed-runtime claim".into(),
        },
    )
    .unwrap()
}
fn aave_initial(value: &str) -> (pb::Events, QualifiedModel) {
    let epoch = model(ReferenceModel::AaveAtoken(Era::Floor));
    let rows = vec![
        global(&epoch, pb::StateField::AaveLiquidityIndex, RAY, RAY),
        global(&epoch, pb::StateField::AaveCurrentLiquidityRate, "31536000000000000000000000", RAY),
        global(&epoch, pb::StateField::AaveLastUpdateTimestamp, "100", "1"),
    ];
    let events = events(10, 100, vec![epoch.clone()], vec![holder(&epoch, value)], rows);
    let qualified = qualify(&events, ReferenceModel::AaveAtoken(Era::Floor));
    (events, qualified)
}
fn amount(model: &QualifiedModel, state: &ProtocolLedger, metric: Metric) -> String {
    model.evaluate(state, &HOLDER, metric).unwrap().value.to_string()
}

#[test]
fn unchanged_aave_holder_tracks_idle_clock_global_update_undo_and_replacement() {
    let (initial, qualified) = aave_initial("1000000000");
    let original = initial.holder_basis[0].clone();
    let mut state = seed(initial);
    assert_eq!(amount(&qualified, &state, Metric::AaveBalanceOf), "1000000000");
    state.apply(&events(11, 101, vec![], vec![], vec![])).unwrap();
    assert_eq!(amount(&qualified, &state, Metric::AaveBalanceOf), "1000000001");
    let update = events(
        12,
        102,
        vec![],
        vec![],
        vec![
            global(&qualified.epoch, pb::StateField::AaveLiquidityIndex, "2000000000000000000000000000", RAY),
            global(&qualified.epoch, pb::StateField::AaveCurrentLiquidityRate, "0", RAY),
            global(&qualified.epoch, pb::StateField::AaveLastUpdateTimestamp, "102", "1"),
        ],
    );
    state.apply(&update).unwrap();
    assert_eq!(amount(&qualified, &state, Metric::AaveBalanceOf), "2000000000");
    let retained = state.holder(&MARKET, &HOLDER).unwrap();
    assert_eq!(retained.row, original);
    assert!(matches!(retained.origin, Origin::Checkpoint { block: 10, .. }));
    assert_eq!(state.report().raw_basis_ledger.emitted_rows, 0);
    state.undo(11).unwrap();
    assert_eq!(amount(&qualified, &state, Metric::AaveBalanceOf), "1000000001");
    let mut replacement = update;
    replacement.clocks[0].hash = vec![99; 32];
    replacement.global_state[0].value = "3000000000000000000000000000".into();
    state.apply(&replacement).unwrap();
    let result = qualified.evaluate(&state, &HOLDER, Metric::AaveBalanceOf).unwrap();
    assert_eq!(result.value.to_string(), "3000000000");
    assert_eq!(result.clock.hash, vec![99; 32]);
    assert_eq!(result.globals[0].observed_at.hash, vec![99; 32]);
    assert_eq!(result.runtime_qualification, qualified.runtime.clone().unwrap());
}

#[test]
fn missing_holder_global_and_model_are_unknown_even_for_zero() {
    let (initial, qualified) = aave_initial("0");
    let state = seed(initial.clone());
    assert_eq!(amount(&qualified, &state, Metric::AaveBalanceOf), "0");
    assert!(matches!(
        qualified.evaluate(&state, &[88; 20], Metric::AaveBalanceOf),
        Err(Unknown::MissingInput(_))
    ));
    let mut missing = initial.clone();
    missing.global_state.remove(0);
    missing.clocks[0].global_state_count -= 1;
    assert!(matches!(
        qualified.evaluate(&seed(missing), &HOLDER, Metric::AaveBalanceOf),
        Err(Unknown::MissingInput(_))
    ));
    let mut missing = initial;
    missing.epochs.clear();
    missing.clocks[0].epoch_count = 0;
    assert!(matches!(
        qualified.evaluate(&seed(missing), &HOLDER, Metric::AaveBalanceOf),
        Err(Unknown::MissingInput(_))
    ));
}

#[test]
fn known_zero_aave_index_is_distinct_from_a_missing_fact_through_idle_clocks() {
    for shares in ["0", "1"] {
        let (mut initial, qualified) = aave_initial(shares);
        initial.global_state[0].value = "0".into();
        let mut state = seed(initial.clone());
        assert_eq!(amount(&qualified, &state, Metric::AaveBalanceOf), "0");
        state.apply(&events(11, 101, vec![], vec![], vec![])).unwrap();
        assert_eq!(amount(&qualified, &state, Metric::AaveBalanceOf), "0");
        state.undo(10).unwrap();
        assert_eq!(amount(&qualified, &state, Metric::AaveBalanceOf), "0");
        initial.global_state.remove(0);
        initial.clocks[0].global_state_count -= 1;
        assert!(matches!(
            qualified.evaluate(&seed(initial), &HOLDER, Metric::AaveBalanceOf),
            Err(Unknown::MissingInput(_))
        ));
    }
}

#[test]
fn known_zero_sdai_chi_is_distinct_from_missing_inputs_through_idle_clocks() {
    for shares in ["0", "1"] {
        let epoch = model(ReferenceModel::SavingsDai);
        let initial = events(
            10,
            100,
            vec![epoch.clone()],
            vec![holder(&epoch, shares)],
            vec![
                global(&epoch, pb::StateField::MakerPotChi, "0", RAY),
                global(&epoch, pb::StateField::MakerPotDsr, RAY, RAY),
                global(&epoch, pb::StateField::MakerPotRho, "100", "1"),
            ],
        );
        let qualified = qualify(&initial, ReferenceModel::SavingsDai);
        let mut state = seed(initial.clone());
        assert_eq!(amount(&qualified, &state, Metric::SavingsDaiConvertToAssets), "0");
        state.apply(&events(11, 101, vec![], vec![], vec![])).unwrap();
        assert_eq!(amount(&qualified, &state, Metric::SavingsDaiConvertToAssets), "0");
        state.undo(10).unwrap();
        assert_eq!(amount(&qualified, &state, Metric::SavingsDaiConvertToAssets), "0");
        for index in 0..3 {
            let mut missing = initial.clone();
            missing.global_state.remove(index);
            missing.clocks[0].global_state_count -= 1;
            assert!(matches!(
                qualified.evaluate(&seed(missing), &HOLDER, Metric::SavingsDaiConvertToAssets),
                Err(Unknown::MissingInput(_))
            ));
        }
    }
}

#[test]
fn rejected_partial_scale_layout_and_identity_updates_are_atomic() {
    let (initial, qualified) = aave_initial("5");
    let mut state = seed(initial);
    let before = state.report();
    let input = state
        .global(&GlobalKey::new(
            &MARKET,
            pb::StateField::AaveLiquidityIndex,
            &[],
            pb::Observation::ObservedWrite,
        ))
        .unwrap()
        .clone();
    let valid = events(
        11,
        101,
        vec![],
        vec![holder(&qualified.epoch, "8")],
        vec![global(&qualified.epoch, pb::StateField::AaveLiquidityIndex, RAY, RAY)],
    );
    let mut invalid = Vec::new();
    let mut e = valid.clone();
    e.clocks[0].global_state_count += 1;
    invalid.push(e);
    let mut e = valid.clone();
    e.global_state[0].scale = "1".into();
    invalid.push(e);
    let mut e = valid.clone();
    e.holder_basis[0].bit_width = 128;
    invalid.push(e);
    let mut e = valid.clone();
    e.global_state.push(e.global_state[0].clone());
    e.clocks[0].global_state_count += 1;
    invalid.push(e);
    let mut e = valid.clone();
    e.clocks[0].parameters_sha256 = "cd".repeat(32);
    invalid.push(e);
    let mut e = valid.clone();
    e.clocks[0].spec_revision += 1;
    invalid.push(e);
    let mut e = valid.clone();
    e.clocks[0].chain_id = 1;
    invalid.push(e);
    let mut e = valid.clone();
    e.clocks[0].package_version = "2".into();
    invalid.push(e);
    let mut e = valid.clone();
    e.clocks[0].parent_hash = vec![0; 32];
    invalid.push(e);
    let mut e = valid.clone();
    e.clocks[0].timestamp = 99;
    invalid.push(e);
    let mut e = valid;
    e.global_state[0].epoch += 1;
    invalid.push(e);
    for block in invalid {
        assert!(state.apply(&block).is_err());
        assert_eq!(state.report(), before);
        assert_eq!(
            state
                .global(&GlobalKey::new(
                    &MARKET,
                    pb::StateField::AaveLiquidityIndex,
                    &[],
                    pb::Observation::ObservedWrite
                ))
                .unwrap(),
            &input
        );
        assert_eq!(amount(&qualified, &state, Metric::HolderBasis), "5");
    }
}

#[test]
fn qualifications_fail_closed_for_source_scale_epoch_and_metric_changes() {
    let (initial, qualified) = aave_initial("5");
    let state = seed(initial);
    for edit in 0..10 {
        let mut q = qualified.clone();
        match edit {
            0 => q.epoch.source_pin.push_str("-unreviewed"),
            1 => q.epoch.basis_scale = "1".into(),
            2 => q.epoch.epoch += 1,
            3 => q.stream.chain_id += 1,
            4 => q.inputs[0].storage_slot = vec![99; 32],
            5 => q.inputs[0].observation = pb::Observation::ObservedLog,
            6 => q.evidence.clear(),
            7 => q.runtime = None,
            8 => q.runtime.as_mut().unwrap().at.hash = vec![88; 32],
            _ => q.runtime.as_mut().unwrap().evidence.clear(),
        }
        assert!(q.evaluate(&state, &HOLDER, Metric::AaveBalanceOf).is_err());
    }
    assert!(qualified.evaluate(&state, &HOLDER, Metric::LidoBalanceOf).is_err());
}

#[test]
fn epoch_carryover_preserves_original_facts_but_rebinds_rounding() {
    let (mut initial, old) = aave_initial("1");
    initial.global_state[0].value = "1500000000000000000000000000".into();
    initial.global_state[1].value = "0".into();
    let mut state = seed(initial);
    assert_eq!(amount(&old, &state, Metric::AaveBalanceOf), "1");
    let mut next = model(ReferenceModel::AaveAtoken(Era::HalfUp));
    next.epoch = 2;
    next.activation_block = 11;
    let mut qualified = old.clone();
    qualified.model = ReferenceModel::AaveAtoken(Era::HalfUp);
    qualified.epoch = next.clone();
    let transition = events(11, 100, vec![next], vec![], vec![]);
    qualified.runtime.as_mut().unwrap().at = transition.clocks[0].clone();
    state.apply(&transition).unwrap();
    assert_eq!(amount(&qualified, &state, Metric::AaveBalanceOf), "2");
    assert_eq!(state.holder(&MARKET, &HOLDER).unwrap().row.epoch, 1);
    assert_eq!(state.holder(&MARKET, &HOLDER).unwrap().effective_epoch, 2);
    assert!(old.evaluate(&state, &HOLDER, Metric::AaveBalanceOf).is_err());
    state.undo(10).unwrap();
    assert_eq!(amount(&old, &state, Metric::AaveBalanceOf), "1");
    let mut bad = qualified.epoch;
    bad.basis_bit_width = 128;
    assert!(state.apply(&events(11, 100, vec![bad], vec![], vec![])).is_err());
}

#[test]
fn suspension_gap_cannot_be_repaired_by_heartbeat_or_carryover_flags() {
    let (initial, qualified) = aave_initial("5");
    let mut state = seed(initial);
    let invalidated = pb::ModelEpoch {
        kind: pb::EpochEventKind::Invalidated as i32,
        reason: pb::InvalidationReason::CodeChange as i32,
        ordinal: 4,
        ..qualified.epoch.clone()
    };
    state.apply(&events(11, 101, vec![invalidated], vec![], vec![])).unwrap();
    let heartbeat = pb::ModelEpoch {
        kind: pb::EpochEventKind::Reaffirmed as i32,
        ..qualified.epoch.clone()
    };
    state.apply(&events(12, 102, vec![heartbeat], vec![], vec![])).unwrap();
    assert!(matches!(state.holder(&MARKET, &HOLDER), Err(Unavailable::Suspended { .. })));
    let mut next = qualified.epoch.clone();
    next.epoch = 2;
    next.activation_block = 13;
    state.apply(&events(13, 103, vec![next], vec![], vec![])).unwrap();
    assert!(state.holder(&MARKET, &HOLDER).is_err());
    assert!(state
        .global(&GlobalKey::new(
            &MARKET,
            pb::StateField::AaveLiquidityIndex,
            &[],
            pb::Observation::ObservedWrite
        ))
        .is_err());
    state.undo(10).unwrap();
    assert_eq!(amount(&qualified, &state, Metric::AaveBalanceOf), "5");
}

#[test]
fn derived_inputs_expire_and_log_observations_never_replace_stored_words() {
    let (initial, qualified) = aave_initial("5");
    let mut state = seed(initial);
    let mut derived = global(&qualified.epoch, pb::StateField::LidoTotalPooledEther, "500", "1");
    derived.observation = pb::Observation::Derived as i32;
    let mut log = global(&qualified.epoch, pb::StateField::AaveLiquidityIndex, "999", RAY);
    log.observation = pb::Observation::ObservedLog as i32;
    log.boundary = pb::Boundary::Change as i32;
    state.apply(&events(11, 100, vec![], vec![], vec![derived, log])).unwrap();
    let key = GlobalKey::new(&MARKET, pb::StateField::LidoTotalPooledEther, &[], pb::Observation::Derived);
    assert_eq!(state.global(&key).unwrap().row.value, "500");
    assert_eq!(amount(&qualified, &state, Metric::AaveBalanceOf), "5");
    state.apply(&events(12, 100, vec![], vec![], vec![])).unwrap();
    assert_eq!(state.global(&key), Err(Unavailable::Missing));
    state.undo(11).unwrap();
    assert_eq!(state.global(&key).unwrap().row.value, "500");
}

#[test]
fn oz_donation_uses_explicit_code_bound_constant_and_never_infers_zero_offset() {
    let epoch = model(ReferenceModel::OzVirtualOffset);
    let mut offset = global(&epoch, pb::StateField::Erc4626DecimalsOffset, "0", "1");
    offset.observation = pb::Observation::QualifiedConstant as i32;
    offset.boundary = pb::Boundary::Declaration as i32;
    offset.scope = pb::Scope::Epoch as i32;
    offset.storage_slot.clear();
    offset.bit_width = 0;
    let initial = events(
        10,
        100,
        vec![epoch.clone()],
        vec![holder(&epoch, "10")],
        vec![
            global(&epoch, pb::StateField::Erc4626TotalAssets, "99", "1"),
            global(&epoch, pb::StateField::Erc4626TotalSupply, "99", "1"),
            offset.clone(),
        ],
    );
    let qualified = qualify(&initial, ReferenceModel::OzVirtualOffset);
    let mut missing = initial.clone();
    missing.global_state.pop();
    missing.clocks[0].global_state_count -= 1;
    assert!(qualified.evaluate(&seed(missing), &HOLDER, Metric::OzConvertToAssets).is_err());
    let mut unbound = initial.clone();
    unbound.epochs[0].kind = pb::EpochEventKind::Reaffirmed as i32;
    let mut cold = ProtocolLedger::new(2);
    cold.apply(&unbound).unwrap();
    assert_eq!(cold.report().qualified_constants, 0);
    assert!(qualified.evaluate(&cold, &HOLDER, Metric::OzConvertToAssets).is_err());
    let mut state = seed(initial);
    assert_eq!(amount(&qualified, &state, Metric::OzConvertToAssets), "10");
    state
        .apply(&events(
            11,
            101,
            vec![],
            vec![],
            vec![global(&epoch, pb::StateField::Erc4626TotalAssets, "199", "1")],
        ))
        .unwrap();
    assert_eq!(amount(&qualified, &state, Metric::OzConvertToAssets), "20");
    let before = state.report();
    offset.value = "1".into();
    let heartbeat = pb::ModelEpoch {
        kind: pb::EpochEventKind::Reaffirmed as i32,
        ..epoch
    };
    assert!(state.apply(&events(12, 102, vec![heartbeat], vec![], vec![offset])).is_err());
    assert_eq!(state.report(), before);
}

#[test]
fn static_atoken_and_sdai_project_idle_clocks_without_holder_writes() {
    let (mut initial, _) = aave_initial("1000000000");
    let epoch = model(ReferenceModel::StaticAToken);
    initial.epochs[0] = epoch.clone();
    initial.holder_basis[0] = holder(&epoch, "1000000000");
    let qualified = qualify(&initial, ReferenceModel::StaticAToken);
    let mut state = seed(initial);
    state.apply(&events(11, 101, vec![], vec![], vec![])).unwrap();
    assert_eq!(amount(&qualified, &state, Metric::StaticATokenConvertToAssets), "1000000001");
    assert!(qualified.evaluate(&state, &[], Metric::StaticATokenRate).is_ok());
    let epoch = model(ReferenceModel::SavingsDai);
    let initial = events(
        10,
        100,
        vec![epoch.clone()],
        vec![holder(&epoch, "10")],
        vec![
            global(&epoch, pb::StateField::MakerPotChi, RAY, RAY),
            global(&epoch, pb::StateField::MakerPotDsr, "2000000000000000000000000000", RAY),
            global(&epoch, pb::StateField::MakerPotRho, "100", "1"),
        ],
    );
    let qualified = qualify(&initial, ReferenceModel::SavingsDai);
    let mut state = seed(initial);
    state.apply(&events(11, 102, vec![], vec![], vec![])).unwrap();
    assert_eq!(amount(&qualified, &state, Metric::SavingsDaiConvertToAssets), "40");
}

#[test]
fn missing_field_marker_invalidates_that_input_and_undo_restores_it() {
    let (initial, qualified) = aave_initial("5");
    let mut state = seed(initial);
    state
        .apply(&events(
            11,
            101,
            vec![],
            vec![],
            vec![global(&qualified.epoch, pb::StateField::AaveLiquidityIndex, "", RAY)],
        ))
        .unwrap();
    assert!(matches!(
        qualified.evaluate(&state, &HOLDER, Metric::AaveBalanceOf),
        Err(Unknown::MissingInput(_))
    ));
    state.undo(10).unwrap();
    assert_eq!(amount(&qualified, &state, Metric::AaveBalanceOf), "5");
}

#[test]
fn journal_is_bounded_and_refused_undo_preserves_all_state() {
    let (initial, qualified) = aave_initial("5");
    let mut state = seed(initial);
    for n in 11..20 {
        state.apply(&events(n, 100, vec![], vec![], vec![])).unwrap();
    }
    assert_eq!(state.report().undo_snapshots, 4);
    let before = state.report();
    assert!(state.undo(10).is_err());
    assert_eq!(state.report(), before);
    assert_eq!(amount(&qualified, &state, Metric::AaveBalanceOf), "5");
}

#[test]
fn unbound_basis_counts_are_distinct_from_active_model_holders() {
    let (mut initial, qualified) = aave_initial("0");
    initial.epochs.clear();
    initial.clocks[0].epoch_count = 0;
    let mut state = ProtocolLedger::new(3);
    state.apply(&initial).unwrap();
    assert_eq!(state.report().raw_basis_ledger.initialized_holders, 1);
    assert_eq!(state.report().active_model_basis_holders, 0);
    let mut bound = qualified.epoch;
    bound.activation_block = 11;
    state.apply(&events(11, 101, vec![bound], vec![], vec![])).unwrap();
    assert_eq!(state.report().active_model_basis_holders, 0);
    assert_eq!(state.holder(&MARKET, &HOLDER), Err(Unavailable::Missing));
}

#[test]
fn malformed_optional_model_identities_are_refused() {
    let (initial, _) = aave_initial("1");
    for field in 0..5 {
        let mut malformed = initial.clone();
        match field {
            0 => malformed.epochs[0].implementation = vec![1],
            1 => malformed.epochs[0].implementation_slot = vec![1],
            2 => malformed.epochs[0].implementation_code_hash = vec![1],
            3 => malformed.epochs[0].market_code_hash = vec![1],
            _ => malformed.epochs[0].balance_asset = vec![1],
        }
        assert!(ProtocolLedger::from_checkpoint(
            2,
            Checkpoint {
                events: malformed,
                evidence: "synthetic negative control".into()
            }
        )
        .is_err());
    }
}

#[test]
fn attested_checkpoint_header_and_holder_location_must_match() {
    let (initial, qualified) = aave_initial("7");
    for field in 0..3 {
        let mut changed = initial.clone();
        match field {
            0 => changed.clocks[0].timestamp += 1,
            1 => changed.clocks[0].state_root = vec![77; 32],
            _ => changed.clocks[0].producer_version = 4,
        }
        assert!(qualified.evaluate(&seed(changed), &HOLDER, Metric::AaveBalanceOf).is_err());
    }
    let mut state = seed(initial);
    for field in 0..2 {
        let mut row = holder(&qualified.epoch, "8");
        if field == 0 {
            row.storage_contract = vec![77; 20];
        } else {
            row.storage_slot = vec![77; 32];
        }
        state.apply(&events(11, 101, vec![], vec![row], vec![])).unwrap();
        assert!(qualified.evaluate(&state, &HOLDER, Metric::AaveBalanceOf).is_err());
        state.undo(10).unwrap();
        assert_eq!(amount(&qualified, &state, Metric::AaveBalanceOf), "7");
    }
}

#[test]
fn partial_or_changed_dependency_heartbeat_is_atomic() {
    let (mut initial, _) = aave_initial("7");
    initial.dependencies.push(pb::Dependency {
        chain_id: 56,
        market: MARKET.to_vec(),
        epoch: 1,
        kind: pb::EpochEventKind::Bound as i32,
        role: pb::DependencyRole::Pool as i32,
        contract: vec![2; 20],
        depth: 1,
        binding: pb::BindingKind::CodeHash as i32,
        code_hash: vec![99; 32],
        activation_block: 10,
        source_pin: "synthetic dependency fixture".into(),
        ..Default::default()
    });
    counts(&mut initial);
    let mut state = seed(initial.clone());
    let before = state.report();
    let heartbeat = pb::ModelEpoch {
        kind: pb::EpochEventKind::Reaffirmed as i32,
        ..initial.epochs[0].clone()
    };
    let mut next = events(11, 101, vec![heartbeat], vec![], vec![]);
    assert!(state.apply(&next).is_err());
    assert_eq!(state.report(), before);
    next.dependencies = initial.dependencies.clone();
    next.dependencies[0].kind = pb::EpochEventKind::Reaffirmed as i32;
    next.dependencies[0].contract = vec![3; 20];
    counts(&mut next);
    assert!(state.apply(&next).is_err());
    assert_eq!(state.report(), before);
}

#[test]
fn intermediate_writes_require_a_final_row_and_continuity_is_checked() {
    let (initial, qualified) = aave_initial("7");
    let mut state = seed(initial);
    let before = state.report();
    let final_row = global(&qualified.epoch, pb::StateField::AaveLiquidityIndex, RAY, RAY);
    let mut intermediate = final_row.clone();
    intermediate.boundary = pb::Boundary::Change as i32;
    intermediate.ordinal = 10;
    assert!(state.apply(&events(11, 101, vec![], vec![], vec![intermediate.clone()])).is_err());
    assert_eq!(state.report(), before);
    let mut discontinuous = final_row.clone();
    discontinuous.previous_value = "42".into();
    assert!(state.apply(&events(11, 101, vec![], vec![], vec![discontinuous])).is_err());
    assert_eq!(state.report(), before);
    state.apply(&events(11, 101, vec![], vec![], vec![final_row.clone(), intermediate])).unwrap();
    assert_eq!(
        state
            .global(&GlobalKey::new(
                &MARKET,
                pb::StateField::AaveLiquidityIndex,
                &[],
                pb::Observation::ObservedWrite
            ))
            .unwrap()
            .row,
        final_row
    );
}

#[test]
fn suspended_observation_gaps_do_not_block_other_markets() {
    let (mut initial, first) = aave_initial("7");
    let mut other = first.epoch.clone();
    other.market = vec![2; 20];
    initial.epochs.push(other.clone());
    initial.holder_basis.push(holder(&other, "9"));
    initial.global_state.extend([
        global(&other, pb::StateField::AaveLiquidityIndex, RAY, RAY),
        global(&other, pb::StateField::AaveCurrentLiquidityRate, "0", RAY),
        global(&other, pb::StateField::AaveLastUpdateTimestamp, "100", "1"),
    ]);
    counts(&mut initial);
    let mut qualified = qualify(&initial, ReferenceModel::AaveAtoken(Era::Floor));
    qualified.epoch = other.clone();
    qualified.inputs = initial.global_state.iter().filter(|g| g.market == other.market).map(input).collect();
    qualified.holder_binding.storage_contract = other.market.clone();
    let mut state = seed(initial);
    let invalidation = pb::ModelEpoch {
        kind: pb::EpochEventKind::Invalidated as i32,
        reason: pb::InvalidationReason::CodeChange as i32,
        ..first.epoch.clone()
    };
    state.apply(&events(11, 101, vec![invalidation], vec![], vec![])).unwrap();
    // The invalidated block omitted changes. A stateless projector's next
    // same-epoch rows can therefore start at values this ledger never saw.
    let mut stale_holder = holder(&first.epoch, "101");
    stale_holder.previous_value = "100".into();
    let mut stale_global = global(&first.epoch, pb::StateField::AaveLiquidityIndex, "3000000000000000000000000000", RAY);
    stale_global.previous_value = "2000000000000000000000000000".into();
    let mut active_holder = holder(&other, "10");
    active_holder.previous_value = "9".into();
    state
        .apply(&events(12, 102, vec![], vec![stale_holder, active_holder], vec![stale_global]))
        .unwrap();
    assert!(matches!(state.holder(&MARKET, &HOLDER), Err(Unavailable::Suspended { .. })));
    assert_eq!(qualified.evaluate(&state, &HOLDER, Metric::AaveBalanceOf).unwrap().value.to_string(), "10");
    assert_eq!(state.report().active_model_basis_holders, 1);
    let mut rebound = first.epoch;
    rebound.epoch = 2;
    rebound.activation_block = 13;
    state.apply(&events(13, 103, vec![rebound], vec![], vec![])).unwrap();
    assert_eq!(state.holder(&MARKET, &HOLDER), Err(Unavailable::Missing));
    assert!(state
        .global(&GlobalKey::new(
            &MARKET,
            pb::StateField::AaveLiquidityIndex,
            &[],
            pb::Observation::ObservedWrite
        ))
        .is_err());
    state.undo(10).unwrap();
    assert_eq!(qualified.evaluate(&state, &HOLDER, Metric::AaveBalanceOf).unwrap().value.to_string(), "9");
}

#[test]
fn predecessor_rows_before_uninterrupted_bound_must_match_retained_values() {
    let (initial, first) = aave_initial("7");
    let mut state = seed(initial);
    let before = state.report();
    let mut next = first.epoch.clone();
    next.epoch = 2;
    next.activation_block = 11;
    next.activation_ordinal = 30;
    next.ordinal = 30;
    let mut predecessor = holder(&first.epoch, "9");
    predecessor.previous_value = "42".into();
    let mut global = global(&first.epoch, pb::StateField::AaveLiquidityIndex, "2000000000000000000000000000", RAY);
    global.previous_value = RAY.into();
    let mut transition = events(11, 100, vec![next.clone()], vec![predecessor], vec![global]);
    assert!(state.apply(&transition).is_err());
    assert_eq!(state.report(), before);
    transition.holder_basis[0].previous_value = "7".into();
    transition.global_state[0].previous_value = "42".into();
    assert!(state.apply(&transition).is_err());
    assert_eq!(state.report(), before);
    transition.global_state[0].previous_value = RAY.into();
    state.apply(&transition).unwrap();
    let mut qualified = first.clone();
    qualified.epoch = next;
    qualified.runtime.as_mut().unwrap().at = transition.clocks[0].clone();
    assert_eq!(amount(&qualified, &state, Metric::AaveBalanceOf), "18");
    assert_eq!(state.holder(&MARKET, &HOLDER).unwrap().row.epoch, 1);
    state.undo(10).unwrap();
    assert_eq!(amount(&first, &state, Metric::AaveBalanceOf), "7");
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
            state_root: vec![8; 32],
            timestamp: Some(prost_types::Timestamp {
                seconds: timestamp as i64,
                nanos: 0,
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn counts(events: &mut pb::Events) {
    let clock = &mut events.clocks[0];
    clock.epoch_count = events.epochs.len() as u32;
    clock.dependency_count = events.dependencies.len() as u32;
    clock.holder_basis_count = events.holder_basis.len() as u32;
    clock.global_state_count = events.global_state.len() as u32;
}

#[test]
fn captured_aave_reserve_update_is_an_independent_retained_clock_oracle() {
    // Unmodified saved BSC transaction fixture; source hashes and limitations
    // are in aave/balance-state/tests/fixtures/cases.json. The holder and
    // bootstrap below are synthetic. This checks the captured Pool index,
    // not a real holder getter, complete parent checkpoint or live package.
    let cfg = aave_balance_state::parse(include_str!("../../aave/balance-state/tests/fixtures/bsc-aave-v3-epochs.json")).unwrap();
    let captured = eth::Block::decode(include_bytes!("../../aave/balance-state/tests/fixtures/122288734-tx81-ausdt-supply.pb").as_slice()).unwrap();
    let output = aave_balance_state::project(&captured, &cfg).unwrap();
    let market = cfg.markets[0].atoken.clone();
    let mut declarations = aave_balance_state::project(&block(cfg.markets[0].activation_block, 1789591800), &cfg).unwrap();
    declarations.epochs.retain(|e| e.market == market);
    declarations.dependencies.retain(|d| d.market == market);
    let epoch = declarations.epochs[0].clone();
    declarations.holder_basis = vec![holder(&epoch, RAY)];
    declarations.holder_basis[0].storage_slot = holder_storage_slot(&HOLDER, &cfg.markets[0].user_state_slot).unwrap();
    declarations.global_state = output
        .global_state
        .iter()
        .filter(|g| {
            g.market == market
                && matches!(
                    pb::StateField::try_from(g.field),
                    Ok(pb::StateField::AaveLiquidityIndex | pb::StateField::AaveCurrentLiquidityRate | pb::StateField::AaveLastUpdateTimestamp)
                )
        })
        .cloned()
        .collect();
    assert_eq!(declarations.global_state.len(), 3);
    let observed_index = declarations
        .global_state
        .iter()
        .find(|g| g.field == pb::StateField::AaveLiquidityIndex as i32)
        .unwrap()
        .value
        .clone();
    assert_eq!(observed_index, "1124515086691634348902975804");
    for row in &mut declarations.global_state {
        row.value = row.previous_value.clone();
        row.raw_word = row.raw_previous_word.clone();
    }
    declarations.clocks[0] = output.clocks[0].clone();
    declarations.clocks[0].number -= 1;
    declarations.clocks[0].hash = output.clocks[0].parent_hash.clone();
    declarations.clocks[0].parent_hash = vec![0; 32];
    declarations.clocks[0].timestamp -= 1;
    counts(&mut declarations);
    let mut qualified = qualify(&declarations, ReferenceModel::AaveAtoken(Era::Floor));
    qualified.holder_binding.mapping_slot = cfg.markets[0].user_state_slot.to_vec();
    qualified.evidence = "captured Pool index oracle in 122288734-tx81-ausdt-supply.pb; synthetic holder/bootstrap only".into();
    let mut retained = seed(declarations);
    let mut clock_only = output.clone();
    clock_only.epochs.clear();
    clock_only.dependencies.clear();
    clock_only.holder_basis.clear();
    clock_only.global_state.clear();
    counts(&mut clock_only);
    retained.apply(&clock_only).unwrap();
    let result = qualified.evaluate(&retained, &HOLDER, Metric::AaveBalanceOf).unwrap();
    assert_eq!(result.value.to_string(), observed_index);
    assert_eq!(result.clock.hash, captured.hash);
    assert_eq!(result.holder.unwrap().observed_at.number, 122288733);
}

fn packed(low: u128, high: u128) -> Vec<u8> {
    [high.to_be_bytes(), low.to_be_bytes()].concat()
}

#[test]
fn actual_lido_projector_logs_are_separate_from_stored_getter_inputs() {
    let mut params: serde_json::Value = serde_json::from_str(include_str!("../../lido/balance-state/tests/fixtures/mainnet-steth-v4-epoch.json")).unwrap();
    params["epochs"][0]["activation_block"] = 10.into();
    let cfg = lido_balance_state::parse(&params.to_string()).unwrap();
    let epoch = &cfg.epochs[0];
    let mut input = block(10, 100);
    let holder_slot = lido_balance_state::mapping_key(&HOLDER, &epoch.shares_slot);
    let mut preimage = vec![0; 12];
    preimage.extend_from_slice(&HOLDER);
    preimage.extend_from_slice(&epoch.shares_slot);
    let writes = [
        (holder_slot, packed(1, 0), packed(2, 0)),
        (epoch.total_and_external_shares_slot, packed(2, 0), packed(3, 1)),
        (epoch.buffered_slot, packed(2, 1), packed(1, 0)),
        (epoch.cl_slot, packed(1, 1), packed(0, 0)),
    ];
    let call = eth::Call {
        address: epoch.steth.clone(),
        keccak_preimages: [(hex::encode(holder_slot), hex::encode(preimage))].into(),
        storage_changes: writes
            .into_iter()
            .enumerate()
            .map(|(i, (key, old, new))| eth::StorageChange {
                address: epoch.steth.clone(),
                key: key.to_vec(),
                old_value: old,
                new_value: new,
                ordinal: 20 + i as u64,
            })
            .collect(),
        ..Default::default()
    };
    let log = eth::Log {
        address: epoch.steth.clone(),
        topics: vec![
            lido_balance_state::keccak(b"TokenRebased(uint256,uint256,uint256,uint256,uint256,uint256,uint256)").to_vec(),
            packed(100, 0),
        ],
        data: [3600, 2, 1, 3, 1, 0].into_iter().flat_map(|value| packed(value, 0)).collect(),
        ordinal: 30,
        index: 1,
        block_index: 1,
    };
    input.transaction_traces = vec![eth::TransactionTrace {
        status: eth::TransactionTraceStatus::Succeeded as i32,
        hash: vec![7; 32],
        calls: vec![call],
        receipt: Some(eth::TransactionReceipt {
            logs: vec![log],
            ..Default::default()
        }),
        ..Default::default()
    }];
    let output = lido_balance_state::project(&input, &cfg).unwrap();
    assert_eq!(
        output
            .global_state
            .iter()
            .filter(|g| g.observation == pb::Observation::ObservedLog as i32 && g.boundary == pb::Boundary::Change as i32)
            .count(),
        4
    );
    let qualified = qualify(&output, ReferenceModel::LidoV4);
    let mut state = ProtocolLedger::new(4);
    state.apply(&output).unwrap();
    assert_eq!(qualified.evaluate(&state, &HOLDER, Metric::LidoBalanceOf).unwrap().value.to_string(), "1");
    // A hypothetical conversion at the log's ratio is different from the
    // getter. It is deliberately not a retained-evaluator metric: EOB holder
    // basis need not be the holder's basis at the report ordinal.
    assert_eq!(
        conformance::lido::pooled_eth_from_report(&2u32.into(), &3u32.into(), &1u32.into())
            .unwrap()
            .to_string(),
        "0"
    );
    assert_eq!(state.report().current_block_derived, 1);
    let idle = lido_balance_state::project(&block(11, 101), &cfg).unwrap();
    state.apply(&idle).unwrap();
    assert_eq!(state.report().current_block_derived, 0);
    assert_eq!(qualified.evaluate(&state, &HOLDER, Metric::LidoBalanceOf).unwrap().value.to_string(), "1");
    state.undo(10).unwrap();
    assert_eq!(state.report().current_block_derived, 1);
    // A partial newer log remains evidence only; it cannot affect the
    // stored-input getter or become a historical holder balance.
    let mut partial = idle;
    let mut one_field = output
        .global_state
        .iter()
        .find(|g| g.field == pb::StateField::LidoReportPostTotalEther as i32)
        .unwrap()
        .clone();
    one_field.ordinal = 40;
    one_field.value = "6".into();
    partial.global_state = vec![one_field.clone()];
    counts(&mut partial);
    state.apply(&partial).unwrap();
    assert_eq!(qualified.evaluate(&state, &HOLDER, Metric::LidoBalanceOf).unwrap().value.to_string(), "1");
    state.undo(10).unwrap();
    let mut older = one_field.clone();
    older.ordinal = 35;
    older.value = "4".into();
    partial.global_state = vec![one_field.clone(), older];
    counts(&mut partial);
    state.apply(&partial).unwrap();
    let key = GlobalKey::new(&epoch.steth, pb::StateField::LidoReportPostTotalEther, &[], pb::Observation::ObservedLog);
    assert_eq!(state.global(&key).unwrap().row.value, "6");
    state.undo(10).unwrap();
    partial.global_state = vec![one_field.clone(), one_field];
    counts(&mut partial);
    let before = state.report();
    assert!(state.apply(&partial).is_err());
    assert_eq!(state.report(), before);
}
