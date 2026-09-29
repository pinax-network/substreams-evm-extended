#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::lbp_rewards::fixture;
use primitive_types::U256;
use serde_json::{json, Value};

#[test]
fn lbp_fixture_apply_rejects_late_error_atomically() {
    let rows: Value = serde_json::from_str(include_str!("../../tests/fixtures/lbp-rewards/checkpoint.json")).unwrap();
    let mut state = fixture::checkpoint(&rows).unwrap();
    let before = state.clone();
    let keys: Vec<_> = before.keys().take(2).cloned().collect();
    let a = before[&keys[0]];
    let b = before[&keys[1]];
    let updates = json!([
        {"contract":keys[0].0,"key":keys[0].1,"ordinal":1,"old":a.to_string(),"new":(a+U256::one()).to_string()},
        {"contract":keys[1].0,"key":keys[1].1,"ordinal":2,"old":(b+U256::one()).to_string(),"new":b.to_string()}
    ]);
    assert!(fixture::apply(&mut state, &updates).is_err());
    assert_eq!(state, before, "failed apply must not leave an earlier storage mutation");
}

#[test]
fn lbp_fixture_checkpoint_rejects_conflicting_row_hashes() {
    let mut rows: Value = serde_json::from_str(include_str!("../../tests/fixtures/lbp-rewards/checkpoint.json")).unwrap();
    rows[1]["hash"] = json!(format!("0x{}", "01".repeat(32)));
    assert!(fixture::checkpoint(&rows).is_err(), "every checkpoint row must bind the same exact hash");
}

use erc20_balances_tools::calculated_retention::{
    self as retained,
    binding::{self, Binding, Model},
    *,
};
fn h(n: u8) -> Address {
    let mut a = [0; 20];
    a[19] = n;
    a
}
fn checkpoint(model: Model) -> Checkpoint {
    let binding = Binding::historical(model, 1, 122288005).unwrap();
    let at = At {
        number: 122288005,
        hash: [1; 32],
        parent_hash: None,
        timestamp: 1789591601,
        producer_version: None,
    };
    let evidence = "synthetic controlled raw facts; no deployment qualification".to_owned();
    let l = binding.reflection_layout().unwrap();
    let mut raw = vec![
        (word(l.total_tokens), word(100)),
        (word(l.total_reflections), word(1000)),
        (word(l.excluded_array), word(0)),
    ];
    for holder in [h(1), h(2)] {
        raw.extend([
            (mapping(holder, l.reflections), word(100)),
            (mapping(holder, l.tokens), word(10)),
            (mapping(holder, l.excluded_flag), word(0)),
        ]);
    }
    let words = raw
        .into_iter()
        .map(|(key, word)| Fact {
            slot: Slot {
                contract: binding.token(),
                key,
            },
            word,
            at: at.clone(),
            epoch: 1,
            origin: Origin::Checkpoint { evidence: evidence.clone() },
        })
        .collect();
    Checkpoint {
        binding,
        at,
        holders: vec![h(1), h(2)],
        words,
        evidence,
    }
}
fn ledger() -> Ledger {
    Ledger::from_checkpoint(
        checkpoint(Model::BabyDoge),
        Limits {
            undo_depth: 2,
            ..Limits::default()
        },
    )
    .unwrap()
}
fn next(l: &Ledger) -> BlockInput {
    BlockInput {
        at: At {
            number: l.at().number + 1,
            hash: word(l.at().number + 1),
            parent_hash: Some(l.at().hash),
            timestamp: l.at().timestamp + 1,
            producer_version: Some(5),
        },
        source_sha256: "ab".repeat(32),
        writes: vec![],
        codes: vec![],
    }
}
fn change(l: &Ledger, key: Word, new: Word, ordinal: u64) -> Write {
    let slot = Slot {
        contract: l.binding().token(),
        key,
    };
    Write {
        old: l.facts().iter().find(|f| f.slot == slot).unwrap().word,
        slot,
        new,
        ordinal,
    }
}
fn amount(l: &Ledger, holder: Address) -> String {
    match l.evaluate(holder).unwrap().outcome {
        Outcome::Known { amount, .. } => amount,
        x => panic!("expected amount, got {x:?}"),
    }
}
#[test]
fn closed_bindings_reconstruct_runtime_layout_and_dependencies() {
    for (i, (_, _, _, raw)) in binding::CAPTURES.iter().enumerate() {
        binding::verify_capture(raw, i).unwrap();
        let original: Value = serde_json::from_slice(raw).unwrap();
        let mut bad = original.clone();
        bad["storageLayout"]["storage"][0]["offset"] = json!(1);
        assert!(retained::source::verify(&bad, i).is_err());
        let mut bad = original.clone();
        bad["runtimeBytecode"]["onchainBytecode"] = json!("0x00");
        assert!(retained::source::verify(&bad, i).is_err());
        let mut bad = original.clone();
        bad["stdJsonInput"]["settings"]["optimizer"]["runs"] = json!(999);
        assert!(retained::source::verify(&bad, i).is_err());
        let mut changed = raw.to_vec();
        changed.push(b' ');
        assert!(binding::verify_capture(&changed, i).is_err());
    }
    let l: Value = serde_json::from_slice(binding::CAPTURES[0].3).unwrap();
    let mut h: Value = serde_json::from_slice(binding::CAPTURES[1].3).unwrap();
    let expected = retained::source::lbp_exemptions(&l, &h).unwrap();
    assert_eq!(expected.len(), 8);
    assert!(!expected.contains(&[0; 20]));
    h["runtimeBytecode"]["transformationValues"]["immutables"]["9180"] = json!(format!("0x{}", "00".repeat(32)));
    assert!(retained::source::lbp_exemptions(&l, &h).is_err());
    for index in [0, 1, 2] {
        let mut bad: Value = serde_json::from_slice(binding::CAPTURES[index].3).unwrap();
        let first = bad["runtimeBytecode"]["immutableReferences"]
            .as_object()
            .unwrap()
            .keys()
            .next()
            .unwrap()
            .clone();
        bad["runtimeBytecode"]["immutableReferences"][first][0]["start"] = json!(0);
        assert!(retained::source::verify(&bad, index).is_err());
    }
    let mut bad: Value = serde_json::from_slice(binding::CAPTURES[2].3).unwrap();
    bad["runtimeBytecode"]["transformationValues"]["cborAuxdata"]["1"] = json!("0x00");
    assert!(retained::source::verify(&bad, 2).is_err());
}
#[test]
fn global_only_and_idle_recompute_all_registered_holders() {
    let mut l = ledger();
    assert_eq!(amount(&l, h(1)), "10");
    let mut b = next(&l);
    b.writes.push(change(&l, word(10), word(500), 1));
    let result = l.apply(&b).unwrap();
    assert_eq!(result.evaluations.len(), 2);
    assert_eq!(result.changed, vec![h(1), h(2)]);
    assert_eq!(amount(&l, h(1)), "20");
    assert_eq!(l.counters().evaluations, 2);
    let idle = l.apply(&next(&l)).unwrap();
    assert!(idle.changed.is_empty());
    assert_eq!(idle.evaluations.len(), 2);
    assert!(matches!(l.evaluate(h(3)).unwrap().outcome, Outcome::Unknown { .. }));
}
#[test]
fn provenance_survives_idle_restore_undo_and_distinguishes_equal_amount_inputs() {
    let mut l = ledger();
    let start = l.clone();
    let mut b = next(&l);
    b.writes.push(change(&l, mapping(h(1), 3), word(100), 1));
    l.apply(&b).unwrap();
    let observed = l.clone();
    let fact = l.facts().iter().find(|f| f.slot.key == mapping(h(1), 3)).unwrap();
    assert_eq!(
        fact.origin,
        Origin::Observed {
            ordinal: 1,
            source_sha256: b.source_sha256.clone()
        }
    );
    let first = l.evaluate(h(1)).unwrap();
    let mut other = start;
    b.source_sha256 = "cd".repeat(32);
    other.apply(&b).unwrap();
    assert_eq!(first.outcome, other.evaluate(h(1)).unwrap().outcome);
    assert_ne!(first.input_sha256, other.evaluate(h(1)).unwrap().input_sha256);
    let mut idle = next(&l);
    idle.source_sha256 = "ef".repeat(32);
    l.apply(&idle).unwrap();
    assert_eq!(l.facts(), observed.facts());
    assert_eq!(l.evaluate(h(1)).unwrap().source_sha256, Some(idle.source_sha256));
    let bytes = l.snapshot().unwrap();
    let restored = Ledger::restore(&bytes, &binding::sha(&bytes), l.binding()).unwrap();
    assert_eq!(restored.evaluate(h(1)).unwrap(), l.evaluate(h(1)).unwrap());
    assert_eq!(restored.undo_snapshots(), 0);
    l.undo(observed.at().number, observed.at().hash).unwrap();
    assert_eq!(l, observed);
    let mut alternative = l.clone();
    let mut b = next(&l);
    b.writes.push(change(&l, mapping(h(1), 4), word(11), 2));
    alternative.apply(&b).unwrap();
    let idle = next(&l);
    l.apply(&idle).unwrap();
    assert_eq!(amount(&l, h(1)), amount(&alternative, h(1)));
    assert_ne!(l.evaluate(h(1)).unwrap().input_sha256, alternative.evaluate(h(1)).unwrap().input_sha256);
}
#[test]
fn missing_facts_and_model_refusal_never_return_stale_amount() {
    let mut cp = checkpoint(Model::BabyDoge);
    cp.words.retain(|f| f.slot.key != mapping(h(1), 3));
    let mut l = Ledger::from_checkpoint(cp, Limits::default()).unwrap();
    assert!(matches!(l.evaluate(h(1)).unwrap().outcome,Outcome::Unknown{ref missing} if missing.len()==1));
    let mut b = next(&l);
    b.writes.push(Write {
        slot: Slot {
            contract: l.binding().token(),
            key: mapping(h(1), 3),
        },
        old: word(0),
        new: word(0),
        ordinal: 1,
    });
    l.apply(&b).unwrap();
    assert_eq!(amount(&l, h(1)), "0");
    let mut b = next(&l);
    b.writes.push(change(&l, word(9), word(0), 1));
    l.apply(&b).unwrap();
    assert!(matches!(l.evaluate(h(1)).unwrap().outcome, Outcome::ModelRefusal { .. }));
}
#[test]
fn every_late_failure_is_atomic_including_metadata_noops_code_and_bounds() {
    let l = ledger();
    let base = change(&l, mapping(h(1), 3), word(101), 1);
    let mut variants = vec![];
    let mut b = next(&l);
    b.writes = vec![base.clone(), change(&l, mapping(h(2), 3), word(100), 2)];
    b.writes[1].old = word(999);
    variants.push(b);
    let mut b = next(&l);
    b.writes = vec![base.clone(), base.clone()];
    variants.push(b);
    let mut b = next(&l);
    b.writes = vec![base.clone(), change(&l, word(8), word(129), 2)];
    variants.push(b);
    let mut b = next(&l);
    b.writes = vec![base];
    b.codes.push(Code {
        contract: l.binding().token(),
        old: word(999),
        new: word(998),
        ordinal: 5,
    });
    variants.push(b);
    let mut b = next(&l);
    b.at.parent_hash = Some([9; 32]);
    variants.push(b);
    let mut b = next(&l);
    b.at.timestamp = 0;
    variants.push(b);
    let mut b = next(&l);
    b.source_sha256 = "bad".into();
    variants.push(b);
    for b in variants {
        let mut x = l.clone();
        assert!(x.apply(&b).is_err());
        assert_eq!(x, l);
    }
    let mut x = l.clone();
    x.apply(&next(&x)).unwrap();
    let before = x.clone();
    let mut bad = next(&x);
    bad.at.producer_version = Some(4);
    assert!(x.apply(&bad).is_err());
    assert_eq!(x, before);
}
#[test]
fn distinct_key_permutations_and_ties_are_eob_equivalent_but_per_key_ambiguity_refuses() {
    let l = ledger();
    let mut a = next(&l);
    a.writes = vec![change(&l, mapping(h(1), 3), word(200), 9), change(&l, mapping(h(2), 3), word(300), 9)];
    let mut b = a.clone();
    b.writes.reverse();
    let mut x = l.clone();
    let mut y = l.clone();
    assert_eq!(x.apply(&a).unwrap(), y.apply(&b).unwrap());
    assert_eq!(x, y);
    b.writes[0].ordinal = 8;
    y = l.clone();
    y.apply(&b).unwrap();
    assert_eq!(amount(&x, h(1)), amount(&y, h(1)));
    for ordinal in [0, 8, 9] {
        let mut bad = a.clone();
        bad.writes.push(Write {
            slot: a.writes[0].slot.clone(),
            old: word(200),
            new: word(200),
            ordinal,
        });
        let mut x = l.clone();
        assert!(x.apply(&bad).is_err());
        assert_eq!(x, l);
    }
    let mut valid = a.clone();
    valid.writes.push(Write {
        slot: a.writes[0].slot.clone(),
        old: word(200),
        new: word(200),
        ordinal: 10,
    });
    let mut x = l;
    x.apply(&valid).unwrap();
    assert_eq!(x.counters().writes, 3);
}
#[test]
fn group_apply_does_not_commit_first_model_on_second_model_error() {
    let first = ledger();
    let second = Ledger::from_checkpoint(checkpoint(Model::TenSet), Limits::default()).unwrap();
    let mut b = next(&first);
    b.writes.push(change(&first, mapping(h(1), 3), word(111), 1));
    let mut w = change(&second, mapping(h(1), 1), word(111), 2);
    w.old = word(999);
    b.writes.push(w);
    let mut group = vec![first, second];
    let before = group.clone();
    assert!(apply_all(&mut group, &b).is_err());
    assert_eq!(group, before);
}
#[test]
fn changed_exclusion_closure_removes_stale_facts_and_demands_new_inputs() {
    let mut l = ledger();
    let base = erc20_balances::hash(&word(8));
    let mut b = next(&l);
    b.writes.push(change(&l, word(8), word(1), 1));
    l.apply(&b).unwrap();
    assert!(matches!(l.evaluate(h(1)).unwrap().outcome, Outcome::Unknown { .. }));
    let mut b = next(&l);
    b.writes.push(Write {
        slot: Slot {
            contract: l.binding().token(),
            key: base,
        },
        old: word(0),
        new: word(3),
        ordinal: 1,
    });
    l.apply(&b).unwrap();
    assert!(matches!(l.evaluate(h(1)).unwrap().outcome, Outcome::Unknown { .. }));
    let mut b = next(&l);
    for (root, v) in [(3, 0), (4, 0), (7, 1)] {
        b.writes.push(Write {
            slot: Slot {
                contract: l.binding().token(),
                key: mapping(h(3), root),
            },
            old: word(0),
            new: word(v),
            ordinal: root,
        });
    }
    l.apply(&b).unwrap();
    assert_eq!(amount(&l, h(1)), "10");
    let mut b = next(&l);
    b.writes.push(change(&l, word(8), word(0), 1));
    l.apply(&b).unwrap();
    assert!(!l.facts().iter().any(|f| f.slot.key == base || f.slot.key == mapping(h(3), 3)));
    let mut b = next(&l);
    b.writes.push(change(&l, word(8), word(1), 1));
    l.apply(&b).unwrap();
    assert!(matches!(l.evaluate(h(1)).unwrap().outcome, Outcome::Unknown { .. }));
}
#[test]
fn code_restore_is_sticky_and_only_complete_new_epoch_resets() {
    let mut l = ledger();
    let cp = checkpoint(Model::BabyDoge);
    let original = l.binding().codes[0].runtime_hash;
    let mut b = next(&l);
    b.codes = vec![
        Code {
            contract: l.binding().token(),
            old: original,
            new: [7; 32],
            ordinal: 1,
        },
        Code {
            contract: l.binding().token(),
            old: [7; 32],
            new: original,
            ordinal: 2,
        },
    ];
    l.apply(&b).unwrap();
    assert!(matches!(l.evaluate(h(1)).unwrap().outcome, Outcome::Suspended { .. }));
    l.apply(&next(&l)).unwrap();
    assert!(matches!(l.evaluate(h(1)).unwrap().outcome, Outcome::Suspended { .. }));
    let mut reset = cp.clone();
    reset.at = l.at().clone();
    reset.binding = Binding::historical(Model::BabyDoge, 2, l.at().number).unwrap();
    for f in &mut reset.words {
        f.at = reset.at.clone();
        f.epoch = 2;
    }
    let mut missing = reset.clone();
    missing.words.pop();
    let before = l.clone();
    assert!(l.resume(missing).is_err());
    assert_eq!(l, before);
    let mut old = reset.clone();
    old.binding.epoch = 1;
    assert!(l.resume(old).is_err());
    l.resume(reset).unwrap();
    assert_eq!(l.binding().epoch, 2);
    assert_eq!(l.counters(), &Counters::default());
    assert_eq!(l.undo_snapshots(), 0);
    assert_eq!(amount(&l, h(1)), "10");
    assert!(l.undo(cp.at.number, cp.at.hash).is_err());
}
#[test]
fn snapshot_identity_duplicates_and_bounded_undo_are_strict() {
    let mut l = ledger();
    let original = l.clone();
    for _ in 0..3 {
        l.apply(&next(&l)).unwrap();
    }
    assert!(l.undo(original.at().number, original.at().hash).is_err());
    let before = l.clone();
    assert!(l.undo(l.at().number - 1, [0; 32]).is_err());
    assert_eq!(l, before);
    let bytes = l.snapshot().unwrap();
    assert!(Ledger::restore(&bytes, &"ff".repeat(32), l.binding()).is_err());
    let wrong = Binding::historical(Model::TenSet, 1, 122288005).unwrap();
    assert!(Ledger::restore(&bytes, &binding::sha(&bytes), &wrong).is_err());
    for change in 0..5 {
        let mut bad: Value = serde_json::from_slice(&bytes).unwrap();
        match change {
            0 => bad["schema"] = json!(99),
            1 => {
                let x = bad["state"]["words"][0].clone();
                bad["state"]["words"].as_array_mut().unwrap().push(x);
            }
            2 => bad["state"]["words"][0]["epoch"] = json!(99),
            3 => bad["state"]["source_sha256"] = json!("bad"),
            _ => bad["state"]["holders"][0] = json!(h(2)),
        };
        let raw = serde_json::to_vec(&bad).unwrap();
        assert!(Ledger::restore(&raw, &binding::sha(&raw), l.binding()).is_err());
    }
    let mut restored = Ledger::restore(&bytes, &binding::sha(&bytes), l.binding()).unwrap();
    assert!(restored.undo(l.at().number - 1, word(l.at().number - 1)).is_err());
}
#[test]
fn standalone_lbp_zero_ordinal_and_duplicate_checkpoint_keys_refuse() {
    let rows: Value = serde_json::from_slice(include_bytes!("../../tests/fixtures/lbp-rewards/checkpoint.json")).unwrap();
    let mut state = fixture::checkpoint(&rows).unwrap();
    let before = state.clone();
    let ((contract, key), v) = state.iter().next().unwrap();
    let changes = json!([{"contract":contract,"key":key,"old":v.to_string(),"new":(v+U256::one()).to_string(),"ordinal":0}]);
    assert!(fixture::apply(&mut state, &changes).is_err());
    assert_eq!(state, before);
    let mut dup = rows.clone();
    dup.as_array_mut().unwrap().push(rows[0].clone());
    assert!(fixture::checkpoint(&dup).is_err());
}

fn lbp_checkpoint() -> Checkpoint {
    let binding = Binding::historical(Model::Lbp, 1, 122288005).unwrap();
    let at = checkpoint(Model::BabyDoge).at;
    let evidence = "synthetic LBP raw facts".to_owned();
    let dep = binding.dependency().unwrap();
    let exempt = binding.exemptions[0];
    let packed = U256::one()
        + (U256::from(at.timestamp - 10) << 8)
        + (U256::from(at.timestamp - 10) << 72)
        + ((U256::from(10_000_000u64) * erc20_balances_tools::lbp_rewards::unit()) << 136);
    let mut pairs = vec![
        (dep, word(2), word(erc20_balances_tools::lbp_rewards::unit())),
        (dep, word(6), word(packed)),
        (dep, word(7), word(0)),
        (dep, word(8), word(0)),
        (dep, word(12), word(0)),
        (dep, word(17), word(0)),
    ];
    for a in [h(1), exempt] {
        pairs.push((binding.token(), mapping(a, 0), word(10)));
        for (root, v) in [
            (0, erc20_balances_tools::lbp_rewards::unit()),
            (9, U256::zero()),
            (10, U256::zero()),
            (11, U256::zero()),
        ] {
            pairs.push((dep, mapping(a, root), word(v)));
        }
    }
    let words = pairs
        .into_iter()
        .map(|(contract, key, word)| Fact {
            slot: Slot { contract, key },
            word,
            at: at.clone(),
            epoch: 1,
            origin: Origin::Checkpoint { evidence: evidence.clone() },
        })
        .collect();
    Checkpoint {
        binding,
        at,
        holders: vec![h(1), exempt],
        words,
        evidence,
    }
}
#[test]
fn lbp_clock_only_rewards_and_exempt_pending_refusal_remain_distinct() {
    let cp = lbp_checkpoint();
    let exempt = cp.binding.exemptions[0];
    let mut l = Ledger::from_checkpoint(cp.clone(), Limits::default()).unwrap();
    let before = amount(&l, h(1));
    let result = l.apply(&next(&l)).unwrap();
    assert_ne!(amount(&l, h(1)), before);
    assert_eq!(amount(&l, exempt), "10");
    assert!(result.changed.contains(&h(1)));
    let mut cp = cp;
    let dep = cp.binding.dependency().unwrap();
    cp.words.iter_mut().find(|f| f.slot.contract == dep && f.slot.key == word(7)).unwrap().word = [255; 32];
    let l = Ledger::from_checkpoint(cp, Limits::default()).unwrap();
    assert!(matches!(l.evaluate(h(1)).unwrap().outcome, Outcome::ModelRefusal { .. }));
    assert!(matches!(l.evaluate(exempt).unwrap().outcome,Outcome::Known{ref amount,pending:Some(Pending::ModelRefusal(_))} if amount=="10"));
}
#[test]
fn dependent_runtime_changes_restore_later_but_require_explicit_epoch_reset() {
    let mut l = Ledger::from_checkpoint(lbp_checkpoint(), Limits::default()).unwrap();
    let before = l.clone();
    let dep = l.binding().codes[1].clone();
    let mut b = next(&l);
    b.codes.push(Code {
        contract: dep.contract,
        old: dep.runtime_hash,
        new: [7; 32],
        ordinal: 3,
    });
    l.apply(&b).unwrap();
    let mut cp = lbp_checkpoint();
    cp.at = l.at().clone();
    cp.binding = Binding::historical(Model::Lbp, 2, l.at().number).unwrap();
    for f in &mut cp.words {
        f.at = cp.at.clone();
        f.epoch = 2;
    }
    assert!(l.resume(cp).is_err());
    let mut b = next(&l);
    b.codes.push(Code {
        contract: dep.contract,
        old: [7; 32],
        new: dep.runtime_hash,
        ordinal: 4,
    });
    l.apply(&b).unwrap();
    assert!(matches!(l.evaluate(h(1)).unwrap().outcome, Outcome::Suspended { .. }));
    l.undo(before.at().number, before.at().hash).unwrap();
    assert_eq!(l, before);
}
#[test]
fn collector_preserves_persisted_noops_and_filters_reverted_effects_without_width_bypass() {
    use substreams_ethereum::pb::eth::v2 as eth;
    let l = ledger();
    let b = next(&l);
    let mut header = eth::BlockHeader {
        number: b.at.number,
        parent_hash: l.at().hash.to_vec(),
        timestamp: Some(Default::default()),
        ..Default::default()
    };
    header.timestamp.as_mut().unwrap().seconds = b.at.timestamp as i64;
    let noop = eth::StorageChange {
        address: l.binding().token().to_vec(),
        key: mapping(h(1), 3).to_vec(),
        old_value: word(100).to_vec(),
        new_value: word(100).to_vec(),
        ordinal: 5,
    };
    let mut block = eth::Block {
        ver: 5,
        number: b.at.number,
        hash: b.at.hash.to_vec(),
        header: Some(header),
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        transaction_traces: vec![eth::TransactionTrace {
            status: eth::TransactionTraceStatus::Succeeded as i32,
            begin_ordinal: 1,
            end_ordinal: 10,
            calls: vec![eth::Call {
                begin_ordinal: 1,
                end_ordinal: 10,
                storage_changes: vec![noop],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let decoded = retained::collect::decode(&block, &b.source_sha256).unwrap();
    assert_eq!(decoded.writes.len(), 1);
    assert_eq!(decoded.writes[0].old, decoded.writes[0].new);
    let mut x = l.clone();
    x.apply(&decoded).unwrap();
    assert_eq!(x.counters().writes, 1);
    block.transaction_traces[0].calls[0].storage_changes[0].old_value = vec![0; 33];
    assert!(retained::collect::decode(&block, &b.source_sha256).is_err());
    block.transaction_traces[0].calls[0].state_reverted = true;
    assert!(retained::collect::decode(&block, &b.source_sha256).unwrap().writes.is_empty());
    block.transaction_traces[0].calls[0].state_reverted = false;
    block.transaction_traces[0].status = eth::TransactionTraceStatus::Failed as i32;
    assert!(retained::collect::decode(&block, &b.source_sha256).unwrap().writes.is_empty());
    block.transaction_traces[0].status = eth::TransactionTraceStatus::Succeeded as i32;
    block.transaction_traces[0].calls.clear();
    assert!(retained::collect::decode(&block, &b.source_sha256).is_err());
    block.transaction_traces.clear();
    block.ver = 3;
    assert!(retained::collect::decode(&block, &b.source_sha256).is_err());
    block.ver = 4;
    block.header.as_mut().unwrap().parent_hash.clear();
    assert!(retained::collect::decode(&block, &b.source_sha256).is_err());
}

#[test]
fn nine_actual_captured_immutable_getters_bind_selected_lbp_dependencies() {
    use erc20_balances_tools::ptoken_proof::{cases, vm};
    for (index, getters) in [
        (
            0,
            vec![
                ("pair()", "17591"),
                ("burnVault()", "17594"),
                ("refVault()", "17597"),
                ("polVault()", "17600"),
                ("fomoVault()", "17603"),
                ("hashrate()", "17607"),
            ],
        ),
        (1, vec![("lbp()", "9180"), ("pair()", "9184"), ("refVault()", "9187")]),
    ] {
        let c: Value = serde_json::from_slice(binding::CAPTURES[index].3).unwrap();
        let code = hex::decode(c["runtimeBytecode"]["onchainBytecode"].as_str().unwrap().trim_start_matches("0x")).unwrap();
        let contract = U256::from_big_endian(&address(binding::CAPTURES[index].1).unwrap());
        for (signature, id) in getters {
            let expected = hex::decode(
                c["runtimeBytecode"]["transformationValues"]["immutables"][id]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("0x"),
            )
            .unwrap();
            let result = vm::execute(&code, &cases::call(signature, &[]), 1u64.into(), contract, &vm::State::new());
            assert_eq!(result.exit, vm::Exit::Return(expected), "{signature}");
            assert!(result.reads.is_empty() && result.writes.is_empty() && result.logs.is_empty() && result.committed.is_empty());
        }
    }
}

#[test]
fn duplicate_zero_exclusion_members_keep_source_order_and_complete_raw_closure() {
    let mut l = ledger();
    let token = l.binding().token();
    let base = value(&erc20_balances::hash(&word(8)));
    let mut b = next(&l);
    b.writes.push(change(&l, word(8), word(2), 1));
    for (key, new) in [
        (word(base), word(0)),
        (word(base + U256::one()), word(0)),
        (mapping([0; 20], 3), word(51)),
        (mapping([0; 20], 4), word(0)),
        (mapping([0; 20], 7), word(1)),
    ] {
        b.writes.push(Write {
            slot: Slot { contract: token, key },
            old: word(0),
            new,
            ordinal: b.writes.len() as u64 + 1,
        });
    }
    l.apply(&b).unwrap();
    assert_eq!(amount(&l, h(1)), "12", "both zero-address array positions subtract their reflection words");
    let mut b = next(&l);
    b.writes.push(change(&l, word(8), word(1), 1));
    l.apply(&b).unwrap();
    assert_eq!(amount(&l, h(1)), "11");
    assert!(l.facts().iter().any(|f| f.slot.key == mapping([0; 20], 3)));
    assert!(!l.facts().iter().any(|f| f.slot.key == word(base + U256::one())));
}

#[test]
fn compiled_input_guard_rejects_disk_drift_in_code_and_original_captures() {
    let repo = tempfile::tempdir().unwrap();
    for (path, raw) in retained::snapshots::inputs() {
        let p = repo.path().join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, raw).unwrap();
    }
    retained::snapshots::verify(repo.path()).unwrap();
    for path in [
        "erc20/balances/tools/src/calculated_retention.rs",
        "erc20/balances/tools/src/bin/replay_calculated_retention.rs",
        "erc20/balances/tools/src/lbp_rewards/fixture.rs",
        "erc20/balances/src/persist.rs",
        "erc20/balances/tests/fixtures/lbp-rewards/checkpoint.json",
        "erc20/balances/tools/tests/fixtures/calculated-retention/lbp-source.json",
    ] {
        let p = repo.path().join(path);
        let raw = std::fs::read(&p).unwrap();
        let mut changed = raw.clone();
        changed.push(b' ');
        std::fs::write(&p, changed).unwrap();
        assert!(retained::snapshots::verify(repo.path()).unwrap_err().to_string().contains(path));
        std::fs::write(p, raw).unwrap();
    }
}
#[test]
fn input_digest_binds_checkpoint_registry_even_when_exclusion_facts_are_identical() {
    let mut a = checkpoint(Model::BabyDoge);
    a.words.iter_mut().find(|f| f.slot.key == word(8)).unwrap().word = word(1);
    a.words.push(Fact {
        slot: Slot {
            contract: a.binding.token(),
            key: erc20_balances::hash(&word(8)),
        },
        word: word(1),
        at: a.at.clone(),
        epoch: 1,
        origin: Origin::Checkpoint { evidence: a.evidence.clone() },
    });
    let mut b = a.clone();
    b.holders.retain(|holder| *holder != h(1));
    let a = Ledger::from_checkpoint(a, Limits::default()).unwrap();
    let b = Ledger::from_checkpoint(b, Limits::default()).unwrap();
    assert_eq!(a.facts(), b.facts());
    assert!(matches!(a.evaluate(h(1)).unwrap().outcome, Outcome::Known { .. }));
    assert!(matches!(b.evaluate(h(1)).unwrap().outcome, Outcome::Unknown { .. }));
    assert_ne!(a.evaluate(h(1)).unwrap().input_sha256, b.evaluate(h(1)).unwrap().input_sha256);
}

#[test]
fn unsupported_lbp_packed_padding_is_retained_as_raw_fact_without_widening_model() {
    let cp = lbp_checkpoint();
    let mut l = Ledger::from_checkpoint(cp, Limits::default()).unwrap();
    assert!(matches!(l.evaluate(h(1)).unwrap().outcome, Outcome::Known { .. }));
    let slot = Slot {
        contract: l.binding().dependency().unwrap(),
        key: word(6),
    };
    let old = l.facts().iter().find(|f| f.slot == slot).unwrap().word;
    assert_eq!(old[0], 0);
    let mut new = old;
    new[0] = 1;
    let mut b = next(&l);
    b.writes.push(Write {
        slot: slot.clone(),
        old,
        new,
        ordinal: 1,
    });
    l.apply(&b).unwrap();
    assert_eq!(l.facts().iter().find(|f| f.slot == slot).unwrap().word, new);
    assert!(matches!(l.evaluate(h(1)).unwrap().outcome,Outcome::ModelRefusal{ref reason} if reason.contains("padding")));
    assert!(matches!(l.evaluate(l.binding().exemptions[0]).unwrap().outcome,
        Outcome::Known { ref amount, pending: Some(Pending::ModelRefusal(ref reason)) }
            if amount == "10" && reason.contains("padding")));
    let mut b = next(&l);
    b.writes.push(Write {
        slot,
        old: new,
        new: old,
        ordinal: 1,
    });
    l.apply(&b).unwrap();
    assert!(matches!(l.evaluate(h(1)).unwrap().outcome, Outcome::Known { .. }));
    for holder in [[0; 20], address("0x000000000000000000000000000000000000dead").unwrap(), h(1)] {
        let mut cp = lbp_checkpoint();
        let dep = cp.binding.dependency().unwrap();
        cp.holders.iter_mut().filter(|a| **a == h(1)).for_each(|a| *a = holder);
        for f in &mut cp.words {
            if f.slot.contract == dep && f.slot.key == word(6) {
                f.word[0] = 1;
            }
            for (contract, root) in [(cp.binding.token(), 0), (dep, 0), (dep, 9), (dep, 10), (dep, 11)] {
                if f.slot.contract == contract && f.slot.key == mapping(h(1), root) {
                    f.slot.key = mapping(holder, root);
                    if holder == h(1) && contract == dep && root == 0 {
                        f.word = word(0);
                    }
                }
            }
        }
        let l = Ledger::from_checkpoint(cp, Limits::default()).unwrap();
        assert!(matches!(l.evaluate(holder).unwrap().outcome,
            Outcome::Known { ref amount, pending: Some(Pending::Known(ref pending)) } if amount == "10" && pending == "0"));
    }
}
