#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::{
    og_model::fixture,
    og_retention::{
        address, binding, decode,
        historical::{self, CONTROL_SCOPE, HISTORICAL_WORDS},
        value, Checkpoint, Ledger, Limits, Metric, Origin,
    },
};
use primitive_types::U256;
use serde_json::{json, Value};
use std::{collections::BTreeSet, fmt::Debug};

#[derive(Default, Debug, PartialEq, Eq)]
struct Comparisons {
    values: usize,
    panic11: usize,
    recursive: usize,
}

fn check_metric<T: Debug>(actual: &Metric<T>, expected: &Value, values: impl FnOnce(&T) -> Vec<String>, label: &str, counts: &mut Comparisons) {
    if expected.get("values").is_some() {
        let Metric::Known(actual) = actual else {
            panic!("{label}: expected captured value, got {actual:?}")
        };
        let wanted = historical::expected_values(expected).unwrap().iter().map(U256::to_string).collect::<Vec<_>>();
        assert_eq!(values(actual), wanted, "{label}");
        counts.values += 1;
    } else if expected["rpc_error"].get("data").is_none() {
        assert_eq!(expected["rpc_error"]["code"], 3, "{label}");
        assert_eq!(expected["rpc_error"]["message"], "execution reverted", "{label}");
        assert!(
            matches!(actual, Metric::ScopeRefusal { reason } if reason.starts_with("unmodeled recursive pool")),
            "{label}: {actual:?}"
        );
        counts.recursive += 1;
    } else {
        assert_eq!(expected["rpc_error"]["data"], format!("0x4e487b71{:064x}", 0x11), "{label}");
        assert!(
            matches!(actual, Metric::ModelRefusal { reason } if ["uint256 addition overflow", "uint256 multiplication overflow", "uint256 subtraction underflow"].contains(&reason.as_str())),
            "{label}: {actual:?}"
        );
        counts.panic11 += 1;
    }
}

fn check_adapter(cp: &Checkpoint, holder: &str, expected: &Value, label: &str, counts: &mut Comparisons) {
    let ledger = Ledger::from_checkpoint(cp.clone(), Limits::default()).unwrap();
    let result = ledger.evaluate(address(holder).unwrap()).unwrap();
    let prefix = if expected.get("holder_balance").is_some() { "holder_" } else { "" };
    check_metric(
        &result.hourly,
        &expected[format!("{prefix}hourly")],
        |v| vec![v.amount.clone(), v.stopping_hour.clone()],
        label,
        counts,
    );
    check_metric(&result.daily, &expected[format!("{prefix}daily")], |v| vec![v.clone()], label, counts);
    check_metric(&result.observable, &expected[format!("{prefix}balance")], |v| vec![v.clone()], label, counts);
    if let Some(expected) = expected.get("pool_balance") {
        check_metric(&decode::pool_terminal(&ledger), expected, |v| vec![v.clone()], label, counts);
    }
}

fn check_old(actual: anyhow::Result<Vec<U256>>, expected: &Value, label: &str) {
    match actual {
        Ok(values) => assert_eq!(values, historical::expected_values(expected).unwrap(), "{label}"),
        Err(error) if expected["rpc_error"].get("data").is_none() => {
            assert!(error.to_string().starts_with("unmodeled recursive pool"), "{label}: {error}");
            assert_eq!(expected["rpc_error"]["code"], 3);
            assert_eq!(expected["rpc_error"]["message"], "execution reverted");
        }
        Err(error) => {
            assert!(
                ["uint256 addition overflow", "uint256 multiplication overflow", "uint256 subtraction underflow"].contains(&error.to_string().as_str()),
                "{label}: {error}"
            );
            assert_eq!(expected["rpc_error"]["data"], format!("0x4e487b71{:064x}", 0x11), "{label}");
        }
    }
}

fn check_old_decoder(snapshot: &Value, expected: &Value, label: &str) {
    let decoded = fixture::decode(snapshot).unwrap();
    let prefix = if expected.get("holder_balance").is_some() { "holder_" } else { "" };
    check_old(decoded.hourly().map(|(a, stop)| vec![a, stop]), &expected[format!("{prefix}hourly")], label);
    check_old(decoded.daily().map(|v| vec![v]), &expected[format!("{prefix}daily")], label);
    check_old(decoded.evaluate().map(|v| vec![v.balance]), &expected[format!("{prefix}balance")], label);
    if let Some(expected) = expected.get("pool_balance") {
        check_old(decoded.pool_balance_if_terminal().map(|v| vec![v]), expected, label);
    }
}

#[test]
fn complete_snapshots_keep_all_raw_words_and_two_explicit_holders() {
    let raw = binding::original("historical.json").unwrap();
    let checkpoints = historical::checkpoints().unwrap();
    assert_eq!(checkpoints.len(), 5);
    for ((cp, row), words) in checkpoints.iter().zip(raw.as_array().unwrap()).zip(HISTORICAL_WORDS) {
        assert_eq!(cp.facts.len(), words);
        assert_eq!(cp.universe.len(), words);
        assert_eq!(cp.facts.iter().map(|f| &f.slot).collect::<Vec<_>>(), cp.universe.iter().collect::<Vec<_>>());
        assert_eq!(
            cp.holders.iter().copied().collect::<BTreeSet<_>>(),
            [address(row["holder"].as_str().unwrap()).unwrap(), binding::pool_a()].into_iter().collect()
        );
        assert_eq!(cp.at.number, row["block"].as_u64().unwrap());
        assert_eq!(cp.at.hash, binding::hexword(&row["hash"]).unwrap());
        assert_eq!(
            cp.at.timestamp,
            u64::from_str_radix(row["timestamp"].as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
        );
        assert_eq!(cp.at.parent_hash, None);
        assert_eq!(cp.at.producer_version, None);
        assert_eq!(cp.binding.activation_block, cp.at.number);
        assert_eq!(cp.binding.epoch, 1);
        for fact in &cp.facts {
            let matching: Vec<_> = row["raw_state"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| address(v["contract"].as_str().unwrap()).unwrap() == fact.slot.contract && binding::hexword(&v["key"]).unwrap() == fact.slot.key)
                .collect();
            assert_eq!(matching.len(), 1);
            assert_eq!(fact.word, binding::hexword(&matching[0]["value"]).unwrap());
            assert_eq!(fact.at, cp.at);
            assert_eq!(fact.origin, Origin::Checkpoint { evidence: cp.evidence.clone() });
        }
    }
    assert_eq!(checkpoints.iter().map(|c| c.facts.len()).sum::<usize>(), 1128);
    assert_eq!(checkpoints, historical::checkpoints().unwrap());
}

#[test]
fn all_391_external_observations_match_adapter_and_separate_strict_decoder() {
    let raw = binding::original("historical.json").unwrap();
    let mut counts = Comparisons::default();
    for (checkpoint, row) in historical::checkpoints().unwrap().iter().zip(raw.as_array().unwrap()) {
        check_adapter(checkpoint, row["holder"].as_str().unwrap(), &row["expected"], "historical", &mut counts);
        check_old_decoder(row, &row["expected"], "historical");
    }
    let controls = historical::controls().unwrap();
    assert_eq!(controls.len(), 111);
    for control in controls {
        check_adapter(
            &control.checkpoint,
            control.snapshot["holder"].as_str().unwrap(),
            &control.expected,
            &control.label,
            &mut counts,
        );
        check_old_decoder(&control.snapshot, &control.expected, &control.label);
        for extra in &control.extra_gas_controls {
            // Same scope refusal is compared with each independently saved gas
            // observation. Neither this adapter nor this test executes gas.
            check_adapter(
                &control.checkpoint,
                control.snapshot["holder"].as_str().unwrap(),
                &extra["expected"],
                &control.label,
                &mut counts,
            );
            check_old_decoder(&control.snapshot, &extra["expected"], &control.label);
        }
    }
    assert_eq!(
        counts,
        Comparisons {
            values: 303,
            panic11: 40,
            recursive: 48
        }
    );
    assert_eq!(historical::external_observations().unwrap(), 391);
}

#[test]
fn counterfactual_clocks_and_original_error_labels_are_preserved() {
    let controls = historical::controls().unwrap();
    let mut shifted = 0;
    let mut recursive = 0;
    let mut legacy_unsupported = 0;
    for control in controls {
        assert_eq!(control.scope, CONTROL_SCOPE);
        assert!(control.checkpoint.evidence.contains(CONTROL_SCOPE));
        assert_eq!(control.checkpoint.at.number, control.captured_at.number);
        assert_eq!(control.checkpoint.at.hash, control.captured_at.hash);
        assert_eq!(control.checkpoint.at.timestamp, control.timestamp_override);
        assert_eq!(control.expected, control.original["expected"]);
        assert_eq!(control.gas, control.original.get("gas").map(|v| v.as_u64().unwrap()));
        assert_eq!(
            control.extra_gas_controls,
            control
                .original
                .get("extra_gas_controls")
                .map(|v| v.as_array().unwrap().clone())
                .unwrap_or_default()
        );
        shifted += usize::from(control.timestamp_override != control.captured_at.timestamp);
        legacy_unsupported += usize::from(control.original["status"] == "explicitly_unsupported");
        if control.file == "preview.json" {
            assert_eq!(control.checkpoint.facts.len(), 201);
        }
        if control.file == "recursive.json" {
            assert_eq!(control.checkpoint.facts.len(), 162);
            if control.original["classification"] == "guarded_recursive_revert" {
                recursive += 1;
                assert_eq!(control.original["recursive_guard"], true);
                assert_eq!(control.original["initial_comparator_match"], false);
                assert_eq!(control.gas, Some(2_000_000));
                assert_eq!(control.extra_gas_controls.len(), 1);
                assert_eq!(control.extra_gas_controls[0]["gas"], 1_000_000);
                assert_eq!(control.extra_gas_controls[0]["initial_comparator_match"], false);
                for evidence in [&control.expected, &control.extra_gas_controls[0]["expected"]] {
                    for outcome in evidence.as_object().unwrap().values() {
                        assert!(outcome["rpc_error"].get("data").is_none());
                        assert!(outcome.get("values").is_none());
                    }
                }
            }
        }
    }
    assert!(shifted > 0);
    assert_eq!(recursive, 6);
    assert_eq!(legacy_unsupported, 3);
    let previews = binding::original("preview.json").unwrap();
    assert_eq!(previews["internal_preview_returns"].as_array().unwrap().len(), 10);
    // The ten internal pairs remain evidence for the unchanged old model test;
    // this importer makes no new internal-preview implementation or comparison.
}

#[test]
fn expectations_cannot_seed_or_repair_raw_facts() {
    let raw = binding::original("historical.json").unwrap();
    let original = &raw[0];
    let cp = historical::checkpoint_from_raw(original, "independent raw inputs").unwrap();
    let mut poisoned = original.clone();
    poisoned["expected"] = json!({"balance":{"values":["0"]}});
    poisoned["original_failure"]["rpc"] = json!("0");
    assert_eq!(cp, historical::checkpoint_from_raw(&poisoned, "independent raw inputs").unwrap());
    let holder = address(original["holder"].as_str().unwrap()).unwrap();
    let raw_key = decode::holder_key(holder, 0, 0);
    let observed_raw = cp.facts.iter().find(|f| f.slot == raw_key).unwrap().word;
    assert_ne!(value(&observed_raw), historical::expected_values(&original["expected"]["balance"]).unwrap()[0]);
    let mut missing = original.clone();
    missing["raw_state"]
        .as_array_mut()
        .unwrap()
        .retain(|r| !(address(r["contract"].as_str().unwrap()).unwrap() == raw_key.contract && binding::hexword(&r["key"]).unwrap() == raw_key.key));
    let cp = historical::checkpoint_from_raw(&missing, "missing raw fact; expectation remains").unwrap();
    let ledger = Ledger::from_checkpoint(cp, Limits::default()).unwrap();
    let evaluation = ledger.evaluate(holder).unwrap();
    assert!(matches!(evaluation.raw_basis, Metric::Unknown { ref missing } if missing.contains(&raw_key)));
    assert!(matches!(evaluation.observable, Metric::Unknown { .. }));
    assert!(fixture::decode(&missing).is_err());
    let controls = binding::original("controls.json").unwrap();
    let plain = historical::control_from_raw(original, &controls[0], "controls.json").unwrap();
    let mut changed_expected = controls[0].clone();
    changed_expected["expected"]["balance"] = json!({"rpc_error":{"code":-32000,"message":"unreviewed router error","data":"0xdeadbeef"}});
    let changed = historical::control_from_raw(original, &changed_expected, "controls.json").unwrap();
    assert_eq!(plain.checkpoint, changed.checkpoint);
    assert_eq!(changed.expected["balance"], changed_expected["expected"]["balance"]);
    assert_eq!(changed.expected["balance"]["rpc_error"]["data"], "0xdeadbeef");
}

#[test]
fn raw_parser_rejects_duplicate_aliases_widths_runtimes_and_false_history() {
    let rows = binding::original("historical.json").unwrap();
    let base = rows[0].clone();
    for kind in 0..10 {
        let mut bad = base.clone();
        match kind {
            0 => {
                let duplicate = bad["raw_state"][0].clone();
                bad["raw_state"].as_array_mut().unwrap().push(duplicate);
            }
            1 => {
                let mut duplicate = bad["raw_state"][0].clone();
                let text = duplicate["contract"].as_str().unwrap().to_owned();
                duplicate["contract"] = json!(format!("0x{}", text[2..].to_uppercase()));
                bad["raw_state"].as_array_mut().unwrap().push(duplicate);
            }
            2 => bad["raw_state"][0]["key"] = json!(format!("0x{}", "00".repeat(33))),
            3 => bad["raw_state"][0]["value"] = json!(format!("0x{}", "00".repeat(33))),
            4 => bad["raw_state"][0]["contract"] = json!(format!("0x{}", "00".repeat(21))),
            5 => {
                let duplicate = bad["runtime_bindings"][0].clone();
                bad["runtime_bindings"].as_array_mut().unwrap().push(duplicate);
            }
            6 => bad["runtime_bindings"][0]["runtime_hash"] = json!(format!("0x{}", "00".repeat(32))),
            7 => bad["hash"] = json!(format!("0x{}", "00".repeat(32))),
            8 => bad["timestamp"] = json!("0x6aab0060"),
            _ => bad["holder"] = rows[1]["holder"].clone(),
        }
        assert!(historical::checkpoint_from_raw(&bad, "invalid input").is_err(), "mutation {kind}");
    }
    assert!(historical::checkpoint_from_raw(&base, "").is_err());
    let controls = binding::original("controls.json").unwrap();
    let mut duplicate = controls[0].clone();
    let row = duplicate["state_diff"][0].clone();
    duplicate["state_diff"].as_array_mut().unwrap().push(row);
    assert!(historical::control_from_raw(&base, &duplicate, "controls.json").is_err());
    let mut new_key = controls[0].clone();
    new_key["state_diff"][0]["key"] = json!(format!("0x{}", "ff".repeat(32)));
    assert!(historical::control_from_raw(&base, &new_key, "controls.json").is_err());
    let mut wrong_base = controls[0].clone();
    wrong_base["base_case"] = json!(2);
    assert!(historical::control_from_raw(&base, &wrong_base, "controls.json").is_err());
}
