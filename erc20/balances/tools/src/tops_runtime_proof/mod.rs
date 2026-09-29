//! Exact original TOPS runtime under a bounded synthetic external-read script.
pub mod cases;
pub use crate::tops_proof::{bytes, kh, mapped_sources, sha, source_map, vm, CONTRACT, NAME, SOURCE};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{fs, path::Path};
pub const FIXTURE: &str = "erc20/balances/tests/fixtures/tops-runtime-cleanup-proof";
pub const SPEC_PIN: &str = "3e170675290673a68eeb4652501fb2ae74fea0cb";
pub const LIMITS: &str = "Exact capture-bound original TOPS runtime with synthetic local storage, explicit origin/time and ordered read-only external responses. GAS words are supplied values, not gas accounting or sufficiency. No callee bytecode executes. CALLVALUE is zero. No live chain, deployed state, caller authorization, constructor, swap/router, package, holder, validator or candidate qualification. Historical cleanup harness remains source-semantics-only. Unexpected interactions fail the host boundary even where source catch could absorb a normal callee revert.";
pub const HISTORICAL_REPORT_SHA: &str = "e473e5c6284bcc36ee0f325f356912c940eaa91dca29dd3c1006df3f6fde265a";
pub const HISTORICAL_INVENTORY_SHA: &str = "a3d755532cdc9c868b2941943e6e42dcc0c469062843d73781e3246c0c456c92";
pub const SAVED_CASES_PATH: &str = "erc20/balances/tests/fixtures/bsc-exclusions-20260928/cases.json";
pub const SAVED_CASES: &[u8] = include_bytes!("../../../tests/fixtures/bsc-exclusions-20260928/cases.json");
pub const SETTINGS_PATH: &str = "erc20/balances/tests/fixtures/tops-operation-proof/settings.json";
pub const SETTINGS: &[u8] = include_bytes!("../../../tests/fixtures/tops-operation-proof/settings.json");
pub fn verify_compiled_fixtures(cases: &[u8], settings: &[u8]) -> Result<()> {
    ensure!(
        cases == SAVED_CASES && sha(cases) == "72b1c4de708a81de0ce50049b78e5262db7290cb1513a92b06130404e7eb47ff" && cases.len() == 21392,
        "exact compiled historical 23-store fixture"
    );
    ensure!(
        settings == SETTINGS && sha(settings) == "9b4f4951f0782e9a503ccab777f429b90602ce0a4db247cf11a0556a27c02801" && settings.len() == 678,
        "exact compiled historical settings fixture"
    );
    Ok(())
}

pub const SPECS: [(&str, &str, &str); 4] = [
    (
        "environment.py",
        "src/ethereum/forks/cancun/vm/instructions/environment.py",
        "a01c01b4266eed06bff799ef5886f86d7103ee8599f4e7fcba5b11a9b631669d",
    ),
    (
        "system.py",
        "src/ethereum/forks/cancun/vm/instructions/system.py",
        "32bc20f15831feb8327cc6770f6b963d56abfc996a36bc1b49655a8917ab85df",
    ),
    (
        "control_flow.py",
        "src/ethereum/forks/cancun/vm/instructions/control_flow.py",
        "cfd4781581c0f65d3a25534f8a4eb8cd2e749aa7d54c337ef4a89752fa894448",
    ),
    (
        "LICENSE-execution-specs",
        "LICENSE.md",
        "f81577cb97c59ed748f9117fa839b80e3be0ec605a4b66c234e07468bc011808",
    ),
];
pub fn specs(path: &Path) -> Result<Value> {
    let mut out = vec![];
    for (file, upstream, digest) in SPECS {
        let raw = fs::read(path.join(file))?;
        ensure!(sha(&raw) == digest, "exact primary specification {file}");
        out.push(json!({"file":file,"sha256":digest,"url":format!("https://raw.githubusercontent.com/ethereum/execution-specs/{SPEC_PIN}/{upstream}")}));
    }
    Ok(json!(out))
}
pub fn artifacts(path: &Path) -> Result<Value> {
    let mut records = std::collections::BTreeMap::new();
    for e in fs::read_dir(path)? {
        let p = e?.path();
        if p.is_file()
            && p.file_name()
                .is_some_and(|x| x != "README.md" && x != "report.json" && x != "source-inventory.json")
        {
            let raw = fs::read(&p)?;
            records.insert(
                p.file_name().unwrap().to_string_lossy().into_owned(),
                json!({"sha256":sha(&raw),"bytes":raw.len()}),
            );
        }
    }
    Ok(json!(records))
}
/// Reuses exact historical compiler pins, never relabels them as a fresh compiler run.
pub fn verify_inputs(path: &Path) -> Result<(Value, Value)> {
    let (capture, compiled, _, _) = crate::tops_proof::verify_source_directory(path)?;
    specs(path)?;
    let raw = fs::read(path.join("historical-compiler-report.json"))?;
    ensure!(sha(&raw) == HISTORICAL_REPORT_SHA, "historical compiler report pin");
    let report: Value = serde_json::from_slice(&raw)?;
    ensure!(
        report["status"] == "passed" && report["source_inventory_sha256"] == HISTORICAL_INVENTORY_SHA,
        "historical report status/inventory"
    );
    ensure!(
        sha(&fs::read(path.join("historical-compiler-inventory.json"))?) == HISTORICAL_INVENTORY_SHA,
        "historical compiler inventory pin"
    );
    let inventory: Value = serde_json::from_slice(&fs::read(path.join("historical-compiler-inventory.json"))?)?;
    verify_compiled_fixtures(SAVED_CASES, &fs::read(path.join("settings.json"))?)?;
    for (name, raw) in [(SAVED_CASES_PATH, SAVED_CASES), (SETTINGS_PATH, SETTINGS)] {
        ensure!(
            inventory["files"][name]["sha256"] == sha(raw) && inventory["files"][name]["bytes"] == raw.len(),
            "compiled fixture binds original compiler inventory {name}"
        );
    }
    for artifact in report["artifacts"].as_array().context("historical artifacts")? {
        let file = artifact["file"].as_str().context("artifact file")?;
        ensure!(artifact["sha256"] == sha(&fs::read(path.join(file))?), "exact historical artifact bytes {file}");
    }
    Ok((capture, compiled))
}
/// Snapshot every actual compiled adapter/driver/helper, in addition to the entire
/// host-source inventory. A stale binary cannot bless current disk source bytes.
pub fn snapshot_sources(out: &Path) -> Result<String> {
    verify_compiled_fixtures(&fs::read(SAVED_CASES_PATH)?, &fs::read(SETTINGS_PATH)?)?;
    crate::tops_proof::snapshot_sources(out)?;
    let mut inventory: Value = serde_json::from_slice(&fs::read(out.join("source-inventory.json"))?)?;
    inventory["scope"] = json!("TOPS original runtime host proof; fresh executable inventory, historical compiler artifacts separately pinned");
    for (name, path, text) in [
        (
            "runtime-binding.rs",
            "erc20/balances/tools/src/tops_runtime_proof/mod.rs",
            include_str!("mod.rs"),
        ),
        (
            "runtime-cases.rs",
            "erc20/balances/tools/src/tops_runtime_proof/cases.rs",
            include_str!("cases.rs"),
        ),
        (
            "runtime-preparer.rs",
            "erc20/balances/tools/src/bin/prepare_tops_runtime_proof.rs",
            include_str!("../bin/prepare_tops_runtime_proof.rs"),
        ),
        (
            "runtime-executor.rs",
            "erc20/balances/tools/src/bin/execute_tops_runtime_proof.rs",
            include_str!("../bin/execute_tops_runtime_proof.rs"),
        ),
        (
            "vm-external.rs",
            "erc20/balances/tools/src/ptoken_proof/vm_external.rs",
            include_str!("../ptoken_proof/vm_external.rs"),
        ),
        (
            "vm-context-tests.rs",
            "erc20/balances/tools/tests/tops_runtime_context.rs",
            include_str!("../../tests/tops_runtime_context.rs"),
        ),
        (
            "runtime-proof-tests.rs",
            "erc20/balances/tools/tests/tops_runtime_proof.rs",
            include_str!("../../tests/tops_runtime_proof.rs"),
        ),
        (
            "abi-adapter.rs",
            "erc20/balances/tools/src/ptoken_proof/cases.rs",
            include_str!("../ptoken_proof/cases.rs"),
        ),
        (
            "host-binding-helpers.rs",
            "erc20/balances/tools/src/ptoken_proof/mod.rs",
            include_str!("../ptoken_proof/mod.rs"),
        ),
        ("host-lib.rs", "erc20/balances/tools/src/lib.rs", include_str!("../lib.rs")),
        ("keccak-helper.rs", "erc20/balances/src/lib.rs", include_str!("../../../src/lib.rs")),
    ] {
        ensure!(fs::read(path)? == text.as_bytes(), "stale executable {path}");
        fs::write(out.join("as-run").join(name), text)?;
    }
    for e in fs::read_dir(FIXTURE)? {
        let p = e?.path();
        if p.is_file() && p.file_name().is_none_or(|n| n != "README.md") {
            let raw = fs::read(&p)?;
            inventory["files"][p.to_string_lossy().as_ref()] = json!({"sha256":sha(&raw),"bytes":raw.len()});
        }
    }
    let raw = serde_json::to_vec_pretty(&inventory)?;
    fs::write(out.join("source-inventory.json"), &raw)?;
    Ok(sha(&raw))
}
pub fn verify_report(path: &Path, current_inventory: &str) -> Result<(Value, Value, Value)> {
    let (capture, compiled) = verify_inputs(path)?;
    let report: Value = serde_json::from_slice(&fs::read(path.join("report.json"))?)?;
    ensure!(
        report["status"] == "passed" && report["qualified"] == false && report["chain_calls"] == 0 && report["scope"] == LIMITS,
        "successful bounded preparation report"
    );
    ensure!(
        report["historical_compiler_report_sha256"] == HISTORICAL_REPORT_SHA && report["historical_compiler_inventory_sha256"] == HISTORICAL_INVENTORY_SHA,
        "separate historical provenance"
    );
    ensure!(
        report["artifacts"] == artifacts(path)? && report["specifications"] == specs(path)?,
        "complete exact artifact/spec manifests"
    );
    let raw = fs::read(path.join("source-inventory.json"))?;
    ensure!(
        sha(&raw) == current_inventory && report["source_inventory_sha256"] == current_inventory,
        "same preparation/executor/current compiled inventory"
    );
    Ok((capture, compiled, report))
}
