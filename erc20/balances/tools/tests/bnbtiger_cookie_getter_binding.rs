#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::bnbtiger_cookie_proof as p;
use serde_json::{json, Value};
use std::{fs, path::PathBuf};
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/bnbtiger-cookie-getter-proof")
}
fn compiled(t: p::Target) -> Vec<u8> {
    fs::read(fixture().join(format!("{}-compiler-output.json", t.label()))).unwrap()
}
#[test]
fn complete_original_capture_and_fresh_compilers_bind_both_targets() {
    for t in p::Target::ALL {
        let c = p::verify_capture(t.captured(), t).unwrap();
        let o = p::verify_compiled(&c, &compiled(t), t).unwrap();
        assert_eq!(p::selected(&o, t)["metadata"], p::selected(&c["stdJsonOutput"], t)["metadata"]);
        for (f, k, creation) in [("runtimeBytecode", "deployedBytecode", false), ("creationBytecode", "bytecode", true)] {
            let code = p::bytes(&p::selected(&o, t)["evm"][k]["object"]).unwrap();
            assert_eq!(p::reconstruct(&code, t, creation).unwrap(), p::bytes(&c[f]["onchainBytecode"]).unwrap());
        }
        assert_eq!(p::kh(&p::runtime(&c).unwrap()), t.runtime_hash());
    }
}
#[test]
fn exact_capture_refuses_identity_layout_transformation_and_source_tampering() {
    for t in p::Target::ALL {
        let c = p::verify_capture(t.captured(), t).unwrap();
        let mut mutations = vec![];
        for f in ["address", "chainId", "match", "runtimeMatch", "creationMatch"] {
            let mut v = c.clone();
            v[f] = json!("wrong");
            mutations.push(v);
        }
        for pointer in [
            "/runtimeBytecode/transformations",
            "/creationBytecode/transformationValues",
            "/runtimeBytecode/immutableReferences",
            "/runtimeBytecode/linkReferences",
            "/creationBytecode/sourceMap",
            "/storageLayout/storage/0/slot",
            "/compilation/compilerSettings",
            "/deployment/blockNumber",
        ] {
            let mut v = c.clone();
            *v.pointer_mut(pointer).unwrap() = json!("wrong");
            mutations.push(v);
        }
        let mut v = c.clone();
        v["sources"][t.source()]["content"] = json!("changed");
        mutations.push(v);
        for v in mutations {
            assert!(p::verify_components(&v, t).is_err());
            assert!(p::verify_capture(&serde_json::to_vec(&v).unwrap(), t).is_err());
        }
        let mut raw = t.captured().to_vec();
        raw.push(b'\n');
        assert!(p::verify_capture(&raw, t).is_err());
    }
}
#[test]
fn compiler_bindings_refuse_semantic_and_serialization_drift() {
    for t in p::Target::ALL {
        let c = p::verify_capture(t.captured(), t).unwrap();
        let raw = compiled(t);
        let o: Value = serde_json::from_slice(&raw).unwrap();
        let mut trailing = raw.clone();
        trailing.push(b' ');
        assert!(p::verify_compiled(&c, &trailing, t).is_err());
        for field in ["abi", "metadata", "storageLayout", "devdoc", "userdoc"] {
            let mut v = o.clone();
            v["contracts"][t.source()][t.name()][field] = json!("changed");
            assert!(p::check_compiled(&c, &serde_json::to_vec(&v).unwrap(), t).is_err());
        }
        for kind in ["bytecode", "deployedBytecode"] {
            for field in ["object", "sourceMap", "linkReferences"] {
                let mut v = o.clone();
                v["contracts"][t.source()][t.name()]["evm"][kind][field] = json!("changed");
                assert!(p::check_compiled(&c, &serde_json::to_vec(&v).unwrap(), t).is_err());
            }
        }
        let mut v = o.clone();
        let m = p::selected(&v, t)["metadata"].as_str().unwrap().to_owned();
        v["contracts"][t.source()][t.name()]["metadata"] = json!(format!(" {m}"));
        assert!(p::check_compiled(&c, &serde_json::to_vec(&v).unwrap(), t).is_err());
    }
}
#[test]
fn immutable_and_cbor_reconstruction_is_exact_and_never_strips_bytes() {
    for t in p::Target::ALL {
        let c = p::verify_capture(t.captured(), t).unwrap();
        for (f, creation) in [("runtimeBytecode", false), ("creationBytecode", true)] {
            let original = p::bytes(&c[f]["recompiledBytecode"]).unwrap();
            for size in [original.len() - 1, original.len() + 1] {
                let mut code = original.clone();
                code.resize(size, 0);
                assert!(p::reconstruct(&code, t, creation).is_err());
            }
            let mut code = original.clone();
            let last = code.len() - 1;
            code[last] ^= 1;
            assert!(p::reconstruct(&code, t, creation).is_err());
            if t == p::Target::Bnbtiger && !creation {
                for off in [1347, 3534] {
                    let mut code = original.clone();
                    code[off] = 1;
                    assert!(p::reconstruct(&code, t, creation).is_err());
                }
            }
        }
    }
}
#[test]
fn output_selection_is_the_only_added_input_field() {
    for t in p::Target::ALL {
        let c = p::verify_capture(t.captured(), t).unwrap();
        let mut input = p::input(&c).unwrap();
        assert!(input["settings"].as_object_mut().unwrap().remove("outputSelection").is_some());
        assert_eq!(input, c["stdJsonInput"]);
        assert_eq!(c["sources"], input["sources"]);
    }
}
#[test]
fn writer_review_keeps_constructor_only_and_reachable_metadata_distinct() {
    for t in p::Target::ALL {
        let c = p::verify_capture(t.captured(), t).unwrap();
        let review = p::writers::review(&c, t).unwrap();
        assert_eq!(review["fields"].as_array().unwrap().len(), if t == p::Target::Bnbtiger { 36 } else { 23 });
        for r in review["fields"].as_array().unwrap() {
            assert!(!r["anchors"].as_array().unwrap().is_empty());
            let label = r["declaration"]["label"].as_str().unwrap();
            if ["_name", "_symbol", "_decimals"].contains(&label) || label == "maxHoldingRate" || (t == p::Target::Bnbtiger && label == "_totalSupply") {
                assert_eq!(r["classification"], "constructor_only");
            }
            for a in r["anchors"].as_array().unwrap() {
                let body = c["sources"][a["path"].as_str().unwrap()]["content"].as_str().unwrap();
                let start = a["byte_start"].as_u64().unwrap() as usize;
                let len = a["byte_length"].as_u64().unwrap() as usize;
                assert_eq!(&body[start..start + len], a["literal"].as_str().unwrap());
                assert_eq!(p::sha(body.as_bytes()), a["source_sha256"]);
            }
        }
    }
}

#[test]
fn primary_sources_and_licenses_preserve_exact_files_and_origin_gaps() {
    for t in p::Target::ALL {
        let c = p::verify_capture(t.captured(), t).unwrap();
        let raw = fs::read(fixture().join(format!("{}-primary-sources.json", t.label()))).unwrap();
        let primary: Value = serde_json::from_slice(&raw).unwrap();
        p::verify_primary(&c, &primary, t).unwrap();
        for field in ["path", "capture_sha256", "url", "classification", "primary_sha256", "content"] {
            let mut wrong = primary.clone();
            wrong["sources"][0][field] = json!("changed");
            assert!(p::verify_primary(&c, &wrong, t).is_err());
        }
        let mut missing = primary.clone();
        missing["sources"].as_array_mut().unwrap().pop();
        assert!(p::verify_primary(&c, &missing, t).is_err());
        if t == p::Target::Cookie {
            for field in ["url", "primary_sha256", "content"] {
                let mut wrong = primary.clone();
                wrong["sources"][3][field] = json!("changed");
                assert!(p::verify_primary(&c, &wrong, t).is_err());
            }
        }
        let notices: Value = serde_json::from_slice(&fs::read(fixture().join(format!("{}-source-licenses.json", t.label()))).unwrap()).unwrap();
        assert_eq!(notices, p::source_licenses(&c).unwrap());
    }
    for (name, _, _, hash) in p::LICENSES {
        assert_eq!(p::sha(&fs::read(fixture().join(format!("LICENSE-{name}"))).unwrap()), hash);
    }
    let manifest = fs::read(fixture().join("solc-list.json")).unwrap();
    for t in p::Target::ALL {
        p::verify_manifest(&manifest, t).unwrap();
        let mut wrong = manifest.clone();
        wrong.push(b' ');
        assert!(p::verify_manifest(&wrong, t).is_err());
    }
}
#[test]
fn compiler_report_rejects_failed_stale_partial_and_tampered_provenance() {
    let inventory = b"exact test source inventory";
    let digest = p::sha(inventory);
    let artifacts = json!([{"file":"bound-artifact","sha256":"expected"}]);
    let licenses=json!(p::LICENSES.into_iter().map(|(name,repo,pin,hash)|json!({"file":format!("LICENSE-{name}"),"url":format!("https://raw.githubusercontent.com/{repo}/{pin}/LICENSE"),"sha256":hash})).collect::<Vec<_>>());
    // Isolated report validator: the executor separately verifies every real source,
    // compiler binary, body and output before any measured calls can be accepted.
    let good = json!({"status":"passed","qualified":false,"scope":p::LIMITS,"source_gap":p::PRIMARY_GAP,"chain_calls":0,"source_inventory_sha256":digest,"artifacts":artifacts,"targets":p::source_targets(),"licenses":licenses});
    p::validate_source_report(&good, &artifacts, inventory, &digest).unwrap();
    for (field, value) in [
        ("status", json!("failed")),
        ("qualified", json!(true)),
        ("scope", json!("broader")),
        ("source_inventory_sha256", json!("stale")),
        ("artifacts", json!([])),
        ("targets", json!([])),
        ("licenses", json!([])),
    ] {
        let mut bad = good.clone();
        bad[field] = value;
        assert!(p::validate_source_report(&bad, &artifacts, inventory, &digest).is_err());
    }
    assert!(p::validate_source_report(&good, &artifacts, b"changed inventory", &digest).is_err());
    assert!(p::validate_source_report(&good, &artifacts, inventory, "different current executable").is_err());
    let mut bad = good.clone();
    bad["artifacts"][0]["sha256"] = json!("tampered");
    assert!(p::validate_source_report(&bad, &artifacts, inventory, &digest).is_err());
    let mut bad = good;
    bad["targets"][0]["runtime_keccak256"] = json!("wrong runtime");
    assert!(p::validate_source_report(&bad, &artifacts, inventory, &digest).is_err());
}
