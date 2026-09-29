#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::{
    calculated_retention::{binding::sha, Code, Write},
    ybc_retention::{self as y, binding as b, decode as d, historical as h, *},
};
use primitive_types::U256;
use serde_json::Value;
fn ledger() -> Ledger {
    Ledger::from_checkpoint(h::checkpoint(h::PARENT).unwrap(), Limits::default()).unwrap()
}
fn holder() -> Address {
    address(h::CONTROL_HOLDER).unwrap()
}
fn block(l: &Ledger) -> BlockInput {
    BlockInput {
        at: At {
            number: l.at().number + 1,
            hash: word(l.at().number + 1),
            parent_hash: Some(l.at().hash),
            timestamp: l.at().timestamp,
            producer_version: Some(5),
        },
        source_sha256: "aa".repeat(32),
        writes: vec![],
        codes: vec![],
    }
}
fn set(cp: &mut Checkpoint, key: Slot, new: U256) {
    let f = cp.facts.iter_mut().find(|f| f.slot == key).unwrap();
    f.word = word(new);
}
fn selected_case(name: &str) -> Checkpoint {
    let o = b::original("overrides.json").unwrap();
    let i = o["cases"].as_array().unwrap().iter().position(|v| v["name"] == name).unwrap();
    h::override_checkpoint(i).unwrap()
}
#[test]
fn raw_checkpoints_and_thirty_override_maps_match_independent_getters() {
    let r = h::check_captured().unwrap();
    assert_eq!(r["historical_getters"], 48);
    assert_eq!(r["override_getters"], 60);
    assert_eq!(r["recognized_override_reverts"], 17);
}
#[test]
fn original_pins_bind_every_byte_before_parse() {
    for o in b::ORIGINALS {
        assert!(b::verify_original(o.file, o.raw).is_ok());
        let mut x = o.raw.to_vec();
        x.push(b' ');
        assert!(b::verify_original(o.file, &x).is_err());
    }
}
#[test]
fn closed_binding_rejects_network_model_epoch_and_revision_changes() {
    let original = Binding::historical(1, h::PARENT).unwrap();
    for field in ["schema", "chain_id", "model_revision", "persistence_revision", "epoch"] {
        let mut v = serde_json::to_value(&original).unwrap();
        v[field] = 0.into();
        assert!(serde_json::from_value::<Binding>(v).unwrap().validate().is_err());
    }
    let mut b = original;
    b.evidence.pop();
    assert!(b.validate().is_err());
}
#[test]
fn branch_early_exits_do_not_read_unreachable_hourly_or_pool_state() {
    let mut cp = selected_case("static_below_threshold");
    cp.facts
        .retain(|f| [b::pointer(), d::holder_key(holder(), 0, 0), d::holder_key(holder(), 22, 4)].contains(&f.slot));
    let l = Ledger::from_checkpoint(cp, Limits::default()).unwrap();
    let e = l.evaluate(holder()).unwrap();
    assert_eq!(
        e.pending,
        Metric::Known(Reward {
            pending_reward: "0".into(),
            stopping_hour: "0".into()
        })
    );
    assert_eq!(e.used.len(), 3);
    for name in ["closed_cycle", "already_claimed", "future_claim"] {
        let mut cp = selected_case(name);
        cp.facts.retain(|f| {
            [
                b::pointer(),
                d::holder_key(holder(), 0, 0),
                d::holder_key(holder(), 22, 4),
                d::scalar(b::token(), 31),
                d::holder_key(holder(), 23, 0),
            ]
            .contains(&f.slot)
        });
        let e = Ledger::from_checkpoint(cp, Limits::default()).unwrap().evaluate(holder()).unwrap();
        assert!(matches!(e.pending, Metric::Known(_)));
    }
    let mut cp = selected_case("launch_in_future");
    cp.facts
        .retain(|f| [b::pointer(), d::holder_key(holder(), 22, 4), d::scalar(b::token(), 31)].contains(&f.slot));
    assert!(matches!(
        Ledger::from_checkpoint(cp, Limits::default()).unwrap().evaluate(holder()).unwrap().pending,
        Metric::ModelRefusal { .. }
    ));
}
#[test]
fn explicit_zero_missing_reward_raw_and_overflow_are_distinct() {
    let cp = selected_case("zero_user_rate");
    let l = Ledger::from_checkpoint(cp.clone(), Limits::default()).unwrap();
    assert!(matches!(l.evaluate(holder()).unwrap().pending, Metric::Known(_)));
    let mut cp = cp;
    cp.facts.retain(|f| f.slot != d::holder_key(holder(), 0, 0));
    let e = Ledger::from_checkpoint(cp, Limits::default()).unwrap().evaluate(holder()).unwrap();
    assert!(matches!(e.pending, Metric::Known(_)));
    assert!(e.observable.is_missing());
    let e = Ledger::from_checkpoint(selected_case("balance_overflow"), Limits::default())
        .unwrap()
        .evaluate(holder())
        .unwrap();
    assert!(matches!(e.pending, Metric::Known(_)));
    assert!(matches!(e.observable, Metric::ModelRefusal { .. }));
}
#[test]
fn absent_first_required_hour_is_unknown_and_final_key_names_are_not_values() {
    let mut l = ledger();
    let cp = h::checkpoint(h::FINAL).unwrap();
    let key = d::holder_key(holder(), 23, 0);
    let old = l.fact(&key).unwrap().word;
    let new = cp.facts.iter().find(|f| f.slot == key).unwrap().word;
    assert_eq!(value(&old), 1695.into());
    assert_eq!(value(&new), 3135.into());
    let retired = d::hour_key(holder(), 1695.into(), 28);
    let prior = l.fact(&retired).unwrap().clone();
    let opened = d::hour_key(holder(), 3135.into(), 28);
    assert!(l.universe().contains(&opened));
    assert!(l.fact(&opened).is_none());
    let mut next = block(&l);
    next.writes.push(Write {
        slot: key,
        old,
        new,
        ordinal: 1,
    });
    l.apply(&next).unwrap();
    let e = l.evaluate(holder()).unwrap();
    assert!(e.pending.is_missing());
    assert_eq!(l.fact(&retired), Some(&prior));
    assert!(l.fact(&opened).is_none());
}
#[test]
fn huge_cursor_does_not_allocate_a_gap_and_idle_clock_changes_are_evaluated() {
    let mut cp = selected_case("base");
    set(&mut cp, d::holder_key(holder(), 23, 0), U256::MAX);
    let l = Ledger::from_checkpoint(cp, Limits::default()).unwrap();
    assert_eq!(
        l.evaluate(holder()).unwrap().pending,
        Metric::Known(Reward {
            pending_reward: "0".into(),
            stopping_hour: "0".into()
        })
    );
    let mut cp = selected_case("already_claimed");
    cp.facts.retain(|f| f.slot != d::hour_key(holder(), 103.into(), 49));
    let mut l = Ledger::from_checkpoint(cp.clone(), Limits::default()).unwrap();
    let first = l.evaluate(holder()).unwrap();
    let mut next = block(&l);
    next.at.timestamp += 3600;
    l.apply(&next).unwrap();
    assert!(l.evaluate(holder()).unwrap().pending.is_missing());
    assert_eq!(
        l.facts(),
        cp.facts.iter_mut().collect::<Vec<_>>().into_iter().map(|f| f.clone()).collect::<Vec<_>>()
    );
    assert_ne!(first.input_sha256, l.evaluate(holder()).unwrap().input_sha256);
}
#[test]
fn same_key_order_continuity_and_late_errors_are_atomic() {
    let l = ledger();
    let key = d::holder_key(holder(), 0, 0);
    let old = l.fact(&key).unwrap().word;
    for ord in [0, 1, 2] {
        let mut next = block(&l);
        next.writes = vec![
            Write {
                slot: key.clone(),
                old,
                new: word(5),
                ordinal: 2,
            },
            Write {
                slot: key.clone(),
                old: word(5),
                new: word(6),
                ordinal: ord,
            },
        ];
        let mut x = l.clone();
        assert!(x.apply(&next).is_err());
        assert_eq!(x, l);
    }
    let mut next = block(&l);
    next.writes = vec![
        Write {
            slot: key.clone(),
            old,
            new: word(5),
            ordinal: 1,
        },
        Write {
            slot: key,
            old: word(4),
            new: word(6),
            ordinal: 2,
        },
    ];
    let mut x = l.clone();
    assert!(x.apply(&next).is_err());
    assert_eq!(x, l);
}
#[test]
fn distinct_keys_commute_and_noops_initialize_only_observed_values() {
    let mut cp = selected_case("base");
    let k = d::holder_key(holder(), 0, 0);
    let p = d::holder_key(holder(), 22, 5);
    cp.facts.retain(|f| f.slot != k);
    let l = Ledger::from_checkpoint(cp, Limits::default()).unwrap();
    let mut next = block(&l);
    next.writes = vec![
        Write {
            slot: k.clone(),
            old: word(123),
            new: word(123),
            ordinal: 1,
        },
        Write {
            slot: p.clone(),
            old: l.fact(&p).unwrap().word,
            new: word(1),
            ordinal: 1,
        },
    ];
    let mut a = l.clone();
    a.apply(&next).unwrap();
    next.writes.reverse();
    let mut c = l;
    c.apply(&next).unwrap();
    assert_eq!(a, c);
    assert_eq!(
        a.fact(&k).unwrap().origin,
        Origin::Observed {
            ordinal: 1,
            source_sha256: next.source_sha256
        }
    );
}
#[test]
fn runtime_pointer_and_helper_storage_excursions_suspend_even_when_restored() {
    let l = ledger();
    for axis in 0..3 {
        let mut x = l.clone();
        let mut next = block(&x);
        if axis == 0 {
            let hash = b::codes()[0].1;
            next.codes = vec![
                Code {
                    contract: b::token(),
                    old: hash,
                    new: word(1),
                    ordinal: 1,
                },
                Code {
                    contract: b::token(),
                    old: word(1),
                    new: hash,
                    ordinal: 2,
                },
            ];
        } else {
            let key = if axis == 1 { b::pointer() } else { d::scalar(b::helper(), 8) };
            let old = if axis == 1 { b::helper_word() } else { word(0) };
            next.writes = vec![
                Write {
                    slot: key.clone(),
                    old,
                    new: word(1),
                    ordinal: 1,
                },
                Write {
                    slot: key,
                    old: word(1),
                    new: old,
                    ordinal: 2,
                },
            ];
        }
        x.apply(&next).unwrap();
        assert!(matches!(x.evaluate(holder()).unwrap().pending, Metric::Suspended { .. }));
        x.apply(&block(&x)).unwrap();
        assert!(x.suspension().is_some());
    }
}
#[test]
fn checkpoint_scope_and_pool_orientation_refuse_instead_of_inferring() {
    let mut cp = h::checkpoint(h::PARENT).unwrap();
    cp.facts.push(cp.facts[0].clone());
    assert!(Ledger::from_checkpoint(cp, Limits::default()).is_err());
    for (key, v) in [
        (d::holder_key(b::pool(), 22, 4), U256::from(1_000_000_000u64)),
        (d::scalar(b::pool(), 7), U256::MAX),
    ] {
        let mut cp = selected_case("base");
        set(&mut cp, key, v);
        let e = Ledger::from_checkpoint(cp, Limits::default()).unwrap().evaluate(holder()).unwrap();
        assert!(matches!(e.pending, Metric::ScopeRefusal { .. }));
    }
    let mut cp = selected_case("base");
    cp.facts.retain(|f| f.slot != b::pointer());
    assert!(Ledger::from_checkpoint(cp, Limits::default())
        .unwrap()
        .evaluate(holder())
        .unwrap()
        .pending
        .is_missing());
}
#[test]
fn hash_clock_undo_replacement_snapshot_and_bounds_are_exact() {
    let original = ledger();
    let mut l = original.clone();
    let next = block(&l);
    l.apply(&next).unwrap();
    let bytes = l.snapshot().unwrap();
    let digest = sha(&bytes);
    let restored = Ledger::restore(&bytes, &digest, l.binding()).unwrap();
    assert_eq!(restored.snapshot().unwrap(), bytes);
    assert!(Ledger::restore(&bytes, &"00".repeat(32), l.binding()).is_err());
    assert!(l.undo(original.at().number, word(9)).is_err());
    l.undo(original.at().number, original.at().hash).unwrap();
    assert_eq!(l, original);
    let mut replacement = next;
    replacement.at.hash = word(99);
    l.apply(&replacement).unwrap();
    for axis in 0..4 {
        let mut next = block(&l);
        match axis {
            0 => next.at.number += 1,
            1 => next.at.parent_hash = Some(word(99)),
            2 => next.at.timestamp = 0,
            _ => next.at.producer_version = Some(4),
        };
        if axis == 1 {
            next.at.parent_hash = Some(word(123));
        }
        let before = l.clone();
        assert!(l.apply(&next).is_err());
        assert_eq!(l, before);
    }
    let mut v: Value = serde_json::from_slice(&bytes).unwrap();
    v["state"]["facts"][0]["origin"] = serde_json::json!({"Observed":{"ordinal":0,"source_sha256":"aa".repeat(32)}});
    let bad = serde_json::to_vec(&v).unwrap();
    assert!(Ledger::restore(&bad, &sha(&bad), l.binding()).is_err());
    assert!(Ledger::from_checkpoint(h::checkpoint(h::PARENT).unwrap(), Limits { keys: 10, ..Limits::default() }).is_err());
}

#[test]
fn restart_refuses_pre_epoch_facts_and_incompatible_observed_producer() {
    let mut l = ledger();
    let mut next = block(&l);
    let key = d::holder_key(holder(), 0, 0);
    let old = l.fact(&key).unwrap().word;
    next.writes.push(Write {
        slot: key,
        old,
        new: old,
        ordinal: 1,
    });
    l.apply(&next).unwrap();
    let raw = l.snapshot().unwrap();
    for axis in 0..2 {
        let mut v: Value = serde_json::from_slice(&raw).unwrap();
        if axis == 0 {
            v["state"]["facts"][0]["at"]["number"] = (h::PARENT - 1).into();
        } else {
            let fs = v["state"]["facts"].as_array_mut().unwrap();
            let observed = fs.iter_mut().find(|f| f["origin"].get("Observed").is_some()).unwrap();
            observed["at"]["number"] = h::PARENT.into();
            observed["at"]["producer_version"] = 4.into();
        }
        let bytes = serde_json::to_vec(&v).unwrap();
        assert!(Ledger::restore(&bytes, &sha(&bytes), l.binding()).is_err(), "axis {axis}");
    }
}

#[test]
fn explicit_reset_requires_new_epoch_and_drops_all_old_origins_and_undo() {
    let mut l = ledger();
    let mut next = block(&l);
    next.writes = vec![
        Write {
            slot: b::pointer(),
            old: b::helper_word(),
            new: word(0),
            ordinal: 1,
        },
        Write {
            slot: b::pointer(),
            old: word(0),
            new: b::helper_word(),
            ordinal: 2,
        },
    ];
    l.apply(&next).unwrap();
    let mut cp = h::checkpoint(h::PARENT).unwrap();
    cp.at = l.at().clone();
    cp.binding = Binding::historical(2, cp.at.number).unwrap();
    cp.evidence = "independently declared synthetic reset".into();
    for f in &mut cp.facts {
        f.at = cp.at.clone();
        f.epoch = 2;
        f.origin = Origin::Checkpoint { evidence: cp.evidence.clone() };
    }
    let before = l.clone();
    let mut bad = cp.clone();
    bad.facts.retain(|f| f.slot != b::pointer());
    assert!(l.resume(bad).is_err());
    assert_eq!(l, before);
    let mut bad = cp.clone();
    bad.binding.epoch = 1;
    assert!(l.resume(bad).is_err());
    assert_eq!(l, before);
    l.resume(cp).unwrap();
    assert!(l.suspension().is_none());
    assert!(l.facts().iter().all(|f| f.epoch == 2 && matches!(f.origin, Origin::Checkpoint { .. })));
    assert!(l.undo(h::PARENT, before.at().hash).is_err());
    assert_eq!(l.counters().blocks, 0);
}
#[test]
fn actual_collector_retains_noops_filters_failures_and_requires_extended_identity() {
    use prost::Message;
    use substreams_ethereum::pb::eth::v2 as eth;
    let l = ledger();
    let n = block(&l);
    let key = d::holder_key(holder(), 0, 0);
    let old = l.fact(&key).unwrap().word;
    let mut header = eth::BlockHeader {
        number: n.at.number,
        parent_hash: l.at().hash.to_vec(),
        timestamp: Some(Default::default()),
        ..Default::default()
    };
    header.timestamp.as_mut().unwrap().seconds = n.at.timestamp as i64;
    let mut pb = eth::Block {
        number: n.at.number,
        hash: n.at.hash.to_vec(),
        header: Some(header),
        ver: 5,
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        transaction_traces: vec![eth::TransactionTrace {
            status: eth::TransactionTraceStatus::Succeeded as i32,
            begin_ordinal: 1,
            end_ordinal: 10,
            calls: vec![eth::Call {
                begin_ordinal: 1,
                end_ordinal: 10,
                storage_changes: vec![eth::StorageChange {
                    address: b::token().to_vec(),
                    key: key.key.to_vec(),
                    old_value: old.to_vec(),
                    new_value: old.to_vec(),
                    ordinal: 5,
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let input = y::collect::decode(&pb, &sha(&pb.encode_to_vec())).unwrap();
    assert_eq!(input.writes.len(), 1);
    let mut x = l;
    x.apply(&input).unwrap();
    assert!(matches!(x.fact(&key).unwrap().origin, Origin::Observed { ordinal: 5, .. }));
    pb.transaction_traces[0].calls[0].storage_changes[0].new_value = vec![0; 33];
    assert!(y::collect::decode(&pb, &n.source_sha256).is_err());
    pb.transaction_traces[0].calls[0].state_reverted = true;
    assert!(y::collect::decode(&pb, &n.source_sha256).unwrap().writes.is_empty());
    pb.transaction_traces[0].calls[0].state_reverted = false;
    pb.transaction_traces[0].status = eth::TransactionTraceStatus::Failed as i32;
    assert!(y::collect::decode(&pb, &n.source_sha256).unwrap().writes.is_empty());
    pb.transaction_traces[0].calls.clear();
    assert!(y::collect::decode(&pb, &n.source_sha256).is_err());
    pb.transaction_traces.clear();
    pb.ver = 3;
    assert!(y::collect::decode(&pb, &n.source_sha256).is_err());
    pb.ver = 4;
    assert!(y::collect::decode(&pb, &n.source_sha256).is_ok());
    pb.header.as_mut().unwrap().timestamp.as_mut().unwrap().nanos = 1;
    assert!(y::collect::decode(&pb, &n.source_sha256).is_err());
}

#[test]
fn uninitialized_pointer_restoration_is_an_observed_excursion_but_bound_noop_is_not() {
    let mut cp = selected_case("static_below_threshold");
    cp.facts.retain(|f| f.slot != b::pointer());
    let l = Ledger::from_checkpoint(cp, Limits::default()).unwrap();
    for old in [word(9), b::helper_word()] {
        let mut x = l.clone();
        let mut next = block(&x);
        next.writes.push(Write {
            slot: b::pointer(),
            old,
            new: b::helper_word(),
            ordinal: 1,
        });
        x.apply(&next).unwrap();
        if old == b::helper_word() {
            assert!(matches!(x.evaluate(holder()).unwrap().pending, Metric::Known(_)));
        } else {
            assert!(matches!(x.evaluate(holder()).unwrap().pending, Metric::Suspended { .. }));
        }
    }
}
#[test]
fn active_missing_late_inputs_conservatively_precede_unexecuted_model_errors() {
    let mut cp = selected_case("reward_multiplication_overflow");
    cp.facts.retain(|f| f.slot != d::scalar(b::pool(), 8));
    let l = Ledger::from_checkpoint(cp, Limits::default()).unwrap();
    assert!(l.evaluate(holder()).unwrap().pending.is_missing());
}

#[test]
fn changed_raw_trace_with_same_header_cannot_satisfy_original_pb_pin() {
    use prost::Message;
    use substreams_ethereum::pb::eth::v2 as eth;
    let l = ledger();
    let n = block(&l);
    let mut header = eth::BlockHeader {
        number: n.at.number,
        parent_hash: l.at().hash.to_vec(),
        timestamp: Some(Default::default()),
        ..Default::default()
    };
    header.timestamp.as_mut().unwrap().seconds = n.at.timestamp as i64;
    let mut pb = eth::Block {
        number: n.at.number,
        hash: n.at.hash.to_vec(),
        header: Some(header),
        ver: 5,
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        ..Default::default()
    };
    let raw = pb.encode_to_vec();
    let pin = sha(&raw);
    assert!(y::collect::decode_original(&raw, n.at.number, n.at.hash, &pin).is_ok());
    pb.size = 1;
    assert!(y::collect::decode_original(&pb.encode_to_vec(), n.at.number, n.at.hash, &pin).is_err());
    assert!(y::collect::decode_original(&raw, n.at.number + 1, n.at.hash, &pin).is_err());
}

#[test]
fn portable_original_pb_journal_preserves_unknown_hours_restart_and_exact_undo() {
    let inputs = y::journal::verify(y::journal::BYTES).unwrap();
    let mut l = ledger();
    let mut restarted: Option<Ledger> = None;
    let mut known = 0;
    let mut unknown = 0;
    let mut first_unknown = None;
    for (i, input) in inputs.iter().enumerate() {
        let applied = l.apply(input).unwrap();
        for e in &applied.evaluations {
            match &e.observable {
                Metric::Known(_) => known += 1,
                Metric::Unknown { .. } => {
                    unknown += 1;
                    first_unknown.get_or_insert(input.at.number);
                    assert_eq!(e.holder, holder());
                }
                other => panic!("unexpected saved result {other:?}"),
            };
            assert!(matches!(e.raw_basis, Metric::Known(_)));
        }
        if let Some(shadow) = &mut restarted {
            assert_eq!(shadow.apply(input).unwrap(), applied);
        }
        if i == 511 {
            let raw = l.snapshot().unwrap();
            restarted = Some(Ledger::restore(&raw, &sha(&raw), l.binding()).unwrap());
        }
    }
    assert_eq!((known, unknown, first_unknown), (11380, 908, Some(122288122)));
    assert_eq!(l.counters().retained_writes, 181);
    assert_eq!(l.snapshot().unwrap(), restarted.unwrap().snapshot().unwrap());
    let expected = h::checkpoint(h::FINAL).unwrap();
    let mut matches = 0;
    let mut missing = 0;
    for f in expected.facts {
        if let Some(found) = l.fact(&f.slot) {
            assert_eq!(found.word, f.word);
            matches += 1;
        } else {
            missing += 1;
        }
    }
    assert_eq!((matches, missing), (746, 959));
    let e = l.evaluate(holder()).unwrap();
    assert!(e.pending.is_missing());
    let independent = Ledger::from_checkpoint(h::checkpoint(h::FINAL).unwrap(), Limits::default()).unwrap();
    assert!(matches!(independent.evaluate(holder()).unwrap().pending, Metric::Known(_)));
    let final_snapshot = l.snapshot().unwrap();
    let boundary = &inputs[1007].at;
    l.undo(boundary.number, boundary.hash).unwrap();
    for input in &inputs[1008..] {
        l.apply(input).unwrap();
    }
    assert_eq!(l.snapshot().unwrap(), final_snapshot);
    let mut changed = y::journal::BYTES.to_vec();
    changed.push(b' ');
    assert!(y::journal::verify(&changed).is_err());
}
#[test]
fn skipped_hour_payload_and_unused_burn_parameter_are_not_invented_facts() {
    let cp = selected_case("skip_burn_rate_changes");
    let expected = Ledger::from_checkpoint(cp.clone(), Limits::default()).unwrap().evaluate(holder()).unwrap();
    let mut cp = cp;
    let skipped: Vec<_> = [28, 29, 26].map(|root| d::hour_key(holder(), 101.into(), root)).to_vec();
    cp.facts.retain(|f| !skipped.contains(&f.slot) && f.slot != d::scalar(b::token(), 36));
    let l = Ledger::from_checkpoint(cp, Limits::default()).unwrap();
    let e = l.evaluate(holder()).unwrap();
    assert_eq!(e.pending, expected.pending);
    assert_eq!(e.observable, expected.observable);
    assert!(skipped.iter().all(|k| l.fact(k).is_none()));
}
