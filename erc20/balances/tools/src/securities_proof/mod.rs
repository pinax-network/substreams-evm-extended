//! Exact selected SecuritiesToken host proof, never ingestion admission.
pub mod cases;
pub use crate::ptoken_proof::{bytes, kh, sha, source_map, vm};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
pub const CONTRACT: &str = "0xcfed6c4679297ea4889f8183bc057b4a86c64e46";
pub const SOURCE: &str = "src/SecuritiesToken.sol";
pub const NAME: &str = "SecuritiesToken";
pub const CAPTURE_SHA: &str = "fd44489fbc3e55ef5669b0a90246f32cd8b248079aa1cd58c5af2288a27daef2";
pub const RUNTIME_HASH: &str = "0x060dc28d4dd8d9bb8a381d4009bccbac129ce743a10b3d8aa0ffef5034b50544";
pub const COMPILED_SHA: &str = "0b44f0b9acc0d1f33aa3bc5af37e6d12bf64ed46909568ae73b4fd4d3cf4b739";
pub const SAVED_METADATA_SHA: &str = "44f85de3053c3bcbe52f78c3cbd7ed520c5ec965fd74d4ab642d88894848f3b6";
pub const FRESH_METADATA_SHA: &str = "c49c374e44cc250b2d11f362d3b48c5687ba46114d8fd2d2374d91b01189db9b";
pub const SOLC_PIN: &str = "16a99b8c26ed33a91796e209ff6797ee7baf2b0d";
pub const SOLC_SHA: &str = "cc2d44c706905ccc382f484625dff61d741e0c24232d226f139a6835fc644f3f";
pub const SOLC_KECCAK: &str = "0xf8c5313dfe054e7f3c208905ee6bb4097a1c7a1dd360050f90fab4b182ac1c24";
pub const SOLC_VERSION: &str = "0.8.24+commit.e11b9ed9";
pub const MANIFEST_SHA: &str = "22e8ba1c7c8d0fc5eb60964083b238d267ea2c1afec9757521fea20c45b206af";
pub const OZ_PIN: &str = "e4f70216d759d8e6a64144a9e1f7bbeed78e7079";
pub const OZ_UPGRADEABLE_PIN: &str = "60b305a8f3ff0c7688f02ac470417b6bbf1c4d27";
pub const BEP_PIN: &str = "ff17399a4723e2e344f7d4c52a3eecf2ca007f6b";
pub const FIXTURE: &str = "erc20/balances/tests/fixtures/securities-operation-proof";
pub const PRIMARY_GAP: &str = "Eight captured custom sources have no exact independent primary body: three BEP-677 counterparts differ, and five SecuritiesToken/compliance/pause sources have no recovered public revision. The full original captured source is compiled without normalization or substitution. These gaps, null on-chain creation/deployment evidence, and proxy/beacon history remain unqualified.";
pub const LICENSES: [(&str, &str, &str); 3] = [
    ("openzeppelin", "OpenZeppelin/openzeppelin-contracts", OZ_PIN),
    (
        "openzeppelin-upgradeable",
        "OpenZeppelin/openzeppelin-contracts-upgradeable",
        OZ_UPGRADEABLE_PIN,
    ),
    ("bep677", "bnb-chain/bep-677-contracts", BEP_PIN),
];
pub fn license_sha(label: &str) -> Result<&'static str> {
    match label {
        "openzeppelin" | "openzeppelin-upgradeable" => Ok("13cd784a6c31361f0e0c6aa3b410a1cb9a079868b7314c13e3eb8a75351746b9"),
        "bep677" => Ok("57f37895dd6b66624e7fbb9219bff3ca29ebe39ecd40eaad9c92b8266c00bb21"),
        _ => anyhow::bail!("unknown license"),
    }
}
pub fn verify_auxiliary(path: &std::path::Path, capture: &Value) -> Result<()> {
    use std::fs;
    let original: Value = serde_json::from_slice(&fs::read(path.join("compiler-input-original.json"))?)?;
    let mut input: Value = serde_json::from_slice(&fs::read(path.join("compiler-input.json"))?)?;
    ensure!(original == capture["stdJsonInput"], "preserved original input");
    ensure!(
        input["settings"]["outputSelection"]
            == json!({"*":{"*":["abi","metadata","devdoc","userdoc","storageLayout","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}}),
        "exact output selection"
    );
    input["settings"].as_object_mut().context("input settings")?.remove("outputSelection");
    ensure!(input == original, "no source/settings substitution");
    ensure!(sha(&fs::read(path.join("solc-list.json"))?) == MANIFEST_SHA, "complete official manifest pin");
    let version = fs::read_to_string(path.join("compiler-version.txt"))?;
    ensure!(
        version == "solc, the solidity compiler commandline interface\nVersion: 0.8.24+commit.e11b9ed9.Darwin.appleclang\n",
        "exact compiler version response"
    );
    for (label, _, _) in LICENSES {
        ensure!(
            sha(&fs::read(path.join(format!("LICENSE-{label}")))?) == license_sha(label)?,
            "exact notice {label}"
        );
    }
    Ok(())
}
pub fn verify_metadata_files(path: &std::path::Path) -> Result<()> {
    for (name, digest) in [("metadata-saved.json", SAVED_METADATA_SHA), ("metadata-fresh.json", FRESH_METADATA_SHA)] {
        ensure!(sha(&std::fs::read(path.join(name))?) == digest, "exact preserved {name}");
    }
    Ok(())
}
pub fn runtime(v: &Value) -> Result<Vec<u8>> {
    bytes(&v["runtimeBytecode"]["onchainBytecode"])
}
pub fn settings() -> Value {
    json!({"evmVersion":"cancun","metadata":{"appendCBOR":true,"bytecodeHash":"ipfs","useLiteralContent":false},"optimizer":{"enabled":true,"runs":200},"viaIR":false,"remappings":["@openzeppelin/contracts/=lib/openzeppelin-contracts/contracts/","@openzeppelin/contracts-upgradeable/=lib/openzeppelin-contracts-upgradeable/contracts/","forge-std/=lib/forge-std/src/","erc4626-tests/=lib/openzeppelin-contracts-upgradeable/lib/erc4626-tests/","openzeppelin-contracts-upgradeable/=lib/openzeppelin-contracts-upgradeable/contracts/","openzeppelin-contracts/=lib/openzeppelin-contracts/contracts/"]})
}
pub fn verify_capture(raw: &[u8]) -> Result<Value> {
    ensure!(sha(raw) == CAPTURE_SHA, "complete original capture digest");
    let v = serde_json::from_slice(raw)?;
    verify_components(&v)?;
    Ok(v)
}
pub fn verify_components(v: &Value) -> Result<()> {
    ensure!(v["chainId"] == "56" && bytes(&v["address"])? == hex::decode(&CONTRACT[2..])?, "network/address");
    ensure!(
        v["match"] == "exact_match" && v["runtimeMatch"] == "exact_match" && v["creationMatch"].is_null(),
        "original match labels including absent creation"
    );
    ensure!(
        v["deployment"] == json!({"blockNumber":null,"deployer":null,"transactionHash":null,"transactionIndex":null}),
        "deployment evidence absent"
    );
    ensure!(
        v["compilation"]["compilerVersion"] == SOLC_VERSION && v["metadata"]["compiler"]["version"] == SOLC_VERSION,
        "compiler version"
    );
    ensure!(
        v["compilation"]["compiler"] == "solc"
            && v["compilation"]["language"] == "Solidity"
            && v["compilation"]["name"] == NAME
            && v["compilation"]["fullyQualifiedName"] == format!("{SOURCE}:{NAME}"),
        "selected target"
    );
    ensure!(
        v["compilation"]["compilerSettings"] == settings() && v["stdJsonInput"]["settings"] == settings() && v["stdJsonInput"]["language"] == "Solidity",
        "exact settings/remappings"
    );
    ensure!(
        v["sources"] == v["stdJsonInput"]["sources"] && v["sourceIds"] == v["stdJsonOutput"]["sources"],
        "source input/IDs"
    );
    let sources = v["sources"].as_object().context("sources")?;
    ensure!(
        sources.len() == 31
            && sources.keys().eq(v["metadata"]["sources"].as_object().context("metadata sources")?.keys())
            && sources.keys().eq(v["sourceIds"].as_object().context("IDs")?.keys()),
        "all 31 source identities"
    );
    let mut ids = std::collections::BTreeSet::new();
    for (name, s) in sources {
        ensure!(
            v["metadata"]["sources"][name]["keccak256"] == kh(s["content"].as_str().context("body")?.as_bytes()),
            "metadata source {name}"
        );
        ensure!(ids.insert(v["sourceIds"][name]["id"].as_u64().context("source ID")?), "unique source IDs");
        primary_spec(name)?;
    }
    ensure!(ids == (0..31).collect(), "exact ID domain");
    let o = &v["stdJsonOutput"]["contracts"][SOURCE][NAME];
    ensure!(
        sha(o["metadata"].as_str().context("saved raw metadata")?.as_bytes()) == SAVED_METADATA_SHA,
        "exact saved metadata serialization"
    );
    ensure!(
        serde_json::from_str::<Value>(o["metadata"].as_str().context("metadata")?)? == v["metadata"],
        "complete saved metadata"
    );
    for field in ["abi", "devdoc", "userdoc", "storageLayout", "transientStorageLayout"] {
        ensure!(o[field] == v[field], "saved {field}");
    }
    let cbor = "a2646970667358221220bf80be0ec28927e6d12033fbdcc2eced0b69a60176cedbee9f222e3e6fbe997164736f6c63430008180033";
    for (field, kind, size, off, digest) in [
        (
            "runtimeBytecode",
            "deployedBytecode",
            10836,
            10783,
            "814fc45a704716dbda5b91791b69b7780924f66ae604606d670bded136bc339c",
        ),
        (
            "creationBytecode",
            "bytecode",
            11063,
            11010,
            "4b08d81f0f7817dd62eace6b371f254d5fbdf5970214d6997694018fa042f7ec",
        ),
    ] {
        let b = &v[field];
        let code = bytes(&b["recompiledBytecode"])?;
        ensure!(
            code.len() == size && sha(&code) == digest && code == bytes(&o["evm"][kind]["object"])? && b["sourceMap"] == o["evm"][kind]["sourceMap"],
            "saved whole compiler code/map"
        );
        ensure!(b["linkReferences"] == json!({}) && o["evm"][kind]["linkReferences"] == json!({}), "no links");
        ensure!(
            b["cborAuxdata"] == json!({"1":{"value":format!("0x{cbor}"),"offset":off}}) && hex::encode(&code[off..]) == cbor,
            "exact unmodified metadata trailer"
        );
    }
    let r = &v["runtimeBytecode"];
    ensure!(
        r["immutableReferences"] == json!({})
            && o["evm"]["deployedBytecode"]["immutableReferences"] == json!({})
            && r["transformations"] == json!([])
            && r["transformationValues"] == json!({}),
        "no runtime transformations"
    );
    ensure!(
        runtime(v)? == bytes(&r["recompiledBytecode"])? && kh(&runtime(v)?) == RUNTIME_HASH,
        "full exact saved runtime"
    );
    let c = &v["creationBytecode"];
    ensure!(
        c["onchainBytecode"].is_null() && c["transformations"].is_null() && c["transformationValues"].is_null(),
        "creation absence must not become equality"
    );
    let constructors: Vec<_> = v["abi"].as_array().context("ABI")?.iter().filter(|a| a["type"] == "constructor").collect();
    ensure!(
        constructors.len() == 1 && constructors[0]["inputs"] == json!([]),
        "argument-free synthetic constructor"
    );
    verify_layout(v)
}
pub fn verify_layout(v: &Value) -> Result<()> {
    let s = v["storageLayout"]["storage"].as_array().context("layout")?;
    for (label, slot, offset) in [
        ("mintEnabled", "150", 0),
        ("burnEnabled", "150", 1),
        ("_nameOverride", "151", 0),
        ("_symbolOverride", "152", 0),
        ("identifier", "153", 0),
    ] {
        let found: Vec<_> = s.iter().filter(|x| x["label"] == label).collect();
        ensure!(
            found.len() == 1 && found[0]["slot"] == slot && found[0]["offset"] == offset,
            "exact custom layout {label}"
        );
    }
    // ERC-7201 structs are not entries in Solidity's linear storage layout.
    // Their constants/bodies remain bound by all-source capture and fresh code.
    Ok(())
}
/// URL and exact/near/gap status are selected by the captured source identity.
pub fn primary_spec(path: &str) -> Result<(Option<String>, &'static str, Option<&'static str>)> {
    for (prefix, repo, pin) in [
        ("lib/openzeppelin-contracts/contracts/", "OpenZeppelin/openzeppelin-contracts", OZ_PIN),
        (
            "lib/openzeppelin-contracts-upgradeable/contracts/",
            "OpenZeppelin/openzeppelin-contracts-upgradeable",
            OZ_UPGRADEABLE_PIN,
        ),
    ] {
        if let Some(tail) = path.strip_prefix(prefix) {
            return Ok((Some(format!("https://raw.githubusercontent.com/{repo}/{pin}/contracts/{tail}")), "exact", None));
        }
    }
    if let Some(tail) = path.strip_prefix("src/scaledUIToken/ERC8056/") {
        let near = match tail {
            "ERC8056BaseUpgradeable.sol" => Some("831a80db44c2b8d53b28d2f6d34c616db16b6fcbb5ff3b8d40187882fab2570b"),
            "IERC8056Scheduled.sol" => Some("667d6abcefb583e40678c15d7dbf1bba857e80071bf05867bd053800f16394e0"),
            "IScaledUIAmount.sol" => Some("e9f7de093a02cec46520f281eda90e6551c9ced19f37cbbc624e4d0363c01d2f"),
            "IScaledUIAmountBalances.sol" | "IScaledUIAmountConversion.sol" | "IScaledUIAmountNewUIMultiplier.sol" => None,
            _ => anyhow::bail!("unknown custom source"),
        };
        return Ok((
            Some(format!(
                "https://raw.githubusercontent.com/bnb-chain/bep-677-contracts/{BEP_PIN}/contracts/{tail}"
            )),
            if near.is_some() { "near_not_exact" } else { "exact" },
            near,
        ));
    }
    ensure!(
        [
            SOURCE,
            "src/scaledUIToken/compliance/ComplianceClientUpgradeable.sol",
            "src/scaledUIToken/compliance/ICompliance.sol",
            "src/scaledUIToken/pauseManager/PauseManagerClientUpgradeable.sol",
            "src/scaledUIToken/pauseManager/IPauseManager.sol"
        ]
        .contains(&path),
        "known unresolved custom source"
    );
    Ok((None, "no_recovered_primary", None))
}
pub fn verify_primary(capture: &Value, primary: &Value) -> Result<()> {
    ensure!(primary["source_gap"] == PRIMARY_GAP, "explicit primary gap");
    let rows = primary["sources"].as_array().context("primary inventory")?;
    ensure!(rows.len() == 31, "complete primary classifications");
    let mut seen = std::collections::BTreeSet::new();
    let mut counts = [0usize; 3];
    for row in rows {
        let name = row["path"].as_str().context("primary path")?;
        ensure!(seen.insert(name), "unique primary path");
        let body = capture["sources"][name]["content"].as_str().context("captured primary source")?;
        ensure!(row["capture_sha256"] == sha(body.as_bytes()), "captured body hash");
        let (url, status, near) = primary_spec(name)?;
        ensure!(row["url"] == json!(url) && row["classification"] == status, "immutable primary identity/status");
        if let Some(want) = near {
            let found = row["content"].as_str().context("near primary body")?;
            ensure!(
                sha(found.as_bytes()) == want && row["sha256"] == want && found != body,
                "preserved exact mismatch"
            );
            counts[1] += 1;
        } else if url.is_some() {
            ensure!(row["content"] == body && row["sha256"] == sha(body.as_bytes()), "literal primary equality");
            counts[0] += 1;
        } else {
            ensure!(
                row["content"].is_null() && row["sha256"].is_null() && body.contains("SPDX-License-Identifier: BUSL-1.1"),
                "unresolved body/license boundary"
            );
            counts[2] += 1;
        }
    }
    ensure!(
        counts == [23, 3, 5] && seen.len() == capture["sources"].as_object().unwrap().len(),
        "exact/near/gap counts"
    );
    Ok(())
}
pub fn check_compiled(capture: &Value, raw: &[u8]) -> Result<Value> {
    let out: Value = serde_json::from_slice(raw)?;
    ensure!(
        !out["errors"].as_array().into_iter().flatten().any(|e| e["severity"] == "error"),
        "compiler errors"
    );
    ensure!(out["sources"] == capture["sourceIds"], "fresh source identities");
    let a = &out["contracts"][SOURCE][NAME];
    let b = &capture["stdJsonOutput"]["contracts"][SOURCE][NAME];
    for f in ["abi", "devdoc", "userdoc", "storageLayout"] {
        ensure!(a[f] == b[f], "fresh {f}");
    }
    // The capture preserved literal Unicode; official solc serializes escapes.
    // Bind each exact raw string and compare the full parsed object below.
    ensure!(
        sha(a["metadata"].as_str().context("fresh raw metadata")?.as_bytes()) == FRESH_METADATA_SHA
            && sha(b["metadata"].as_str().context("saved raw metadata")?.as_bytes()) == SAVED_METADATA_SHA,
        "exact fresh and saved metadata serializations"
    );
    ensure!(
        serde_json::from_str::<Value>(a["metadata"].as_str().context("fresh metadata")?)? == capture["metadata"],
        "fresh parsed metadata"
    );
    for (field, kind) in [("runtimeBytecode", "deployedBytecode"), ("creationBytecode", "bytecode")] {
        ensure!(
            bytes(&a["evm"][kind]["object"])? == bytes(&capture[field]["recompiledBytecode"])?
                && a["evm"][kind]["sourceMap"] == capture[field]["sourceMap"]
                && a["evm"][kind]["linkReferences"] == json!({}),
            "fresh whole compiler bytes/map/links"
        );
        mapped_sources(capture, &out, kind)?;
    }
    ensure!(a["evm"]["deployedBytecode"]["immutableReferences"] == json!({}), "fresh no immutables");
    Ok(out)
}
pub fn verify_compiled(capture: &Value, raw: &[u8]) -> Result<Value> {
    ensure!(sha(raw) == COMPILED_SHA, "complete fresh compiler-output digest");
    check_compiled(capture, raw)
}
pub fn mapped_sources(capture: &Value, compiled: &Value, kind: &str) -> Result<std::collections::BTreeMap<i64, (String, String)>> {
    let mut result = std::collections::BTreeMap::new();
    for (path, info) in compiled["sources"].as_object().context("source IDs")? {
        ensure!(
            result
                .insert(
                    info["id"].as_i64().context("source ID")?,
                    (path.clone(), capture["sources"][path]["content"].as_str().context("source body")?.into())
                )
                .is_none(),
            "duplicate source ID"
        );
    }
    let generated = compiled["contracts"][SOURCE][NAME]["evm"][kind]["generatedSources"]
        .as_array()
        .context("generated sources")?;
    for g in generated {
        ensure!(
            g["id"] == 31
                && g["language"] == "Yul"
                && result
                    .insert(
                        31,
                        (
                            g["name"].as_str().context("generated name")?.into(),
                            g["contents"].as_str().context("generated body")?.into()
                        )
                    )
                    .is_none(),
            "exact generated source identity"
        );
    }
    ensure!(kind != "deployedBytecode" || generated.len() == 1, "runtime generated source 31");
    Ok(result)
}
pub fn snapshot_sources(out: &std::path::Path) -> Result<String> {
    use std::{fs, path::Path};
    let own = [
        ("mod.rs", include_str!("mod.rs")),
        ("cases.rs", include_str!("cases.rs")),
        (
            "../../tests/securities_operation_binding.rs",
            include_str!("../../tests/securities_operation_binding.rs"),
        ),
        (
            "../../tests/securities_operation_cases.rs",
            include_str!("../../tests/securities_operation_cases.rs"),
        ),
        ("../bin/build_securities_proof.rs", include_str!("../bin/build_securities_proof.rs")),
        ("../bin/execute_securities_proof.rs", include_str!("../bin/execute_securities_proof.rs")),
        ("../ptoken_proof/vm.rs", include_str!("../ptoken_proof/vm.rs")),
        ("../ptoken_proof/vm_tests.rs", include_str!("../ptoken_proof/vm_tests.rs")),
        ("../ptoken_proof/source_map.rs", include_str!("../ptoken_proof/source_map.rs")),
    ];
    fs::create_dir(out.join("as-run"))?;
    for (name, text) in own {
        let path = Path::new("erc20/balances/tools/src/securities_proof").join(name);
        ensure!(fs::read(&path)? == text.as_bytes(), "stale executable source {}", path.display());
        fs::write(out.join("as-run").join(Path::new(name).file_name().unwrap()), text)?;
    }
    let mut files = serde_json::Map::new();
    fn walk(path: &Path, files: &mut serde_json::Map<String, Value>) -> Result<()> {
        if path.is_dir() {
            for e in fs::read_dir(path)? {
                walk(&e?.path(), files)?;
            }
        } else if path.extension().is_some_and(|s| s == "rs" || s == "toml" || s == "lock") {
            let raw = fs::read(path)?;
            files.insert(path.to_string_lossy().into(), json!({"sha256":sha(&raw),"bytes":raw.len()}));
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
        "docs/follow-up.md",
        "docs/handoff.md",
        "docs/research/README.md",
        "erc20/balances/docs/role-shape-audit.md",
    ] {
        let raw = fs::read(p)?;
        files.insert(p.into(), json!({"sha256":sha(&raw),"bytes":raw.len()}));
    }
    let raw = serde_json::to_vec_pretty(&json!({"scope":"SecuritiesToken host proof and local host dependencies; no chain calls","files":files}))?;
    fs::write(out.join("source-inventory.json"), &raw)?;
    Ok(sha(&raw))
}
