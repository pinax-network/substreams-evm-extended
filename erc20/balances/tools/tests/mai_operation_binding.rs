#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::mai_proof as p;
use primitive_types::U256;
use serde_json::{json, Value};
use std::{fs, path::PathBuf};
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/mai-operation-proof")
}
fn capture() -> Value {
    p::verify_capture(p::captured()).unwrap()
}
fn compiled() -> Vec<u8> {
    fs::read(fixture().join("compiler-output.json")).unwrap()
}
#[test]
fn exact_capture_source_compiler_and_runtime_binding() {
    let (c, o) = p::verify_source_directory(&fixture()).unwrap();
    assert_eq!(p::sha(&compiled()), p::COMPILED_SHA);
    assert_eq!(p::runtime(&c).unwrap().len(), 11296);
    assert_eq!(p::kh(&p::runtime(&c).unwrap()), p::RUNTIME_HASH);
    assert_eq!(p::bytes(&c["creationBytecode"]["onchainBytecode"]).unwrap().len(), 12772);
    assert_eq!(p::cap(), U256::exp10(29));
    assert_eq!(
        o["contracts"][p::SOURCE][p::NAME]["metadata"],
        c["stdJsonOutput"]["contracts"][p::SOURCE][p::NAME]["metadata"]
    );
}
#[test]
fn full_capture_pin_rejects_missing_source_and_relabelled_fields() {
    assert!(p::verify_capture(b"{}").is_err());
    let c = capture();
    for path in [
        "/address",
        "/chainId",
        "/runtimeMatch",
        "/creationMatch",
        "/sources/contracts~1Mai.sol/content",
        "/deployment/blockNumber",
        "/runtimeBytecode/transformationValues/immutables/1258",
        "/runtimeBytecode/immutableReferences/1258/0/start",
        "/creationBytecode/transformations",
    ] {
        let mut v = c.clone();
        *v.pointer_mut(path).unwrap() = json!(null);
        assert!(p::verify_components(&v).is_err(), "{path}");
        assert!(p::verify_capture(&serde_json::to_vec(&v).unwrap()).is_err());
    }
}
#[test]
fn cap_reconstruction_is_one_source_derived_word_only() {
    let c = capture();
    let placeholder = p::bytes(&c["runtimeBytecode"]["recompiledBytecode"]).unwrap();
    let mut reconstructed = placeholder.clone();
    assert_eq!(&placeholder[1767..1799], &[0; 32]);
    reconstructed[1767..1799].copy_from_slice(&p::vm::word(U256::exp10(29)));
    assert_eq!(reconstructed, p::runtime(&c).unwrap());
    assert_ne!(p::kh(&placeholder), p::RUNTIME_HASH);
    let mut wrong = reconstructed.clone();
    wrong[1766] ^= 1;
    assert_ne!(p::kh(&wrong), p::RUNTIME_HASH);
    assert_eq!(c["runtimeBytecode"]["immutableReferences"], json!({"1258":[{"start":1767,"length":32}]}));
}
#[test]
fn compiler_output_pin_ast_literal_and_generated_sources_are_bound() {
    let c = capture();
    let raw = compiled();
    let o: Value = serde_json::from_slice(&raw).unwrap();
    let mut v = raw.clone();
    v.push(b' ');
    assert!(p::verify_compiled(&c, &v).is_err());
    for field in ["abi", "metadata", "storageLayout"] {
        let mut v = o.clone();
        v["contracts"][p::SOURCE][p::NAME][field] = json!(null);
        assert!(p::check_compiled(&c, &serde_json::to_vec(&v).unwrap()).is_err());
    }
    for kind in ["bytecode", "deployedBytecode"] {
        for field in ["object", "sourceMap", "linkReferences"] {
            let mut v = o.clone();
            v["contracts"][p::SOURCE][p::NAME]["evm"][kind][field] = json!(null);
            assert!(p::check_compiled(&c, &serde_json::to_vec(&v).unwrap()).is_err());
        }
        let mut v = o.clone();
        v["contracts"][p::SOURCE][p::NAME]["evm"][kind]["generatedSources"][0]["contents"] = json!("changed");
        assert!(p::verify_compiled(&c, &serde_json::to_vec(&v).unwrap()).is_err());
    }
    for source in [p::SOURCE, "@openzeppelin/contracts/token/ERC20/extensions/ERC20Capped.sol"] {
        let mut v = o.clone();
        v["sources"][source]["ast"] = json!({});
        assert!(p::check_compiled(&c, &serde_json::to_vec(&v).unwrap()).is_err());
    }
}
#[test]
fn compiler_settings_are_unchanged_except_explicit_outputs() {
    let c = capture();
    let mut v = p::input(&c).unwrap();
    v["settings"].as_object_mut().unwrap().remove("outputSelection");
    assert_eq!(v, c["stdJsonInput"]);
    assert_eq!(v["settings"]["optimizer"], json!({"enabled":false,"runs":200}));
    assert!(v["settings"].get("evmVersion").is_none());
}
#[test]
fn dependency_inventory_requires_every_exact_body_url_and_custom_gap() {
    let c = capture();
    let p0: Value = serde_json::from_slice(&fs::read(fixture().join("primary-sources.json")).unwrap()).unwrap();
    p::verify_primary(&c, &p0).unwrap();
    assert_eq!(p0["sources"].as_array().unwrap().iter().filter(|v| v["classification"] == "exact").count(), 14);
    for field in ["content", "sha256", "url", "path"] {
        let mut v = p0.clone();
        v["sources"][0][field] = json!("changed");
        assert!(p::verify_primary(&c, &v).is_err());
    }
    let mut v = p0.clone();
    v["sources"].as_array_mut().unwrap().pop();
    assert!(p::verify_primary(&c, &v).is_err());
    let mut v = p0.clone();
    v["sources"][14]["classification"] = json!("exact");
    assert!(p::verify_primary(&c, &v).is_err());
    let mut v = p0;
    v["source_gap"] = json!("qualified");
    assert!(p::verify_primary(&c, &v).is_err());
}
#[test]
fn membership_set_and_balance_layouts_remain_distinct() {
    let c = capture();
    for i in 0..7 {
        let mut v = c["storageLayout"].clone();
        v["storage"][i]["slot"] = json!("99");
        assert!(p::verify_layout(&v).is_err());
    }
    let mut v = c["storageLayout"].clone();
    v["storage"].as_array_mut().unwrap().pop();
    assert!(p::verify_layout(&v).is_err());
    let mut v = c["storageLayout"].clone();
    v["types"]["t_struct(RoleData)19_storage"]["members"][1]["slot"] = json!("2");
    assert!(p::verify_layout(&v).is_err());
}
#[test]
fn failed_cli_preserves_report_and_refuses_output_reuse() {
    let dir = std::env::temp_dir().join(format!("mai-proof-binding-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    assert!(p::verify_source_directory(&dir).is_err());
    let out = dir.join("attempt");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_execute_mai_proof"))
        .args([dir.as_os_str(), out.as_os_str()])
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.."))
        .status()
        .unwrap();
    assert!(!status.success());
    let raw = fs::read(out.join("report.json")).unwrap();
    let report: Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(report["status"], "failed");
    assert_eq!(report["qualified"], false);
    assert_eq!(report["attempted_calls"], 0);
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_execute_mai_proof"))
        .args([dir.as_os_str(), out.as_os_str()])
        .status()
        .unwrap();
    assert!(!status.success());
    assert_eq!(raw, fs::read(out.join("report.json")).unwrap());
    fs::remove_dir_all(dir).unwrap();
}
