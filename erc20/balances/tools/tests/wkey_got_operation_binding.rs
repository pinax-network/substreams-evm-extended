#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::wkey_got_proof as p;
use serde_json::{json, Value};
use std::{fs, path::PathBuf};
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/wkey-got-operation-proof")
}
fn capture(t: p::Target) -> Value {
    p::verify_capture(t.captured(), t).unwrap()
}
fn compiled(t: p::Target) -> Vec<u8> {
    fs::read(fixture().join(t.label()).join("compiler-output.json")).unwrap()
}
#[test]
fn complete_two_runtime_bindings_are_independent() {
    let targets = p::verify_source_directory(&fixture()).unwrap();
    assert_eq!(targets.len(), 2);
    for (t, c, out) in targets {
        assert_eq!(p::runtime(&c).unwrap().len(), t.runtime_len());
        assert_eq!(p::kh(&p::runtime(&c).unwrap()), t.runtime_hash());
        assert_eq!(p::sha(&compiled(t)), t.compiled_sha());
        assert_eq!(
            out["contracts"][p::SOURCE][t.name()]["metadata"],
            c["stdJsonOutput"]["contracts"][p::SOURCE][t.name()]["metadata"]
        );
    }
    assert_ne!(p::Target::Wkeydao.compiled_sha(), p::Target::Got.compiled_sha());
    assert_eq!(p::Target::Wkeydao.role_root(), 9.into());
    assert_eq!(p::Target::Got.role_root(), 8.into());
}
#[test]
fn capture_pin_precedes_parsing_and_refuses_relabelled_records() {
    assert!(p::verify_capture(b"{}", p::Target::Wkeydao).is_err());
    assert!(p::verify_capture(p::Target::Got.captured(), p::Target::Wkeydao).is_err());
    for t in p::Target::ALL {
        let c = capture(t);
        for key in ["address", "chainId", "runtimeMatch", "creationMatch"] {
            let mut v = c.clone();
            v[key] = json!("changed");
            assert!(p::verify_components(&v, t).is_err(), "{key}");
        }
        let mut v = c.clone();
        v["sources"] = json!({});
        assert!(p::verify_components(&v, t).is_err());
        let mut v = c.clone();
        v["runtimeBytecode"]["transformations"] = json!([{"offset":0}]);
        assert!(p::verify_components(&v, t).is_err());
        let mut v = c;
        v["creationBytecode"]["transformationValues"]["constructorArguments"] = json!("0x");
        assert!(p::verify_components(&v, t).is_err());
    }
}
#[test]
fn whole_compiler_pin_and_component_checks_reject_mutations() {
    for t in p::Target::ALL {
        let c = capture(t);
        let raw = compiled(t);
        let out: Value = serde_json::from_slice(&raw).unwrap();
        let mut corrupted = raw.clone();
        corrupted.push(b' ');
        assert!(p::verify_compiled(&c, &corrupted, t).is_err());
        for path in ["abi", "metadata", "storageLayout", "devdoc"] {
            let mut v = out.clone();
            v["contracts"][p::SOURCE][t.name()][path] = json!(null);
            assert!(p::check_compiled(&c, &serde_json::to_vec(&v).unwrap(), t).is_err(), "{path}");
        }
        for kind in ["bytecode", "deployedBytecode"] {
            for field in ["object", "sourceMap", "linkReferences"] {
                let mut v = out.clone();
                v["contracts"][p::SOURCE][t.name()]["evm"][kind][field] = json!("changed");
                assert!(p::check_compiled(&c, &serde_json::to_vec(&v).unwrap(), t).is_err(), "{kind} {field}");
            }
        }
        let mut v = out;
        v["contracts"][p::SOURCE][t.name()]["evm"]["deployedBytecode"]["generatedSources"] = json!([{"id":99}]);
        assert!(p::check_compiled(&c, &serde_json::to_vec(&v).unwrap(), t).is_err());
    }
}
#[test]
fn compiler_settings_are_preserved_except_declared_outputs() {
    for t in p::Target::ALL {
        let c = capture(t);
        let mut input = p::input(&c).unwrap();
        assert!(input["settings"]["outputSelection"].is_object());
        input["settings"].as_object_mut().unwrap().remove("outputSelection");
        assert_eq!(input, c["stdJsonInput"]);
        assert!(input["settings"].get("evmVersion").is_none());
    }
}
#[test]
fn complete_primary_artifacts_pin_body_url_set_and_custom_gap() {
    for t in p::Target::ALL {
        let c = capture(t);
        let primary: Value = serde_json::from_slice(&fs::read(fixture().join(t.label()).join("primary-sources.json")).unwrap()).unwrap();
        p::verify_primary(&c, &primary, t).unwrap();
        for field in ["content", "sha256", "url", "path"] {
            let mut v = primary.clone();
            v["sources"][0][field] = json!("changed");
            assert!(p::verify_primary(&c, &v, t).is_err());
        }
        let mut v = primary.clone();
        v["sources"].as_array_mut().unwrap().pop();
        assert!(p::verify_primary(&c, &v, t).is_err());
        let mut v = primary.clone();
        v["source_gap"] = json!("qualified");
        assert!(p::verify_primary(&c, &v, t).is_err());
        let mut v = primary;
        let last = v["sources"].as_array().unwrap().len() - 1;
        v["sources"][last]["classification"] = json!("exact");
        assert!(p::verify_primary(&c, &v, t).is_err());
    }
}
#[test]
fn full_layout_preserves_cap_nonce_domain_and_combined_role_fields() {
    for t in p::Target::ALL {
        let c = capture(t);
        let layout = &c["storageLayout"];
        p::verify_layout(layout, t).unwrap();
        for (i, label) in [(6, "MaxSupply"), (7, "_nonces")] {
            assert_eq!(layout["storage"][i]["label"], label);
            let mut v = layout.clone();
            v["storage"][i]["slot"] = json!("99");
            assert!(p::verify_layout(&v, t).is_err());
        }
        let mut v = layout.clone();
        v["storage"][t.role_root().as_usize()]["offset"] = json!(1);
        assert!(p::verify_layout(&v, t).is_err());
        let mut v = layout.clone();
        v["storage"].as_array_mut().unwrap().pop();
        assert!(p::verify_layout(&v, t).is_err());
    }
}
#[test]
fn malformed_source_directory_fails_and_cli_keeps_failure_report() {
    let dir = std::env::temp_dir().join(format!("wkey-got-proof-binding-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    assert!(p::verify_source_directory(&dir).is_err());
    let out = dir.join("attempt");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_execute_wkey_got_proof"))
        .args([dir.as_os_str(), out.as_os_str()])
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.."))
        .status()
        .unwrap();
    assert!(!status.success());
    let raw = fs::read(out.join("report.json")).unwrap();
    let report: Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(report["status"], "failed");
    assert_eq!(report["attempted_calls"], 0);
    assert_eq!(report["qualified"], false);
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_execute_wkey_got_proof"))
        .args([dir.as_os_str(), out.as_os_str()])
        .status()
        .unwrap();
    assert!(!status.success());
    assert_eq!(raw, fs::read(out.join("report.json")).unwrap());
    fs::remove_dir_all(dir).unwrap();
}
