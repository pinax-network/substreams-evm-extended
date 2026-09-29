#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::{
    calculated_retention::{binding::sha, Code, Write},
    og_retention::{
        binding, collect,
        decode::{self, holder_key, period_key, scalar},
        *,
    },
};
use primitive_types::U256;
use serde_json::{json, Value};
fn h() -> Address {
    let mut a = [0; 20];
    a[19] = 1;
    a
}
fn cp() -> Checkpoint {
    let at = At {
        number: 122288107,
        hash: [1; 32],
        parent_hash: None,
        timestamp: 86400,
        producer_version: None,
    };
    let evidence = "synthetic complete finite inputs, no deployment claim".to_string();
    let mut words = std::collections::BTreeMap::new();
    for a in [h(), binding::pool_a()] {
        for i in 0..12 {
            words.insert(holder_key(a, 9, i), word(0));
        }
        for i in 0..4 {
            words.insert(holder_key(a, 11, i), word(0));
        }
        for root in [0, 10, 39, 40] {
            words.insert(holder_key(a, root, 0), word(0));
        }
    }
    for (s, a) in binding::pointers() {
        words.insert(s, word(U256::from_big_endian(&a)));
    }
    words.insert(scalar(binding::token(), 29), word(0));
    for a in [binding::pool_a(), binding::pool_b()] {
        words.insert(scalar(a, 8), word(U256::exp10(20)));
    }
    let facts: Vec<_> = words
        .into_iter()
        .map(|(slot, word)| Fact {
            slot,
            word,
            at: at.clone(),
            epoch: 1,
            origin: Origin::Checkpoint { evidence: evidence.clone() },
        })
        .collect();
    Checkpoint {
        binding: Binding::historical(1, at.number).unwrap(),
        at,
        holders: vec![h(), binding::pool_a()],
        universe: facts.iter().map(|f| f.slot.clone()).collect(),
        facts,
        evidence,
    }
}
fn set(c: &mut Checkpoint, s: Slot, w: Word) {
    if let Some(f) = c.facts.iter_mut().find(|f| f.slot == s) {
        f.word = w;
    } else {
        c.universe.push(s.clone());
        c.facts.push(Fact {
            slot: s,
            word: w,
            at: c.at.clone(),
            epoch: c.binding.epoch,
            origin: Origin::Checkpoint { evidence: c.evidence.clone() },
        });
    }
}
fn time(c: &mut Checkpoint, t: u64) {
    c.at.timestamp = t;
    for f in &mut c.facts {
        f.at = c.at.clone();
    }
}
fn ledger(c: Checkpoint) -> Ledger {
    Ledger::from_checkpoint(c, Limits { undo: 2, ..Limits::default() }).unwrap()
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
fn change(l: &Ledger, s: Slot, w: Word, o: u64) -> Write {
    Write {
        old: l.fact(&s).unwrap().word,
        slot: s,
        new: w,
        ordinal: o,
    }
}
fn active(days: usize) -> Checkpoint {
    let mut c = cp();
    time(&mut c, (days as u64) * 86400);
    set(&mut c, holder_key(h(), 9, 4), word(1));
    set(&mut c, holder_key(h(), 9, 5), word(1));
    set(&mut c, holder_key(h(), 9, 2), word(0)); // cap zero; period planning is still mandatory.
    for i in 0..168.min(days * 24) {
        for root in [23, 12, 24] {
            set(&mut c, period_key(h(), i.into(), root), word(1));
        }
    }
    for i in 0..days {
        for root in [25, 13, 27] {
            set(&mut c, period_key(h(), i.into(), root), word(1));
        }
    }
    c
}
#[test]
fn original_bindings_reconstruct_five_runtimes_and_literal_addresses() {
    binding::verify_uncached().unwrap();
    for a in binding::ARTIFACTS.iter().chain(binding::REPORTS) {
        let mut bad = a.raw.to_vec();
        bad.push(b' ');
        assert!(binding::verify_artifact(a.file, &bad).is_err());
    }
    for a in binding::ORIGINALS {
        let mut bad = a.raw.to_vec();
        bad.push(b' ');
        assert!(binding::verify_original(a.file, &bad).is_err());
    }
    let code = binding::runtime(binding::helper()).unwrap();
    binding::verify_literal(&code, 421, binding::router()).unwrap();
    for (pc, a) in [(422, binding::router()), (421, binding::token())] {
        assert!(binding::verify_literal(&code, pc, a).is_err());
    }
    let b = Binding::historical(1, 122288107).unwrap();
    let mut bad = b.clone();
    bad.evidence.reverse();
    assert!(bad.validate().is_err());
}
#[test]
fn early_zero_paths_need_no_history_but_missing_record_never_becomes_zero() {
    let mut c = cp();
    c.facts.retain(|f| f.slot != scalar(binding::token(), 29));
    let l = ledger(c.clone());
    let e = l.evaluate(h()).unwrap();
    assert_eq!(e.observable, Metric::Known("0".into()));
    c.facts.retain(|f| f.slot != holder_key(h(), 9, 11));
    let e = ledger(c).evaluate(h()).unwrap();
    assert!(e.hourly.is_missing());
    assert_eq!(e.raw_basis, Metric::Known("0".into()));
    assert!(e.observable.is_missing());
}
#[test]
fn active_planning_is_conservative_before_cap_short_circuits() {
    let c = active(4);
    assert!(matches!(ledger(c.clone()).evaluate(h()).unwrap().daily, Metric::Known(_)));
    for slot in [
        scalar(binding::pool_a(), 8),
        holder_key(h(), 40, 0),
        holder_key(binding::pool_a(), 9, 11),
        period_key(h(), 3.into(), 25),
    ] {
        let mut bad = c.clone();
        bad.facts.retain(|f| f.slot != slot);
        let e = ledger(bad).evaluate(h()).unwrap();
        assert!(
            e.daily.is_missing(),
            "missing {slot:?} must not be invented even with zero cap/three earlier positive periods"
        );
    }
}
#[test]
fn hourly_limit_and_daily_resource_bound_are_explicit() {
    let c = active(1000);
    let e = ledger(c.clone()).evaluate(h()).unwrap();
    assert_eq!(
        e.hourly,
        Metric::Known(Reward {
            amount: "0".into(),
            stopping_hour: "168".into()
        })
    );
    assert_eq!(e.daily, Metric::Known("0".into()));
    let mut bad = c;
    time(&mut bad, 1001 * 86400);
    assert!(matches!(ledger(bad).evaluate(h()).unwrap().daily, Metric::ScopeRefusal { .. }));
}
#[test]
fn combined_overflow_precedes_missing_daily_without_hiding_independent_daily() {
    let mut c = historical::checkpoints().unwrap().remove(0);
    let holder = c.holders.iter().copied().find(|h| *h != binding::pool_a()).unwrap();
    set(&mut c, holder_key(holder, 0, 0), word(U256::MAX));
    c.facts.retain(|f| f.slot != holder_key(holder, 11, 1));
    let e = ledger(c).evaluate(holder).unwrap();
    assert!(e.daily.is_missing());
    assert!(matches!(e.hourly,Metric::Known(ref h) if h.amount!="0"));
    assert!(matches!(e.observable,Metric::ModelRefusal{ref reason} if reason=="uint256 addition overflow"));
}
#[test]
fn pool_recursion_requires_all_four_cursor_words_and_is_not_fabricated_revert() {
    let mut c = active(1);
    set(&mut c, holder_key(binding::pool_a(), 9, 4), word(1));
    set(&mut c, holder_key(binding::pool_a(), 11, 0), word(24));
    assert!(matches!(decode::pool_terminal(&ledger(c.clone())), Metric::Known(_)));
    for i in 0..4 {
        let mut x = c.clone();
        x.facts.retain(|f| f.slot != holder_key(binding::pool_a(), 11, i));
        assert!(decode::pool_terminal(&ledger(x)).is_missing());
    }
    set(&mut c, holder_key(binding::pool_a(), 11, 0), word(0));
    assert!(matches!(decode::pool_terminal(&ledger(c)), Metric::ScopeRefusal { .. }));
}
#[test]
fn failed_apply_preserves_entire_ledger_even_after_retained_and_dependency_prefix() {
    let mut l = ledger(cp());
    l.apply(&next(&l)).unwrap();
    let before = l.clone();
    for kind in 0..5 {
        let mut b = next(&l);
        b.writes.push(change(&l, holder_key(h(), 0, 0), word(9), 1));
        match kind {
            0 => b.writes.push(Write {
                slot: holder_key(h(), 0, 0),
                old: word(8),
                new: word(10),
                ordinal: 2,
            }),
            1 => b.writes.push(Write {
                slot: holder_key(h(), 0, 0),
                old: word(9),
                new: word(9),
                ordinal: 1,
            }),
            2 => b.writes.push(Write {
                slot: holder_key(h(), 40, 0),
                old: word(0),
                new: word(0),
                ordinal: 0,
            }),
            3 => b.codes.push(Code {
                contract: binding::token(),
                old: [0; 32],
                new: [1; 32],
                ordinal: 3,
            }),
            _ => {
                b.writes.push(Write {
                    slot: scalar(binding::router(), 0),
                    old: word(0),
                    new: word(1),
                    ordinal: 2,
                });
                b.writes.push(Write {
                    slot: holder_key(h(), 0, 0),
                    old: word(99),
                    new: word(1),
                    ordinal: 3,
                });
            }
        }
        assert!(l.apply(&b).is_err());
        assert_eq!(l, before);
    }
}
#[test]
fn clock_producer_source_and_effect_limits_reject_atomically() {
    let mut l = ledger(cp());
    l.apply(&next(&l)).unwrap();
    let before = l.clone();
    for k in 0..8 {
        let mut b = next(&l);
        match k {
            0 => b.at.number += 1,
            1 => b.at.parent_hash = Some([0; 32]),
            2 => b.at.timestamp -= 2,
            3 => b.at.producer_version = Some(4),
            4 => b.at.producer_version = None,
            5 => b.source_sha256 = "not-sha".into(),
            6 => b.at.producer_version = Some(3),
            _ => b.writes = vec![change(&l, holder_key(h(), 0, 0), word(1), 1); Limits::default().effects + 1],
        };
        assert!(l.apply(&b).is_err());
        assert_eq!(l, before);
    }
}
#[test]
fn equality_and_distinct_key_permutations_preserve_physical_facts_and_provenance() {
    let mut a = ledger(cp());
    let mut b = a.clone();
    let mut input = next(&a);
    input.writes = vec![change(&a, holder_key(h(), 0, 0), word(0), 4), change(&a, holder_key(h(), 40, 0), word(1), 4)];
    let mut other = input.clone();
    other.writes.reverse();
    a.apply(&input).unwrap();
    b.apply(&other).unwrap();
    assert_eq!(a, b);
    let f = a.fact(&holder_key(h(), 0, 0)).unwrap().clone();
    assert!(matches!(f.origin, Origin::Observed { ordinal: 4, .. }));
    let before = a.evaluate(h()).unwrap();
    a.apply(&next(&a)).unwrap();
    assert_eq!(a.fact(&f.slot), Some(&f));
    assert_ne!(before.input_sha256, a.evaluate(h()).unwrap().input_sha256);
    let mut alternate = ledger(cp());
    input.source_sha256 = "cd".repeat(32);
    alternate.apply(&input).unwrap();
    assert_ne!(b.evaluate(h()).unwrap().input_sha256, alternate.evaluate(h()).unwrap().input_sha256);
}
#[test]
fn all_five_runtime_roundtrips_suspend_sticky_and_later_idle_is_not_known() {
    for (account, hash) in binding::codes() {
        let mut l = ledger(cp());
        let mut b = next(&l);
        b.codes = vec![
            Code {
                contract: account,
                old: hash,
                new: [9; 32],
                ordinal: 1,
            },
            Code {
                contract: account,
                old: [9; 32],
                new: hash,
                ordinal: 2,
            },
        ];
        l.apply(&b).unwrap();
        l.apply(&next(&l)).unwrap();
        assert!(l.suspension().is_some());
        assert!(matches!(l.evaluate(h()).unwrap().observable, Metric::Suspended { .. }));
    }
}
#[test]
fn every_pointer_guards_both_endpoints_and_masks_only_high_bytes() {
    for (slot, target) in binding::pointers() {
        let mut c = cp();
        let bound = word(U256::from_big_endian(&target));
        c.facts.retain(|f| f.slot != slot);
        let mut l = ledger(c);
        let mut b = next(&l);
        b.writes.push(Write {
            slot: slot.clone(),
            old: word(99),
            new: bound,
            ordinal: 1,
        });
        l.apply(&b).unwrap();
        assert!(l.suspension().is_some(), "cold foreign-to-bound");
        let mut l = ledger(cp());
        let mut b = next(&l);
        b.writes = vec![
            change(&l, slot.clone(), word(99), 1),
            Write {
                slot: slot.clone(),
                old: word(99),
                new: bound,
                ordinal: 2,
            },
        ];
        l.apply(&b).unwrap();
        assert!(l.suspension().is_some());
        let mut l = ledger(cp());
        let mut b = next(&l);
        let mut padded = bound;
        padded[0] = 255;
        let is_helper = slot.contract == binding::helper();
        b.writes.push(change(&l, slot, padded, 1));
        l.apply(&b).unwrap();
        assert_eq!(l.suspension().is_some(), is_helper);
        if !is_helper {
            assert_eq!(l.evaluate(h()).unwrap().observable, Metric::Known("0".into()));
        }
    }
}
#[test]
fn unreviewed_dependencies_suspend_but_reserves_and_token_words_remain_raw() {
    for a in [binding::helper(), binding::router(), binding::pool_a(), binding::pool_b()] {
        let mut l = ledger(cp());
        let mut b = next(&l);
        b.writes.push(Write {
            slot: scalar(a, 77),
            old: word(0),
            new: word(1),
            ordinal: 1,
        });
        l.apply(&b).unwrap();
        assert!(l.suspension().is_some());
    }
    let mut l = ledger(cp());
    let mut b = next(&l);
    b.writes.push(change(&l, scalar(binding::pool_a(), 8), word(0), 1));
    b.writes.push(Write {
        slot: scalar(binding::token(), 999),
        old: word(0),
        new: word(1),
        ordinal: 2,
    });
    l.apply(&b).unwrap();
    assert!(l.suspension().is_none());
    assert!(l.fact(&scalar(binding::token(), 999)).is_none());
}
#[test]
fn snapshots_undo_and_reset_have_strict_identity_and_no_carryover() {
    let mut l = ledger(cp());
    let parent = l.clone();
    let mut b = next(&l);
    b.writes.push(change(&l, holder_key(h(), 0, 0), word(8), 1));
    l.apply(&b).unwrap();
    let after = l.clone();
    let raw = l.snapshot().unwrap();
    let mut restored = Ledger::restore(&raw, &sha(&raw), l.binding()).unwrap();
    assert_eq!(restored.snapshot().unwrap(), raw);
    assert!(restored.undo(parent.at().number, parent.at().hash).is_err());
    assert!(l.undo(parent.at().number, [9; 32]).is_err());
    assert_eq!(l, after);
    l.undo(parent.at().number, parent.at().hash).unwrap();
    assert_eq!(l, parent);
    l.apply(&b).unwrap();
    assert_eq!(l, after);
    for edit in 0..4 {
        let mut v: Value = serde_json::from_slice(&raw).unwrap();
        match edit {
            0 => v["state"]["facts"][0]["at"]["number"] = json!(1),
            1 => v["state"]["facts"][0]["at"]["producer_version"] = json!(4),
            2 => {
                let duplicate = v["state"]["facts"][0].clone();
                v["state"]["facts"].as_array_mut().unwrap().push(duplicate);
            }
            _ => v["state"]["binding"]["epoch"] = json!(99),
        };
        let bad = serde_json::to_vec(&v).unwrap();
        assert!(Ledger::restore(&bad, &sha(&bad), after.binding()).is_err());
    }
    let mut b = next(&l);
    b.writes.push(Write {
        slot: scalar(binding::router(), 77),
        old: word(0),
        new: word(1),
        ordinal: 1,
    });
    l.apply(&b).unwrap();
    let mut c = cp();
    c.at = l.at().clone();
    c.binding = Binding::historical(2, c.at.number).unwrap();
    c.holders.sort();
    c.universe.sort();
    for f in &mut c.facts {
        f.at = c.at.clone();
        f.epoch = 2;
    }
    let mut missing = c.clone();
    missing.facts.retain(|f| f.slot != holder_key(h(), 0, 0));
    let before = l.clone();
    assert!(l.resume(missing).is_err());
    assert_eq!(l, before);
    l.resume(c).unwrap();
    assert!(l.suspension().is_none());
    assert_eq!(l.counters().blocks, 0);
    assert_eq!(l.evaluate(h()).unwrap().raw_basis, Metric::Known("0".into()));
    assert!(l.undo(parent.at().number, parent.at().hash).is_err());
}
#[test]
fn malformed_checkpoint_and_resource_bounds_reject() {
    let c = cp();
    let mut bad = c.clone();
    bad.facts.push(bad.facts[0].clone());
    assert!(Ledger::from_checkpoint(bad, Limits::default()).is_err());
    let mut bad = c.clone();
    bad.universe.push(bad.universe[0].clone());
    assert!(Ledger::from_checkpoint(bad, Limits::default()).is_err());
    let mut bad = c.clone();
    bad.holders.push(h());
    assert!(Ledger::from_checkpoint(bad, Limits::default()).is_err());
    assert!(Ledger::from_checkpoint(
        c.clone(),
        Limits {
            keys: 32768,
            undo: 128,
            ..Limits::default()
        }
    )
    .is_err());
    assert!(Ledger::from_checkpoint(
        c,
        Limits {
            effects: 65537,
            ..Limits::default()
        }
    )
    .is_err());
}
#[test]
fn collector_rejects_changed_original_bytes_and_invalid_headers() {
    use prost::Message;
    use substreams_ethereum::pb::eth::v2 as eth;
    let mut b = eth::Block {
        number: 122288108,
        hash: vec![1; 32],
        ver: 5,
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        header: Some(eth::BlockHeader {
            number: 122288108,
            parent_hash: vec![0; 32],
            timestamp: Some(Default::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    b.header.as_mut().unwrap().timestamp.as_mut().unwrap().seconds = 86400;
    let raw = b.encode_to_vec();
    collect::decode_original(&raw, b.number, [1; 32], &sha(&raw)).unwrap();
    let mut reencoded = b.clone();
    reencoded.header.as_mut().unwrap().timestamp.as_mut().unwrap().seconds += 1;
    let valid_altered = reencoded.encode_to_vec();
    assert!(collect::decode_original(&valid_altered, b.number, [1; 32], &sha(&raw))
        .unwrap_err()
        .to_string()
        .contains("original PB digest differs"));
    collect::decode_original(&valid_altered, b.number, [1; 32], &sha(&valid_altered)).unwrap();
    let mut changed = raw.clone();
    changed.push(0);
    assert!(collect::decode_original(&changed, b.number, [1; 32], &sha(&raw)).is_err());
    for mode in 0..4 {
        let mut x = b.clone();
        match mode {
            0 => x.ver = 3,
            1 => x.header.as_mut().unwrap().number += 1,
            2 => x.header.as_mut().unwrap().timestamp.as_mut().unwrap().nanos = 1,
            _ => x.detail_level = eth::block::DetailLevel::DetaillevelBase as i32,
        };
        assert!(collect::decode(&x, &sha(&raw)).is_err());
    }
    b.header = None;
    assert!(collect::decode(&b, &sha(&raw)).is_err());
}

#[test]
fn idle_hour_day_boundaries_and_cursor_changes_recompute_without_backfill() {
    for (before, after, missing) in [(3599, 3600, period_key(h(), 0.into(), 23)), (86399, 86400, period_key(h(), 0.into(), 25))] {
        let mut c = active(1);
        time(&mut c, before);
        c.facts.retain(|f| f.slot != missing);
        let mut l = ledger(c);
        let old = l.evaluate(h()).unwrap();
        if before == 3599 {
            assert!(matches!(old.hourly, Metric::Known(_)));
        } else {
            assert!(matches!(old.daily, Metric::Known(_)));
        }
        let facts = l.facts().to_vec();
        let mut b = next(&l);
        b.at.timestamp = after;
        l.apply(&b).unwrap();
        let new = l.evaluate(h()).unwrap();
        if before == 3599 {
            assert!(new.hourly.is_missing());
        } else {
            assert!(new.daily.is_missing());
        }
        assert!(new.observable.is_missing());
        assert_eq!(l.facts(), facts);
        assert!(l.fact(&missing).is_none());
        let cursor = holder_key(h(), 11, if before == 3599 { 0 } else { 1 });
        let mut b = next(&l);
        b.writes.push(change(&l, cursor.clone(), word(U256::MAX), 1));
        l.apply(&b).unwrap();
        let e = l.evaluate(h()).unwrap();
        if before == 3599 {
            assert_eq!(
                e.hourly,
                Metric::Known(Reward {
                    amount: "0".into(),
                    stopping_hour: "0".into()
                })
            );
        } else {
            assert_eq!(e.daily, Metric::Known("0".into()));
        }
        assert!(l.fact(&missing).is_none()); // Future cursor is an early exit, not a fill.
        let old_period = period_key(h(), 0.into(), 24);
        let saved = l.fact(&old_period).unwrap().clone();
        let mut b = next(&l);
        b.writes.push(change(&l, cursor, word(0), 1));
        l.apply(&b).unwrap();
        let e = l.evaluate(h()).unwrap();
        if before == 3599 {
            assert!(e.hourly.is_missing());
        } else {
            assert!(e.daily.is_missing());
        }
        assert_eq!(l.fact(&old_period), Some(&saved));
    }
}

#[test]
fn source_unavailable_helper_upper_word_mutations_suspend_but_bound_noops_do_not() {
    for (slot, target) in binding::pointers().into_iter().filter(|(s, _)| s.contract == binding::helper()) {
        let mut c = cp();
        let mut upper = word(U256::from_big_endian(&target));
        upper[0] = 1;
        set(&mut c, slot.clone(), upper);
        let mut l = ledger(c);
        let mut b = next(&l);
        b.writes.push(change(&l, slot.clone(), upper, 1));
        l.apply(&b).unwrap();
        assert!(l.suspension().is_none());
        let mut changed = upper;
        changed[0] = 2;
        let mut b = next(&l);
        b.writes = vec![
            change(&l, slot.clone(), changed, 1),
            Write {
                slot,
                old: changed,
                new: upper,
                ordinal: 2,
            },
        ];
        l.apply(&b).unwrap();
        assert!(
            l.suspension().is_some(),
            "masked pointer equality is not proof that opaque helper upper-bit changes are harmless"
        );
    }
}

#[test]
fn independent_pool_observations_bind_all_21_words_without_importing_values() {
    let observations = journal::pool_observations().unwrap();
    assert_eq!(observations.len(), 1024);
    let c = historical::checkpoints().unwrap().remove(0);
    let row = &observations[(c.at.number - 122288006) as usize];
    assert!(journal::check_pool(&ledger(c.clone()), row).is_err(), "original153 lacks four cursors");
    let joined = journal::join_initial(&c, row).unwrap();
    assert_eq!(joined.checkpoint.facts.len(), 157);
    assert_eq!(c.facts.len(), 153);
    let l = ledger(joined.checkpoint.clone());
    journal::check_pool(&l, row).unwrap();
    assert_eq!(joined.provenance["pool_artifact"]["overlapping_slots"].as_array().unwrap().len(), 17);
    assert_eq!(joined.provenance["pool_artifact"]["new_raw_facts"].as_array().unwrap().len(), 4);
    for mutation in 0..5 {
        let mut wrong = row.clone();
        match mutation {
            0 => wrong.at.number += 1,
            1 => wrong.at.hash = [0; 32],
            2 => wrong.at.timestamp += 1,
            3 => wrong.words[0].0 = word(99),
            _ => {
                wrong.words.pop();
            }
        };
        assert!(journal::join_initial(&c, &wrong).is_err());
    }
    let mut missing = c.clone();
    missing.facts.pop();
    assert!(journal::join_initial(&missing, row).is_err());
    let mut wrong = c;
    wrong.facts[0].word = word(99);
    assert!(journal::join_initial(&wrong, row).is_err());
    let mut missing = joined.checkpoint;
    missing.facts.retain(|f| f.slot != row.words[0].1);
    let l = ledger(missing);
    assert!(journal::check_pool(&l, row).is_err());
    assert!(l.fact(&row.words[0].1).is_none());
}

#[test]
fn fixed_saved_journal_preserves_raw_pool_facts_sticky_scope_and_restart_identity() {
    let inputs = journal::verify(journal::BYTES).unwrap();
    assert_eq!(inputs.len(), 922);
    let original = historical::checkpoints().unwrap().remove(0);
    let pool = journal::pool_observations().unwrap();
    let start = (original.at.number - 122288006) as usize;
    let joined = journal::join_initial(&original, &pool[start]).unwrap();
    let mut l = Ledger::from_checkpoint(joined.checkpoint.clone(), Limits::default()).unwrap();
    let old = ledger(original);
    for holder in l.holders() {
        let a = old.evaluate(*holder).unwrap();
        let b = l.evaluate(*holder).unwrap();
        assert_eq!((a.raw_basis, a.hourly, a.daily, a.observable), (b.raw_basis, b.hourly, b.daily, b.observable));
    }
    let mut midpoint = None;
    for (i, input) in inputs.iter().enumerate() {
        let out = l.apply(input).unwrap();
        journal::check_pool(&l, &pool[start + i + 1]).unwrap();
        assert_eq!(out.evaluations.len(), 2);
        for e in out.evaluations {
            assert!(matches!(e.raw_basis, Metric::Known(_)));
            assert!(matches!(e.hourly, Metric::Suspended { .. }));
            assert!(matches!(e.daily, Metric::Suspended { .. }));
            assert!(matches!(e.observable, Metric::Suspended { .. }));
        }
        if i == 460 {
            midpoint = Some(l.snapshot().unwrap());
        }
    }
    assert_eq!(l.counters().retained_writes, 274);
    assert_eq!(l.counters().evaluations, 1844);
    let raw = midpoint.unwrap();
    let mut resumed = Ledger::restore(&raw, &sha(&raw), l.binding()).unwrap();
    for input in &inputs[461..] {
        resumed.apply(input).unwrap();
    }
    assert_eq!(resumed.snapshot().unwrap(), l.snapshot().unwrap());
    let last = l.clone();
    let too_old = &inputs[904].at;
    assert!(l.undo(too_old.number, too_old.hash).is_err(), "17 blocks exceed the configured ring");
    assert_eq!(l, last, "out-of-ring refusal preserves the entire ledger");
    let oldest = &inputs[905].at;
    l.undo(oldest.number, oldest.hash).unwrap();
    for input in &inputs[906..] {
        l.apply(input).unwrap();
    }
    assert_eq!(l, last);
    let mut bad = journal::BYTES.to_vec();
    bad.push(b' ');
    assert!(journal::verify(&bad).is_err());
    let mut altered = inputs.clone();
    altered[2].source_sha256 = "ab".repeat(32);
    let raw = altered.iter().map(|v| serde_json::to_string(v).unwrap() + "\n").collect::<String>();
    assert!(journal::decode(raw.as_bytes(), &sha(raw.as_bytes())).is_err());
    let mut missing = joined.checkpoint;
    missing.facts.retain(|f| f.slot != decode::holder_key(binding::pool_a(), 11, 0));
    let mut cold = ledger(missing);
    for input in &inputs {
        cold.apply(input).unwrap();
    }
    assert!(
        cold.fact(&decode::holder_key(binding::pool_a(), 11, 0)).is_none(),
        "later observations cannot repair missing checkpoint fact"
    );
}

#[test]
fn compiled_sources_refuse_restored_disk_drift_in_a_pure_parser_helper() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    snapshots::verify(&repo).unwrap();
    let dir = tempfile::tempdir().unwrap();
    for (path, raw) in snapshots::inputs() {
        let dest = dir.path().join(path);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(dest, raw).unwrap();
    }
    let p = dir.path().join("erc20/balances/tools/src/rpc.rs");
    let mut raw = std::fs::read(&p).unwrap();
    raw.push(b' ');
    std::fs::write(p, raw).unwrap();
    assert!(snapshots::verify(dir.path()).is_err());
}
