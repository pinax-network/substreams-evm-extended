#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::erc20tokenx_proof as proof;
use serde_json::{json, Value};
fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/erc20tokenx-operation-proof")
}
fn capture(i: usize) -> Value {
    proof::verify_capture(&std::fs::read(fixture().join(format!("{}-capture.json", proof::CAPTURES[i].0))).unwrap(), i).unwrap()
}
fn compiled() -> Vec<u8> {
    std::fs::read(fixture().join("compiler-output.json")).unwrap()
}
#[test]
fn erc20tokenx_two_complete_sources_and_all_three_whole_runtimes_agree() {
    let a = capture(0);
    let b = capture(1);
    assert_eq!(a["stdJsonInput"], b["stdJsonInput"]);
    assert_ne!(a["deployment"], b["deployment"]);
    assert_ne!(a["creationBytecode"]["onchainBytecode"], b["creationBytecode"]["onchainBytecode"]);
    for c in [&a, &b] {
        proof::verify_compiled(c, &compiled()).unwrap();
        proof::verify_auxiliary(&fixture(), c).unwrap();
    }
    assert_eq!(proof::runtime(&a).unwrap(), proof::runtime(&b).unwrap());
    proof::verify_phi(&fixture(), &proof::runtime(&a).unwrap()).unwrap();
    let phi: Value = serde_json::from_slice(&std::fs::read(fixture().join("phi-source-request.json")).unwrap()).unwrap();
    assert!(phi.get("sources").is_none());
    assert!(phi.get("deployment").is_none());
    assert!(phi["runtimeMatch"].is_null());
}
#[test]
fn erc20tokenx_each_complete_capture_and_creation_append_fail_closed_on_mutation() {
    for i in 0..2 {
        let c = capture(i);
        for field in [
            "address",
            "chainId",
            "deployment",
            "compilation",
            "sources",
            "sourceIds",
            "stdJsonInput",
            "metadata",
            "storageLayout",
            "abi",
        ] {
            let mut bad = c.clone();
            bad[field] = json!({"changed":true});
            assert!(proof::verify_components(&bad, i).is_err(), "{i}/{field}");
        }
        for section in ["runtimeBytecode", "creationBytecode"] {
            for field in [
                "onchainBytecode",
                "recompiledBytecode",
                "sourceMap",
                "cborAuxdata",
                "transformations",
                "transformationValues",
                "linkReferences",
            ] {
                let mut bad = c.clone();
                bad[section][field] = json!("changed");
                assert!(proof::verify_components(&bad, i).is_err(), "{i}/{section}/{field}");
            }
        }
        let raw = std::fs::read(fixture().join(format!("{}-capture.json", proof::CAPTURES[i].0))).unwrap();
        let mut altered = raw.clone();
        altered.push(b' ');
        assert!(proof::verify_capture(&altered, i).is_err());
        assert!(proof::verify_capture(&raw, 1 - i).is_err());
    }
}
#[test]
fn erc20tokenx_compiler_whole_artifacts_and_source_maps_cannot_be_replaced() {
    let c = capture(0);
    let out = proof::verify_compiled(&c, &compiled()).unwrap();
    for field in ["abi", "metadata", "storageLayout", "devdoc", "userdoc"] {
        let mut bad = out.clone();
        bad["contracts"][proof::SOURCE][proof::NAME][field] = json!("changed");
        assert!(proof::check_compiled(&c, &serde_json::to_vec(&bad).unwrap()).is_err(), "{field}");
    }
    for kind in ["bytecode", "deployedBytecode"] {
        for field in ["object", "sourceMap", "linkReferences", "generatedSources"] {
            let mut bad = out.clone();
            bad["contracts"][proof::SOURCE][proof::NAME]["evm"][kind][field] = json!("changed");
            assert!(proof::check_compiled(&c, &serde_json::to_vec(&bad).unwrap()).is_err(), "{kind}/{field}");
        }
    }
    let mut altered = compiled();
    altered.push(b' ');
    assert!(proof::verify_compiled(&c, &altered).is_err());
}
#[test]
fn erc20tokenx_four_primary_bodies_and_one_custom_gap_are_explicit() {
    let c = capture(0);
    let p: Value = serde_json::from_slice(&std::fs::read(fixture().join("primary-sources.json")).unwrap()).unwrap();
    proof::verify_primary(&c, &p).unwrap();
    for i in 0..5 {
        for field in ["path", "capture_sha256", "url", "classification", "content", "sha256"] {
            let mut bad = p.clone();
            bad["sources"][i][field] = json!("changed");
            assert!(proof::verify_primary(&c, &bad).is_err(), "{i}/{field}");
        }
    }
    let mut bad = p.clone();
    bad["sources"][1] = bad["sources"][0].clone();
    assert!(proof::verify_primary(&c, &bad).is_err());
    let mut bad = p;
    bad["source_gap"] = json!("fully qualified");
    assert!(proof::verify_primary(&c, &bad).is_err());
}
#[test]
fn erc20tokenx_phi_source_gap_and_saved_runtime_identity_cannot_be_rewritten() {
    let temp = tempfile::tempdir().unwrap();
    let c = capture(0);
    let runtime = proof::runtime(&c).unwrap();
    let names = ["phi-source-request.json", "phi-runtime.hex", "phi-runtime-report.json"];
    for n in names {
        std::fs::copy(fixture().join(n), temp.path().join(n)).unwrap();
    }
    proof::verify_phi(temp.path(), &runtime).unwrap();
    for n in names {
        let old = std::fs::read(temp.path().join(n)).unwrap();
        std::fs::write(temp.path().join(n), b"{}").unwrap();
        assert!(proof::verify_phi(temp.path(), &runtime).is_err());
        std::fs::write(temp.path().join(n), old).unwrap();
    }
    let mut bad = runtime;
    bad[0] ^= 1;
    assert!(proof::verify_phi(temp.path(), &bad).is_err());
}
#[test]
fn erc20tokenx_compiler_settings_manifest_version_and_license_are_bound() {
    let temp = tempfile::tempdir().unwrap();
    let c = capture(0);
    let names = [
        "compiler-input-original.json",
        "compiler-input.json",
        "solc-list.json",
        "compiler-version.txt",
        "LICENSE-openzeppelin",
    ];
    for n in names {
        std::fs::copy(fixture().join(n), temp.path().join(n)).unwrap();
    }
    proof::verify_auxiliary(temp.path(), &c).unwrap();
    for n in names {
        let old = std::fs::read(temp.path().join(n)).unwrap();
        std::fs::write(temp.path().join(n), b"{}").unwrap();
        assert!(proof::verify_auxiliary(temp.path(), &c).is_err());
        std::fs::write(temp.path().join(n), old).unwrap();
    }
}
#[test]
fn erc20tokenx_bad_capture_cli_preserves_failure_and_as_run_sources() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("ori-capture.json"), b"{}").unwrap();
    let out = temp.path().join("attempt");
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_execute_erc20tokenx_proof"))
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
    assert_eq!(
        std::fs::read(out.join("as-run/ptoken_proof_vm.rs")).unwrap(),
        include_bytes!("../src/ptoken_proof/vm.rs")
    );
}
#[test]
fn erc20tokenx_compiler_combined_role_struct_has_no_boolean_or_reachable_admin_writer() {
    let c = capture(0);
    let types = c["storageLayout"]["types"].as_object().unwrap();
    for (label, members) in [
        ("struct AccessControl.RoleData", vec![("members", "0"), ("adminRole", "2")]),
        ("struct EnumerableSet.Set", vec![("_values", "0"), ("_indexes", "1")]),
        ("struct EnumerableSet.AddressSet", vec![("_inner", "0")]),
    ] {
        let found: Vec<_> = types.values().filter(|t| t["label"] == label).collect();
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0]["members"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| (v["label"].as_str().unwrap(), v["slot"].as_str().unwrap()))
                .collect::<Vec<_>>(),
            members
        );
        assert!(found[0]["members"].as_array().unwrap().iter().all(|v| v["offset"] == 0));
    }
    // This exact source set is already byte-bound. The complete manual review
    // establishes reachability; the literal count only guards that evidence.
    let declarations: usize = c["sources"]
        .as_object()
        .unwrap()
        .values()
        .map(|v| v["content"].as_str().unwrap().matches("_setRoleAdmin(").count())
        .sum();
    assert_eq!(declarations, 1);
    assert!(!c["abi"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["type"] == "function" && v["name"] == "setRoleAdmin"));
}
