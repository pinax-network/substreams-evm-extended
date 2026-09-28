#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::securities_proof as proof;
use serde_json::{json, Value};
const RAW: &[u8] = include_bytes!("../../tests/fixtures/securities-operation-proof/capture.json");
fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/securities-operation-proof")
}
fn compiled() -> Vec<u8> {
    std::fs::read(fixture().join("compiler-output.json")).unwrap()
}
fn primary() -> Value {
    serde_json::from_slice(&std::fs::read(fixture().join("primary-sources.json")).unwrap()).unwrap()
}
#[test]
fn securities_full_capture_fresh_compiler_and_primary_bodies_are_bound() {
    let capture = proof::verify_capture(RAW).unwrap();
    proof::verify_auxiliary(&fixture(), &capture).unwrap();
    let output = proof::verify_compiled(&capture, &compiled()).unwrap();
    proof::verify_primary(&capture, &primary()).unwrap();
    assert_eq!(output["sources"], capture["sourceIds"]);
    assert!(capture["creationBytecode"]["onchainBytecode"].is_null());
    assert!(capture["creationMatch"].is_null());
    assert!(capture["deployment"].as_object().unwrap().values().all(Value::is_null));
    let original: Value = serde_json::from_slice(&std::fs::read(fixture().join("compiler-input-original.json")).unwrap()).unwrap();
    let mut input: Value = serde_json::from_slice(&std::fs::read(fixture().join("compiler-input.json")).unwrap()).unwrap();
    assert_eq!(
        input["settings"]["outputSelection"],
        json!({"*":{"*":["abi","metadata","devdoc","userdoc","storageLayout","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}})
    );
    input["settings"].as_object_mut().unwrap().remove("outputSelection");
    assert_eq!(input, original);
    assert_eq!(original, capture["stdJsonInput"]);
    let manifest: Value = serde_json::from_slice(&std::fs::read(fixture().join("solc-list.json")).unwrap()).unwrap();
    let entries: Vec<_> = manifest["builds"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["longVersion"] == proof::SOLC_VERSION)
        .collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["sha256"], format!("0x{}", proof::SOLC_SHA));
    assert_eq!(entries[0]["keccak256"], proof::SOLC_KECCAK);
    assert_eq!(manifest["releases"]["0.8.24"], entries[0]["path"]);
}
#[test]
fn securities_source_settings_identity_or_fabricated_creation_fail_closed() {
    let capture = proof::verify_capture(RAW).unwrap();
    let mut appended = RAW.to_vec();
    appended.push(b' ');
    assert!(proof::verify_capture(&appended).is_err());
    for pointer in [
        "/address",
        "/chainId",
        "/match",
        "/runtimeMatch",
        "/creationMatch",
        "/deployment/deployer",
        "/deployment/blockNumber",
        "/deployment/transactionHash",
        "/deployment/transactionIndex",
        "/compilation/compilerVersion",
        "/stdJsonInput/settings/evmVersion",
        "/stdJsonInput/settings/remappings",
        "/stdJsonInput/settings/viaIR",
        "/metadata/compiler/version",
        "/creationBytecode/onchainBytecode",
        "/creationBytecode/transformations",
        "/creationBytecode/transformationValues",
    ] {
        let mut bad = capture.clone();
        *bad.pointer_mut(pointer).unwrap() = json!("fabricated");
        assert!(proof::verify_components(&bad).is_err(), "{pointer}");
    }
    for field in ["sources", "sourceIds"] {
        let mut bad = capture.clone();
        bad[field] = json!({});
        assert!(proof::verify_components(&bad).is_err());
    }
    let paths: Vec<_> = capture["sources"].as_object().unwrap().keys().cloned().collect();
    for path in paths {
        let mut bad = capture.clone();
        bad["sources"][&path]["content"] = json!("substituted");
        bad["stdJsonInput"]["sources"][&path] = bad["sources"][&path].clone();
        assert!(proof::verify_components(&bad).is_err(), "body {path}");
        let mut bad = capture.clone();
        bad["metadata"]["sources"][&path]["keccak256"] = json!("0x00");
        assert!(proof::verify_components(&bad).is_err(), "metadata {path}");
    }
}
#[test]
fn securities_whole_code_cbor_layout_and_transformation_axes_are_exact() {
    let capture = proof::verify_capture(RAW).unwrap();
    for field in ["runtimeBytecode", "creationBytecode"] {
        for index in [0, 7, proof::bytes(&capture[field]["recompiledBytecode"]).unwrap().len() - 1] {
            let mut bad = capture.clone();
            let mut bytes = proof::bytes(&bad[field]["recompiledBytecode"]).unwrap();
            bytes[index] ^= 1;
            bad[field]["recompiledBytecode"] = json!(format!("0x{}", hex::encode(bytes)));
            assert!(proof::verify_components(&bad).is_err());
        }
        for key in ["sourceMap", "cborAuxdata", "linkReferences"] {
            let mut bad = capture.clone();
            bad[field][key] = json!({"unexpected":true});
            assert!(proof::verify_components(&bad).is_err());
        }
    }
    for key in ["immutableReferences", "transformations", "transformationValues", "onchainBytecode"] {
        let mut bad = capture.clone();
        bad["runtimeBytecode"][key] = json!({"unexpected":true});
        assert!(proof::verify_components(&bad).is_err());
    }
    let mut bad = capture.clone();
    bad["storageLayout"]["storage"] = json!([]);
    assert!(proof::verify_components(&bad).is_err());
    let mut bad = capture.clone();
    bad["abi"] = json!([]);
    assert!(proof::verify_components(&bad).is_err());
}
#[test]
fn securities_generated_source_evidence_cannot_be_swapped_behind_equal_runtime() {
    let capture = proof::verify_capture(RAW).unwrap();
    let raw = compiled();
    let output = proof::verify_compiled(&capture, &raw).unwrap();
    let sources = proof::mapped_sources(&capture, &output, "deployedBytecode").unwrap();
    assert_eq!(sources.len(), 32);
    assert!(sources.contains_key(&31));
    for field in ["object", "sourceMap", "generatedSources"] {
        let mut bad = output.clone();
        bad["contracts"][proof::SOURCE][proof::NAME]["evm"]["deployedBytecode"][field] = json!("changed");
        assert!(proof::verify_compiled(&capture, &serde_json::to_vec(&bad).unwrap()).is_err());
    }
    let mut bad = output.clone();
    bad["contracts"][proof::SOURCE][proof::NAME]["evm"]["deployedBytecode"]["generatedSources"][0]["contents"] = json!("fake source at same PC");
    assert!(proof::verify_compiled(&capture, &serde_json::to_vec(&bad).unwrap()).is_err());
    let mut raw = raw;
    raw.push(b' ');
    assert!(proof::verify_compiled(&capture, &raw).is_err());
}

#[test]
fn securities_unicode_metadata_serializations_are_separately_pinned_and_semantically_equal() {
    let capture = proof::verify_capture(RAW).unwrap();
    let output = proof::verify_compiled(&capture, &compiled()).unwrap();
    let saved = capture["stdJsonOutput"]["contracts"][proof::SOURCE][proof::NAME]["metadata"].as_str().unwrap();
    let fresh = output["contracts"][proof::SOURCE][proof::NAME]["metadata"].as_str().unwrap();
    assert_ne!(saved, fresh);
    assert_eq!(proof::sha(saved.as_bytes()), proof::SAVED_METADATA_SHA);
    assert_eq!(proof::sha(fresh.as_bytes()), proof::FRESH_METADATA_SHA);
    assert_eq!(serde_json::from_str::<Value>(saved).unwrap(), serde_json::from_str::<Value>(fresh).unwrap());
    let mut bad = output.clone();
    bad["contracts"][proof::SOURCE][proof::NAME]["metadata"] = json!(format!(" {fresh}"));
    assert!(proof::check_compiled(&capture, &serde_json::to_vec(&bad).unwrap()).is_err());
    let mut bad_capture = capture.clone();
    bad_capture["stdJsonOutput"]["contracts"][proof::SOURCE][proof::NAME]["metadata"] = json!(format!(" {saved}"));
    assert!(proof::verify_components(&bad_capture).is_err());
    assert!(proof::check_compiled(&bad_capture, &compiled()).is_err());
    let mut semantic: Value = serde_json::from_str(fresh).unwrap();
    semantic["compiler"]["version"] = json!("different");
    bad["contracts"][proof::SOURCE][proof::NAME]["metadata"] = json!(serde_json::to_string(&semantic).unwrap());
    assert!(proof::check_compiled(&capture, &serde_json::to_vec(&bad).unwrap()).is_err());
}
#[test]
fn securities_all_primary_classifications_urls_bodies_and_gaps_are_checked() {
    let capture = proof::verify_capture(RAW).unwrap();
    let original = primary();
    proof::verify_primary(&capture, &original).unwrap();
    for i in 0..31 {
        for field in ["path", "url", "classification", "capture_sha256", "sha256", "content"] {
            let mut bad = original.clone();
            bad["sources"][i][field] = json!("changed");
            assert!(proof::verify_primary(&capture, &bad).is_err(), "{i}/{field}");
        }
    }
    let mut bad = original.clone();
    bad["sources"].as_array_mut().unwrap().pop();
    assert!(proof::verify_primary(&capture, &bad).is_err());
    let mut bad = original.clone();
    bad["sources"][1] = bad["sources"][0].clone();
    assert!(proof::verify_primary(&capture, &bad).is_err());
    let mut bad = original;
    bad["source_gap"] = json!("all sources exact");
    assert!(proof::verify_primary(&capture, &bad).is_err());
}

#[test]
fn securities_source_input_manifest_version_and_notices_cannot_be_replaced() {
    let capture = proof::verify_capture(RAW).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let files = [
        "compiler-input-original.json",
        "compiler-input.json",
        "solc-list.json",
        "compiler-version.txt",
        "LICENSE-openzeppelin",
        "LICENSE-openzeppelin-upgradeable",
        "LICENSE-bep677",
    ];
    for file in files {
        std::fs::copy(fixture().join(file), temp.path().join(file)).unwrap();
    }
    proof::verify_auxiliary(temp.path(), &capture).unwrap();
    for file in files {
        let original = std::fs::read(temp.path().join(file)).unwrap();
        std::fs::write(temp.path().join(file), b"{}").unwrap();
        assert!(proof::verify_auxiliary(temp.path(), &capture).is_err(), "{file}");
        std::fs::write(temp.path().join(file), original).unwrap();
    }
    let mut input: Value = serde_json::from_slice(&std::fs::read(temp.path().join("compiler-input.json")).unwrap()).unwrap();
    input["settings"]["optimizer"]["runs"] = json!(201);
    std::fs::write(temp.path().join("compiler-input.json"), serde_json::to_vec(&input).unwrap()).unwrap();
    assert!(proof::verify_auxiliary(temp.path(), &capture).is_err());
}

#[test]
fn securities_bad_capture_cli_preserves_failure_report_and_as_run_sources() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("capture.json"), b"{}").unwrap();
    let out = temp.path().join("attempt");
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_execute_securities_proof"))
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
