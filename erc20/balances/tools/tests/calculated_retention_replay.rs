#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::calculated_retention::{binding, historical, journal, *};
use std::sync::OnceLock;
fn inputs() -> &'static [BlockInput] {
    static I: OnceLock<Vec<BlockInput>> = OnceLock::new();
    I.get_or_init(|| journal::verify(journal::BYTES).unwrap())
}
#[test]
fn all_original_getters_match_new_finite_journal_with_exact_resume_and_undo() {
    let mut ledgers = historical::checkpoints()
        .unwrap()
        .into_iter()
        .map(|c| Ledger::from_checkpoint(c, Limits::default()).unwrap())
        .collect::<Vec<_>>();
    let expected = historical::observations().unwrap();
    let mut checked = 0;
    let check = |ledgers: &[Ledger], checked: &mut usize| {
        for o in expected.iter().filter(|o| o.number == ledgers[0].at().number) {
            let l = ledgers.iter().find(|l| l.binding().model == o.model).unwrap();
            let actual = l.evaluate(o.holder).unwrap();
            assert_eq!(actual.at.hash, o.hash);
            match actual.outcome {
                Outcome::Known { amount, pending } => {
                    assert_eq!(amount, value(&o.amount).to_string());
                    if let Some(p) = o.pending {
                        assert_eq!(pending, Some(Pending::Known(value(&p).to_string())));
                    }
                }
                other => panic!("historical refusal: {other:?}"),
            }
            *checked += 1;
        }
    };
    check(&ledgers, &mut checked);
    assert_eq!(checked, 64);
    let mut resume: Option<Vec<Ledger>> = None;
    for (i, b) in inputs().iter().enumerate() {
        let actual = apply_all(&mut ledgers, b).unwrap();
        assert_eq!(actual.iter().map(|r| r.evaluations.len()).sum::<usize>(), 64);
        if let Some(shadow) = &mut resume {
            assert_eq!(actual, apply_all(shadow, b).unwrap());
        }
        check(&ledgers, &mut checked);
        if i == 511 {
            resume = Some(
                ledgers
                    .iter()
                    .map(|l| {
                        let raw = l.snapshot().unwrap();
                        Ledger::restore(&raw, &binding::sha(&raw), l.binding()).unwrap()
                    })
                    .collect::<Vec<_>>(),
            );
        }
    }
    assert_eq!(checked, 923);
    assert_eq!(ledgers.iter().map(|l| l.counters().evaluations).sum::<u64>(), 65536);
    for (l, r) in ledgers.iter().zip(resume.unwrap()) {
        assert_eq!(l.snapshot().unwrap(), r.snapshot().unwrap());
    }
    let final_state = ledgers.clone();
    let boundary = &inputs()[1007];
    for l in &mut ledgers {
        assert_eq!(l.undo(boundary.at.number, boundary.at.hash).unwrap(), 16);
    }
    for b in &inputs()[1008..] {
        apply_all(&mut ledgers, b).unwrap();
    }
    assert_eq!(ledgers, final_state);
}
#[test]
fn journal_replacement_and_late_applied_fork_do_not_repair_state() {
    let mut raw = journal::BYTES.to_vec();
    raw.push(b' ');
    assert!(journal::verify(&raw).is_err());
    let mut changed = inputs().to_vec();
    changed[1].source_sha256 = "00".repeat(32);
    let raw = changed.iter().map(|b| serde_json::to_string(b).unwrap() + "\n").collect::<String>();
    assert!(journal::verify(raw.as_bytes()).is_err());
    let mut ledgers = historical::checkpoints()
        .unwrap()
        .into_iter()
        .map(|c| Ledger::from_checkpoint(c, Limits::default()).unwrap())
        .collect::<Vec<_>>();
    apply_all(&mut ledgers, &inputs()[0]).unwrap();
    let before = ledgers.clone();
    let mut bad = inputs()[1].clone();
    bad.at.parent_hash = Some([0; 32]);
    assert!(apply_all(&mut ledgers, &bad).is_err());
    assert_eq!(ledgers, before);
}
