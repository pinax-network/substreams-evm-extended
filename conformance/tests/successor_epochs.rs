//! Actual projectors and host consumer fed synthetic Extended blocks. No runtime,
//! deployment, holder enumeration or live stream qualification is inferred.
#![cfg(not(target_arch = "wasm32"))]
use evm_retention::protocol::{Checkpoint, GlobalKey, ProtocolLedger, Unavailable};
use proto::pb::evm::balance_state::v1 as pb;
use substreams_ethereum::pb::eth::v2 as eth;

type Project = Box<dyn Fn(&eth::Block) -> Result<pb::Events, String>>;
struct Case {
    name: &'static str,
    markets: [Vec<u8>; 2],
    basis_slot: [u8; 32],
    global_slot: [u8; 32],
    pointer_slot: [u8; 32],
    expected_pointer: [u8; 32],
    shared_code: Vec<u8>,
    project: Project,
}

fn params(source: &str, list: &str, address: &str) -> String {
    let mut raw: serde_json::Value = serde_json::from_str(source).unwrap();
    let mut first = raw[list][0].clone();
    first["epoch"] = 1.into();
    first["activation_block"] = 1.into();
    first["activation_ordinal"] = 0.into();
    let mut second = first.clone();
    second[address] = format!("0x{}", hex::encode([0x88; 20])).into();
    if address == "atoken" {
        // Each Aave reserve has a single active aToken, even though the Pool
        // and implementation dependencies are shared across reserves.
        second["underlying"] = format!("0x{}", hex::encode([0x77; 20])).into();
    }
    // Deliberately synthetic second deployment with shared dependencies.
    let mut successor = first.clone();
    successor["epoch"] = 3.into();
    successor["activation_block"] = 2.into();
    successor["activation_ordinal"] = 30.into();
    raw[list] = serde_json::json!([first, second, successor]);
    raw["heartbeat_blocks"] = 1.into();
    raw.to_string()
}

fn word(value: u8) -> [u8; 32] {
    let mut out = [0; 32];
    out[31] = value;
    out
}
fn address_word(address: &[u8]) -> [u8; 32] {
    let mut out = [0; 32];
    out[12..].copy_from_slice(address);
    out
}

fn case(name: &str) -> Case {
    match name {
        "aave" => {
            let a = aave_balance_state::parse(&params(
                include_str!("../../aave/balance-state/tests/fixtures/bsc-aave-v3-epochs.json"),
                "markets",
                "atoken",
            ))
            .unwrap();
            Case {
                name: "aave",
                markets: [a.markets[0].atoken.clone(), a.markets[1].atoken.clone()],
                basis_slot: a.markets[0].user_state_slot,
                global_slot: a.markets[0].total_supply_slot,
                pointer_slot: a.markets[0].implementation_slot,
                expected_pointer: address_word(&a.markets[0].implementation),
                shared_code: a.pool.as_ref().unwrap().address.clone(),
                project: Box::new(move |b| aave_balance_state::project(b, &a).map_err(|e| e.to_string())),
            }
        }
        "compound-v2" => {
            let v2 = compound_v2_balance_state::parse(&params(
                include_str!("../../compound-v2/balance-state/tests/fixtures/mainnet-ctoken-epochs.json"),
                "markets",
                "ctoken",
            ))
            .unwrap();
            Case {
                name: "compound-v2",
                markets: [v2.markets[0].ctoken.clone(), v2.markets[1].ctoken.clone()],
                basis_slot: v2.markets[0].account_tokens_slot,
                global_slot: v2.markets[0]
                    .scalars
                    .iter()
                    .find(|(_, f, _)| *f == pb::StateField::CompoundV2TotalSupply)
                    .unwrap()
                    .0,
                pointer_slot: v2.markets[0].rate_model_slot,
                expected_pointer: address_word(&v2.markets[0].rate_model),
                shared_code: v2.markets[0].rate_model.clone(),
                project: Box::new(move |b| compound_v2_balance_state::project(b, &v2).map_err(|e| e.to_string())),
            }
        }
        "compound-v3" => {
            let v3 = compound_v3_balance_state::parse(&params(
                include_str!("../../compound-v3/balance-state/tests/fixtures/mainnet-cusdcv3-epoch.json"),
                "markets",
                "comet",
            ))
            .unwrap();
            Case {
                name: "compound-v3",
                markets: [v3.markets[0].comet.clone(), v3.markets[1].comet.clone()],
                basis_slot: v3.markets[0].user_basic_slot,
                global_slot: v3.markets[0].totals_slot,
                pointer_slot: v3.markets[0].implementation_slot,
                expected_pointer: address_word(&v3.markets[0].implementation),
                shared_code: v3.markets[0].implementation.clone(),
                project: Box::new(move |b| compound_v3_balance_state::project(b, &v3).map_err(|e| e.to_string())),
            }
        }
        "lido" => {
            let l = lido_balance_state::parse(&params(
                include_str!("../../lido/balance-state/tests/fixtures/mainnet-steth-v4-epoch.json"),
                "epochs",
                "steth",
            ))
            .unwrap();
            Case {
                name: "lido",
                markets: [l.epochs[0].steth.clone(), l.epochs[1].steth.clone()],
                basis_slot: l.epochs[0].shares_slot,
                global_slot: l.epochs[0].total_and_external_shares_slot,
                pointer_slot: l.epochs[0].aragon.kernel_slot,
                expected_pointer: address_word(&l.epochs[0].aragon.kernel),
                shared_code: l.epochs[0].aragon.kernel.clone(),
                project: Box::new(move |b| lido_balance_state::project(b, &l).map_err(|e| e.to_string())),
            }
        }
        "erc4626" => {
            let v = erc4626_balance_state::parse(&params(
                include_str!("../../erc4626/balance-state/tests/fixtures/bsc-stata-usdt-epoch.json"),
                "vaults",
                "vault",
            ))
            .unwrap();
            let erc4626_balance_state::Model::AaveStaticAToken { pool, .. } = &v.vaults[0].model else {
                panic!()
            };
            Case {
                name: "erc4626",
                markets: [v.vaults[0].vault.clone(), v.vaults[1].vault.clone()],
                basis_slot: v.vaults[0].balances_slot,
                global_slot: v.vaults[0].total_supply_slot,
                pointer_slot: v.vaults[0].implementation_slot.unwrap(),
                expected_pointer: address_word(v.vaults[0].implementation.as_ref().unwrap()),
                shared_code: pool.clone(),
                project: Box::new(move |b| erc4626_balance_state::project(b, &v).map_err(|e| e.to_string())),
            }
        }
        _ => panic!("unknown family"),
    }
}

fn block(number: u64) -> eth::Block {
    eth::Block {
        ver: 5,
        number,
        hash: vec![number as u8; 32],
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        header: Some(eth::BlockHeader {
            number,
            parent_hash: vec![(number - 1) as u8; 32],
            state_root: vec![7; 32],
            timestamp: Some(prost_types::Timestamp {
                seconds: 1_789_689_600 + number as i64,
                nanos: 0,
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn tx(call: eth::Call) -> eth::TransactionTrace {
    eth::TransactionTrace {
        status: eth::TransactionTraceStatus::Succeeded as i32,
        hash: vec![0x77; 32],
        index: 9,
        calls: vec![call],
        ..Default::default()
    }
}
fn write(address: &[u8], slot: [u8; 32], old: [u8; 32], new: [u8; 32], ordinal: u64) -> eth::StorageChange {
    eth::StorageChange {
        address: address.to_vec(),
        key: slot.to_vec(),
        old_value: old.to_vec(),
        new_value: new.to_vec(),
        ordinal,
    }
}
fn holder_call(case: &Case, market: usize, old: u8, new: u8, ordinal: u64) -> eth::Call {
    let holder = [0x99; 20];
    let key = lido_balance_state::mapping_key(&holder, &case.basis_slot);
    let mut preimage = [0; 64];
    preimage[12..32].copy_from_slice(&holder);
    preimage[32..].copy_from_slice(&case.basis_slot);
    eth::Call {
        index: 1,
        address: case.markets[market].clone(),
        keccak_preimages: [(hex::encode(key), hex::encode(preimage))].into(),
        storage_changes: vec![write(&case.markets[market], key, word(old), word(new), ordinal)],
        ..Default::default()
    }
}

fn counts(events: &mut pb::Events) {
    let clock = &mut events.clocks[0];
    clock.holder_basis_count = events.holder_basis.len() as u32;
    clock.global_state_count = events.global_state.len() as u32;
    clock.epoch_count = events.epochs.len() as u32;
    clock.dependency_count = events.dependencies.len() as u32;
}

fn exercise(name: &str) {
    let case = case(name);
    for suffix in [false, true] {
        for invalidated in [false, true] {
            let mut ledger = ProtocolLedger::new(8);
            let mut first = block(1);
            for market in 0..2 {
                let mut call = holder_call(&case, market, 0, 7, 10);
                call.storage_changes.push(write(&case.markets[market], case.global_slot, word(0), word(8), 11));
                first.transaction_traces.push(tx(call));
            }
            let initial = (case.project)(&first).unwrap();
            ledger.apply(&initial).unwrap();
            let global_row = initial
                .global_state
                .iter()
                .find(|row| row.market == case.markets[0] && row.observation == pb::Observation::ObservedWrite as i32)
                .unwrap();
            let global_key = GlobalKey {
                market: global_row.market.clone(),
                field: global_row.field,
                key: global_row.key.clone(),
                observation: global_row.observation,
            };
            assert_eq!(ledger.holder(&case.markets[0], &[0x99; 20]).unwrap().row.value, "7");
            let mut transition = block(2);
            let mut call = holder_call(&case, 0, 7, 9, 20);
            call.storage_changes.push(write(&case.markets[0], case.global_slot, word(8), word(9), 21));
            if invalidated {
                call.storage_changes
                    .push(write(&case.markets[0], case.pointer_slot, case.expected_pointer, case.expected_pointer, 25));
            }
            if suffix {
                call.storage_changes.push(holder_call(&case, 0, 9, 0, 30).storage_changes.remove(0));
                call.storage_changes.push(write(&case.markets[0], case.global_slot, word(9), word(0), 31));
            }
            transition.transaction_traces.push(tx(call));
            let output = (case.project)(&transition).unwrap();
            assert!(output.holder_basis.iter().all(|row| row.epoch == 3), "{}", case.name);
            assert!(output
                .global_state
                .iter()
                .filter(|row| row.market == case.markets[0] && row.observation == pb::Observation::ObservedWrite as i32)
                .all(|row| row.epoch == 3));
            // A shared dependency change at the successor start belongs to it,
            // and cannot be mistaken for only a retired predecessor trigger.
            let mut dependency_block = transition.clone();
            dependency_block.transaction_traces[0].calls[0].code_changes.push(eth::CodeChange {
                address: case.shared_code.clone(),
                ordinal: 30,
                new_hash: vec![8; 32],
                ..Default::default()
            });
            let dependency_output = (case.project)(&dependency_block).unwrap();
            let mut suspended = ledger.clone();
            suspended.apply(&dependency_output).unwrap();
            assert!(matches!(suspended.model(&case.markets[0]), Err(Unavailable::Suspended { epoch: 3, .. })));
            suspended.undo(1).unwrap();
            assert_eq!(suspended.holder(&case.markets[0], &[0x99; 20]).unwrap().row.value, "7");
            ledger.apply(&output).unwrap();
            assert_eq!(ledger.model(&case.markets[0]).unwrap().effective_epoch, 3);
            assert!(!ledger.model(&case.markets[0]).unwrap().row.basis_carryover);
            assert!(!ledger.model(&case.markets[0]).unwrap().row.global_carryover);
            assert_eq!(ledger.holder(&case.markets[1], &[0x99; 20]).unwrap().row.value, "7");
            assert!(ledger.dependencies(&case.markets[0]).unwrap().iter().all(|row| row.epoch == 3));
            if suffix {
                assert_eq!(ledger.holder(&case.markets[0], &[0x99; 20]).unwrap().row.value, "0");
                assert_eq!(ledger.global(&global_key).unwrap().effective_epoch, 3);
            } else {
                assert_eq!(ledger.holder(&case.markets[0], &[0x99; 20]), Err(Unavailable::Missing));
                assert_eq!(ledger.global(&global_key), Err(Unavailable::Missing));
            }
            let quiet = (case.project)(&block(3)).unwrap();
            ledger.apply(&quiet).unwrap();
            assert!(quiet.epochs.iter().filter(|row| row.market == case.markets[0]).all(|row| row.epoch == 3));
            if !suffix {
                assert_eq!(ledger.holder(&case.markets[0], &[0x99; 20]), Err(Unavailable::Missing));
            }
            ledger.undo(1).unwrap();
            assert_eq!(ledger.holder(&case.markets[0], &[0x99; 20]).unwrap().row.value, "7");
            ledger.apply(&output).unwrap();
            ledger.apply(&quiet).unwrap();
            // A new exact checkpoint names only the final active identities and
            // its explicitly initialized set; old heartbeat/dependency rows are not imported.
            let mut snapshot = output.clone();
            snapshot
                .epochs
                .retain(|row| row.market == case.markets[0] && row.epoch == 3 && row.kind == pb::EpochEventKind::Bound as i32);
            snapshot.dependencies.retain(|row| row.market == case.markets[0] && row.epoch == 3);
            snapshot.global_state.retain(|row| row.market == case.markets[0] && row.epoch == 3);
            counts(&mut snapshot);
            let mut restored = ProtocolLedger::from_checkpoint(
                8,
                Checkpoint {
                    events: snapshot,
                    evidence: "synthetic exact post-transition initialized set".into(),
                },
            )
            .unwrap();
            restored.apply(&quiet).unwrap();
            if suffix {
                assert_eq!(restored.holder(&case.markets[0], &[0x99; 20]).unwrap().row.value, "0");
            } else {
                assert_eq!(restored.holder(&case.markets[0], &[0x99; 20]), Err(Unavailable::Missing));
            }
        }
    }
}

#[test]
fn aave_successor_reset_reaches_host_without_carrying_a_prefix() {
    exercise("aave");
}
#[test]
fn compound_v2_successor_reset_reaches_host_without_carrying_a_prefix() {
    exercise("compound-v2");
}
#[test]
fn comet_successor_reset_reaches_host_without_carrying_a_prefix() {
    exercise("compound-v3");
}
#[test]
fn lido_successor_reset_reaches_host_without_carrying_a_prefix() {
    exercise("lido");
}
#[test]
fn erc4626_successor_reset_reaches_host_without_carrying_a_prefix() {
    exercise("erc4626");
}

fn aave_qualification(events: &pb::Events, market: &[u8], epoch: u32, mapping_slot: &[u8]) -> conformance::retained::QualifiedModel {
    use conformance::retained::{HolderBinding, InputBinding, QualifiedModel, ReferenceModel, RuntimeQualification};
    let clock = &events.clocks[0];
    let bound = events
        .epochs
        .iter()
        .find(|row| row.market == market && row.epoch == epoch && row.kind == pb::EpochEventKind::Bound as i32)
        .unwrap();
    let dependencies: Vec<_> = events
        .dependencies
        .iter()
        .filter(|row| row.market == market && row.epoch == epoch)
        .cloned()
        .collect();
    QualifiedModel {
        stream: evm_retention::Stream {
            chain_id: clock.chain_id,
            package: clock.package.clone(),
            package_version: clock.package_version.clone(),
            spec_revision: clock.spec_revision,
            parameters_sha256: clock.parameters_sha256.clone(),
        },
        epoch: bound.clone(),
        inputs: events
            .global_state
            .iter()
            .filter(|row| row.market == market && row.epoch == epoch)
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
        holder_binding: HolderBinding {
            storage_contract: market.to_vec(),
            mapping_slot: mapping_slot.to_vec(),
        },
        model: ReferenceModel::AaveAtoken(conformance::aave::Era::Floor),
        evidence: "synthetic schedule evaluation binding, not a qualified deployment".into(),
        runtime: Some(RuntimeQualification {
            at: clock.clone(),
            market_code_hash: vec![7; 32],
            implementation_code_hash: vec![8; 32],
            dependency_code_hashes: dependencies
                .iter()
                .map(|d| (d.contract.clone(), if d.contract == bound.implementation { vec![8; 32] } else { vec![9; 32] }))
                .collect(),
            evidence: "synthetic runtime attestation for exact fixture header only".into(),
        }),
        dependencies,
    }
}

#[test]
fn aave_successor_evaluation_requires_fresh_inputs_and_an_independent_binding() {
    use conformance::retained::{holder_storage_slot, Metric};
    use conformance::Unknown;
    let source = include_str!("../../aave/balance-state/tests/fixtures/bsc-aave-v3-epochs.json");
    let mut raw: serde_json::Value = serde_json::from_str(&params(source, "markets", "atoken")).unwrap();
    raw["markets"][2]["user_state_slot"] = format!("0x{}", hex::encode(word(63))).into();
    let config = aave_balance_state::parse(&raw.to_string()).unwrap();
    let old = &config.markets[0];
    let next = &config.markets[2];
    let pool = &config.pool.as_ref().unwrap().address;
    let holder = [0x99; 20];
    let wide = |v: u128| {
        let mut out = [0; 32];
        out[16..].copy_from_slice(&v.to_be_bytes());
        out
    };
    let basis = |m: &aave_balance_state::Market, old, new, ordinal| {
        let mut preimage = [0; 64];
        preimage[12..32].copy_from_slice(&holder);
        preimage[32..].copy_from_slice(&m.user_state_slot);
        let key: [u8; 32] = holder_storage_slot(&holder, &m.user_state_slot).unwrap().try_into().unwrap();
        eth::Call {
            address: m.atoken.clone(),
            keccak_preimages: [(hex::encode(key), hex::encode(preimage))].into(),
            storage_changes: vec![write(&m.atoken, key, wide(old), wide(new), ordinal)],
            ..Default::default()
        }
    };
    let offset = |mut slot: [u8; 32], value: u8| {
        let mut carry = u16::from(value);
        for byte in slot.iter_mut().rev() {
            let sum = u16::from(*byte) + carry;
            *byte = sum as u8;
            carry = sum >> 8;
        }
        slot
    };
    let index_slot = offset(old.reserve_base, 1);
    let clock_slot = offset(old.reserve_base, 3);
    let clock_word = |number: u64| {
        let mut out = [0; 32];
        out[..16].copy_from_slice(&(1_789_689_600u128 + number as u128).to_be_bytes());
        out
    };
    let ray = 10u128.pow(27);
    let mut first = block(1);
    let mut call = basis(old, 0, 7, 10);
    call.storage_changes.extend([
        write(pool, index_slot, wide(0), wide(ray), 11),
        write(pool, clock_slot, wide(0), clock_word(1), 12),
    ]);
    first.transaction_traces.push(tx(call));
    let initial = aave_balance_state::project(&first, &config).unwrap();
    let q1 = aave_qualification(&initial, &old.atoken, 1, &old.user_state_slot);
    let mut ledger = ProtocolLedger::new(8);
    ledger.apply(&initial).unwrap();
    assert_eq!(q1.evaluate(&ledger, &holder, Metric::AaveBalanceOf).unwrap().value.to_string(), "7");
    let mut transition = block(2);
    let mut call = basis(old, 7, 9, 20);
    let fresh = basis(next, 0, 5, 30);
    call.keccak_preimages.extend(fresh.keccak_preimages);
    call.storage_changes.extend(fresh.storage_changes);
    call.storage_changes.extend([
        write(pool, index_slot, wide(ray), wide(2 * ray), 21),
        write(pool, index_slot, wide(2 * ray), wide(3 * ray), 31),
    ]);
    transition.transaction_traces.push(tx(call));
    let partial = aave_balance_state::project(&transition, &config).unwrap();
    transition.transaction_traces[0].calls[0]
        .storage_changes
        .push(write(pool, clock_slot, clock_word(1), clock_word(2), 32));
    let complete = aave_balance_state::project(&transition, &config).unwrap();
    let q3 = aave_qualification(&complete, &next.atoken, 3, &next.user_state_slot);
    ledger.apply(&partial).unwrap();
    assert!(matches!(q3.evaluate(&ledger, &holder, Metric::AaveBalanceOf), Err(Unknown::MissingInput(_))));
    assert!(q1.evaluate(&ledger, &holder, Metric::AaveBalanceOf).is_err());
    ledger.undo(1).unwrap();
    ledger.apply(&complete).unwrap();
    let result = q3.evaluate(&ledger, &holder, Metric::AaveBalanceOf).unwrap();
    assert_eq!(result.value.to_string(), "15");
    assert!(result.globals.iter().all(|fact| fact.effective_epoch == 3 && fact.row.epoch == 3));
    assert_eq!(
        result.holder.unwrap().row.storage_slot,
        holder_storage_slot(&holder, &next.user_state_slot).unwrap()
    );
    ledger.apply(&aave_balance_state::project(&block(3), &config).unwrap()).unwrap();
    assert_eq!(q3.evaluate(&ledger, &holder, Metric::AaveBalanceOf).unwrap().value.to_string(), "15");
    // Editing a running stream's schedule changes its parameter identity.
    raw["markets"].as_array_mut().unwrap().pop();
    let previous_config = aave_balance_state::parse(&raw.to_string()).unwrap();
    let mut old_stream = ProtocolLedger::new(8);
    old_stream.apply(&aave_balance_state::project(&first, &previous_config).unwrap()).unwrap();
    let before = format!("{old_stream:?}");
    assert!(old_stream.apply(&complete).is_err());
    assert_eq!(format!("{old_stream:?}"), before);
}

fn ordinary_noop_continuity(name: &str) {
    let case = case(name);
    for ordinal in [30, 25, 20, 0] {
        let mut input = block(2);
        let mut call = holder_call(&case, 0, 7, 9, 20);
        // Ordinal 30 is successor-owned, 25 is a discarded predecessor
        // prefix, 20 is ambiguous and 0 lacks a valid execution position.
        call.storage_changes.push(holder_call(&case, 0, 8, 8, ordinal).storage_changes.remove(0));
        input.transaction_traces.push(tx(call));
        assert!(
            (case.project)(&input).is_err(),
            "{} admitted inconsistent ordinary noop at {ordinal}",
            case.name
        );
    }
    let mut input = block(2);
    let mut call = holder_call(&case, 0, 7, 9, 20);
    call.storage_changes.push(holder_call(&case, 0, 9, 9, 30).storage_changes.remove(0));
    input.transaction_traces.push(tx(call));
    let output = (case.project)(&input).unwrap();
    assert!(output.holder_basis.is_empty(), "a valid noop must not initialize successor holder state");
}

#[test]
fn aave_ordinary_noop_cannot_hide_a_physical_discontinuity() {
    ordinary_noop_continuity("aave");
}
#[test]
fn comet_ordinary_noop_cannot_hide_a_physical_discontinuity() {
    ordinary_noop_continuity("compound-v3");
}
#[test]
fn erc4626_ordinary_noop_cannot_hide_a_physical_discontinuity() {
    ordinary_noop_continuity("erc4626");
}
#[test]
fn lido_ordinary_noop_cannot_hide_a_physical_discontinuity() {
    ordinary_noop_continuity("lido");
}
