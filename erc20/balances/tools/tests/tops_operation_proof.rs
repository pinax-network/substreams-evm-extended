#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::tops_proof as p;
use serde_json::{json, Value};
fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/tops-operation-proof")
}
fn capture() -> Value {
    p::verify_capture(&std::fs::read(root().join("capture.json")).unwrap()).unwrap()
}
#[test]
fn complete_artifact_and_primary_binding() {
    p::verify_source_directory(&root()).unwrap();
}
#[test]
fn capture_digest_rejects_source_and_immutable_tampering_before_parse() {
    let mut c = capture();
    c["sources"] = json!({});
    assert!(p::verify_capture(&serde_json::to_vec(&c).unwrap())
        .unwrap_err()
        .to_string()
        .contains("before parsing"));
    let mut c = capture();
    c["runtimeBytecode"]["immutableReferences"]["4240"][0]["start"] = json!(0);
    assert!(p::verify_components(&c).is_err());
    let mut c = capture();
    c["runtimeBytecode"]["transformationValues"]["immutables"]["4324"] = json!("0x0000000000000000000000000000000000000000000000000000000000000001");
    assert!(p::patch_runtime(&c).is_err());
    let mut c = capture();
    c["chainId"] = json!("1");
    assert!(p::verify_components(&c).is_err());
}
#[test]
fn full_compiler_pin_binds_ast_yul_and_source_maps() {
    let raw = std::fs::read(root().join("full-output.json")).unwrap();
    p::verify_compiled(&capture(), &raw).unwrap();
    let mut c: Value = serde_json::from_slice(&raw).unwrap();
    c["sources"][p::SOURCE]["ast"]["id"] = json!(777);
    assert!(p::verify_compiled(&capture(), &serde_json::to_vec(&c).unwrap()).is_err());
    let mut c: Value = serde_json::from_slice(&raw).unwrap();
    c["contracts"][p::SOURCE][p::NAME]["evm"]["deployedBytecode"]["sourceMap"] = json!("");
    assert!(p::check_compiled(&capture(), &serde_json::to_vec(&c).unwrap()).is_err());
}
#[test]
fn exact_harness_bodies_and_storage_stride() {
    let c = capture();
    let (h, _) = p::harness(&c).unwrap();
    assert_eq!(h, std::fs::read_to_string(root().join(p::HARNESS_SOURCE)).unwrap());
    for marker in [
        "struct LPInfo",
        "function _processExpiredLPInfo(",
        "function getWithdrawableLPAmount(",
        "function getUserLPDetails(",
    ] {
        assert_eq!(
            p::extract(&h, marker).unwrap(),
            p::extract(c["sources"][p::SOURCE]["content"].as_str().unwrap(), marker).unwrap()
        );
    }
    let o: Value = serde_json::from_slice(&std::fs::read(root().join("harness-output.json")).unwrap()).unwrap();
    let mut layout = o["contracts"][p::HARNESS_SOURCE][p::HARNESS_NAME]["storageLayout"].clone();
    p::verify_layout(&layout, true).unwrap();
    layout["storage"][1]["slot"] = json!("30");
    assert!(p::verify_layout(&layout, true).is_err());
    let mut c = c;
    c["sources"][p::SOURCE]["content"] = json!(h);
    assert!(p::harness(&c).is_err());
}
#[test]
fn artifact_mutations_fail_even_if_report_is_rewritten() {
    let tmp = tempfile::tempdir().unwrap();
    for e in std::fs::read_dir(root()).unwrap() {
        let e = e.unwrap();
        if e.path().is_file() {
            std::fs::copy(e.path(), tmp.path().join(e.file_name())).unwrap();
        }
    }
    p::verify_source_directory(tmp.path()).unwrap();
    for file in [
        "primary-sources.json",
        "full-output.json",
        "harness-output.json",
        "TOPSCleanup.sol",
        "harness-extraction.json",
        "solc-list.json",
        "execution-spec-block.py",
    ] {
        let path = tmp.path().join(file);
        let original = std::fs::read(&path).unwrap();
        let mut changed = original.clone();
        changed.push(b' ');
        std::fs::write(&path, changed).unwrap();
        // JSON-only formatting is accepted where a structural equality contract is intentional.
        if file == "harness-extraction.json" {
            let mut v: Value = serde_json::from_slice(&original).unwrap();
            v["extractions"][0]["sha256"] = json!("wrong");
            std::fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
        }
        assert!(p::verify_source_directory(tmp.path()).is_err(), "{file}");
        std::fs::write(&path, original).unwrap();
    }
}
#[test]
fn full_runtime_append_and_getter_matrix() {
    let c = capture();
    let code = p::bytes(&c["runtimeBytecode"]["onchainBytecode"]).unwrap();
    let mut proof = p::cases::Proof::new(&code);
    proof.full_matrix(&mut |_| Ok(())).unwrap();
    assert!(proof.cases.len() > 280);
}
#[test]
fn cleanup_source_matrix_and_broken_runtime_control() {
    let o: Value = serde_json::from_slice(&std::fs::read(root().join("harness-output.json")).unwrap()).unwrap();
    let code = p::bytes(&o["contracts"][p::HARNESS_SOURCE][p::HARNESS_NAME]["evm"]["deployedBytecode"]["object"]).unwrap();
    let mut proof = p::cases::Proof::new(&code);
    proof.harness_matrix(&mut |_| Ok(())).unwrap();
    assert!(proof.cases.len() > 300);
    assert!(p::cases::Proof::new(&[0]).harness_matrix(&mut |_| Ok(())).is_err());
}
#[test]
fn timestamp_reference_is_exact_and_legacy_default_is_unsupported() {
    let body = std::fs::read(root().join("execution-spec-block.py")).unwrap();
    assert_eq!(p::sha(&body), p::TIMESTAMP_SPEC_SHA);
    assert!(String::from_utf8(body).unwrap().contains("push(evm.stack, evm.message.block_env.time)"));
    assert_eq!(
        p::sha(&std::fs::read(root().join("LICENSE-execution-specs")).unwrap()),
        p::TIMESTAMP_LICENSE_SHA
    );
    assert!(matches!(
        p::vm::execute(&[0x42], &[], 1.into(), 2.into(), &p::vm::State::new()).exit,
        p::vm::Exit::HarnessFailure(_)
    ));
}
#[test]
fn captured_five_pop_subsequence_agrees_with_source_harness_only() {
    use p::vm::{Exit, State};
    use primitive_types::U256;
    let fixture: Value = serde_json::from_str(include_str!("../../tests/fixtures/bsc-exclusions-20260928/cases.json")).unwrap();
    let case = fixture.as_array().unwrap().iter().find(|x| x["token"] == "tops").unwrap();
    let writes = case["calls"][0]["writes"].as_array().unwrap();
    assert_eq!(writes.len(), 23);
    let user = U256::from_str_radix("28ebbb1c3003a30f9e627647862dd85aa4c165ce", 16).unwrap();
    let word = |v: &Value| U256::from_big_endian(&p::bytes(v).unwrap());
    let mut pre = State::new();
    for w in &writes[..21] {
        pre.entry(word(&w["key"])).or_insert(word(&w["old"]));
    }
    pre.insert(777.into(), 888.into());
    let now = (0..5).map(|i| word(&writes[4 * i + 2]["old"])).max().unwrap(); // Synthetic all-expired cutoff, not a claimed captured clock.
    let o: Value = serde_json::from_slice(&std::fs::read(root().join("harness-output.json")).unwrap()).unwrap();
    let code = p::bytes(&o["contracts"][p::HARNESS_SOURCE][p::HARNESS_NAME]["evm"]["deployedBytecode"]["object"]).unwrap();
    let e = p::vm::execute_with_timestamp(
        &code,
        &erc20_balances_tools::ptoken_proof::cases::call("process(address)", &[user]),
        1.into(),
        2.into(),
        &pre,
        Some(now),
    );
    assert_eq!(e.exit, Exit::Return(vec![]));
    assert_eq!(e.writes.len(), 21);
    for (a, b) in e.writes.iter().zip(&writes[..21]) {
        assert_eq!((a.key, a.old, a.new), (word(&b["key"]), word(&b["old"]), word(&b["new"])));
    }
    let mut expected = pre.clone();
    for w in &writes[..21] {
        expected.insert(word(&w["key"]), word(&w["new"]));
    }
    assert_eq!(e.committed, expected);
    assert!(e.logs.is_empty());
}
#[test]
fn original_transfer_and_constructor_have_exact_unsupported_boundaries_and_rollback() {
    let c = capture();
    let runtime = p::bytes(&c["runtimeBytecode"]["onchainBytecode"]).unwrap();
    let mut proof = p::cases::Proof::new(&runtime);
    let pre = p::cases::seed(55.into(), &[], 0.into());
    proof
        .boundary(
            "original_transfer_unsupported",
            &erc20_balances_tools::ptoken_proof::cases::call("transfer(address,uint256)", &[56.into(), 0.into()]),
            &pre,
            "unsupported opcode 0x5a at pc 7757",
            &mut |_| Ok(()),
        )
        .unwrap();
    let code = p::bytes(&c["creationBytecode"]["onchainBytecode"]).unwrap();
    p::cases::Proof::new(&code)
        .boundary(
            "original_constructor_unsupported",
            &[],
            &p::vm::State::new(),
            "unsupported opcode 0x5a at pc 913",
            &mut |_| Ok(()),
        )
        .unwrap();
}
#[test]
fn cli_binding_failure_saves_report_and_refuses_to_overwrite_attempt() {
    let tmp = tempfile::tempdir().unwrap();
    let output = tmp.path().join("failed-attempt");
    let run = || {
        std::process::Command::new(env!("CARGO_BIN_EXE_execute_tops_proof"))
            .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.."))
            .arg(tmp.path().join("missing-source"))
            .arg(&output)
            .output()
            .unwrap()
    };
    assert!(!run().status.success());
    let bytes = std::fs::read(output.join("report.json")).unwrap();
    let report: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(report["status"], "failed");
    assert_eq!(report["qualified"], false);
    assert_eq!(report["attempted_calls"], 0);
    assert_eq!(std::fs::read(output.join("raw-cases-inventory.json")).unwrap(), b"[]");
    assert!(!run().status.success());
    assert_eq!(std::fs::read(output.join("report.json")).unwrap(), bytes);
}
