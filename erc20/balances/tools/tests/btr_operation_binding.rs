#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::btr_proof as proof;
use serde_json::{json, Value};
const RAW: [&[u8]; 2] = [
    include_bytes!("../../tests/fixtures/btr-operation-proof/implementation-capture.json"),
    include_bytes!("../../tests/fixtures/btr-operation-proof/proxy-capture.json"),
];
const COMPILED: [&[u8]; 2] = [
    include_bytes!("../../tests/fixtures/btr-operation-proof/implementation-compiler-output.json"),
    include_bytes!("../../tests/fixtures/btr-operation-proof/proxy-compiler-output.json"),
];
#[test]
fn btr_complete_two_capture_compiler_and_primary_bindings_are_independent() {
    let mut captures = vec![];
    for (i, p) in proof::CAPTURES.iter().enumerate() {
        let c = proof::verify_capture(RAW[i], p).unwrap();
        let o = proof::verify_compiled(&c, COMPILED[i], p).unwrap();
        assert_eq!(o["sources"], c["sourceIds"]);
        captures.push(c);
    }
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/btr-operation-proof");
    for (c, p) in captures.iter().zip(&proof::CAPTURES) {
        let original: Value = serde_json::from_slice(&std::fs::read(fixture.join(format!("{}-compiler-input-original.json", p.label))).unwrap()).unwrap();
        let mut used: Value = serde_json::from_slice(&std::fs::read(fixture.join(format!("{}-compiler-input.json", p.label))).unwrap()).unwrap();
        assert_eq!(
            used["settings"]["outputSelection"],
            json!({"*":{"*":["abi","metadata","devdoc","userdoc","storageLayout","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}})
        );
        used["settings"].as_object_mut().unwrap().remove("outputSelection");
        assert_eq!(used, original);
        assert_eq!(original, c["stdJsonInput"]);
    }
    let primary: Value = serde_json::from_slice(include_bytes!("../../tests/fixtures/btr-operation-proof/primary-sources.json")).unwrap();
    proof::verify_primary(&captures, &primary).unwrap();
    let manifest: Value = serde_json::from_slice(include_bytes!("../../tests/fixtures/btr-operation-proof/solc-list.json")).unwrap();
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
fn btr_missing_source_metadata_settings_and_original_labels_fail_component_checks() {
    for (raw, p) in RAW.iter().zip(&proof::CAPTURES) {
        let c = proof::verify_capture(raw, p).unwrap();
        let mut changed = raw.to_vec();
        changed.push(b' ');
        assert!(proof::verify_capture(&changed, p).is_err());
        for pointer in [
            "/address",
            "/chainId",
            "/match",
            "/runtimeMatch",
            "/creationMatch",
            "/compilation/compilerVersion",
            "/stdJsonInput/settings/evmVersion",
            "/metadata/compiler/version",
        ] {
            let mut bad = c.clone();
            *bad.pointer_mut(pointer).unwrap() = json!("changed");
            assert!(proof::verify_components(&bad, p).is_err(), "{pointer}");
        }
        for field in ["sources", "sourceIds"] {
            let mut bad = c.clone();
            bad[field] = json!({});
            assert!(proof::verify_components(&bad, p).is_err());
        }
        for field in ["sources", "settings"] {
            let mut bad = c.clone();
            bad["stdJsonInput"][field] = json!({});
            assert!(proof::verify_components(&bad, p).is_err());
        }
        let mut bad = c.clone();
        bad["metadata"]["sources"] = json!({});
        assert!(proof::verify_components(&bad, p).is_err());
        let mut bad = c.clone();
        bad["storageLayout"]["storage"] = json!([{"slot":"1"}]);
        assert!(proof::verify_components(&bad, p).is_err());
    }
}
#[test]
fn btr_all_five_immutable_sites_and_every_constructor_byte_are_bound() {
    for (raw, p) in RAW.iter().zip(&proof::CAPTURES) {
        let c = proof::verify_capture(raw, p).unwrap();
        for field in ["runtimeBytecode", "creationBytecode"] {
            for sub in ["onchainBytecode", "recompiledBytecode"] {
                let mut bad = c.clone();
                let mut b = proof::bytes(&bad[field][sub]).unwrap();
                b[0] ^= 1;
                bad[field][sub] = json!(format!("0x{}", hex::encode(b)));
                assert!(proof::verify_components(&bad, p).is_err());
            }
        }
        for field in ["cborAuxdata", "linkReferences", "transformations", "transformationValues"] {
            let mut bad = c.clone();
            bad["runtimeBytecode"][field] = json!({"unexpected":true});
            assert!(proof::verify_components(&bad, p).is_err());
        }
        if p.label == "implementation" {
            for i in 0..5 {
                let mut bad = c.clone();
                bad["runtimeBytecode"]["immutableReferences"]["1168"][i]["start"] = json!(1);
                assert!(proof::verify_components(&bad, p).is_err());
            }
            let mut bad = c.clone();
            bad["runtimeBytecode"]["transformationValues"]["immutables"]["1168"] =
                json!(format!("0x{}", hex::encode(proof::vm::word(proof::address(proof::PROXY)))));
            assert!(proof::verify_components(&bad, p).is_err());
        } else {
            let args = proof::constructor_arguments(p);
            assert_eq!(args.len(), 416);
            assert_eq!(proof::initializer_arguments().len(), 292);
            for i in 0..args.len() {
                let mut bad = c.clone();
                let mut altered = args.clone();
                altered[i] ^= 1;
                bad["creationBytecode"]["transformationValues"]["constructorArguments"] = json!(format!("0x{}", hex::encode(altered)));
                assert!(proof::verify_components(&bad, p).is_err(), "constructor byte {i}");
            }
        }
    }
}
#[test]
fn btr_fresh_generated_sources_source_maps_and_primary_proofs_cannot_be_replaced() {
    let captures: Vec<_> = RAW
        .iter()
        .zip(&proof::CAPTURES)
        .map(|(raw, p)| proof::verify_capture(raw, p).unwrap())
        .collect();
    for (i, p) in proof::CAPTURES.iter().enumerate() {
        let o = proof::verify_compiled(&captures[i], COMPILED[i], p).unwrap();
        for field in ["object", "sourceMap", "generatedSources"] {
            let mut bad = o.clone();
            bad["contracts"][p.source][p.name]["evm"]["deployedBytecode"][field] = json!("changed");
            assert!(proof::verify_compiled(&captures[i], &serde_json::to_vec(&bad).unwrap(), p).is_err());
        }
        let mut raw = COMPILED[i].to_vec();
        raw.push(b' ');
        assert!(proof::verify_compiled(&captures[i], &raw, p).is_err());
    }
    let original: Value = serde_json::from_slice(include_bytes!("../../tests/fixtures/btr-operation-proof/primary-sources.json")).unwrap();
    for field in ["content", "url", "sha256", "path", "capture"] {
        let mut bad = original.clone();
        bad["sources"][0][field] = json!("changed");
        assert!(proof::verify_primary(&captures, &bad).is_err());
    }
    let mut bad = original.clone();
    bad["sources"].as_array_mut().unwrap().pop();
    assert!(proof::verify_primary(&captures, &bad).is_err());
    let mut bad = original.clone();
    bad["sources"][1] = bad["sources"][0].clone();
    assert!(proof::verify_primary(&captures, &bad).is_err());
    let mut bad = original;
    bad["token_gap"] = json!("all exact");
    assert!(proof::verify_primary(&captures, &bad).is_err());
}
#[test]
fn btr_cli_invalid_source_preserves_failure_report_and_as_run_code() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source");
    std::fs::create_dir(&input).unwrap();
    std::fs::write(input.join("implementation-capture.json"), b"{}").unwrap();
    let out = temp.path().join("attempt");
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_execute_btr_proof"))
        .current_dir(root)
        .arg(input)
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
