#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::ptoken_proof::{self as proof, cases::Proof, vm};
use serde_json::Value;
const CAPTURE: &[u8] = include_bytes!("../../tests/fixtures/ptoken-operation-proof/capture.json");
const COMPILED: &[u8] = include_bytes!("../../tests/fixtures/ptoken-operation-proof/compiler-output.json");
fn inputs() -> (Value, Value, Vec<u8>) {
    let c = proof::verify_capture(CAPTURE).unwrap();
    let o = proof::verify_compiled(&c, COMPILED).unwrap();
    let code = proof::bytes(&o["contracts"][proof::SOURCE][proof::NAME]["evm"]["deployedBytecode"]["object"]).unwrap();
    (c, o, code)
}
#[test]
fn ptoken_complete_source_compiler_input_output_and_public_dependency_binding() {
    let (c, o, _) = inputs();
    let original: Value = serde_json::from_slice(include_bytes!("../../tests/fixtures/ptoken-operation-proof/compiler-input-original.json")).unwrap();
    let mut used: Value = serde_json::from_slice(include_bytes!("../../tests/fixtures/ptoken-operation-proof/compiler-input.json")).unwrap();
    assert_eq!(original, c["stdJsonInput"]);
    assert_eq!(
        used["settings"]["outputSelection"],
        serde_json::json!({"*":{"*":["abi","metadata","devdoc","userdoc","storageLayout","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}})
    );
    used["settings"].as_object_mut().unwrap().remove("outputSelection");
    assert_eq!(used, original);
    assert_eq!(original["settings"]["evmVersion"], "cancun");
    assert_eq!(original["settings"]["optimizer"]["runs"], 10000);
    let primary: Value = serde_json::from_slice(include_bytes!("../../tests/fixtures/ptoken-operation-proof/primary-sources.json")).unwrap();
    assert_eq!(primary["token_gap"], proof::PRIMARY_GAP);
    assert_eq!(primary["sources"].as_object().unwrap().len(), 20);
    for (path, source) in primary["sources"].as_object().unwrap() {
        assert_eq!(source["content"], c["sources"][path]["content"]);
        let content = source["content"].as_str().unwrap();
        assert_eq!(source["sha256"], proof::sha(content.as_bytes()));
        assert_eq!(
            source["url"],
            format!(
                "https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{}/{}",
                proof::OZ_PIN,
                path.strip_prefix("lib/openzeppelin-contracts/").unwrap()
            )
        );
    }
    let manifest: Value = serde_json::from_slice(include_bytes!("../../tests/fixtures/ptoken-operation-proof/solc-list.json")).unwrap();
    let compiler: Vec<_> = manifest["builds"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["longVersion"] == proof::SOLC_VERSION)
        .collect();
    assert_eq!(compiler.len(), 1);
    assert_eq!(compiler[0]["sha256"], format!("0x{}", proof::SOLC_SHA));
    assert_eq!(o["sources"], c["sourceIds"]);
}
#[test]
fn ptoken_capture_compiler_runtime_creation_maps_and_generated_source_tampering_refused() {
    let (c, o, _) = inputs();
    for pointer in [
        "/chainId",
        "/address",
        "/match",
        "/metadata/compiler/version",
        "/stdJsonInput/settings/evmVersion",
        "/runtimeBytecode/onchainBytecode",
        "/creationBytecode/onchainBytecode",
        "/sources/src~1v2~1PToken.sol/content",
    ] {
        let mut wrong = c.clone();
        *wrong.pointer_mut(pointer).unwrap() = serde_json::json!("mutated");
        assert!(proof::verify_capture(&serde_json::to_vec(&wrong).unwrap()).is_err(), "{pointer}");
    }
    for kind in ["object", "sourceMap", "generatedSources"] {
        let mut wrong = o.clone();
        wrong["contracts"][proof::SOURCE][proof::NAME]["evm"]["deployedBytecode"][kind] = serde_json::json!("mutated");
        assert!(proof::verify_compiled(&c, &serde_json::to_vec(&wrong).unwrap()).is_err());
    }
    let mut changed = COMPILED.to_vec();
    changed.push(b' ');
    assert!(proof::verify_compiled(&c, &changed).is_err());
}
#[test]
fn ptoken_actual_selected_creation_runtime_and_coupled_operation_matrix() {
    let (c, _, runtime) = inputs();
    let mut p = Proof::new(&runtime);
    let mut names = std::collections::BTreeSet::new();
    let mut save = |_: usize, v: &Value| {
        names.insert(v["name"].as_str().unwrap().to_owned());
        Ok(())
    };
    p.constructor(&proof::bytes(&c["creationBytecode"]["onchainBytecode"]).unwrap(), &mut save)
        .unwrap();
    p.matrix(&mut save).unwrap();
    assert_eq!(p.calls, 382);
    for name in [
        "remove_tail",
        "remove_zero_tail",
        "remove_middle_zero_tail",
        "renounce_absent",
        "sequence_5",
        "transfer_from_prefix_rollback",
        "incoherent_empty_set_prefix_revert",
        "incoherent_max_length_wrap",
    ] {
        assert!(names.contains(name));
    }
    let wrap = p.cases.iter().find(|v| v["name"] == "incoherent_max_length_wrap").unwrap();
    assert_eq!(wrap["execution"]["exit"]["kind"], "return");
    assert!(p
        .cases
        .iter()
        .any(|v| v["execution"]["trace"].as_array().unwrap().iter().any(|s| s["opcode"] == 0x5e)));
}
#[test]
fn ptoken_broken_membership_store_cannot_pass_operation_expectations() {
    let (_, _, mut runtime) = inputs();
    assert_eq!(runtime[4149], 0x55);
    runtime[4149] = 0x50;
    let mut p = Proof::new(&runtime);
    let mut attempted = 0;
    let err = p
        .matrix(&mut |_, _| {
            attempted += 1;
            Ok(())
        })
        .unwrap_err();
    assert!(attempted > 0);
    assert!(!err.to_string().is_empty());
}
#[test]
fn ptoken_source_locations_bind_effect_pcs_across_all_sources_and_generated_yul() {
    let (c, o, runtime) = inputs();
    let output = &o["contracts"][proof::SOURCE][proof::NAME]["evm"]["deployedBytecode"];
    let map = proof::source_map::decode(&runtime, output["sourceMap"].as_str().unwrap()).unwrap();
    let sources = proof::source_map::sources(&c, &o, "deployedBytecode").unwrap();
    assert!(sources.len() >= 21);
    let mut state = vm::State::new();
    proof::cases::set_role(&mut state, 0.into(), &[1.into()]);
    let e = vm::execute(
        &runtime,
        &proof::cases::call("grantRole(bytes32,address)", &[proof::cases::role("R"), 2.into()]),
        1.into(),
        2.into(),
        &state,
    );
    assert_eq!(e.writes.len(), 4);
    let mut paths = std::collections::BTreeSet::new();
    for write in &e.writes {
        let span = proof::source_map::describe(write.pc, &map, &sources).unwrap();
        assert_eq!(runtime[write.pc], 0x55);
        paths.insert(span["path"].as_str().unwrap().to_owned());
    }
    assert!(paths.iter().any(|p| p.ends_with("AccessControl.sol")));
    assert!(paths.iter().any(|p| p.ends_with("EnumerableSet.sol")));
}
#[test]
fn ptoken_cli_bad_input_preserves_failure_and_as_run_sources() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("capture.json"), b"{}").unwrap();
    let out = temp.path().join("attempt");
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_execute_ptoken_proof"))
        .current_dir(root)
        .arg(source)
        .arg(&out)
        .output()
        .unwrap();
    assert!(!result.status.success());
    let report: Value = serde_json::from_slice(&std::fs::read(out.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["status"], "failed");
    assert_eq!(report["attempted_calls"], 0);
    assert!(report["error"].as_str().unwrap().contains("capture digest"));
    assert_eq!(std::fs::read(out.join("as-run/vm.rs")).unwrap(), include_bytes!("../src/ptoken_proof/vm.rs"));
}
