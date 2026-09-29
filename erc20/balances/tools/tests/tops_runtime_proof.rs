#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::{tops_proof as old, tops_runtime_proof as p};
use p::{
    cases::{self, Proof},
    vm,
};
use primitive_types::U256;
use serde_json::{json, Value};
use std::{fs, path::Path, sync::OnceLock};
fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).unwrap()
}
fn runtime() -> Vec<u8> {
    let v = old::verify_capture(include_bytes!("../../tests/fixtures/tops-operation-proof/capture.json")).unwrap();
    old::bytes(&v["runtimeBytecode"]["onchainBytecode"]).unwrap()
}
fn matrix() -> &'static Vec<Value> {
    static M: OnceLock<Vec<Value>> = OnceLock::new();
    M.get_or_init(|| {
        let code = runtime();
        let mut p = Proof::new(&code);
        p.matrix(&mut |_| Ok(())).unwrap();
        p.cases
    })
}
fn case(name: &str) -> &'static Value {
    matrix().iter().find(|v| v["name"] == name).unwrap()
}
fn copies() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    for p in [old::FIXTURE, p::FIXTURE] {
        for e in fs::read_dir(root().join(p)).unwrap() {
            let p = e.unwrap().path();
            if p.is_file() && p.file_name().unwrap() != "README.md" {
                fs::copy(&p, d.path().join(p.file_name().unwrap())).unwrap();
            }
        }
    }
    d
}
#[test]
fn original_artifact_and_specification_pins_are_complete() {
    let d = copies();
    let (capture, compiled) = p::verify_inputs(d.path()).unwrap();
    assert_eq!(p::sha(&fs::read(d.path().join("full-output.json")).unwrap()), old::COMPILED_SHA);
    assert_eq!(
        capture["runtimeBytecode"]["immutableReferences"],
        compiled["contracts"][old::SOURCE][old::NAME]["evm"]["deployedBytecode"]["immutableReferences"]
    );
    assert_eq!(p::specs(d.path()).unwrap().as_array().unwrap().len(), 4);
    let mut b = fs::read(d.path().join("environment.py")).unwrap();
    b.push(b' ');
    fs::write(d.path().join("environment.py"), b).unwrap();
    assert!(p::verify_inputs(d.path()).is_err());
    let d = copies();
    let mut b = fs::read(d.path().join("capture.json")).unwrap();
    b.push(b' ');
    fs::write(d.path().join("capture.json"), b).unwrap();
    assert!(p::verify_inputs(d.path()).is_err());
}
#[test]
fn preparation_report_rejects_stale_failed_partial_and_relabelled_evidence() {
    let d = copies();
    let raw = b"fresh test source inventory";
    fs::write(d.path().join("source-inventory.json"), raw).unwrap();
    let digest = p::sha(raw);
    let report = json!({"status":"passed","qualified":false,"chain_calls":0,"scope":p::LIMITS,"historical_compiler_report_sha256":p::HISTORICAL_REPORT_SHA,"historical_compiler_inventory_sha256":p::HISTORICAL_INVENTORY_SHA,"artifacts":p::artifacts(d.path()).unwrap(),"specifications":p::specs(d.path()).unwrap(),"source_inventory_sha256":digest});
    fs::write(d.path().join("report.json"), serde_json::to_vec(&report).unwrap()).unwrap();
    p::verify_report(d.path(), &digest).unwrap();
    assert!(p::verify_report(d.path(), "another executable").is_err());
    for (key, value) in [
        ("status", json!("failed")),
        ("qualified", json!(true)),
        ("chain_calls", json!(1)),
        ("scope", json!("live")),
        ("historical_compiler_report_sha256", json!("fresh")),
        ("historical_compiler_inventory_sha256", json!(digest)),
        ("artifacts", json!({})),
        ("specifications", json!([])),
        ("source_inventory_sha256", json!("stale")),
    ] {
        let mut v = report.clone();
        v[key] = value;
        fs::write(d.path().join("report.json"), serde_json::to_vec(&v).unwrap()).unwrap();
        assert!(p::verify_report(d.path(), &digest).is_err(), "{key}");
    }
}
#[test]
fn exact_original_runtime_matrix_and_bounded_interactions() {
    assert!(matrix().len() >= 400);
    let digest = p::sha(&runtime());
    for v in matrix() {
        assert_eq!(v["code_sha256"], digest);
        assert_ne!(v["execution"]["exit"]["kind"], "invalid");
        if v["signature"] == "transfer(address,uint256)" && v["execution"]["exit"]["kind"] == "return" {
            assert_eq!(v["context_witness"]["consumed_calls"], 2);
            assert_eq!(v["context_witness"]["consumed_gas"], 2);
            assert_eq!(v["context_witness"]["calls"][0]["pc"], 7758);
            assert_eq!(v["context_witness"]["calls"][1]["pc"], 8143);
        }
    }
}
#[test]
fn cleanup_wrap_gate_and_checked_getters_remain_distinct() {
    for name in ["wrapped_expired_zero", "all_zero_no_cleanup"] {
        assert_eq!(case(name)["execution"]["writes"].as_array().unwrap().len(), 2);
    }
    assert_eq!(case("wrapped_expired_nonzero")["execution"]["writes"].as_array().unwrap().len(), 11);
    let checks: Vec<_> = matrix().iter().filter(|v| v["name"] == "checked_withdrawable_before_cleanup").collect();
    assert_eq!(checks.len(), 3);
    assert_eq!(checks.iter().filter(|v| v["execution"]["exit"]["kind"] == "revert").count(), 2);
}
#[test]
fn prefix_compaction_equality_and_tail_clears_are_measured() {
    assert_eq!(case("prefix_n6_k1_mode1")["execution"]["writes"].as_array().unwrap().len(), 22);
    assert_eq!(case("prefix_n6_k6_mode1")["execution"]["writes"].as_array().unwrap().len(), 27);
    assert_eq!(case("prefix_n6_k6_mode0")["execution"]["writes"].as_array().unwrap().len(), 2);
    assert_ne!(case("before_expiry")["execution"]["writes"], case("at_expiry")["execution"]["writes"]);
    assert_eq!(
        case("origin_is_sender_in_swap")["context"]["origin"],
        case("origin_is_sender_in_swap")["caller"]
    );
}
#[test]
fn late_allowance_preserves_attempted_prefix_and_rolls_back() {
    let v = case("late_allowance_6_7");
    assert_eq!(v["execution"]["exit"]["kind"], "revert");
    assert_eq!(v["execution"]["committed_storage"], v["prestate"]);
    assert_eq!(v["execution"]["committed_logs"], 0);
    assert_eq!(v["execution"]["logs"].as_array().unwrap().len(), 1);
    assert!(!v["execution"]["writes"].as_array().unwrap().is_empty());
    let max = format!("late_allowance_{}_7", U256::MAX);
    assert_eq!(
        case("late_allowance_7_7")["execution"]["writes"].as_array().unwrap().len(),
        case(&max)["execution"]["writes"].as_array().unwrap().len() + 1
    );
}
#[test]
fn saved_five_pop_reference_binds_all_23_stores() {
    let v = case("saved_five_pops_all_23_original_runtime_stores");
    assert_eq!(v["execution"]["writes"].as_array().unwrap().len(), 23);
    assert_eq!(v["execution"]["logs"].as_array().unwrap().len(), 1);
    assert_eq!(v["context_witness"]["consumed_calls"], 2);
    let mut linked = None;
    let code = runtime();
    Proof::new(&code)
        .matrix(&mut |v| {
            if v["name"] == "saved_five_pops_all_23_original_runtime_stores" {
                linked = Some(v["historical_reference"].clone());
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(linked.unwrap()["transaction_index"], 63);
}
#[test]
fn router_and_liquidity_boundaries_never_become_caught_success() {
    for n in [
        "nonzero_reserves_router_boundary",
        "origin_sender_router_catch_cannot_hide_boundary",
        "add_liquidity_after_cleanup_unsupported",
        "remove_liquidity_after_cleanup_unsupported",
    ] {
        let v = case(n);
        assert_eq!(v["execution"]["exit"]["kind"], "harness_failure");
        assert_eq!(v["prestate"], v["execution"]["committed_storage"]);
        assert_eq!(v["execution"]["committed_logs"], 0);
    }
    assert!(!case("add_liquidity_after_cleanup_unsupported")["execution"]["writes"]
        .as_array()
        .unwrap()
        .is_empty());
    let code = runtime();
    let pre = cases::seed(55.into(), &[], 0.into(), 11.into(), 22.into());
    let e = vm::execute_with_timestamp(
        &code,
        &erc20_balances_tools::ptoken_proof::cases::call("transfer(address,uint256)", &[22.into(), 0.into()]),
        11.into(),
        cases::address(p::CONTRACT),
        &pre,
        Some(100.into()),
    );
    assert_eq!(e.exit, vm::Exit::HarnessFailure("unsupported opcode 0x5a at pc 7757".into()));
}
#[test]
fn incorrect_stores_logs_interleaving_and_failed_attempts_are_rejected() {
    let code = runtime();
    let records = vec![[9.into(), 0.into(), 100.into()]];
    let pre = cases::seed(55.into(), &records, 3.into(), 11.into(), 22.into());
    let expected = cases::expected_transfer(&pre, 55.into(), &records, 100.into(), 11.into(), 22.into(), 7.into(), None);
    let data = erc20_balances_tools::ptoken_proof::cases::call("transfer(address,uint256)", &[22.into(), 7.into()]);
    let context = cases::script(55.into(), 100.into());
    let e = vm::execute_with_read_only_context(&code, &data, 11.into(), cases::address(p::CONTRACT), &pre, &context);
    cases::validate("control", &pre, &e, &expected).unwrap();
    for mode in 0..5 {
        let mut e = e.clone();
        match mode {
            0 => {
                e.execution.logs.clear();
            }
            1 => {
                e.execution.writes.pop();
            }
            2 => e.execution.logs[0].step = 0,
            3 => e.execution.logs[0].data[0] ^= 1,
            _ => e.execution.committed.insert(999.into(), 1.into()).map(|_| ()).unwrap_or(()),
        };
        assert!(cases::validate("mutant", &pre, &e, &expected).is_err());
    }
    let mut attempted = 0;
    let mut p = Proof::new(&[0]);
    assert!(p
        .run("broken", "transfer(address,uint256)", &data, 11.into(), &pre, &context, expected, &mut |_| {
            attempted += 1;
            Ok(())
        })
        .is_err());
    assert_eq!(attempted, 1);
}
#[test]
fn historical_artifact_raw_bytes_cannot_drift_behind_parsed_equality() {
    for file in [
        "original-input.json",
        "full-input.json",
        "harness-input.json",
        "harness-extraction.json",
        "settings.json",
    ] {
        let d = copies();
        let path = d.path().join(file);
        let mut raw = fs::read(&path).unwrap();
        raw.push(b' ');
        fs::write(path, raw).unwrap();
        assert!(p::verify_inputs(d.path()).is_err(), "whitespace mutation accepted for {file}");
    }
}
#[test]
fn compiled_reference_and_settings_bytes_cannot_follow_disk_drift() {
    p::verify_compiled_fixtures(p::SAVED_CASES, p::SETTINGS).unwrap();
    let mut cases = p::SAVED_CASES.to_vec();
    cases.push(b' ');
    assert!(p::verify_compiled_fixtures(&cases, p::SETTINGS).is_err());
    let mut settings = p::SETTINGS.to_vec();
    settings.push(b' ');
    assert!(p::verify_compiled_fixtures(p::SAVED_CASES, &settings).is_err());
    cases = p::SAVED_CASES.to_vec();
    cases[0] ^= 1;
    assert!(p::verify_compiled_fixtures(&cases, p::SETTINGS).is_err());
}
