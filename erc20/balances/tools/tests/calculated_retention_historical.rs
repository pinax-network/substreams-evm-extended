#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::calculated_retention::{
    binding::{self, Model},
    historical::{self, LAST_NUMBER, ORIGINALS, PARENT_HASH, PARENT_NUMBER, PARENT_TIMESTAMP},
    mapping, value, Ledger, Limits, Origin, Outcome, Pending, Slot,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[test]
fn original_bytes_and_crosslinks_are_closed_before_parsing() {
    assert_eq!(ORIGINALS.len(), 9);
    for original in &ORIGINALS {
        assert_eq!(binding::sha(original.raw), original.sha256);
        assert_eq!(
            historical::verify_original(original.label, original.raw).unwrap(),
            historical::original(original.label).unwrap()
        );
        let mut changed = original.raw.to_vec();
        changed.push(b' ');
        assert!(
            historical::verify_original(original.label, &changed).is_err(),
            "even serialization-only changes are outside the original pin"
        );
    }
    assert!(historical::original("unbound").is_err());
    assert!(historical::verify_original("lbp-checks", ORIGINALS[0].raw).is_err());
    let mut manifest = historical::original("lbp-manifest").unwrap();
    manifest["checkpoint_sha256"] = json!("00".repeat(32));
    assert!(historical::verify_original("lbp-manifest", &serde_json::to_vec(&manifest).unwrap()).is_err());
    // A duplicate JSON key must be rejected before Value can silently collapse it.
    let original = ORIGINALS.iter().find(|v| v.label == "lbp-manifest").unwrap();
    let mut duplicate = String::from_utf8(original.raw.to_vec()).unwrap();
    let start = duplicate.find('{').unwrap() + 1;
    duplicate.insert_str(start, "\"chain_id\":1,");
    assert!(historical::verify_original(original.label, duplicate.as_bytes()).is_err());
}

#[test]
fn checkpoints_preserve_finite_raw_units_and_absent_parent_metadata() {
    let checkpoints = historical::checkpoints().unwrap();
    assert_eq!(checkpoints.len(), 3);
    for (checkpoint, (model, holders, words)) in checkpoints
        .iter()
        .zip([(Model::Lbp, 33, 171), (Model::BabyDoge, 19, 102), (Model::TenSet, 12, 43)])
    {
        assert_eq!(checkpoint.binding.model, model);
        assert_eq!(checkpoint.binding.epoch, 1);
        assert_eq!(checkpoint.binding.activation_block, PARENT_NUMBER);
        assert_eq!(checkpoint.binding.chain_id, 56);
        assert_eq!(checkpoint.holders.len(), holders);
        assert_eq!(checkpoint.words.len(), words);
        assert_eq!(checkpoint.at.number, PARENT_NUMBER);
        assert_eq!(format!("0x{}", hex::encode(checkpoint.at.hash)), PARENT_HASH);
        assert_eq!(checkpoint.at.timestamp, PARENT_TIMESTAMP);
        assert_eq!(checkpoint.at.parent_hash, None);
        assert_eq!(checkpoint.at.producer_version, None);
        assert_eq!(checkpoint.at.chain_clock(), None);
        assert!(checkpoint.holders.windows(2).all(|p| p[0] < p[1]));
        assert!(checkpoint.words.windows(2).all(|p| p[0].slot < p[1].slot));
        assert!(binding::is_sha(&checkpoint.evidence));
        for fact in &checkpoint.words {
            assert_eq!(fact.at, checkpoint.at);
            assert_eq!(fact.epoch, checkpoint.binding.epoch);
            assert_eq!(
                fact.origin,
                Origin::Checkpoint {
                    evidence: checkpoint.evidence.clone()
                }
            );
        }
        Ledger::from_checkpoint(checkpoint.clone(), Limits::default()).unwrap();
    }
    assert_eq!(checkpoints.iter().map(|cp| cp.holders.len()).sum::<usize>(), 64);
    assert_eq!(checkpoints.iter().map(|cp| cp.words.len()).sum::<usize>(), 316);
    assert_eq!(checkpoints, historical::checkpoints().unwrap());
}

#[test]
fn initial_calculations_match_all_64_independent_getters() {
    let observations = historical::observations().unwrap();
    let mut checked = 0;
    for cp in historical::checkpoints().unwrap() {
        let model = cp.binding.model;
        let ledger = Ledger::from_checkpoint(cp, Limits::default()).unwrap();
        for observation in observations.iter().filter(|o| o.model == model && o.number == PARENT_NUMBER) {
            let evaluation = ledger.evaluate(observation.holder).unwrap();
            assert_eq!(evaluation.at.hash, observation.hash);
            match evaluation.outcome {
                Outcome::Known { amount, pending } => {
                    assert_eq!(amount, value(&observation.amount).to_string());
                    assert_eq!(pending, observation.pending.map(|v| Pending::Known(value(&v).to_string())));
                }
                other => panic!("initial historical input must be complete: {other:?}"),
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 64);
}

#[test]
fn checkpoint_words_equal_storage_captures_and_never_getter_amounts() {
    let observations = historical::observations().unwrap();
    for cp in historical::checkpoints().unwrap() {
        let raw = historical::original(match cp.binding.model {
            Model::Lbp => "lbp-checkpoint",
            Model::BabyDoge => "babydoge",
            Model::TenSet => "tenset",
        })
        .unwrap();
        for fact in &cp.words {
            let key = format!("0x{}", hex::encode(fact.slot.key));
            let expected = if cp.binding.model == Model::Lbp {
                let contract = format!("0x{}", hex::encode(fact.slot.contract));
                let rows: Vec<_> = raw
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|r| r["contract"] == contract && r["key"] == key)
                    .collect();
                assert_eq!(rows.len(), 1);
                erc20_balances_tools::data::uint(&rows[0]["word"]).unwrap()
            } else {
                let text = raw["snapshots"][0]["storage"][&key].as_str().unwrap();
                primitive_types::U256::from_big_endian(&hex::decode(text.strip_prefix("0x").unwrap()).unwrap())
            };
            assert_eq!(value(&fact.word), expected);
        }
        let holder = cp.holders[0];
        assert!(observations
            .iter()
            .any(|o| o.model == cp.binding.model && o.number == PARENT_NUMBER && o.holder == holder));
        let root = cp.binding.reflection_layout().map_or(0, |l| l.reflections);
        let slot = Slot {
            contract: cp.binding.token(),
            key: mapping(holder, root),
        };
        let mut incomplete = cp.clone();
        incomplete.words.retain(|f| f.slot != slot);
        assert_eq!(incomplete.words.len(), cp.words.len() - 1);
        let ledger = Ledger::from_checkpoint(incomplete, Limits::default()).unwrap();
        assert!(
            matches!(ledger.evaluate(holder).unwrap().outcome, Outcome::Unknown { missing } if missing.contains(&slot)),
            "saved RPC expectation must never repair missing raw input"
        );
    }
}

#[test]
fn independent_expectation_counts_separate_parent_and_subsequent_checks() {
    let observations = historical::observations().unwrap();
    assert_eq!(observations.len(), 923);
    for (model, total, parent, holders) in [(Model::Lbp, 594, 33, 33), (Model::BabyDoge, 209, 19, 19), (Model::TenSet, 120, 12, 12)] {
        let selected: Vec<_> = observations.iter().filter(|o| o.model == model).collect();
        assert_eq!(selected.len(), total);
        assert_eq!(selected.iter().filter(|o| o.number == PARENT_NUMBER).count(), parent);
        assert_eq!(selected.iter().map(|o| o.holder).collect::<BTreeSet<_>>().len(), holders);
        assert!(selected.iter().all(|o| (PARENT_NUMBER..=LAST_NUMBER).contains(&o.number)));
        assert_eq!(selected.iter().map(|o| (o.number, o.holder)).collect::<BTreeSet<_>>().len(), total);
    }
    assert_eq!(observations.iter().filter(|o| o.model != Model::Lbp && o.number > PARENT_NUMBER).count(), 298);
    let controls = historical::original("reflection-controls").unwrap();
    assert_eq!(
        controls["tokens"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["cases"].as_array().unwrap().len())
            .sum::<usize>(),
        48
    );
    let passive = historical::original("tenset-passive").unwrap();
    assert_eq!(passive["cases"].as_array().unwrap().len(), 12);
    assert_eq!(passive["independent_rpc_checks"].as_array().unwrap().len(), 24);
}

#[test]
fn historical_checkpoint_alias_duplicates_and_wrong_identity_refuse() {
    for cp in historical::checkpoints().unwrap() {
        let mut duplicate = cp.clone();
        duplicate.words.push(cp.words[0].clone());
        assert!(Ledger::from_checkpoint(duplicate, Limits::default()).is_err());
        let mut mixed_hash = cp.clone();
        mixed_hash.words[0].at.hash[0] ^= 1;
        assert!(Ledger::from_checkpoint(mixed_hash, Limits::default()).is_err());
        let mut mixed_block = cp.clone();
        mixed_block.words[0].at.number += 1;
        assert!(Ledger::from_checkpoint(mixed_block, Limits::default()).is_err());
        let mut mixed_origin = cp.clone();
        mixed_origin.words[0].origin = Origin::Checkpoint { evidence: "unrelated".into() };
        assert!(Ledger::from_checkpoint(mixed_origin, Limits::default()).is_err());
        let mut encoded: Value = serde_json::to_value(&cp).unwrap();
        let first = encoded["words"][0].clone();
        encoded["words"].as_array_mut().unwrap().push(first);
        let decoded = serde_json::from_value(encoded).unwrap();
        assert!(
            Ledger::from_checkpoint(decoded, Limits::default()).is_err(),
            "record-list serialization must preserve duplicates for rejection"
        );
    }
}
