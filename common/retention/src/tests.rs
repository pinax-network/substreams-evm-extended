use super::*;

fn clock(n: u64) -> Clock {
    Clock {
        number: n,
        hash: vec![n as u8; 32],
        parent_hash: vec![(n - 1) as u8; 32],
    }
}
fn key(contract: u8, holder: u8) -> Key {
    Key {
        contract: Some(vec![contract; 20]),
        address: vec![holder; 20],
    }
}
fn row(contract: u8, holder: u8, amount: &str) -> balances::Balance {
    balances::Balance {
        contract: Some(vec![contract; 20]),
        address: vec![holder; 20],
        amount: amount.into(),
    }
}
fn known(l: &Ledger, k: &Key) -> Option<String> {
    match l.lookup(k) {
        Lookup::Known(e) => Some(e.value.clone()),
        _ => None,
    }
}

#[test]
fn absent_writes_never_become_zero_and_known_zero_stays_distinct() {
    let mut l = Ledger::new(4);
    l.apply(&clock(10), &[row(1, 1, "5"), row(1, 2, "0")]).unwrap();
    assert_eq!(known(&l, &key(1, 1)).as_deref(), Some("5"));
    assert_eq!(known(&l, &key(1, 2)).as_deref(), Some("0"));
    assert_eq!(l.lookup(&key(1, 3)), Lookup::Unknown);
    // Burn to zero and mint from unknown are transitions between distinct states.
    l.apply(&clock(11), &[row(1, 1, "0"), row(1, 3, "7")]).unwrap();
    assert_eq!(known(&l, &key(1, 1)).as_deref(), Some("0"));
    assert_eq!(known(&l, &key(1, 3)).as_deref(), Some("7"));
    let e = &l.entries()[&key(1, 3)];
    assert_eq!((e.since, e.updated, &e.origin), (11, 11, &Origin::Observed));
    let c = l.compare(&[
        (key(1, 1), "0".into()),
        (key(1, 2), "0".into()),
        (key(1, 3), "7".into()),
        (key(1, 4), "9".into()),
        (key(1, 5), "0".into()),
    ]);
    assert_eq!(
        (c.matches, c.known_zero_matches, c.unknown, c.unknown_nonzero, c.mismatches.len()),
        (3, 2, 2, 1, 0)
    );
    assert_eq!(c.status(), "coverage_gap");
    let r = l.report();
    assert_eq!(
        (
            r.initialized_holders,
            r.observed_holders,
            r.known_zero_holders,
            r.emitted_rows,
            r.cold_unknown_lookups
        ),
        (3, 3, 2, 4, 3)
    );
    assert!(r.enumerated.is_none());
}

#[test]
fn empty_blocks_and_global_only_updates_retain_holders_unchanged() {
    let mut l = Ledger::new(4);
    l.apply(&clock(10), &[row(1, 1, "5")]).unwrap();
    for n in 11..=14 {
        assert_eq!(l.apply(&clock(n), &[]).unwrap(), Applied::default());
    }
    let e = &l.entries()[&key(1, 1)];
    assert_eq!((e.value.as_str(), e.since, e.updated), ("5", 10, 10));
    // A passive reserve update touches no holder entry: the basis stays,
    // the evaluation clock moves.
    let events = state::Events {
        clocks: vec![state::BlockClock {
            number: 15,
            hash: vec![15; 32],
            ..Default::default()
        }],
        global_state: vec![state::GlobalState {
            market: vec![2; 20],
            field: 1,
            value: "1000000000000000000000000001".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    l.apply_state(&clock(15), &events).unwrap();
    assert_eq!(l.entries().len(), 1);
    assert_eq!(l.report().blocks_applied, 6);
    let wrong_clock = state::Events {
        clocks: vec![state::BlockClock {
            number: 99,
            hash: vec![16; 32],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert!(l.apply_state(&clock(16), &wrong_clock).unwrap_err().to_string().contains("block clock"));
}

#[test]
fn checkpoint_and_deployment_origins_are_distinct_and_bounded() {
    let mut l = Ledger::new(2);
    l.seed_checkpoint(key(1, 1), "42", 9, "rpc-checkpoint bsc-122288005.json#sha256=abc").unwrap();
    assert!(l.seed_checkpoint(key(1, 1), "1", 9, "x").unwrap_err().to_string().contains("already seeded"));
    assert!(l.seed_checkpoint(key(1, 2), "-0", 9, "x").is_err());
    assert!(l.seed_checkpoint(key(1, 2), "01", 9, "x").is_err());
    assert!(l.seed_checkpoint(key(1, 2), "1", 9, "").is_err());
    // A checkpoint must sit at the parent of the first block.
    assert!(l.apply(&clock(11), &[]).unwrap_err().to_string().contains("not the parent"));
    l.apply(&clock(10), &[]).unwrap();
    assert!(l
        .seed_checkpoint(key(1, 2), "1", 9, "x")
        .unwrap_err()
        .to_string()
        .contains("before the first block"));
    let holders: BTreeSet<Vec<u8>> = [vec![7; 20], vec![8; 20]].into();
    assert_eq!(l.seed_deployment_zero(&[3; 20], &holders, 10).unwrap(), 2);
    assert!(l.seed_deployment_zero(&[3; 20], &holders, 10).unwrap_err().to_string().contains("already has"));
    assert!(l
        .seed_deployment_zero(&[4; 20], &holders, 11)
        .unwrap_err()
        .to_string()
        .contains("after the last"));
    let r = l.report();
    assert_eq!(
        (
            r.checkpoint_seeded_holders,
            r.deployment_seeded_holders,
            r.observed_holders,
            r.known_zero_holders
        ),
        (1, 2, 0, 2)
    );
    l.apply(&clock(11), &[row(1, 1, "43")]).unwrap();
    let e = &l.entries()[&key(1, 1)];
    assert_eq!((e.since, e.updated, &e.origin), (9, 11, &Origin::Observed));
    let c = l.compare(&[(key(1, 1), "43".into()), (key(3, 7), "0".into())]);
    assert_eq!((c.status(), c.matches), ("bounded_parity", 2));
}

#[test]
fn unsupported_contracts_are_visible_not_carried_and_enumeration_needs_evidence() {
    let mut l = Ledger::new(2);
    l.mark_unsupported(&[9; 20], "computed balance; no qualified layout");
    assert!(l.apply(&clock(10), &[row(9, 1, "1")]).unwrap_err().to_string().contains("unsupported"));
    l.apply(&clock(10), &[row(1, 1, "1")]).unwrap();
    assert_eq!(l.lookup(&key(9, 1)), Lookup::Unsupported("computed balance; no qualified layout"));
    let c = l.compare(&[(key(9, 1), "1".into())]);
    assert_eq!((c.unsupported, c.status()), (1, "coverage_gap"));
    let holders: BTreeSet<Vec<u8>> = [vec![1; 20], vec![2; 20]].into();
    assert!(l.attach_enumeration(&[1; 20], &holders, 10, "").is_err());
    l.attach_enumeration(&[1; 20], &holders, 10, "independent holder enumeration evidence.json")
        .unwrap();
    let e = l.report().enumerated.unwrap();
    assert_eq!((e.holders, e.retained_missing), (2, 1));
}

#[test]
fn gaps_and_forks_are_rejected_and_undo_restores_exact_prior_state() {
    let mut l = Ledger::new(2);
    l.apply(&clock(10), &[row(1, 1, "1"), row(1, 2, "2")]).unwrap();
    let after_10 = l.entries().clone();
    l.apply(&clock(11), &[row(1, 1, "10"), row(1, 3, "3")]).unwrap();
    l.apply(&clock(12), &[row(1, 2, "0")]).unwrap();
    assert!(l.apply(&clock(14), &[]).unwrap_err().to_string().contains("gap"));
    let mut forked = clock(13);
    forked.parent_hash = vec![0xff; 32];
    assert!(l.apply(&forked, &[]).unwrap_err().to_string().contains("fork"));
    assert!(l.undo(9).unwrap_err().to_string().contains("depth"));
    assert_eq!(l.undo(10).unwrap(), 2);
    assert_eq!(l.entries(), &after_10);
    let r = l.report();
    assert_eq!((r.blocks_applied, r.undone_blocks, r.emitted_rows, r.last_block), (1, 2, 2, Some(10)));
    // The replacement block must chain to the retained hash of block 10.
    let mut replacement = clock(11);
    replacement.hash = vec![0xaa; 32];
    l.apply(&replacement, &[row(1, 1, "11")]).unwrap();
    assert_eq!(known(&l, &key(1, 1)).as_deref(), Some("11"));
    assert_eq!(l.lookup(&key(1, 3)), Lookup::Unknown);
    assert!(l.undo(11).unwrap_err().to_string().contains("not below"));
}

#[test]
fn migrations_suspend_and_rebinding_decides_carryover() {
    let mut l = Ledger::new(4);
    let market = vec![2; 20];
    let events = |n: u64, epochs: Vec<state::ModelEpoch>, basis: Vec<state::HolderBasis>| state::Events {
        clocks: vec![state::BlockClock {
            number: n,
            hash: vec![n as u8; 32],
            ..Default::default()
        }],
        epochs,
        holder_basis: basis,
        ..Default::default()
    };
    let basis = |holder: u8, value: &str, signed: bool| state::HolderBasis {
        market: market.clone(),
        holder: vec![holder; 20],
        value: value.into(),
        signed,
        ..Default::default()
    };
    let epoch = |kind: state::EpochEventKind, epoch: u32, reason: state::InvalidationReason, carry: bool| state::ModelEpoch {
        market: market.clone(),
        epoch,
        kind: kind as i32,
        reason: reason as i32,
        basis_carryover: carry,
        ..Default::default()
    };
    l.apply_state(
        &clock(10),
        &events(
            10,
            vec![epoch(state::EpochEventKind::Bound, 1, state::InvalidationReason::Unspecified, true)],
            vec![basis(1, "-5", true), basis(2, "9", false)],
        ),
    )
    .unwrap();
    assert!(l.apply_state(&clock(11), &events(11, vec![], vec![basis(3, "-1", false)])).is_err());
    let k = Key {
        contract: Some(market.clone()),
        address: vec![1; 20],
    };
    assert_eq!(known(&l, &k).as_deref(), Some("-5"));
    l.apply_state(
        &clock(11),
        &events(
            11,
            vec![epoch(state::EpochEventKind::Invalidated, 1, state::InvalidationReason::RateModelChange, true)],
            vec![basis(2, "10", false)],
        ),
    )
    .unwrap();
    assert_eq!(
        l.lookup(&k),
        Lookup::Suspended {
            epoch: 1,
            reason: state::InvalidationReason::RateModelChange as i32
        }
    );
    assert_eq!(l.compare(&[(k.clone(), "-5".into())]).suspended, 1);
    // Re-binding with carryover keeps the basis; without it, holders are unknown again.
    l.apply_state(
        &clock(12),
        &events(
            12,
            vec![epoch(state::EpochEventKind::Bound, 2, state::InvalidationReason::Unspecified, true)],
            vec![],
        ),
    )
    .unwrap();
    assert_eq!(known(&l, &k).as_deref(), Some("-5"));
    l.apply_state(
        &clock(13),
        &events(
            13,
            vec![epoch(state::EpochEventKind::Bound, 3, state::InvalidationReason::Unspecified, false)],
            vec![],
        ),
    )
    .unwrap();
    assert_eq!(l.lookup(&k), Lookup::Unknown);
    assert_eq!(l.report().suspended_markets, 0);
    // Undo through the migration restores the suspension and the entries.
    l.undo(11).unwrap();
    assert_eq!(
        l.lookup(&k),
        Lookup::Suspended {
            epoch: 1,
            reason: state::InvalidationReason::RateModelChange as i32
        }
    );
    l.undo(10).unwrap();
    assert_eq!(known(&l, &k).as_deref(), Some("-5"));
    assert_eq!(
        known(
            &l,
            &Key {
                contract: Some(market.clone()),
                address: vec![2; 20]
            }
        )
        .as_deref(),
        Some("9")
    );
}

#[test]
fn final_snapshot_comparison_reports_mismatches_separately_from_gaps() {
    let mut l = Ledger::new(1);
    l.apply(&clock(10), &[row(1, 1, "1"), row(1, 2, "2")]).unwrap();
    assert!(l
        .apply(&clock(11), &[row(1, 1, "1"), row(1, 1, "2")])
        .unwrap_err()
        .to_string()
        .contains("duplicate"));
    assert!(l.apply(&clock(11), &[row(1, 1, "1e3")]).unwrap_err().to_string().contains("invalid amount"));
    l.apply(&clock(11), &[row(1, 2, "3")]).unwrap();
    let c = l.compare(&[(key(1, 1), "1".into()), (key(1, 2), "4".into()), (key(1, 3), "0".into())]);
    assert_eq!((c.matches, c.mismatches.len(), c.unknown, c.unknown_nonzero), (1, 1, 1, 0));
    assert_eq!(c.mismatches[0], (key(1, 2), "3".into(), "4".into()));
    assert_eq!(c.status(), "mismatch");
    assert!(serde_json::to_string(&c).unwrap().contains("\"mismatches\""));
    assert!(serde_json::to_string(&l.report()).unwrap().contains("\"cold_unknown_lookups\""));
}
