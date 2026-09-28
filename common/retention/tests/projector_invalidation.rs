//! Synthetic projector-to-consumer contract checks; no chain or RPC access.
#![cfg(not(target_arch = "wasm32"))]

use evm_retention::{Clock, Domain, Key, Ledger, Lookup};
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
    raw[list] = serde_json::json!([first, second]);
    raw["heartbeat_blocks"] = 0.into();
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

fn cases() -> Vec<Case> {
    let a = aave_balance_state::parse(&params(
        include_str!("../../../aave/balance-state/tests/fixtures/bsc-aave-v3-epochs.json"),
        "markets",
        "atoken",
    ))
    .unwrap();
    let v2 = compound_v2_balance_state::parse(&params(
        include_str!("../../../compound-v2/balance-state/tests/fixtures/mainnet-ctoken-epochs.json"),
        "markets",
        "ctoken",
    ))
    .unwrap();
    let v3 = compound_v3_balance_state::parse(&params(
        include_str!("../../../compound-v3/balance-state/tests/fixtures/mainnet-cusdcv3-epoch.json"),
        "markets",
        "comet",
    ))
    .unwrap();
    let l = lido_balance_state::parse(&params(
        include_str!("../../../lido/balance-state/tests/fixtures/mainnet-steth-v4-epoch.json"),
        "epochs",
        "steth",
    ))
    .unwrap();
    let v = erc4626_balance_state::parse(&params(
        include_str!("../../../erc4626/balance-state/tests/fixtures/bsc-stata-usdt-epoch.json"),
        "vaults",
        "vault",
    ))
    .unwrap();
    let erc4626_balance_state::Model::AaveStaticAToken { pool, .. } = &v.vaults[0].model else {
        panic!()
    };
    vec![
        Case {
            name: "aave",
            markets: [a.markets[0].atoken.clone(), a.markets[1].atoken.clone()],
            basis_slot: a.markets[0].user_state_slot,
            global_slot: a.markets[0].total_supply_slot,
            pointer_slot: a.markets[0].implementation_slot,
            expected_pointer: address_word(&a.markets[0].implementation),
            shared_code: a.pool.as_ref().unwrap().address.clone(),
            project: Box::new(move |b| aave_balance_state::project(b, &a).map_err(|e| e.to_string())),
        },
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
        },
        Case {
            name: "compound-v3",
            markets: [v3.markets[0].comet.clone(), v3.markets[1].comet.clone()],
            basis_slot: v3.markets[0].user_basic_slot,
            global_slot: v3.markets[0].totals_slot,
            pointer_slot: v3.markets[0].implementation_slot,
            expected_pointer: address_word(&v3.markets[0].implementation),
            shared_code: v3.markets[0].implementation.clone(),
            project: Box::new(move |b| compound_v3_balance_state::project(b, &v3).map_err(|e| e.to_string())),
        },
        Case {
            name: "lido",
            markets: [l.epochs[0].steth.clone(), l.epochs[1].steth.clone()],
            basis_slot: l.epochs[0].shares_slot,
            global_slot: l.epochs[0].total_and_external_shares_slot,
            pointer_slot: l.epochs[0].aragon.kernel_slot,
            expected_pointer: address_word(&l.epochs[0].aragon.kernel),
            shared_code: l.epochs[0].aragon.kernel.clone(),
            project: Box::new(move |b| lido_balance_state::project(b, &l).map_err(|e| e.to_string())),
        },
        Case {
            name: "erc4626",
            markets: [v.vaults[0].vault.clone(), v.vaults[1].vault.clone()],
            basis_slot: v.vaults[0].balances_slot,
            global_slot: v.vaults[0].total_supply_slot,
            pointer_slot: v.vaults[0].implementation_slot.unwrap(),
            expected_pointer: address_word(v.vaults[0].implementation.as_ref().unwrap()),
            shared_code: pool.clone(),
            project: Box::new(move |b| erc4626_balance_state::project(b, &v).map_err(|e| e.to_string())),
        },
    ]
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
fn clock(b: &eth::Block) -> Clock {
    Clock {
        number: b.number,
        hash: b.hash.clone(),
        parent_hash: b.header.as_ref().unwrap().parent_hash.clone(),
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
fn check_counts(events: &pb::Events) {
    let [c] = events.clocks.as_slice() else {
        panic!("one complete clock required")
    };
    assert_eq!(c.holder_basis_count as usize, events.holder_basis.len());
    assert_eq!(c.global_state_count as usize, events.global_state.len());
    assert_eq!(c.epoch_count as usize, events.epochs.len());
    assert_eq!(c.dependency_count as usize, events.dependencies.len());
}
fn key(case: &Case, market: usize) -> Key {
    Key {
        contract: Some(case.markets[market].clone()),
        address: vec![0x99; 20],
    }
}
fn known(ledger: &Ledger, key: &Key, expected: &str) {
    let Lookup::Known(entry) = ledger.lookup(key) else {
        panic!("expected known {expected}")
    };
    assert_eq!(entry.value, expected);
}

#[test]
fn every_projector_suspends_without_partial_basis_and_undo_restores_it() {
    for case in cases() {
        for noop in [false, true] {
            let mut ledger = Ledger::new(8, Domain::BalanceState);
            for market in 0..2 {
                ledger
                    .seed_checkpoint(key(&case, market), "1", 0, &[0; 32], "synthetic exact-parent checkpoint")
                    .unwrap();
            }
            let mut first = block(1);
            first.transaction_traces = vec![tx(holder_call(&case, 0, 1, 2, 1)), tx(holder_call(&case, 1, 1, 2, 2))];
            let output = (case.project)(&first).unwrap();
            check_counts(&output);
            assert_eq!(ledger.apply_state(&clock(&first), &output).unwrap().rows, 2, "{}", case.name);
            let mut upgrade = block(2);
            let mut call = holder_call(&case, 0, 2, 3, 5);
            let holder_slot = lido_balance_state::mapping_key(&[0x99; 20], &case.basis_slot);
            call.storage_changes.extend([
                write(&case.markets[0], case.global_slot, word(1), word(2), 6),
                write(&case.markets[0], case.global_slot, word(2), word(3), 22),
                write(
                    &case.markets[0],
                    case.pointer_slot,
                    case.expected_pointer,
                    if noop { case.expected_pointer } else { word(0x44) },
                    20,
                ),
                // Equality is outside the old epoch, just like later effects.
                write(&case.markets[0], [0xaa; 32], word(0), word(1), 20),
                write(&case.markets[0], holder_slot, word(3), word(4), 20),
                write(&case.markets[0], holder_slot, word(4), word(5), 21),
            ]);
            if !noop {
                call.storage_changes
                    .push(write(&case.markets[0], case.pointer_slot, word(0x44), case.expected_pointer, 25));
            }
            let mut unaffected = holder_call(&case, 1, 2, 8, 30);
            unaffected.storage_changes.push(write(&case.markets[1], case.global_slot, word(1), word(9), 31));
            upgrade.transaction_traces = vec![tx(call), tx(unaffected)];
            let output = (case.project)(&upgrade).unwrap_or_else(|e| panic!("{}: {e}", case.name));
            check_counts(&output);
            assert_eq!(output.epochs.len(), if noop { 1 } else { 2 }, "{}", case.name);
            assert!(output
                .holder_basis
                .iter()
                .all(|h| h.market == case.markets[1] && h.boundary == pb::Boundary::EndOfBlock as i32));
            assert!(output
                .global_state
                .iter()
                .all(|g| g.market != case.markets[0] || g.observation == pb::Observation::QualifiedConstant as i32));
            assert!(output
                .global_state
                .iter()
                .any(|g| g.market == case.markets[1] && g.boundary == pb::Boundary::EndOfBlock as i32));
            let before_order = output.clone();
            upgrade.transaction_traces.reverse();
            for t in &mut upgrade.transaction_traces {
                t.calls[0].storage_changes.reverse();
            }
            assert_eq!((case.project)(&upgrade).unwrap(), before_order, "{}", case.name);
            assert_eq!(ledger.apply_state(&clock(&upgrade), &output).unwrap().rows, 1);
            assert!(matches!(ledger.lookup(&key(&case, 0)), Lookup::Suspended { .. }), "{}", case.name);
            known(&ledger, &key(&case, 1), "8");
            // No partial pre-cutoff value overwrote the retained basis.
            assert_eq!(ledger.entries()[&key(&case, 0)].value, "2");
            // A qualified successor can resume the market, but its storage
            // compatibility cannot restore omitted writes from this block.
            let successor = block(3);
            let mut rebound = (case.project)(&successor).unwrap();
            let mut binding = output.epochs[0].clone();
            binding.kind = pb::EpochEventKind::Bound as i32;
            binding.epoch += 1;
            binding.basis_carryover = true;
            binding.ordinal = 0;
            binding.activation_block = 3;
            binding.activation_ordinal = 0;
            binding.reason = pb::InvalidationReason::Unspecified as i32;
            rebound.epochs = vec![binding];
            rebound.clocks[0].epoch_count = 1;
            ledger.apply_state(&clock(&successor), &rebound).unwrap();
            assert_eq!(ledger.lookup(&key(&case, 0)), Lookup::Unknown, "{}", case.name);
            known(&ledger, &key(&case, 1), "8");
            ledger.undo(2).unwrap();
            assert!(matches!(ledger.lookup(&key(&case, 0)), Lookup::Suspended { .. }));
            assert_eq!(ledger.entries()[&key(&case, 0)].value, "2");
            ledger.undo(1).unwrap();
            known(&ledger, &key(&case, 0), "2");
            known(&ledger, &key(&case, 1), "2");
            ledger.apply_state(&clock(&upgrade), &output).unwrap();
            ledger.undo(1).unwrap();
            let mut replacement = block(2);
            replacement.hash = vec![0x55; 32];
            replacement.transaction_traces = vec![tx(holder_call(&case, 0, 2, 6, 5))];
            let output = (case.project)(&replacement).unwrap();
            check_counts(&output);
            ledger.apply_state(&clock(&replacement), &output).unwrap();
            known(&ledger, &key(&case, 0), "6");
        }
    }
}

#[test]
fn unknown_before_cutoff_still_fails_and_reverted_triggers_cannot_quarantine_it() {
    for case in cases() {
        let mut b = block(2);
        let trigger = tx(eth::Call {
            storage_changes: vec![write(&case.markets[0], case.pointer_slot, case.expected_pointer, word(0x44), 20)],
            ..Default::default()
        });
        for ordinal in [19, 20, 21] {
            b.transaction_traces = vec![
                trigger.clone(),
                tx(eth::Call {
                    storage_changes: vec![write(&case.markets[0], [0xaa; 32], word(0), word(1), ordinal)],
                    ..Default::default()
                }),
            ];
            let result = (case.project)(&b);
            if ordinal < 20 {
                assert!(result.unwrap_err().contains("unresolved"), "{}", case.name);
            } else {
                assert_eq!(result.unwrap().epochs.len(), 1, "{}", case.name);
            }
        }
        for status in [eth::TransactionTraceStatus::Failed, eth::TransactionTraceStatus::Reverted] {
            b.transaction_traces[0].status = status as i32;
            assert!((case.project)(&b).unwrap_err().contains("unresolved"), "{}", case.name);
        }
        b.transaction_traces[0].status = eth::TransactionTraceStatus::Succeeded as i32;
        b.transaction_traces[0].calls[0].state_reverted = true;
        assert!((case.project)(&b).unwrap_err().contains("unresolved"), "{}", case.name);
    }
}

#[test]
fn cutoff_does_not_relax_structural_validation_of_later_writes() {
    for case in cases() {
        for tied in [false, true] {
            let mut b = block(2);
            b.transaction_traces = vec![tx(eth::Call {
                storage_changes: vec![
                    write(&case.markets[0], case.pointer_slot, case.expected_pointer, word(0x44), 20),
                    write(&case.markets[0], [0xaa; 32], word(0), word(1), 21),
                    write(
                        &case.markets[0],
                        [0xaa; 32],
                        if tied { word(1) } else { word(3) },
                        word(4),
                        if tied { 21 } else { 22 },
                    ),
                ],
                ..Default::default()
            })];
            let error = (case.project)(&b).unwrap_err();
            assert!(error.contains(if tied { "ambiguous" } else { "discontinuous" }), "{}: {error}", case.name);
        }
    }
}

#[test]
fn shared_dependency_code_change_cuts_off_every_affected_market() {
    for case in cases() {
        let mut b = block(2);
        b.transaction_traces = vec![tx(holder_call(&case, 0, 2, 3, 5)), tx(holder_call(&case, 1, 2, 3, 6))];
        b.code_changes = vec![eth::CodeChange {
            address: case.shared_code.clone(),
            old_hash: vec![1; 32],
            new_hash: vec![2; 32],
            ordinal: 20,
            ..Default::default()
        }];
        b.transaction_traces.push(tx(eth::Call {
            storage_changes: case.markets.iter().map(|m| write(m, [0xaa; 32], word(0), word(1), 20)).collect(),
            ..Default::default()
        }));
        let output = (case.project)(&b).unwrap_or_else(|e| panic!("{}: {e}", case.name));
        check_counts(&output);
        assert_eq!(output.epochs.len(), 2, "{}", case.name);
        assert!(output.holder_basis.is_empty() && output.global_state.is_empty(), "{}", case.name);
        // A code change at an unrelated contract cannot exempt unknown writes.
        b.code_changes[0].address = vec![0xab; 20];
        assert!((case.project)(&b).unwrap_err().contains("unresolved"), "{}", case.name);
    }
}
