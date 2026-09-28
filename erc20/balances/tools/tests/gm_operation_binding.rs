#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::gm_proof as proof;
use serde_json::{json, Value};
const RAW: [&[u8]; 3] = [
    include_bytes!("../../tests/fixtures/gm-operation-proof/implementation-capture.json"),
    include_bytes!("../../tests/fixtures/gm-operation-proof/proxy-a-capture.json"),
    include_bytes!("../../tests/fixtures/gm-operation-proof/proxy-b-capture.json"),
];
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/gm-operation-proof")
            .join(name),
    )
    .unwrap()
}
fn captures() -> Vec<Value> {
    RAW.iter()
        .zip(&proof::CAPTURES)
        .map(|(raw, p)| proof::verify_capture(raw, p).unwrap())
        .collect()
}
#[test]
fn gm_full_three_capture_compiler_primary_and_license_bindings() {
    let c = captures();
    assert_eq!(c[1]["stdJsonInput"], c[2]["stdJsonInput"]);
    assert_eq!(c[1]["runtimeBytecode"], c[2]["runtimeBytecode"]);
    for (v, p) in c.iter().zip(&proof::CAPTURES) {
        proof::verify_compiled(v, &fixture(&format!("{}-compiler-output.json", p.label)), p).unwrap();
        let original: Value = serde_json::from_slice(&fixture(&format!("{}-compiler-input-original.json", p.label))).unwrap();
        let mut input: Value = serde_json::from_slice(&fixture(&format!("{}-compiler-input.json", p.label))).unwrap();
        assert_eq!(
            input["settings"]["outputSelection"],
            json!({"*":{"*":["abi","metadata","devdoc","userdoc","storageLayout","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}})
        );
        input["settings"].as_object_mut().unwrap().remove("outputSelection");
        assert_eq!(input, original);
        assert_eq!(original, v["stdJsonInput"]);
    }
    let primary: Value = serde_json::from_slice(&fixture("primary-sources.json")).unwrap();
    proof::verify_primary(&c, &primary).unwrap();
    let count = |status: &str| primary["sources"].as_array().unwrap().iter().filter(|s| s["status"] == status).count();
    assert_eq!(count("exact_ondo_vendor"), 15);
    assert_eq!(count("exact_upstream_oz"), 14);
    assert_eq!(count("contextual_nonmatch_primary_gap"), 5);
    let licenses: Value = serde_json::from_slice(&fixture("source-licenses.json")).unwrap();
    assert_eq!(licenses["sources"].as_array().unwrap().len(), 34);
    assert_eq!(licenses["upstream_proxy_license"]["sha256"], proof::sha(&fixture("LICENSE-proxy-upstream")));
    for s in licenses["sources"].as_array().unwrap() {
        let i = proof::CAPTURES.iter().position(|p| s["capture"] == p.label).unwrap();
        let path = s["path"].as_str().unwrap();
        assert_eq!(s["license"], c[i]["metadata"]["sources"][path]["license"]);
        let body = c[i]["sources"][path]["content"].as_str().unwrap();
        assert_eq!(s["sha256"], proof::sha(body.as_bytes()));
        assert_eq!(s["original_first_line"], body.lines().next().unwrap());
    }
    let manifest: Value = serde_json::from_slice(&fixture("solc-list.json")).unwrap();
    let entries: Vec<_> = manifest["builds"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["longVersion"] == proof::SOLC_VERSION)
        .collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["sha256"], format!("0x{}", proof::SOLC_SHA));
    assert_eq!(entries[0]["keccak256"], proof::SOLC_KECCAK);
    assert_eq!(manifest["releases"]["0.8.16"], entries[0]["path"]);
}
#[test]
fn gm_raw_source_settings_and_address_mutations_fail() {
    for (v, p) in captures().iter().zip(&proof::CAPTURES) {
        let mut raw = serde_json::to_vec(v).unwrap();
        raw.push(b' ');
        assert!(proof::verify_capture(&raw, p).is_err());
        for pointer in [
            "/chainId",
            "/address",
            "/match",
            "/runtimeMatch",
            "/creationMatch",
            "/compilation/compilerVersion",
            "/metadata/compiler/version",
            "/stdJsonInput/settings/evmVersion",
        ] {
            let mut bad = v.clone();
            *bad.pointer_mut(pointer).unwrap() = json!("changed");
            assert!(proof::verify_components(&bad, p).is_err(), "{pointer}");
        }
        for field in ["sources", "sourceIds"] {
            let mut bad = v.clone();
            bad[field] = json!({});
            assert!(proof::verify_components(&bad, p).is_err());
        }
        let mut bad = v.clone();
        bad["metadata"]["sources"] = json!({});
        assert!(proof::verify_components(&bad, p).is_err());
        let mut bad = v.clone();
        bad["stdJsonInput"]["sources"] = json!({});
        assert!(proof::verify_components(&bad, p).is_err());
        let name = v["sources"].as_object().unwrap().keys().next().unwrap();
        let mut bad = v.clone();
        bad["sources"][name]["content"] = json!("altered");
        assert!(proof::verify_components(&bad, p).is_err());
        let mut bad = v.clone();
        bad["metadata"]["sources"][name]["license"] = json!("UNLICENSED");
        assert!(proof::verify_components(&bad, p).is_err());
    }
}
#[test]
fn gm_exact_metadata_replacement_and_null_creation_are_not_general_normalization() {
    for (v, p) in captures().iter().zip(&proof::CAPTURES) {
        assert_ne!(v["runtimeBytecode"]["onchainBytecode"], v["runtimeBytecode"]["recompiledBytecode"]);
        for field in [
            "transformations",
            "transformationValues",
            "cborAuxdata",
            "immutableReferences",
            "linkReferences",
        ] {
            let mut bad = v.clone();
            bad["runtimeBytecode"][field] = json!({"wrong":true});
            assert!(proof::verify_components(&bad, p).is_err());
        }
        for key in ["onchainBytecode", "transformations", "transformationValues"] {
            for replacement in [json!([]), json!({}), json!("")] {
                let mut bad = v.clone();
                bad["creationBytecode"][key] = replacement;
                assert!(proof::verify_components(&bad, p).is_err(), "null != empty {key}");
            }
        }
        for kind in ["runtimeBytecode", "creationBytecode"] {
            let mut bad = v.clone();
            let mut b = proof::bytes(&v[kind]["recompiledBytecode"]).unwrap();
            b[0] ^= 1;
            bad[kind]["recompiledBytecode"] = json!(hex::encode(b));
            assert!(proof::verify_components(&bad, p).is_err());
        }
        for offset in [0, p.runtime_bytes - 54, p.runtime_bytes - 53, p.runtime_bytes - 1] {
            let mut bad = v.clone();
            let mut b = proof::runtime(v).unwrap();
            b[offset] ^= 1;
            bad["runtimeBytecode"]["onchainBytecode"] = json!(hex::encode(b));
            assert!(proof::verify_components(&bad, p).is_err());
        }
        let mut bad = v.clone();
        bad["runtimeBytecode"]["transformations"][0]["offset"] = json!(p.runtime_bytes - 54);
        assert!(proof::verify_components(&bad, p).is_err());
    }
}
#[test]
fn gm_compiler_output_and_primary_classifications_are_pinned() {
    let c = captures();
    for (v, p) in c.iter().zip(&proof::CAPTURES) {
        let raw = fixture(&format!("{}-compiler-output.json", p.label));
        let out: Value = serde_json::from_slice(&raw).unwrap();
        for field in ["object", "sourceMap", "generatedSources"] {
            let mut bad = out.clone();
            bad["contracts"][p.source][p.name]["evm"]["deployedBytecode"][field] = json!("changed");
            assert!(proof::verify_compiled(v, &serde_json::to_vec(&bad).unwrap(), p).is_err());
        }
        let mut raw = raw;
        raw.push(b' ');
        assert!(proof::verify_compiled(v, &raw, p).is_err());
    }
    let primary: Value = serde_json::from_slice(&fixture("primary-sources.json")).unwrap();
    for field in ["capture", "path", "url", "sha256", "content", "captured_sha256", "status"] {
        for i in [0, 19, 33] {
            let mut bad = primary.clone();
            bad["sources"][i][field] = json!("changed");
            assert!(proof::verify_primary(&c, &bad).is_err());
        }
    }
    let mut bad = primary.clone();
    bad["sources"][1] = bad["sources"][0].clone();
    assert!(proof::verify_primary(&c, &bad).is_err());
    let mut bad = primary.clone();
    bad["sources"].as_array_mut().unwrap().pop();
    assert!(proof::verify_primary(&c, &bad).is_err());
    let mut bad = primary;
    bad["custom_gap"] = json!("all exact");
    assert!(proof::verify_primary(&c, &bad).is_err());
}
#[test]
fn gm_runtime_metadata_exclusion_checks_pc_push_and_codecopy_ranges() {
    use erc20_balances_tools::ptoken_proof::cases::w;
    let suffix: usize = 8271 - 53;
    let trace = |pc, opcode, stack| json!({"trace":[{"pc":pc,"opcode":opcode,"stack_top":stack}]});
    assert!(proof::verify_runtime_trace(&trace(suffix - 1, 0, json!([]))).is_ok());
    assert!(proof::verify_runtime_trace(&trace(suffix, 0, json!([]))).is_err());
    assert!(proof::verify_runtime_trace(&trace(suffix - 1, 0x60, json!([]))).is_err());
    for (src, len, ok) in [
        (0usize, 1usize, true),
        (suffix - 1, 1, true),
        (suffix - 1, 2, false),
        (suffix, 1, false),
        (suffix, 0, true),
        (8271, 1, true),
    ] {
        assert_eq!(
            proof::verify_runtime_trace(&trace(0, 0x39, json!([w(0.into()), w(src.into()), w(len.into())]))).is_ok(),
            ok
        );
    }
    assert!(proof::verify_runtime_trace(&trace(0, 0x39, json!([]))).is_err());
}
#[test]
fn gm_invalid_source_attempt_preserves_report_and_as_run_inputs() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source");
    std::fs::create_dir(&input).unwrap();
    std::fs::write(input.join("implementation-capture.json"), b"{}").unwrap();
    let out = temp.path().join("attempt");
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_execute_gm_proof"))
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
#[test]
fn gm_auxiliary_provenance_rejects_self_consistent_input_and_scope_tampering() {
    let temp = tempfile::tempdir().unwrap();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/gm-operation-proof");
    for entry in std::fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            std::fs::copy(entry.path(), temp.path().join(entry.file_name())).unwrap();
        }
    }
    let captures = captures();
    proof::verify_auxiliary(temp.path(), &captures).unwrap();
    for name in ["solc-list.json", "compiler-version.txt", "LICENSE-proxy-upstream", "source-inventory.json"] {
        let path = temp.path().join(name);
        let original = std::fs::read(&path).unwrap();
        let mut bad = original.clone();
        bad.push(b' ');
        std::fs::write(&path, bad).unwrap();
        assert!(proof::verify_auxiliary(temp.path(), &captures).is_err(), "{name}");
        std::fs::write(path, original).unwrap();
    }
    for (name, pointer, replacement) in [
        ("implementation-compiler-input-original.json", "/settings/evmVersion", json!("paris")),
        ("implementation-compiler-input.json", "/settings/metadata/useLiteralContent", json!(true)),
        ("proxy-a-compiler-input.json", "/settings/outputSelection", json!({})),
        ("source-licenses.json", "/sources/0/license", json!("BUSL-1.1")),
        ("report.json", "/custom_primary_gaps", json!(0)),
        ("report.json", "/qualified", json!(true)),
        ("report.json", "/compiled/0/creation_match", json!("exact_match")),
        ("report.json", "/compiled/2/compiler_output_sha256", json!("changed")),
    ] {
        let path = temp.path().join(name);
        let original = std::fs::read(&path).unwrap();
        let mut bad: Value = serde_json::from_slice(&original).unwrap();
        *bad.pointer_mut(pointer).unwrap() = replacement;
        std::fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(proof::verify_auxiliary(temp.path(), &captures).is_err(), "{name}:{pointer}");
        std::fs::write(path, original).unwrap();
    }
}
#[test]
fn gm_contextual_nonmatch_body_cannot_be_replaced_with_another_self_hashed_body() {
    let captures = captures();
    let primary: Value = serde_json::from_slice(&fixture("primary-sources.json")).unwrap();
    for (i, source) in primary["sources"].as_array().unwrap().iter().enumerate() {
        if source["status"] == "contextual_nonmatch_primary_gap" {
            let mut bad = primary.clone();
            bad["sources"][i]["content"] = json!("some other nonmatch");
            bad["sources"][i]["sha256"] = json!(proof::sha(b"some other nonmatch"));
            assert!(proof::verify_primary(&captures, &bad).is_err());
        }
    }
}
