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
fn balances_ledger(undo_depth: usize) -> Ledger {
    Ledger::new(undo_depth, Domain::Balances)
}
fn state_ledger(undo_depth: usize) -> Ledger {
    Ledger::new(undo_depth, Domain::BalanceState)
}

const CHAIN: u64 = 56;

/// A complete balance-state block: counts equal the rows and the stream
/// identity is the same for every block.
fn state_events(n: u64, epochs: Vec<state::ModelEpoch>, holder_basis: Vec<state::HolderBasis>) -> state::Events {
    let clock = clock(n);
    state::Events {
        clocks: vec![state::BlockClock {
            chain_id: CHAIN,
            number: n,
            hash: clock.hash,
            parent_hash: clock.parent_hash,
            package: "test_balance_state".into(),
            package_version: "v0.1.0".into(),
            spec_revision: 1,
            parameters_sha256: "ab".repeat(32),
            epoch_count: epochs.len() as u32,
            holder_basis_count: holder_basis.len() as u32,
            ..Default::default()
        }],
        epochs,
        holder_basis,
        ..Default::default()
    }
}
fn with_globals(mut events: state::Events, globals: Vec<state::GlobalState>) -> state::Events {
    events.clocks[0].global_state_count = globals.len() as u32;
    events.global_state = globals;
    events
}
/// Unsigned shares of epoch 1; see `basis_in` for other epochs.
fn basis(contract: u8, holder: u8, value: &str) -> state::HolderBasis {
    basis_in(1, contract, holder, value)
}
fn basis_in(epoch: u32, contract: u8, holder: u8, value: &str) -> state::HolderBasis {
    state::HolderBasis {
        chain_id: CHAIN,
        market: vec![contract; 20],
        holder: vec![holder; 20],
        epoch,
        basis_kind: state::BasisKind::Shares as i32,
        value: value.into(),
        observation: state::Observation::ObservedWrite as i32,
        boundary: state::Boundary::EndOfBlock as i32,
        scope: state::Scope::Transaction as i32,
        ..Default::default()
    }
}
fn principal(contract: u8, holder: u8, value: &str) -> state::HolderBasis {
    state::HolderBasis {
        basis_kind: state::BasisKind::SignedPrincipal as i32,
        signed: true,
        ..basis(contract, holder, value)
    }
}
fn global(contract: u8, value: &str) -> state::GlobalState {
    state::GlobalState {
        chain_id: CHAIN,
        market: vec![contract; 20],
        epoch: 1,
        field: state::StateField::AaveLiquidityIndex as i32,
        value: value.into(),
        observation: state::Observation::ObservedWrite as i32,
        boundary: state::Boundary::EndOfBlock as i32,
        scope: state::Scope::Transaction as i32,
        ..Default::default()
    }
}
fn epoch_row(contract: u8, epoch: u32, kind: state::EpochEventKind, ordinal: u64, carryover: bool) -> state::ModelEpoch {
    state::ModelEpoch {
        chain_id: CHAIN,
        market: vec![contract; 20],
        epoch,
        ordinal,
        kind: kind as i32,
        reason: if kind == state::EpochEventKind::Bound {
            state::InvalidationReason::Unspecified as i32
        } else {
            state::InvalidationReason::RateModelChange as i32
        },
        basis_carryover: carryover,
        ..Default::default()
    }
}
fn bound(contract: u8, epoch: u32, ordinal: u64, carryover: bool) -> state::ModelEpoch {
    epoch_row(contract, epoch, state::EpochEventKind::Bound, ordinal, carryover)
}
fn invalidated(contract: u8, epoch: u32, ordinal: u64) -> state::ModelEpoch {
    epoch_row(contract, epoch, state::EpochEventKind::Invalidated, ordinal, false)
}

fn assert_unchanged(actual: &Ledger, before: &Ledger) {
    assert_eq!(actual.entries, before.entries);
    assert_eq!(actual.suspended, before.suspended);
    assert_eq!(actual.unsupported, before.unsupported);
    assert_eq!(actual.epochs, before.epochs);
    assert_eq!(actual.contracts, before.contracts);
    assert_eq!(actual.stream, before.stream);
    assert_eq!(actual.enumerations, before.enumerations);
    assert_eq!(actual.last, before.last);
    assert_eq!(actual.journal, before.journal);
    assert_eq!(actual.report(), before.report());
}

#[test]
fn absent_writes_never_become_zero_and_known_zero_stays_distinct() {
    let mut l = balances_ledger(4);
    let applied = l.apply(&clock(10), &[row(1, 1, "5"), row(1, 2, "0")]).unwrap();
    assert_eq!(
        applied,
        Applied {
            rows: 2,
            new_holders: 2,
            zero_rows: 1
        }
    );
    assert_eq!(known(&l, &key(1, 1)).as_deref(), Some("5"));
    assert_eq!(known(&l, &key(1, 2)).as_deref(), Some("0"));
    assert_eq!(l.lookup(&key(1, 3)), Lookup::Unknown);
    // Burn to zero and mint from unknown are transitions between distinct states.
    let applied = l.apply(&clock(11), &[row(1, 1, "0"), row(1, 3, "7")]).unwrap();
    assert_eq!(
        applied,
        Applied {
            rows: 2,
            new_holders: 1,
            zero_rows: 1
        }
    );
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
            r.cold_unknown_lookups,
            r.first_block
        ),
        (3, 3, 2, 4, 3, Some(10))
    );
    assert!(r.enumerated.is_empty());
    // An empty reference establishes nothing.
    assert_eq!(l.compare(&[]).status(), "no_reference");
}

#[test]
fn empty_blocks_and_global_only_updates_retain_holders_unchanged() {
    let mut l = balances_ledger(4);
    l.apply(&clock(10), &[row(1, 1, "5")]).unwrap();
    for n in 11..=14 {
        assert_eq!(l.apply(&clock(n), &[]).unwrap(), Applied::default());
    }
    let e = &l.entries()[&key(1, 1)];
    assert_eq!((e.value.as_str(), e.since, e.updated), ("5", 10, 10));
    // A passive reserve update touches no holder entry. The ledger retains
    // basis only; evaluating a holder at a later clock needs the market's
    // global words from the stream.
    let mut s = state_ledger(4);
    s.apply_state(&clock(10), &state_events(10, vec![bound(2, 1, 0, false)], vec![basis(2, 1, "5")]))
        .unwrap();
    let passive = with_globals(state_events(11, vec![], vec![]), vec![global(2, "1000000000000000000000000001")]);
    assert_eq!(s.apply_state(&clock(11), &passive).unwrap(), Applied::default());
    let e = &s.entries()[&key(2, 1)];
    assert_eq!((e.value.as_str(), e.since, e.updated), ("5", 10, 10));
    assert_eq!(s.report().blocks_applied, 2);
    let mut wrong_clock = state_events(12, vec![], vec![]);
    wrong_clock.clocks[0].number = 99;
    assert!(s.apply_state(&clock(12), &wrong_clock).unwrap_err().to_string().contains("block clock"));
}

#[test]
fn checkpoint_and_deployment_origins_are_distinct_and_bounded() {
    let mut l = balances_ledger(2);
    l.seed_checkpoint(key(1, 1), "42", 9, &[9; 32], "rpc-checkpoint bsc-122288005.json#sha256=abc")
        .unwrap();
    assert!(l
        .seed_checkpoint(key(1, 1), "1", 9, &[9; 32], "x")
        .unwrap_err()
        .to_string()
        .contains("already seeded"));
    assert!(l.seed_checkpoint(key(1, 2), "-0", 9, &[9; 32], "x").is_err());
    assert!(l.seed_checkpoint(key(1, 2), "01", 9, &[9; 32], "x").is_err());
    assert!(l.seed_checkpoint(key(1, 2), "1", 9, &[9; 32], "").is_err());
    // A checkpoint must sit at the parent of the first block.
    assert!(l.apply(&clock(11), &[]).unwrap_err().to_string().contains("not the parent"));
    l.apply(&clock(10), &[]).unwrap();
    assert!(l
        .seed_checkpoint(key(1, 2), "1", 9, &[9; 32], "x")
        .unwrap_err()
        .to_string()
        .contains("before the first block"));
    let holders: BTreeSet<Vec<u8>> = [vec![7; 20], vec![8; 20]].into();
    assert_eq!(l.seed_deployment_zero(&[3; 20], &holders, &clock(10)).unwrap(), 2);
    assert!(l
        .seed_deployment_zero(&[3; 20], &holders, &clock(10))
        .unwrap_err()
        .to_string()
        .contains("already seeded"));
    assert!(l
        .seed_deployment_zero(&[4; 20], &holders, &clock(11))
        .unwrap_err()
        .to_string()
        .contains("latest applied"));
    // The checkpointed token existed before block 10.
    assert!(l
        .seed_deployment_zero(&[1; 20], &holders, &clock(10))
        .unwrap_err()
        .to_string()
        .contains("before its creation block"));
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
fn a_constructor_mint_keeps_its_row_and_the_other_holders_start_at_zero() {
    let mut l = balances_ledger(2);
    l.apply(&clock(10), &[row(3, 1, "1000")]).unwrap();
    let holders: BTreeSet<Vec<u8>> = [vec![1; 20], vec![2; 20]].into();
    assert_eq!(l.seed_deployment_zero(&[3; 20], &holders, &clock(10)).unwrap(), 1);
    assert_eq!(l.entries()[&key(3, 1)].origin, Origin::Observed);
    assert_eq!(known(&l, &key(3, 1)).as_deref(), Some("1000"));
    assert!(matches!(l.lookup(&key(3, 2)), Lookup::Known(Entry { origin: Origin::DeploymentZero { block: 10, .. }, value, .. }) if value == "0"));
    l.undo(9).unwrap();
    assert!(l.entries().is_empty());
    // After the creation block is undone the token is new again.
    l.apply(&clock(10), &[]).unwrap();
    assert_eq!(l.seed_deployment_zero(&[3; 20], &holders, &clock(10)).unwrap(), 2);
}

#[test]
fn a_token_with_earlier_state_is_never_new_again_even_after_its_entries_are_dropped() {
    let holders: BTreeSet<Vec<u8>> = [vec![1; 20]].into();
    let mut l = balances_ledger(4);
    l.apply(&clock(10), &[row(3, 1, "5")]).unwrap();
    l.apply(&clock(11), &[]).unwrap();
    let before = l.clone();
    assert!(l.seed_deployment_zero(&[3; 20], &holders, &clock(11)).is_err());
    assert_unchanged(&l, &before);
    // A BOUND without carryover drops the basis, not the market's history.
    let mut s = state_ledger(4);
    s.apply_state(&clock(10), &state_events(10, vec![bound(3, 1, 0, false)], vec![basis(3, 1, "500")]))
        .unwrap();
    s.apply_state(&clock(11), &state_events(11, vec![bound(3, 2, 0, false)], vec![])).unwrap();
    assert_eq!(s.lookup(&key(3, 1)), Lookup::Unknown);
    let before = s.clone();
    assert!(s
        .seed_deployment_zero(&[3; 20], &holders, &clock(11))
        .unwrap_err()
        .to_string()
        .contains("before its creation block"));
    assert_unchanged(&s, &before);
}

#[test]
fn deployment_seed_journaling_preserves_a_row_dropped_earlier_in_the_creation_block() {
    // An epoch-1 row before the epoch-2 activation ordinal is applied and
    // then dropped by the BOUND; the seed must journal the pre-block state.
    let mut l = state_ledger(2);
    let events = state_events(10, vec![bound(1, 2, 50, false)], vec![basis_in(1, 1, 1, "42")]);
    l.apply_state(&clock(10), &events).unwrap();
    assert_eq!(l.lookup(&key(1, 1)), Lookup::Unknown);
    assert_eq!(l.seed_deployment_zero(&[1; 20], &[vec![1; 20], vec![2; 20]].into(), &clock(10)).unwrap(), 2);
    l.undo(9).unwrap();
    assert!(l.entries().is_empty());
    assert_eq!(l.epoch(&[1; 20]), None);
}

#[test]
fn unsupported_contracts_are_visible_not_carried_and_enumeration_needs_evidence() {
    let mut l = balances_ledger(2);
    l.mark_unsupported(&[9; 20], "computed balance; no qualified layout");
    assert!(l.apply(&clock(10), &[row(9, 1, "1")]).unwrap_err().to_string().contains("unsupported"));
    l.apply(&clock(10), &[row(1, 1, "1")]).unwrap();
    assert_eq!(l.lookup(&key(9, 1)), Lookup::Unsupported("computed balance; no qualified layout"));
    let c = l.compare(&[(key(9, 1), "1".into())]);
    assert_eq!((c.unsupported, c.status()), (1, "coverage_gap"));
    let holders: BTreeSet<Vec<u8>> = [vec![1; 20], vec![2; 20]].into();
    assert!(l.attach_enumeration(&[1; 20], &holders, 10, "").is_err());
    // An enumeration describes the latest applied block.
    assert!(l.attach_enumeration(&[1; 20], &holders, 500, "evidence.json").is_err());
    l.attach_enumeration(&[1; 20], &holders, 10, "independent holder enumeration evidence.json")
        .unwrap();
    l.apply(&clock(11), &[row(1, 2, "3")]).unwrap();
    l.attach_enumeration(&[1; 20], &holders, 11, "second enumeration.json").unwrap();
    let r = l.report();
    let listed: Vec<(u64, usize, usize)> = r.enumerated.iter().map(|e| (e.block, e.holders, e.retained_missing)).collect();
    assert_eq!(listed, vec![(10, 2, 1), (11, 2, 0)]);
    // Undoing block 11 discards the enumeration that described it.
    l.undo(10).unwrap();
    let listed: Vec<u64> = l.report().enumerated.iter().map(|e| e.block).collect();
    assert_eq!(listed, vec![10]);
}

#[test]
fn entries_of_unsupported_contracts_are_not_counted_and_cannot_be_seeded() {
    let mut l = balances_ledger(2);
    l.mark_unsupported(&[9; 20], "unqualified");
    assert!(l.seed_checkpoint(key(9, 1), "3", 9, &[9; 32], "qualified checkpoint").is_err());
    l.apply(&clock(10), &[row(1, 1, "0"), row(1, 2, "4")]).unwrap();
    assert!(l
        .seed_deployment_zero(&[9; 20], &[vec![1; 20]].into(), &clock(10))
        .unwrap_err()
        .to_string()
        .contains("unsupported"));
    l.mark_unsupported(&[1; 20], "layout withdrawn");
    let r = l.report();
    assert_eq!(
        (r.initialized_holders, r.observed_holders, r.known_zero_holders, r.unsupported_entries),
        (0, 0, 0, 2)
    );
    assert_eq!(l.lookup(&key(1, 2)), Lookup::Unsupported("layout withdrawn"));
}

#[test]
fn gaps_and_forks_are_rejected_and_undo_restores_exact_prior_state() {
    let mut l = balances_ledger(2);
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
fn undoing_every_applied_block_clears_the_first_block() {
    let mut l = balances_ledger(2);
    l.apply(&clock(10), &[row(1, 1, "1")]).unwrap();
    l.undo(9).unwrap();
    let r = l.report();
    assert_eq!((r.first_block, r.last_block, r.blocks_applied), (None, Some(9), 0));
    l.apply(&clock(10), &[]).unwrap();
    assert_eq!(l.report().first_block, Some(10));
}

#[test]
fn migrations_suspend_and_rebinding_decides_carryover() {
    let mut l = state_ledger(4);
    let k = key(2, 1);
    l.apply_state(
        &clock(10),
        &state_events(10, vec![bound(2, 1, 0, true)], vec![principal(2, 1, "-5"), basis(2, 2, "9")]),
    )
    .unwrap();
    // An unsigned basis cannot be negative.
    assert!(l.apply_state(&clock(11), &state_events(11, vec![], vec![basis(2, 3, "-1")])).is_err());
    assert_eq!(known(&l, &k).as_deref(), Some("-5"));
    l.apply_state(&clock(11), &state_events(11, vec![invalidated(2, 1, 5)], vec![basis(2, 2, "10")]))
        .unwrap();
    let suspended = Lookup::Suspended {
        epoch: 1,
        reason: state::InvalidationReason::RateModelChange as i32,
    };
    assert_eq!(l.lookup(&k), suspended);
    assert_eq!(l.compare(&[(k.clone(), "-5".into())]).suspended, 1);
    // Re-binding with carryover keeps the basis; without it, holders are unknown again.
    l.apply_state(&clock(12), &state_events(12, vec![bound(2, 2, 0, true)], vec![])).unwrap();
    assert_eq!(known(&l, &k).as_deref(), Some("-5"));
    l.apply_state(&clock(13), &state_events(13, vec![bound(2, 3, 0, false)], vec![])).unwrap();
    assert_eq!(l.lookup(&k), Lookup::Unknown);
    assert_eq!((l.report().suspended_markets, l.epoch(&[2; 20])), (0, Some(3)));
    // Undo through the migration restores the suspension, the epoch and the entries.
    l.undo(11).unwrap();
    assert_eq!(l.lookup(&k), suspended);
    assert_eq!(l.epoch(&[2; 20]), Some(1));
    l.undo(10).unwrap();
    assert_eq!(known(&l, &k).as_deref(), Some("-5"));
    assert_eq!(known(&l, &key(2, 2)).as_deref(), Some("9"));
}

#[test]
fn suspended_suspends_reaffirmed_does_not_resume_and_only_a_newer_bound_does() {
    let mut l = state_ledger(4);
    l.apply_state(&clock(10), &state_events(10, vec![bound(2, 1, 0, true)], vec![basis(2, 1, "7")]))
        .unwrap();
    let suspend = epoch_row(2, 1, state::EpochEventKind::Suspended, 3, true);
    l.apply_state(&clock(11), &state_events(11, vec![suspend], vec![])).unwrap();
    assert!(matches!(l.lookup(&key(2, 1)), Lookup::Suspended { epoch: 1, .. }));
    let reaffirm = epoch_row(2, 1, state::EpochEventKind::Reaffirmed, 0, true);
    l.apply_state(&clock(12), &state_events(12, vec![reaffirm], vec![])).unwrap();
    assert!(matches!(l.lookup(&key(2, 1)), Lookup::Suspended { epoch: 1, .. }));
    // BOUND must advance the epoch.
    let before = l.clone();
    assert!(l
        .apply_state(&clock(13), &state_events(13, vec![bound(2, 1, 0, true)], vec![]))
        .unwrap_err()
        .to_string()
        .contains("does not advance"));
    assert_unchanged(&l, &before);
    l.apply_state(&clock(13), &state_events(13, vec![bound(2, 2, 0, true)], vec![])).unwrap();
    assert_eq!(known(&l, &key(2, 1)).as_deref(), Some("7"));
    // A market declared SUSPENDED before it is bound may bind the declared epoch.
    let unqualified = epoch_row(4, 1, state::EpochEventKind::Suspended, 0, false);
    l.apply_state(&clock(14), &state_events(14, vec![unqualified], vec![])).unwrap();
    assert!(matches!(l.lookup(&key(4, 1)), Lookup::Suspended { epoch: 1, .. }));
    l.apply_state(&clock(15), &state_events(15, vec![bound(4, 1, 0, false)], vec![basis(4, 1, "1")]))
        .unwrap();
    assert_eq!(known(&l, &key(4, 1)).as_deref(), Some("1"));
}

#[test]
fn epoch_rows_and_holder_rows_must_follow_the_market_epoch_sequence() {
    let mut l = state_ledger(4);
    l.apply_state(&clock(10), &state_events(10, vec![bound(2, 1, 0, false)], vec![])).unwrap();
    l.apply_state(&clock(11), &state_events(11, vec![bound(2, 2, 0, false)], vec![])).unwrap();
    let before = l.clone();
    let refused = [
        // An older epoch cannot suspend the epoch in force, nor be bound again.
        state_events(12, vec![invalidated(2, 1, 0)], vec![]),
        state_events(12, vec![bound(2, 1, 0, true)], vec![]),
        // Rows name the epoch in force, not a stale or future one.
        state_events(12, vec![], vec![basis_in(1, 2, 1, "5")]),
        state_events(12, vec![], vec![basis_in(3, 2, 1, "5")]),
        // Epoch 0 does not exist.
        state_events(12, vec![], vec![basis_in(0, 5, 1, "5")]),
        // Without an epoch row, rows of one block agree on the epoch.
        state_events(12, vec![], vec![basis_in(1, 5, 1, "5"), basis_in(2, 5, 2, "5")]),
    ];
    for events in refused {
        assert!(l.apply_state(&clock(12), &events).is_err(), "{events:?}");
        assert_unchanged(&l, &before);
    }
    l.apply_state(&clock(12), &state_events(12, vec![], vec![basis_in(2, 2, 1, "5")])).unwrap();
    // A market seen first through its rows adopts their epoch.
    l.apply_state(&clock(13), &state_events(13, vec![], vec![basis_in(4, 6, 1, "8")])).unwrap();
    assert_eq!(l.epoch(&[6; 20]), Some(4));
}

#[test]
fn an_activation_block_applies_the_previous_epochs_rows_before_the_new_binding_in_any_input_order() {
    for carryover in [false, true] {
        for reversed in [false, true] {
            let mut l = state_ledger(4);
            l.apply_state(
                &clock(10),
                &state_events(10, vec![bound(2, 1, 0, true)], vec![basis(2, 1, "5"), basis(2, 2, "6")]),
            )
            .unwrap();
            // Epoch 1 is invalidated at ordinal 100, epoch 2 is bound at 500;
            // holder 1 was written before 500 (epoch 1) and holder 2 on both
            // sides, so it has one row per epoch.
            let mut epochs = vec![invalidated(2, 1, 100), bound(2, 2, 500, carryover)];
            let mut rows = vec![basis_in(1, 2, 1, "77"), basis_in(1, 2, 2, "60"), basis_in(2, 2, 2, "61")];
            if reversed {
                epochs.reverse();
                rows.reverse();
            }
            l.apply_state(&clock(11), &state_events(11, epochs, rows)).unwrap();
            assert_eq!((l.epoch(&[2; 20]), l.report().suspended_markets), (Some(2), 0));
            let holder_1 = known(&l, &key(2, 1));
            assert_eq!(holder_1.as_deref(), if carryover { Some("77") } else { None }, "carryover {carryover}");
            assert_eq!(known(&l, &key(2, 2)).as_deref(), Some("61"));
            l.undo(10).unwrap();
            assert_eq!(
                (known(&l, &key(2, 1)).as_deref(), known(&l, &key(2, 2)).as_deref(), l.epoch(&[2; 20])),
                (Some("5"), Some("6"), Some(1))
            );
        }
    }
}

#[test]
fn a_block_is_refused_when_its_clock_counts_differ_or_its_stream_changes() {
    let mut l = state_ledger(4);
    l.apply_state(&clock(10), &state_events(10, vec![bound(2, 1, 0, false)], vec![basis(2, 1, "5")]))
        .unwrap();
    let before = l.clone();
    type Mutation = fn(&mut state::BlockClock);
    let mutations: [(&str, Mutation); 9] = [
        ("partial block", |c| c.epoch_count = 2),
        ("partial block", |c| c.holder_basis_count = 0),
        ("partial block", |c| c.global_state_count = 1),
        ("partial block", |c| c.dependency_count = 1),
        ("partial block", |c| c.alias_count = 1),
        ("different stream", |c| c.parameters_sha256 = "cd".repeat(32)),
        ("different stream", |c| c.package = "other_balance_state".into()),
        ("different stream", |c| c.package_version = "v0.2.0".into()),
        ("chain and package", |c| c.chain_id = 0),
    ];
    for (message, mutate) in mutations {
        let mut events = state_events(11, vec![invalidated(2, 1, 0)], vec![basis(2, 1, "4")]);
        mutate(&mut events.clocks[0]);
        let error = l.apply_state(&clock(11), &events).unwrap_err().to_string();
        assert!(error.contains(message), "expected `{message}`, got `{error}`");
        assert_unchanged(&l, &before);
    }
    // Rows of another chain are refused too.
    let mut events = state_events(11, vec![], vec![basis(2, 1, "4")]);
    events.holder_basis[0].chain_id = 1;
    assert!(l.apply_state(&clock(11), &events).is_err());
    // Undoing the first block releases the stream identity.
    l.undo(9).unwrap();
    assert!(l.report().stream.is_none());
    let mut events = state_events(10, vec![], vec![]);
    events.clocks[0].parameters_sha256 = "cd".repeat(32);
    l.apply_state(&clock(10), &events).unwrap();
}

#[test]
fn row_enums_signs_values_and_global_rows_are_validated_before_mutation() {
    let mut l = state_ledger(4);
    l.mark_unsupported(&[9; 20], "unqualified");
    l.apply_state(&clock(10), &state_events(10, vec![bound(2, 1, 0, false)], vec![])).unwrap();
    let before = l.clone();
    let mut bad_rows = Vec::new();
    for mutate in [
        |b: &mut state::HolderBasis| b.boundary = 999,
        |b: &mut state::HolderBasis| b.boundary = state::Boundary::Change as i32,
        |b: &mut state::HolderBasis| b.basis_kind = 0,
        |b: &mut state::HolderBasis| b.observation = 0,
        |b: &mut state::HolderBasis| b.scope = 77,
        // A signed flag that does not match the basis kind.
        |b: &mut state::HolderBasis| {
            b.signed = true;
            b.value = "-1".into()
        },
    ] {
        let mut b = basis(2, 1, "4");
        mutate(&mut b);
        bad_rows.push(state_events(11, vec![], vec![b]));
    }
    let mut unsigned_principal = principal(2, 1, "4");
    unsigned_principal.signed = false;
    bad_rows.push(state_events(11, vec![], vec![unsigned_principal]));
    for g in [global(9, "1"), global(2, "not-a-number"), global(2, "-1"), global(2, "01")] {
        bad_rows.push(with_globals(state_events(11, vec![], vec![]), vec![g]));
    }
    let mut unknown_field = global(2, "1");
    unknown_field.field = 9999;
    bad_rows.push(with_globals(state_events(11, vec![], vec![]), vec![unknown_field]));
    for events in bad_rows {
        assert!(l.apply_state(&clock(11), &events).is_err(), "{events:?}");
        assert_unchanged(&l, &before);
    }
    l.apply_state(
        &clock(11),
        &with_globals(state_events(11, vec![], vec![principal(2, 1, "-4")]), vec![global(2, "1")]),
    )
    .unwrap();
    assert_eq!(known(&l, &key(2, 1)).as_deref(), Some("-4"));
}

#[test]
fn values_are_exact_uint256_or_int256_decimals() {
    let max = UINT256_MAX;
    let above = "115792089237316195423570985008687907853269984665640564039457584007913129639936";
    assert!(valid_decimal(max, false) && !valid_decimal(above, false));
    assert!(valid_decimal("0", false) && !valid_decimal("", false) && !valid_decimal("-5", false));
    for bad in ["-0", "+1", " 1", "1 ", "1e3", "007", "-", "١"] {
        assert!(!valid_decimal(bad, true), "{bad}");
    }
    let min = format!("-{INT256_MIN_MAGNITUDE}");
    let below = "-57896044618658097711785492504343953926634992332820282019728792003956564819969";
    assert!(valid_decimal(&min, true) && !valid_decimal(below, true));
    assert!(valid_decimal(INT256_MAX, true) && !valid_decimal(INT256_MIN_MAGNITUDE, true));
    assert!(!valid_decimal(&"9".repeat(100), false));
}

#[test]
fn one_ledger_retains_one_output_and_checkpoint_signs_follow_it() {
    let mut balances = balances_ledger(2);
    assert!(balances.seed_checkpoint(key(1, 1), "-5", 9, &[9; 32], "x").is_err());
    let native = Key {
        contract: None,
        address: vec![1; 20],
    };
    assert!(balances.seed_checkpoint(native.clone(), "-7", 9, &[9; 32], "x").is_err());
    balances.seed_checkpoint(native.clone(), "7", 9, &[9; 32], "x").unwrap();
    assert!(balances
        .apply_state(&clock(10), &state_events(10, vec![], vec![]))
        .unwrap_err()
        .to_string()
        .contains("one ledger per output"));
    let mut basis_ledger = state_ledger(2);
    assert!(basis_ledger.seed_checkpoint(native, "7", 9, &[9; 32], "x").is_err());
    basis_ledger
        .seed_checkpoint(key(2, 1), "-5", 9, &[9; 32], "comet principal checkpoint")
        .unwrap();
    assert!(basis_ledger.apply(&clock(10), &[]).unwrap_err().to_string().contains("one ledger per output"));
    assert_eq!(basis_ledger.report().domain, Domain::BalanceState);
}

#[test]
fn refused_balance_blocks_leave_every_retained_field_unchanged_and_can_be_retried() {
    let mut base = balances_ledger(4);
    base.apply(&clock(10), &[row(1, 1, "10")]).unwrap();
    base.mark_unsupported(&[9; 20], "unqualified");
    for bad in [row(1, 2, "1e3"), row(1, 1, "30"), row(9, 1, "1")] {
        let mut l = base.clone();
        assert!(l.apply(&clock(11), &[row(1, 1, "20"), row(1, 3, "0"), bad]).is_err());
        assert_unchanged(&l, &base);
        assert_eq!(l.apply(&clock(11), &[]).unwrap(), Applied::default());
        l.undo(10).unwrap();
        assert_eq!(l.entries(), base.entries());
        assert_eq!(l.last(), base.last());
        assert_eq!(known(&l, &key(1, 1)).as_deref(), Some("10"));
        assert_eq!(l.lookup(&key(1, 3)), Lookup::Unknown);
    }
}

#[test]
fn refused_state_blocks_restore_suspensions_dropped_basis_and_all_bookkeeping() {
    let mut base = state_ledger(4);
    base.apply_state(
        &clock(10),
        &state_events(
            10,
            vec![bound(1, 1, 0, false), bound(2, 1, 0, false)],
            vec![basis(1, 1, "10"), basis(2, 1, "0")],
        ),
    )
    .unwrap();
    base.mark_unsupported(&[9; 20], "unqualified");
    let suspend = epoch_row(2, 1, state::EpochEventKind::Suspended, 1, false);
    base.apply_state(&clock(11), &state_events(11, vec![suspend], vec![])).unwrap();
    for bad in [basis(1, 2, "-1"), basis(1, 1, "30"), basis(9, 1, "1")] {
        let mut l = base.clone();
        let epochs = vec![invalidated(1, 1, 1), bound(2, 2, 2, false)];
        assert!(l
            .apply_state(&clock(12), &state_events(12, epochs, vec![basis(1, 1, "20"), basis(3, 1, "0"), bad]))
            .is_err());
        assert_unchanged(&l, &base);
        l.apply_state(&clock(12), &state_events(12, vec![], vec![])).unwrap();
        l.undo(11).unwrap();
        assert_eq!(l.entries(), base.entries());
        assert_eq!(l.suspended, base.suspended);
        assert_eq!(l.epochs, base.epochs);
        assert_eq!(l.last(), base.last());
    }
    for bad_kind in [0, 999] {
        let mut l = base.clone();
        let mut bad = bound(3, 1, 3, false);
        bad.kind = bad_kind;
        let epochs = vec![invalidated(1, 1, 1), bound(2, 2, 2, false), bad];
        assert!(l.apply_state(&clock(12), &state_events(12, epochs, vec![])).is_err());
        assert_unchanged(&l, &base);
    }
}

#[test]
fn checkpoint_identity_is_explicit_consistent_and_binds_both_row_apis() {
    for domain in [Domain::Balances, Domain::BalanceState] {
        let mut l = Ledger::new(4, domain);
        l.seed_checkpoint(key(1, 1), "0", 9, &[9; 32], "qualified checkpoint").unwrap();
        let before = l.clone();
        for (number, hash) in [(9, vec![9; 31]), (9, vec![8; 32]), (8, vec![9; 32])] {
            assert!(l.seed_checkpoint(key(1, 2), "42", number, &hash, "qualified checkpoint").is_err());
            assert_unchanged(&l, &before);
        }
        let mut fork = clock(10);
        fork.parent_hash = vec![0xff; 32];
        let mut fork_events = state_events(10, vec![], vec![]);
        fork_events.clocks[0].parent_hash = fork.parent_hash.clone();
        let apply = |l: &mut Ledger, clock: &Clock, events: &state::Events| match domain {
            Domain::Balances => l.apply(clock, &[]).map(|_| ()),
            Domain::BalanceState => l.apply_state(clock, events).map(|_| ()),
        };
        assert!(apply(&mut l, &fork, &fork_events).is_err());
        assert_unchanged(&l, &before);
        apply(&mut l, &clock(10), &state_events(10, vec![], vec![])).unwrap();
        assert!(matches!(l.lookup(&key(1, 1)), Lookup::Known(Entry { origin: Origin::Checkpoint { hash, .. }, .. }) if hash == &[9; 32]));
        l.undo(9).unwrap();
        assert!(apply(&mut l, &fork, &fork_events).is_err());
        apply(&mut l, &clock(10), &state_events(10, vec![], vec![])).unwrap();
    }
}

#[test]
fn state_clock_requires_exact_number_hash_and_parent_and_one_clock() {
    let mut l = state_ledger(4);
    l.apply_state(&clock(10), &state_events(10, vec![bound(1, 1, 0, false)], vec![basis(1, 1, "10")]))
        .unwrap();
    let before = l.clone();
    for mutation in 0..5 {
        let mut events = state_events(11, vec![invalidated(1, 1, 1)], vec![basis(1, 1, "20")]);
        match mutation {
            0 => events.clocks[0].number = 12,
            1 => events.clocks[0].hash = vec![0xff; 32],
            2 => events.clocks[0].parent_hash = vec![0xff; 32],
            3 => events.clocks.clear(),
            4 => events.clocks.push(events.clocks[0].clone()),
            _ => unreachable!(),
        }
        assert!(l.apply_state(&clock(11), &events).is_err());
        assert_unchanged(&l, &before);
    }
}

#[test]
fn deployment_seeds_require_exact_applied_creation_and_disappear_when_creation_is_undone() {
    let mut l = balances_ledger(4);
    let holders: BTreeSet<Vec<u8>> = [vec![1; 20], vec![2; 20]].into();
    let before = l.clone();
    assert!(l.seed_deployment_zero(&[1; 20], &holders, &clock(10)).is_err());
    assert_unchanged(&l, &before);
    l.apply(&clock(10), &[]).unwrap();
    let before = l.clone();
    let mut fork_hash = clock(10);
    fork_hash.hash = vec![0xff; 32];
    let mut fork_parent = clock(10);
    fork_parent.parent_hash = vec![0xff; 32];
    for bad in [clock(9), clock(11), fork_hash, fork_parent] {
        assert!(l.seed_deployment_zero(&[1; 20], &holders, &bad).is_err());
        assert_unchanged(&l, &before);
    }
    l.seed_deployment_zero(&[1; 20], &holders, &clock(10)).unwrap();
    assert!(matches!(l.lookup(&key(1, 1)), Lookup::Known(Entry { origin: Origin::DeploymentZero { block: 10, hash }, .. }) if hash == &[10; 32]));
    l.apply(&clock(11), &[row(1, 1, "10")]).unwrap();
    assert!(l.seed_deployment_zero(&[2; 20], &holders, &clock(10)).is_err());
    l.undo(10).unwrap();
    assert_eq!(known(&l, &key(1, 1)).as_deref(), Some("0"));
    assert_eq!(l.report().deployment_seeded_holders, 2);
    l.undo(9).unwrap();
    assert_eq!(l.lookup(&key(1, 1)), Lookup::Unknown);
    assert_eq!(l.lookup(&key(1, 2)), Lookup::Unknown);
    assert_eq!(l.report().initialized_holders, 0);
    assert_eq!(l.report().emitted_rows, 0);
    let mut replacement = clock(10);
    replacement.hash = vec![0xaa; 32];
    l.apply(&replacement, &[]).unwrap();
    assert!(l.seed_deployment_zero(&[1; 20], &holders, &clock(10)).is_err());
    l.seed_deployment_zero(&[1; 20], &holders, &replacement).unwrap();
    l.undo(9).unwrap();
    assert!(l.entries().is_empty());
}

#[test]
fn deployment_seeding_requires_undo_capacity_and_clock_successor_never_wraps() {
    let mut l = balances_ledger(0);
    l.apply(&clock(10), &[]).unwrap();
    let before = l.clone();
    assert!(l.seed_deployment_zero(&[1; 20], &[vec![1; 20]].into(), &clock(10)).is_err());
    assert_unchanged(&l, &before);
    let mut l = balances_ledger(2);
    l.seed_checkpoint(key(1, 1), "0", u64::MAX, &[9; 32], "impossible successor").unwrap();
    let before = l.clone();
    assert!(l.apply(&clock(10), &[]).is_err());
    assert_unchanged(&l, &before);
    let mut l = balances_ledger(2);
    l.apply(&clock(u64::MAX), &[]).unwrap();
    let before = l.clone();
    assert!(l.apply(&clock(10), &[]).is_err());
    assert_unchanged(&l, &before);
}

#[test]
fn undo_preserves_full_prior_clock_even_after_it_leaves_the_bounded_journal() {
    let mut l = balances_ledger(1);
    l.apply(&clock(10), &[]).unwrap();
    l.apply(&clock(11), &[]).unwrap();
    l.undo(10).unwrap();
    assert_eq!(l.last(), Some(&clock(10)));
}

#[test]
fn final_snapshot_comparison_reports_mismatches_separately_from_gaps() {
    let mut l = balances_ledger(1);
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

#[test]
fn genesis_undo_never_invents_a_pre_genesis_clock() {
    let genesis = Clock {
        number: 0,
        hash: vec![0; 32],
        parent_hash: vec![0xff; 32],
    };
    let mut l = balances_ledger(2);
    l.apply(&genesis, &[row(1, 1, "0")]).unwrap();
    let before = l.clone();
    assert!(l.undo(0).is_err());
    assert_unchanged(&l, &before);
    l.apply(&clock(1), &[row(1, 1, "42")]).unwrap();
    l.undo(0).unwrap();
    assert_eq!(l.last(), Some(&genesis));
    assert_eq!(l.entries(), before.entries());
}
