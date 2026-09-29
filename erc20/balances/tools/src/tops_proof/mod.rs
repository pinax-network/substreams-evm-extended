//! Capture-bound TOPS host proof. No ingestion candidate or deployment qualification.
pub mod cases;
pub use crate::ptoken_proof::{bytes, kh, sha, source_map, vm};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
pub const CONTRACT: &str = "0xcdf52c0b13c24f32f1d8d4ec6356203a1ef0826a";
pub const SOURCE: &str = "contracts/v1/token.sol";
pub const NAME: &str = "Token";
pub const CAPTURE_SHA: &str = "bed5d7155189f849140fce44c4971a526a139b40b1ac4cc7d83db48b057a4c1a";
pub const SOURCE_SHA: &str = "d8f25657cce824872f05cb94b8173c1ce33895fbb7ee46bb981879938851f916";
pub const RUNTIME: &str = "0xf4dc8abcac62d4707f5a839731c0e2b3dde17c643accf43299541c421ba0050b";
pub const OZ_PIN: &str = "56a3de2cea907c9a500d32e70c275f68393b7ba6";
pub use crate::ptoken_proof::{SOLC_PIN, SOLC_SHA, SOLC_VERSION};
pub const FIXTURE: &str = "erc20/balances/tests/fixtures/tops-operation-proof";
pub const PRIMARY_GAP: &str = "No exact independent public maintainer source pin was recovered for contracts/v1/token.sol (SPDX Unlicensed). Eight dependency bodies match the pinned OpenZeppelin v5.6.0 commit. Complete saved capture/runtime reconstruction does not establish current deployment or external dependencies.";
pub const LIMITS: &str = "Synthetic local full-runtime append/getters and separately compiled exact-source cleanup semantics only. No external-call emulation; original transfer and constructor remain unsupported. Cleanup harness store order is not deployed-transfer order. No production validator, candidate, package, holder or live qualification.";
pub const HARNESS_SOURCE: &str = "TOPSCleanup.sol";
pub const HARNESS_NAME: &str = "TOPSCleanup";

pub fn verify_capture(raw: &[u8]) -> Result<Value> {
    ensure!(sha(raw) == CAPTURE_SHA, "complete capture SHA before parsing");
    let v: Value = serde_json::from_slice(raw)?;
    verify_components(&v)?;
    Ok(v)
}
pub fn verify_components(v: &Value) -> Result<()> {
    ensure!(v["chainId"] == "56" && bytes(&v["address"])? == hex::decode(&CONTRACT[2..])?, "chain/address");
    for key in ["match", "runtimeMatch", "creationMatch"] {
        ensure!(v[key] == "match", "original match labels");
    }
    ensure!(v["proxyResolution"]["isProxy"] == false, "direct contract");
    ensure!(
        v["compilation"]["compilerVersion"] == SOLC_VERSION && v["metadata"]["compiler"]["version"] == SOLC_VERSION,
        "compiler version"
    );
    ensure!(v["compilation"]["fullyQualifiedName"] == format!("{SOURCE}:{NAME}"), "target");
    let settings: Value = serde_json::from_str(include_str!("../../../tests/fixtures/tops-operation-proof/settings.json"))?;
    ensure!(
        v["stdJsonInput"]["settings"] == settings && v["compilation"]["compilerSettings"] == settings,
        "exact full compiler options"
    );
    ensure!(
        v["stdJsonInput"]["language"] == "Solidity" && v["stdJsonInput"]["sources"] == v["sources"] && v["sourceIds"] == v["stdJsonOutput"]["sources"],
        "input/source IDs"
    );
    let sources = v["sources"].as_object().context("sources")?;
    ensure!(
        sources.len() == 9 && sources.keys().eq(v["metadata"]["sources"].as_object().context("metadata sources")?.keys()),
        "complete sources"
    );
    for (name, source) in sources {
        let content = source["content"].as_str().context("content")?;
        ensure!(kh(content.as_bytes()) == v["metadata"]["sources"][name]["keccak256"], "source hash {name}");
        ensure!(name == SOURCE || name.starts_with("@openzeppelin/contracts/"), "unexpected source");
    }
    ensure!(
        sha(sources[SOURCE]["content"].as_str().context("token source")?.as_bytes()) == SOURCE_SHA,
        "custom source pin"
    );
    let target = &v["stdJsonOutput"]["contracts"][SOURCE][NAME];
    ensure!(
        serde_json::from_str::<Value>(target["metadata"].as_str().context("metadata")?)? == v["metadata"],
        "saved metadata"
    );
    for field in ["abi", "storageLayout", "transientStorageLayout"] {
        ensure!(target[field] == v[field], "saved {field}");
    }
    for (field, kind, len) in [("creationBytecode", "bytecode", 26671), ("runtimeBytecode", "deployedBytecode", 22323)] {
        let original = bytes(&v[field]["recompiledBytecode"])?;
        ensure!(
            original.len() == len && original.ends_with(&hex::decode("a164736f6c634300081c000a")?),
            "size/compiler CBOR"
        );
        ensure!(
            bytes(&target["evm"][kind]["object"])? == original && target["evm"][kind]["sourceMap"] == v[field]["sourceMap"],
            "saved compiler bytecode/map"
        );
        ensure!(
            v[field]["linkReferences"] == json!({}) && v[field]["cborAuxdata"] == json!({}),
            "no link/CBOR replacements"
        );
    }
    ensure!(
        v["creationBytecode"]["transformations"] == json!([]) && v["creationBytecode"]["transformationValues"] == json!({}),
        "no creation transformations"
    );
    ensure!(
        v["creationBytecode"]["onchainBytecode"] == v["creationBytecode"]["recompiledBytecode"],
        "whole creation identity, no arguments"
    );
    let patched = patch_runtime(v)?;
    ensure!(
        patched == bytes(&v["runtimeBytecode"]["onchainBytecode"])? && kh(&patched) == RUNTIME,
        "exact reconstructed runtime"
    );
    verify_layout(&v["storageLayout"], false)?;
    Ok(())
}
pub fn patch_runtime(v: &Value) -> Result<Vec<u8>> {
    let r = &v["runtimeBytecode"];
    let references = r["immutableReferences"].as_object().context("immutables")?;
    let ids = [
        "4240", "4243", "4245", "4324", "4327", "4330", "4333", "4336", "4339", "4342", "4345", "4348", "4351",
    ];
    ensure!(references.keys().map(String::as_str).eq(ids), "exact immutable IDs");
    ensure!(
        r["transformationValues"]["immutables"]
            .as_object()
            .context("values")?
            .keys()
            .eq(references.keys()),
        "exact immutable values"
    );
    ensure!(
        v["stdJsonOutput"]["contracts"][SOURCE][NAME]["evm"]["deployedBytecode"]["immutableReferences"] == r["immutableReferences"],
        "saved compiler references"
    );
    let mut out = bytes(&r["recompiledBytecode"])?;
    let mut sites = std::collections::BTreeSet::new();
    let mut transforms = vec![];
    for (i, id) in ids.iter().enumerate() {
        let value = bytes(&r["transformationValues"]["immutables"][id])?;
        let expected = match i {
            0 => hex::decode("55d398326f99059ff775485246999027b3197955")?,
            1 => hex::decode("10ed43c718714eb63d5aa57b78b54704e256024e")?,
            // External factory result is capture-bound, not derived or newly qualified.
            2 => hex::decode("8ec426ffb466990ac36048ef5c90348783ea4272")?,
            _ => {
                let encoded = [vec![0xd6, 0x94], hex::decode(&CONTRACT[2..])?, vec![(i - 2) as u8]].concat();
                erc20_balances::hash(&encoded)[12..].to_vec()
            }
        };
        ensure!(value == [vec![0; 12], expected].concat(), "literal/CREATE/captured pair immutable {id}");
        for item in references[*id].as_array().context("sites")? {
            let offset = item["start"].as_u64().context("offset")? as usize;
            ensure!(
                item["length"] == 32 && offset.checked_add(32).is_some_and(|n| n <= out.len()),
                "immutable width/bounds"
            );
            for n in offset..offset + 32 {
                ensure!(sites.insert(n), "overlapping immutables");
            }
            ensure!(out[offset..offset + 32] == [0; 32], "zero compiler placeholder");
            out[offset..offset + 32].copy_from_slice(&value);
            transforms.push(json!({"id":id,"type":"replace","offset":offset,"reason":"immutable"}));
        }
    }
    ensure!(
        sites.len() == 91 * 32 && r["transformations"] == json!(transforms),
        "exact 91 transformation sites/order"
    );
    Ok(out)
}
pub fn verify_layout(layout: &Value, harness: bool) -> Result<()> {
    let fields = layout["storage"].as_array().context("storage")?;
    for (label, slot) in [("lpInfos", "31"), ("lpAmount", "32")] {
        let rows: Vec<_> = fields.iter().filter(|x| x["label"] == label).collect();
        ensure!(rows.len() == 1 && rows[0]["slot"] == slot && rows[0]["offset"] == 0, "exact {label} root");
        let ty = &layout["types"][rows[0]["type"].as_str().context("type")?];
        ensure!(ty["encoding"] == "mapping" && ty["key"] == "t_address", "address mapping");
        if label == "lpInfos" {
            let array = &layout["types"][ty["value"].as_str().context("array")?];
            ensure!(array["encoding"] == "dynamic_array", "dynamic array");
            let record = &layout["types"][array["base"].as_str().context("record")?];
            ensure!(record["numberOfBytes"] == "96", "three-word stride");
            let members = record["members"].as_array().context("members")?;
            ensure!(members.len() == 3, "three fields");
            for (i, name) in ["lpAmount", "createdAt", "expireAt"].iter().enumerate() {
                ensure!(
                    members[i]["label"] == *name
                        && members[i]["slot"].as_str() == Some(["0", "1", "2"][i])
                        && members[i]["offset"] == 0
                        && members[i]["type"] == "t_uint256",
                    "exact struct members"
                );
            }
        } else {
            ensure!(ty["value"] == "t_uint256", "credit width");
        }
    }
    if harness {
        ensure!(
            fields.len() == 3
                && fields[0]["label"] == "reserved"
                && fields[0]["slot"] == "0"
                && layout["types"][fields[0]["type"].as_str().context("padding type")?]["numberOfBytes"] == "992",
            "exact untouched 31-word prefix"
        );
    }
    Ok(())
}
/// Braces are balanced in these pinned bodies; no brace-bearing strings/comments occur.
/// Exact capture hash and exact body SHA records make extraction mechanical and reviewable.
pub fn extract(source: &str, marker: &str) -> Result<String> {
    ensure!(source.matches(marker).count() == 1, "unique extraction marker {marker}");
    let start = source.find(marker).context("marker")?;
    let open = source[start..].find('{').context("body")? + start;
    let mut depth = 0;
    for (i, b) in source.as_bytes().iter().enumerate().skip(open) {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(source[start..=i].into());
                }
            }
            _ => {}
        }
    }
    anyhow::bail!("unterminated source body")
}
pub fn harness(capture: &Value) -> Result<(String, Value)> {
    let source = capture["sources"][SOURCE]["content"].as_str().context("source")?;
    ensure!(sha(source.as_bytes()) == SOURCE_SHA, "source extraction pin");
    let mut out = String::from("// SPDX-License-Identifier: Unlicensed\npragma solidity ^0.8.19;\n// Exact captured bodies; synthetic storage and entrypoint. Not the deployed runtime.\ncontract TOPSCleanup {\nuint256[31] private reserved;\n");
    let mut records = vec![];
    for marker in [
        "struct LPInfo",
        "function _processExpiredLPInfo(",
        "function getWithdrawableLPAmount(",
        "function getUserLPDetails(",
    ] {
        let body = extract(source, marker)?;
        records.push(json!({"marker":marker,"start":source.find(marker),"bytes":body.len(),"sha256":sha(body.as_bytes())}));
        out.push_str(&body);
        out.push('\n');
        if marker == "struct LPInfo" {
            out.push_str("mapping(address => LPInfo[]) public lpInfos;\nmapping(address => uint256) public lpAmount;\n");
        }
    }
    out.push_str("function process(address user) external { _processExpiredLPInfo(user); }\n}\n");
    Ok((
        out,
        json!({"original_source_sha256":SOURCE_SHA,"extractions":records,"changes":"Only 31-word padding, exact mapping declarations and external process wrapper; extracted bodies unmodified","scope":"source semantics only; no deployed runtime/order/transfer qualification"}),
    ))
}
pub fn compiler_input(capture: &Value) -> Result<Value> {
    let mut input = capture["stdJsonInput"].clone();
    ensure!(input["settings"].get("outputSelection").is_none(), "original outputSelection absent");
    input["settings"]["outputSelection"] = json!({"*":{"*":["abi","metadata","storageLayout","transientStorageLayout","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers","ir","irOptimized"],"":["ast"]}});
    Ok(input)
}
pub fn check_compiled(capture: &Value, raw: &[u8]) -> Result<Value> {
    let output: Value = serde_json::from_slice(raw)?;
    ensure!(
        !output["errors"].as_array().into_iter().flatten().any(|e| e["severity"] == "error"),
        "Solidity compilation errors"
    );
    let t = &output["contracts"][SOURCE][NAME];
    for (kind, field) in [("bytecode", "creationBytecode"), ("deployedBytecode", "runtimeBytecode")] {
        ensure!(
            bytes(&t["evm"][kind]["object"])? == bytes(&capture[field]["recompiledBytecode"])?,
            "fresh complete {kind} identity"
        );
        ensure!(t["evm"][kind]["sourceMap"] == capture[field]["sourceMap"], "fresh {kind} source map");
    }
    for field in ["abi", "storageLayout", "transientStorageLayout"] {
        ensure!(t[field] == capture[field], "fresh {field}");
    }
    ensure!(
        serde_json::from_str::<Value>(t["metadata"].as_str().context("metadata")?)? == capture["metadata"],
        "complete parsed metadata equality"
    );
    ensure!(
        t["evm"]["deployedBytecode"]["immutableReferences"] == capture["runtimeBytecode"]["immutableReferences"],
        "fresh immutables"
    );
    for name in capture["sources"].as_object().context("sources")?.keys() {
        ensure!(output["sources"][name]["ast"].is_object(), "fresh AST {name}");
    }
    let expected_names = [
        ("4240", "usdt"),
        ("4243", "_swapRouter"),
        ("4245", "mainPair"),
        ("4324", "mainDistributor"),
        ("4327", "lpDistributor"),
        ("4330", "silverDistributor"),
        ("4333", "goldDistributor"),
        ("4336", "diamondDistributor"),
        ("4339", "marketDistributor"),
        ("4342", "treasuryDistributor"),
        ("4345", "levelDistributor"),
        ("4348", "tokenDistributor"),
        ("4351", "liquidityProvider"),
    ];
    fn find(ast: &Value, id: u64) -> Option<&Value> {
        match ast {
            Value::Object(fields) => {
                if ast["id"] == id {
                    Some(ast)
                } else {
                    fields.values().find_map(|v| find(v, id))
                }
            }
            Value::Array(items) => items.iter().find_map(|v| find(v, id)),
            _ => None,
        }
    }
    for (id, name) in expected_names {
        let node = find(&output["sources"][SOURCE]["ast"], id.parse()?).context("immutable AST ID")?;
        ensure!(
            node["nodeType"] == "VariableDeclaration" && node["name"] == name && node["mutability"] == "immutable",
            "immutable AST declaration {id}/{name}"
        );
    }
    ensure!(
        t["ir"].as_str().is_some_and(|s| !s.is_empty()) && t["irOptimized"].as_str().is_some_and(|s| !s.is_empty()),
        "explicit full Yul/optimized Yul outputs"
    );
    Ok(output)
}
pub fn mapped_sources(input: &Value, compiled: &Value, source: &str, name: &str, kind: &str) -> Result<BTreeMap<i64, (String, String)>> {
    let mut result = BTreeMap::new();
    for (path, info) in compiled["sources"].as_object().context("source IDs")? {
        ensure!(
            result
                .insert(
                    info["id"].as_i64().context("ID")?,
                    (path.clone(), input["sources"][path]["content"].as_str().context("body")?.into())
                )
                .is_none(),
            "duplicate source ID"
        );
    }
    for s in compiled["contracts"][source][name]["evm"][kind]["generatedSources"]
        .as_array()
        .context("generated sources")?
    {
        ensure!(
            result
                .insert(
                    s["id"].as_i64().context("generated ID")?,
                    (s["name"].as_str().context("name")?.into(), s["contents"].as_str().context("body")?.into())
                )
                .is_none(),
            "duplicate generated ID"
        );
    }
    Ok(result)
}
pub fn snapshot_sources(out: &std::path::Path) -> Result<String> {
    use std::{fs, path::Path};
    let mut files = serde_json::Map::new();
    fn walk(p: &Path, files: &mut serde_json::Map<String, Value>) -> Result<()> {
        if p.is_dir() {
            for e in fs::read_dir(p)? {
                walk(&e?.path(), files)?;
            }
        } else if p.extension().is_some_and(|s| s == "rs" || s == "toml" || s == "lock") {
            let raw = fs::read(p)?;
            files.insert(p.to_string_lossy().into(), json!({"sha256":sha(&raw),"bytes":raw.len()}));
        }
        Ok(())
    }
    for p in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "erc20/balances/Cargo.toml",
        "erc20/balances/src",
        "erc20/balances/tools/Cargo.toml",
        "erc20/balances/tools/src",
        "erc20/balances/tools/tests",
        "proto/Cargo.toml",
        "proto/src",
        "common/retention/Cargo.toml",
        "common/retention/src",
        "common/persist/Cargo.toml",
        "common/persist/src",
    ] {
        walk(Path::new(p), &mut files)?;
    }
    for p in [
        "erc20/balances/tests/fixtures/tops-operation-proof/settings.json",
        "docs/follow-up.md",
        "docs/handoff.md",
        "docs/research/README.md",
        "erc20/balances/docs/role-shape-audit.md",
    ] {
        let raw = fs::read(p)?;
        files.insert(p.into(), json!({"sha256":sha(&raw),"bytes":raw.len()}));
    }
    fs::create_dir(out.join("as-run"))?;
    for (name, relative, text) in [
        ("binding.rs", "mod.rs", include_str!("mod.rs")),
        ("builder.rs", "../bin/build_tops_proof.rs", include_str!("../bin/build_tops_proof.rs")),
        ("executor.rs", "../bin/execute_tops_proof.rs", include_str!("../bin/execute_tops_proof.rs")),
        ("cases.rs", "cases.rs", include_str!("cases.rs")),
        ("vm.rs", "../ptoken_proof/vm.rs", include_str!("../ptoken_proof/vm.rs")),
        ("vm_tests.rs", "../ptoken_proof/vm_tests.rs", include_str!("../ptoken_proof/vm_tests.rs")),
        ("source_map.rs", "../ptoken_proof/source_map.rs", include_str!("../ptoken_proof/source_map.rs")),
    ] {
        ensure!(
            fs::read(Path::new("erc20/balances/tools/src/tops_proof").join(relative))? == text.as_bytes(),
            "stale executable {relative}"
        );
        fs::write(out.join("as-run").join(name), text)?;
    }
    for entry in fs::read_dir(FIXTURE)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_file() && p.file_name().is_none_or(|s| s != "README.md") {
            let raw = fs::read(&p)?;
            files.insert(p.to_string_lossy().into(), json!({"sha256":sha(&raw),"bytes":raw.len()}));
        }
    }
    let historical = "erc20/balances/tests/fixtures/bsc-exclusions-20260928/cases.json";
    let raw = fs::read(historical)?;
    files.insert(historical.into(), json!({"sha256":sha(&raw),"bytes":raw.len()}));
    let raw = serde_json::to_vec_pretty(&json!({"scope":"TOPS host proof only","files":files}))?;
    fs::write(out.join("source-inventory.json"), &raw)?;
    Ok(sha(&raw))
}

pub const COMPILED_SHA: &str = "653ee0d4f9ac33899d6bcc828f44e18eba517e02882bf09b39114fd3b0d73f7d";
pub const HARNESS_COMPILED_SHA: &str = "d250f4f254f7bb50f8c1fb25995e6ba9d154278f510b1ffbfb7858d339b58956";
pub fn verify_compiled(capture: &Value, raw: &[u8]) -> Result<Value> {
    ensure!(sha(raw) == COMPILED_SHA, "complete fresh compiler output pin including AST/Yul");
    check_compiled(capture, raw)
}
pub fn verify_source_directory(path: &std::path::Path) -> Result<(Value, Value, Value, Value)> {
    use std::fs;
    let capture = verify_capture(&fs::read(path.join("capture.json"))?)?;
    let compiled = verify_compiled(&capture, &fs::read(path.join("full-output.json"))?)?;
    let raw = fs::read(path.join("harness-output.json"))?;
    ensure!(sha(&raw) == HARNESS_COMPILED_SHA, "complete cleanup compiler output");
    let hout: Value = serde_json::from_slice(&raw)?;
    verify_layout(&hout["contracts"][HARNESS_SOURCE][HARNESS_NAME]["storageLayout"], true)?;
    let input = compiler_input(&capture)?;
    ensure!(
        serde_json::from_slice::<Value>(&fs::read(path.join("full-input.json"))?)? == input,
        "exact full input"
    );
    ensure!(
        serde_json::from_slice::<Value>(&fs::read(path.join("original-input.json"))?)? == capture["stdJsonInput"],
        "original input"
    );
    let (hsource, extraction) = harness(&capture)?;
    ensure!(fs::read(path.join(HARNESS_SOURCE))? == hsource.as_bytes(), "exact extracted harness");
    ensure!(
        serde_json::from_slice::<Value>(&fs::read(path.join("harness-extraction.json"))?)? == extraction,
        "exact extraction manifest"
    );
    let mut hinput = input.clone();
    hinput["sources"] = json!({HARNESS_SOURCE:{"content":hsource}});
    ensure!(
        serde_json::from_slice::<Value>(&fs::read(path.join("harness-input.json"))?)? == hinput,
        "harness input"
    );
    for (name, digest) in [
        ("execution-spec-block.py", TIMESTAMP_SPEC_SHA),
        ("LICENSE-execution-specs", TIMESTAMP_LICENSE_SHA),
        ("primary-sources.json", "0e5c56672de6090bf1bb2d404a7d59151b2e2690c236f88aad1dd6c48176340e"),
        ("solc-list.json", "22e8ba1c7c8d0fc5eb60964083b238d267ea2c1afec9757521fea20c45b206af"),
        ("compiler-version.txt", "df9e0e873736454fea76f295ae7f597b21a06ef9db8e44f20465ec7e2522521a"),
        ("LICENSE-OpenZeppelin", "20aebc68b11c063133aa2af0ef4bb29875477c6d16d715718f0daec563938b84"),
        ("metadata-original.json", "86dc8f7a5c56839386bcf34de679f0c6a0cf938ad2c611f6436724a7bd200cda"),
        ("metadata-fresh.json", "86dc8f7a5c56839386bcf34de679f0c6a0cf938ad2c611f6436724a7bd200cda"),
    ] {
        ensure!(sha(&fs::read(path.join(name))?) == digest, "auxiliary artifact {name}");
    }
    let primary: Value = serde_json::from_slice(&fs::read(path.join("primary-sources.json"))?)?;
    ensure!(
        primary["custom_gap"] == PRIMARY_GAP && primary["sources"].as_array().context("primary sources")?.len() == 8,
        "complete primary/gap"
    );
    for s in primary["sources"].as_array().unwrap() {
        let name = s["path"].as_str().context("path")?;
        let official = name.strip_prefix("@openzeppelin/").context("prefix")?;
        let content = capture["sources"][name]["content"].as_str().context("source")?;
        ensure!(
            s["content"] == content
                && s["sha256"] == sha(content.as_bytes())
                && s["status"] == "exact"
                && s["url"] == format!("https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{OZ_PIN}/{official}"),
            "exact primary body/URL"
        );
    }
    Ok((capture, compiled, hinput, hout))
}

pub const TIMESTAMP_SPEC_PIN: &str = "3e170675290673a68eeb4652501fb2ae74fea0cb";
pub const TIMESTAMP_SPEC_SHA: &str = "299ca6ed3ad79e3a3b2c8992744e84bb70c781beb1af0804a7dc4bfdab1e9801";
pub const TIMESTAMP_LICENSE_SHA: &str = "f81577cb97c59ed748f9117fa839b80e3be0ec605a4b66c234e07468bc011808";
