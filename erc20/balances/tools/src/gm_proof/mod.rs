//! Capture-bound GM local operation proof. No ingestion or deployment admission.
pub mod cases;
pub use crate::ptoken_proof::{bytes, kh, sha, source_map, vm};
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde_json::{json, Value};
pub const IMPLEMENTATION: &str = "0x578f397ca4661d1db4d9a65065d6b284a1a850fd";
pub const PROXY: &str = "0x9b8e987e6fec8cf1380c4dca7071e2c7853aeea1";
pub const PROXY_B: &str = "0xa9ee28c80f960b889dfbd1902055218cba016f75";
pub const SOURCE: &str = "contracts/globalMarkets/GMToken.sol";
pub const NAME: &str = "GMToken";
pub const SOLC_PIN: &str = "16a99b8c26ed33a91796e209ff6797ee7baf2b0d";
pub const SOLC_VERSION: &str = "0.8.16+commit.07a7930e";
pub const SOLC_SHA: &str = "7d471cb9bae9a7f29c7ebf402f7e16fa8226b17ba9ab68a88ce107114479dc4d";
pub const SOLC_KECCAK: &str = "0xf3dd3636e7bca007430e091bcacc2dd9b9af72edb838a0e0d77de9610169f7c3";
pub const PRIMARY_GAP: &str = "Five BUSL-1.1 custom GM sources have no recovered exact independent public primary. The contextual rwa-contracts snapshot differs. Fifteen exact Ondo-vendored MIT dependencies are not asserted to be an exact upstream OZ release; seven unique proxy dependencies match the pinned upstream OZ snapshot.";
pub const HISTORY_LIMIT: &str = "All three captures preserve match/runtimeMatch=match and null creationMatch/onchain creation/creation transformations. Exact CBOR-only runtime reconstruction is not whole compiler/captured byte equality or deployment qualification. Synthetic compiler creation returns compiler runtime, not the differing captured metadata. Beacon dispatch and external compliance/pause calls are not executed successfully.";
pub const FIXTURE: &str = "erc20/balances/tests/fixtures/gm-operation-proof";
pub const CUSTOM_PIN: &str = "046c5c58a2d2202e70d7c2c4f13f414f15de47f4";
#[derive(Clone, Copy)]
pub struct Capture {
    pub label: &'static str,
    pub address: &'static str,
    pub sha: &'static str,
    pub source: &'static str,
    pub name: &'static str,
    pub files: usize,
    pub runtime_bytes: usize,
    pub creation_bytes: usize,
    pub runtime_hash: &'static str,
    pub repo: &'static str,
    pub pin: &'static str,
    pub prefix: &'static str,
    pub old_digest: &'static str,
    pub new_digest: &'static str,
    pub compiled_sha: &'static str,
}
const PROXY_SOURCE: &str = "lib/openzeppelin-contracts/contracts/proxy/beacon/BeaconProxy.sol";
pub const CAPTURES: [Capture; 3] = [
    Capture {
        label: "implementation",
        address: IMPLEMENTATION,
        sha: "efecac4287c144c4ecdb00b2b614c327d001885f844dc4de5cd8577fea189de9",
        source: SOURCE,
        name: NAME,
        files: 20,
        runtime_bytes: 8271,
        creation_bytes: 8509,
        runtime_hash: "0x85d44c72e84a34f0b4a3bceca35289b10bd0c5510c5a441466a755e903edc36a",
        repo: "ondoprotocol/usdy",
        pin: "3912ca0698c2992e4db997d0855e62588c44e2c0",
        prefix: "contracts/external/openzeppelin/contracts-upgradeable/",
        old_digest: "df561304cbde14e11b56159b096eda9b0185e26670a0716b47b4a2f4fb25822d",
        new_digest: "3e5e4ba8bedc87e31107179160bb56f66f6455be28687114808cd197813a0399",
        compiled_sha: "efd8c1ea4803647451d7d7fdddd96614049bc26c35573d4811aac2745eca12bc",
    },
    Capture {
        label: "proxy-a",
        address: PROXY,
        sha: "a6d9e367268a5b32ff4ae287f31a55fde22474ffae66038034ad9d249e2fc3cd",
        source: PROXY_SOURCE,
        name: "BeaconProxy",
        files: 7,
        runtime_bytes: 824,
        creation_bytes: 2278,
        runtime_hash: "0x439923c85f956f038ae77871736f789e6d08d22257b6e5fdccfdc83924ecb4d0",
        repo: "OpenZeppelin/openzeppelin-contracts",
        pin: "ecd2ca2cd7cac116f7a37d0e474bbb3d7d5e1c4d",
        prefix: "lib/openzeppelin-contracts/",
        old_digest: "336251e4715d5f1284837460a5ba2459d14f1a57e2c584d411482a947e0d0ab4",
        new_digest: "7453d1b1eb7138522fe386df966e4953f05991bde0fb72ec6e91df1b77506a44",
        compiled_sha: "f7e663f2aef9c04ce10a3aa7cc681a5874f3a66100c85328d3e8b7fa45cd984f",
    },
    Capture {
        label: "proxy-b",
        address: PROXY_B,
        sha: "344951ae7b4b549511d73030672316a17f4bf311972d83fde72a05d821726cfe",
        source: PROXY_SOURCE,
        name: "BeaconProxy",
        files: 7,
        runtime_bytes: 824,
        creation_bytes: 2278,
        runtime_hash: "0x439923c85f956f038ae77871736f789e6d08d22257b6e5fdccfdc83924ecb4d0",
        repo: "OpenZeppelin/openzeppelin-contracts",
        pin: "ecd2ca2cd7cac116f7a37d0e474bbb3d7d5e1c4d",
        prefix: "lib/openzeppelin-contracts/",
        old_digest: "336251e4715d5f1284837460a5ba2459d14f1a57e2c584d411482a947e0d0ab4",
        new_digest: "7453d1b1eb7138522fe386df966e4953f05991bde0fb72ec6e91df1b77506a44",
        compiled_sha: "f7e663f2aef9c04ce10a3aa7cc681a5874f3a66100c85328d3e8b7fa45cd984f",
    },
];
pub fn address(s: &str) -> U256 {
    U256::from_big_endian(&hex::decode(&s[2..]).unwrap())
}
pub fn runtime(v: &Value) -> Result<Vec<u8>> {
    bytes(&v["runtimeBytecode"]["onchainBytecode"])
}
pub fn custom(name: &str) -> bool {
    name.starts_with("contracts/globalMarkets/")
}
pub fn primary_url(p: &Capture, name: &str) -> Result<String> {
    let path = if p.label == "implementation" {
        ensure!(name.starts_with(p.prefix), "exact vendor path");
        name
    } else {
        name.strip_prefix(p.prefix).context("exact proxy prefix")?
    };
    Ok(format!("https://raw.githubusercontent.com/{}/{}/{path}", p.repo, p.pin))
}
pub fn custom_url(name: &str) -> String {
    format!("https://raw.githubusercontent.com/ondoprotocol/rwa-contracts/{CUSTOM_PIN}/{name}")
}
pub fn verify_capture(raw: &[u8], p: &Capture) -> Result<Value> {
    ensure!(sha(raw) == p.sha, "complete original {} capture digest", p.label);
    let v: Value = serde_json::from_slice(raw)?;
    verify_components(&v, p)?;
    Ok(v)
}
pub fn verify_components(v: &Value, p: &Capture) -> Result<()> {
    ensure!(
        v["chainId"] == "56" && bytes(&v["address"])? == hex::decode(&p.address[2..])?,
        "network/address"
    );
    ensure!(
        v["match"] == "match" && v["runtimeMatch"] == "match" && v["creationMatch"].is_null(),
        "original labels/null creation match"
    );
    ensure!(
        v["compilation"]["compilerVersion"] == SOLC_VERSION && v["metadata"]["compiler"]["version"] == SOLC_VERSION,
        "compiler identity"
    );
    ensure!(
        v["compilation"]["language"] == "Solidity"
            && v["compilation"]["compiler"] == "solc"
            && v["compilation"]["name"] == p.name
            && v["compilation"]["fullyQualifiedName"] == format!("{}:{}", p.source, p.name),
        "target identity"
    );
    let settings: Value = serde_json::from_str(if p.label == "implementation" {
        include_str!("../../../tests/fixtures/gm-operation-proof/implementation-settings.json")
    } else {
        include_str!("../../../tests/fixtures/gm-operation-proof/proxy-settings.json")
    })?;
    ensure!(
        v["compilation"]["compilerSettings"] == settings && v["stdJsonInput"]["settings"] == settings && v["stdJsonInput"]["language"] == "Solidity",
        "exact original settings"
    );
    ensure!(
        v["sources"] == v["stdJsonInput"]["sources"] && v["sourceIds"] == v["stdJsonOutput"]["sources"],
        "source input/IDs"
    );
    let sources = v["sources"].as_object().context("sources")?;
    ensure!(
        sources.len() == p.files
            && sources.keys().eq(v["metadata"]["sources"].as_object().context("metadata sources")?.keys())
            && sources.keys().eq(v["sourceIds"].as_object().context("source IDs")?.keys()),
        "complete source sets"
    );
    for (name, s) in sources {
        let content = s["content"].as_str().context("content")?;
        ensure!(v["metadata"]["sources"][name]["keccak256"] == kh(content.as_bytes()), "source Keccak {name}");
        let license = if custom(name) { "BUSL-1.1" } else { "MIT" };
        ensure!(
            v["metadata"]["sources"][name]["license"] == license
                && content
                    .lines()
                    .next()
                    .is_some_and(|l| l.contains(&format!("SPDX-License-Identifier: {license}"))),
            "original license {name}"
        );
    }
    let output = &v["stdJsonOutput"]["contracts"][p.source][p.name];
    let metadata: Value = serde_json::from_str(output["metadata"].as_str().context("metadata")?)?;
    ensure!(metadata == v["metadata"], "complete saved metadata");
    ensure!(metadata["settings"]["compilationTarget"] == json!({p.source:p.name}), "metadata target");
    for f in ["abi", "devdoc", "userdoc", "storageLayout"] {
        ensure!(output[f] == v[f], "saved {f}");
    }
    let cbor = |digest: &str| format!("a2646970667358221220{digest}64736f6c63430008100033");
    for (field, kind, size) in [
        ("runtimeBytecode", "deployedBytecode", p.runtime_bytes),
        ("creationBytecode", "bytecode", p.creation_bytes),
    ] {
        let b = &v[field];
        let compiled = bytes(&b["recompiledBytecode"])?;
        ensure!(
            compiled.len() == size && compiled == bytes(&output["evm"][kind]["object"])? && b["sourceMap"] == output["evm"][kind]["sourceMap"],
            "complete saved compiler {kind}"
        );
        ensure!(
            b["linkReferences"] == json!({}) && output["evm"][kind]["linkReferences"] == json!({}),
            "no links"
        );
        let cbor_offset = if kind == "bytecode" && p.label != "implementation" { 2186 } else { size - 53 };
        ensure!(
            b["cborAuxdata"] == json!({"1":{"value":format!("0x{}",cbor(p.old_digest)),"offset":cbor_offset}})
                && hex::encode(&compiled[cbor_offset..cbor_offset + 53]) == cbor(p.old_digest),
            "exact original CBOR {kind}"
        );
        if cbor_offset + 53 != size {
            ensure!(
                kind == "bytecode" && p.label != "implementation" && &compiled[cbor_offset + 53..] == b"Address: low-level delegate call failed",
                "exact proxy creation post-CBOR literal"
            );
        }
    }
    let r = &v["runtimeBytecode"];
    let off = p.runtime_bytes - 53;
    ensure!(
        r["immutableReferences"] == json!({}) && output["evm"]["deployedBytecode"]["immutableReferences"] == json!({}),
        "no runtime immutables"
    );
    ensure!(
        r["transformations"] == json!([{"id":"1","type":"replace","offset":off,"reason":"cborAuxdata"}])
            && r["transformationValues"] == json!({"cborAuxdata":{"1":format!("0x{}",cbor(p.new_digest))}}),
        "exact single metadata substitution"
    );
    let mut code = bytes(&r["recompiledBytecode"])?;
    code[off..].copy_from_slice(&hex::decode(cbor(p.new_digest))?);
    ensure!(code == runtime(v)? && kh(&code) == p.runtime_hash, "full captured runtime reconstruction/hash");
    let creation = &v["creationBytecode"];
    ensure!(
        ["onchainBytecode", "transformations", "transformationValues"]
            .iter()
            .all(|k| creation[*k].is_null()),
        "creation null binding; no invented append/deployment"
    );
    if p.label == "implementation" {
        verify_layout(v)?;
    } else {
        ensure!(v["storageLayout"] == json!({"storage":[],"types":null}), "stateless proxy");
    }
    Ok(())
}
pub fn verify_layout(v: &Value) -> Result<()> {
    let rows = v["storageLayout"]["storage"].as_array().context("layout")?;
    for (label, slot) in [
        ("_balances", "51"),
        ("_allowances", "52"),
        ("_totalSupply", "53"),
        ("_name", "54"),
        ("_symbol", "55"),
        ("_roles", "201"),
        ("_roleMembers", "251"),
        ("compliance", "301"),
        ("tokenPauseManager", "351"),
        ("nameOverride", "401"),
        ("symbolOverride", "402"),
    ] {
        let found: Vec<_> = rows.iter().filter(|r| r["label"] == label).collect();
        ensure!(found.len() == 1 && found[0]["slot"] == slot && found[0]["offset"] == 0, "layout {label}");
    }
    for (name, labels) in [
        ("struct AccessControlUpgradeable.RoleData", [("members", "0"), ("adminRole", "1")]),
        ("struct EnumerableSetUpgradeable.Set", [("_values", "0"), ("_indexes", "1")]),
    ] {
        let found: Vec<_> = v["storageLayout"]["types"]
            .as_object()
            .context("types")?
            .values()
            .filter(|t| t["label"] == name)
            .collect();
        ensure!(found.len() == 1 && found[0]["numberOfBytes"] == "64", "struct size");
        let members = found[0]["members"].as_array().context("members")?;
        ensure!(
            members.len() == 2
                && members
                    .iter()
                    .zip(labels)
                    .all(|(m, (label, slot))| m["label"] == label && m["slot"] == slot && m["offset"] == 0),
            "struct fields"
        );
    }
    Ok(())
}
pub fn contextual_sha(name: &str) -> Result<&'static str> {
    Ok(match name {
        "contracts/globalMarkets/GMToken.sol" => "e1cd137e4db20a961ac4c848ee54b79bee2f2c33dd5ed4e34f53e70eb10fffc6",
        "contracts/globalMarkets/gmTokenCompliance/IOndoComplianceGMView.sol" => "98e1fa85667500e473eabd2a055cb9cc582890f0e8c5d561583c47fc31b84425",
        "contracts/globalMarkets/gmTokenCompliance/OndoComplianceGMClientUpgradeable.sol" => "87fe0674efbcd3a341be334712c2aff9e379f943617fe38d30b71ef2f63341aa",
        "contracts/globalMarkets/tokenPauseManager/ITokenPauseManager.sol" => "5ffb473d84599670d03be94ce22f7013862f554689e435f86e70c0306d841233",
        "contracts/globalMarkets/tokenPauseManager/TokenPauseManagerClientUpgradeable.sol" => {
            "043521dae82681c999dd9ebf14cf65a39790444a3a644901feb49be30d0f9bd5"
        }
        _ => anyhow::bail!("unknown contextual custom path"),
    })
}
pub fn verify_primary(captures: &[Value], primary: &Value) -> Result<()> {
    ensure!(captures.len() == 3 && primary["custom_gap"] == PRIMARY_GAP, "scope/source gap");
    let entries = primary["sources"].as_array().context("primary sources")?;
    ensure!(entries.len() == 34, "34 complete source records");
    let mut seen = std::collections::BTreeSet::new();
    for s in entries {
        let i = CAPTURES.iter().position(|p| s["capture"] == p.label).context("known capture")?;
        let p = &CAPTURES[i];
        let name = s["path"].as_str().context("path")?;
        ensure!(seen.insert((i, name)), "no duplicate record");
        let content = s["content"].as_str().context("primary body")?;
        let captured = captures[i]["sources"][name]["content"].as_str().context("captured body")?;
        ensure!(
            s["sha256"] == sha(content.as_bytes()) && s["captured_sha256"] == sha(captured.as_bytes()),
            "source hashes"
        );
        if custom(name) {
            ensure!(
                i == 0
                    && s["status"] == "contextual_nonmatch_primary_gap"
                    && s["url"] == custom_url(name)
                    && content != captured
                    && sha(content.as_bytes()) == contextual_sha(name)?,
                "explicit custom gap"
            );
        } else {
            ensure!(
                s["url"] == primary_url(p, name)? && content == captured && s["status"] == if i == 0 { "exact_ondo_vendor" } else { "exact_upstream_oz" },
                "exact dependency classification"
            );
        }
    }
    for (i, c) in captures.iter().enumerate() {
        for name in c["sources"].as_object().context("sources")?.keys() {
            ensure!(seen.contains(&(i, name.as_str())), "complete source inventory");
        }
    }
    Ok(())
}
/// Compare the complete selected source/compiler artifacts as well as the pinned
/// raw compiler-output digest checked by `verify_compiled`.
pub fn check_compiled(capture: &Value, raw: &[u8], p: &Capture) -> Result<Value> {
    let out: Value = serde_json::from_slice(raw)?;
    ensure!(
        !out["errors"].as_array().into_iter().flatten().any(|e| e["severity"] == "error"),
        "compiler errors"
    );
    ensure!(out["sources"] == capture["sourceIds"], "fresh source IDs");
    let a = &out["contracts"][p.source][p.name];
    let b = &capture["stdJsonOutput"]["contracts"][p.source][p.name];
    for f in ["abi", "devdoc", "userdoc", "storageLayout"] {
        ensure!(a[f] == b[f], "fresh {f}");
    }
    let metadata: Value = serde_json::from_str(a["metadata"].as_str().context("fresh metadata")?)?;
    ensure!(metadata == capture["metadata"], "fresh parsed metadata");
    for (field, kind) in [("runtimeBytecode", "deployedBytecode"), ("creationBytecode", "bytecode")] {
        ensure!(
            bytes(&a["evm"][kind]["object"])? == bytes(&capture[field]["recompiledBytecode"])?,
            "fresh whole bytecode"
        );
        ensure!(
            a["evm"][kind]["sourceMap"] == capture[field]["sourceMap"] && a["evm"][kind]["linkReferences"] == json!({}),
            "fresh map/links"
        );
    }
    ensure!(
        a["evm"]["deployedBytecode"]["immutableReferences"] == capture["runtimeBytecode"]["immutableReferences"],
        "fresh immutable schema"
    );
    Ok(out)
}
pub fn verify_compiled(capture: &Value, raw: &[u8], p: &Capture) -> Result<Value> {
    ensure!(sha(raw) == p.compiled_sha, "complete fresh compiler output digest");
    check_compiled(capture, raw, p)
}
pub fn mapped_sources(capture: &Value, compiled: &Value, p: &Capture, kind: &str) -> Result<std::collections::BTreeMap<i64, (String, String)>> {
    let mut out = std::collections::BTreeMap::new();
    for (path, info) in compiled["sources"].as_object().context("source IDs")? {
        ensure!(
            out.insert(
                info["id"].as_i64().context("source ID")?,
                (path.clone(), capture["sources"][path]["content"].as_str().context("source content")?.into())
            )
            .is_none(),
            "duplicate source ID"
        );
    }
    for source in compiled["contracts"][p.source][p.name]["evm"][kind]["generatedSources"]
        .as_array()
        .context("generated sources")?
    {
        ensure!(
            out.insert(
                source["id"].as_i64().context("generated ID")?,
                (
                    source["name"].as_str().context("generated name")?.into(),
                    source["contents"].as_str().context("generated content")?.into()
                )
            )
            .is_none(),
            "duplicate generated ID"
        );
    }
    Ok(out)
}
pub fn snapshot_sources(out: &std::path::Path) -> Result<String> {
    use std::{fs, path::Path};
    let own = [
        ("mod.rs", include_str!("mod.rs")),
        (
            "../../../tests/fixtures/gm-operation-proof/implementation-settings.json",
            include_str!("../../../tests/fixtures/gm-operation-proof/implementation-settings.json"),
        ),
        (
            "../../../tests/fixtures/gm-operation-proof/proxy-settings.json",
            include_str!("../../../tests/fixtures/gm-operation-proof/proxy-settings.json"),
        ),
        ("cases.rs", include_str!("cases.rs")),
        ("../../tests/gm_operation_cases.rs", include_str!("../../tests/gm_operation_cases.rs")),
        ("../../tests/gm_operation_binding.rs", include_str!("../../tests/gm_operation_binding.rs")),
        ("../bin/build_gm_proof.rs", include_str!("../bin/build_gm_proof.rs")),
        ("../bin/execute_gm_proof.rs", include_str!("../bin/execute_gm_proof.rs")),
        ("../ptoken_proof/vm.rs", include_str!("../ptoken_proof/vm.rs")),
        ("../ptoken_proof/vm_tests.rs", include_str!("../ptoken_proof/vm_tests.rs")),
        ("../ptoken_proof/source_map.rs", include_str!("../ptoken_proof/source_map.rs")),
    ];
    fs::create_dir(out.join("as-run"))?;
    for (name, text) in own {
        let path = Path::new("erc20/balances/tools/src/gm_proof").join(name);
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
        "erc20/balances/tests/fixtures/gm-operation-proof/implementation-settings.json",
        "erc20/balances/tests/fixtures/gm-operation-proof/proxy-settings.json",
        "docs/follow-up.md",
        "docs/handoff.md",
        "docs/research/README.md",
        "erc20/balances/docs/role-shape-audit.md",
    ] {
        let raw = fs::read(p)?;
        files.insert(p.into(), json!({"sha256":sha(&raw),"bytes":raw.len()}));
    }
    let raw = serde_json::to_vec_pretty(&json!({"scope":"GM host operation proof and local host dependencies; no chain calls","files":files}))?;
    fs::write(out.join("source-inventory.json"), &raw)?;
    Ok(sha(&raw))
}
/// Bounded executed-path check, not a general equivalence theorem for CBOR.
/// The VM records opcode PCs and the top eight pre-op stack words. CODECOPY's
/// source/length are therefore available without changing the shared VM.
pub fn verify_runtime_trace(execution: &Value) -> Result<()> {
    let end = CAPTURES[0].runtime_bytes;
    let suffix = end - 53;
    for step in execution["trace"].as_array().context("full trace")? {
        let pc = step["pc"].as_u64().context("pc")? as usize;
        let op = step["opcode"].as_u64().context("opcode")?;
        ensure!(op <= 255, "opcode byte");
        let immediate = if (0x60..=0x7f).contains(&op) { (op - 0x5f) as usize } else { 0 };
        ensure!(
            pc < suffix && pc.checked_add(1 + immediate).is_some_and(|n| n <= suffix),
            "executed opcode/immediate reaches metadata"
        );
        if op == 0x39 {
            let stack = step["stack_top"].as_array().context("CODECOPY stack")?;
            ensure!(stack.len() >= 3, "CODECOPY operands");
            let src_bytes = bytes(&stack[1])?;
            let len_bytes = bytes(&stack[2])?;
            ensure!(src_bytes.len() == 32 && len_bytes.len() == 32, "CODECOPY exact word operands");
            let src = U256::from_big_endian(&src_bytes);
            let len = U256::from_big_endian(&len_bytes);
            if !len.is_zero() {
                let (stop, overflow) = src.overflowing_add(len);
                ensure!(!overflow && (src >= end.into() || stop <= suffix.into()), "CODECOPY consumes changed metadata");
            }
        }
    }
    Ok(())
}
pub const MANIFEST_SHA: &str = "22e8ba1c7c8d0fc5eb60964083b238d267ea2c1afec9757521fea20c45b206af";
pub const VERSION_SHA: &str = "4e56e0d29abaea7398f4737e103d0187e4f5ba163374b17b66c28fed6dfdddf4";
pub const LICENSE_SHA: &str = "8c21a3d62814a86c6e6db50768e5a885a1f586873628e9be79a93f4eb6c18820";
pub fn verify_auxiliary(root: &std::path::Path, captures: &[Value]) -> Result<()> {
    use std::fs;
    let read = |name: &str| -> Result<Value> { Ok(serde_json::from_slice(&fs::read(root.join(name))?)?) };
    ensure!(captures.len() == 3, "three captures for auxiliary binding");
    let manifest_raw = fs::read(root.join("solc-list.json"))?;
    ensure!(sha(&manifest_raw) == MANIFEST_SHA, "exact immutable compiler manifest");
    let manifest: Value = serde_json::from_slice(&manifest_raw)?;
    let builds: Vec<_> = manifest["builds"]
        .as_array()
        .context("builds")?
        .iter()
        .filter(|b| b["longVersion"] == SOLC_VERSION)
        .collect();
    ensure!(
        builds.len() == 1
            && builds[0]["path"] == format!("solc-macosx-amd64-v{SOLC_VERSION}")
            && builds[0]["sha256"] == format!("0x{SOLC_SHA}")
            && builds[0]["keccak256"] == SOLC_KECCAK
            && manifest["releases"]["0.8.16"] == builds[0]["path"],
        "official compiler release association"
    );
    ensure!(
        sha(&fs::read(root.join("compiler-version.txt"))?) == VERSION_SHA,
        "exact official compiler version output"
    );
    ensure!(
        sha(&fs::read(root.join("LICENSE-proxy-upstream"))?) == LICENSE_SHA,
        "exact upstream proxy license"
    );
    let licenses = read("source-licenses.json")?;
    let mut expected_licenses = vec![];
    for (c, p) in captures.iter().zip(&CAPTURES) {
        for (name, s) in c["sources"].as_object().context("sources")? {
            let text = s["content"].as_str().context("content")?;
            expected_licenses.push(json!({"capture":p.label,"path":name,"sha256":sha(text.as_bytes()),"license":c["metadata"]["sources"][name]["license"],"original_first_line":text.lines().next()}));
        }
    }
    ensure!(licenses["sources"] == json!(expected_licenses) && licenses["upstream_proxy_license"] == json!({"url":format!("https://raw.githubusercontent.com/{}/{}/LICENSE", CAPTURES[1].repo,CAPTURES[1].pin),"sha256":LICENSE_SHA}) && licenses["vendor_and_custom_notice"] == "Original per-source MIT/BUSL-1.1 headers and compiler metadata retained; no exact custom-source public license artifact or separate vendor root license is claimed. The proxy upstream MIT text does not relabel custom or vendored sources.", "complete license inventory/classification");
    let report = read("report.json")?;
    ensure!(
        report["source_inventory_sha256"] == sha(&fs::read(root.join("source-inventory.json"))?),
        "source inventory report binding"
    );
    ensure!(
        report["status"] == "exact_source_compiler_regeneration_passed"
            && report["qualified"] == false
            && report["compiler"] == SOLC_VERSION
            && report["compiler_sha256"] == SOLC_SHA
            && report["compiler_keccak256"] == SOLC_KECCAK
            && report["compiler_manifest_sha256"] == MANIFEST_SHA
            && report["compiler_manifest_url"] == format!("https://raw.githubusercontent.com/ethereum/solc-bin/{SOLC_PIN}/macosx-amd64/list.json")
            && report["compiler_arguments"] == json!(["--standard-json"]),
        "source compiler report identity"
    );
    ensure!(
        report["original_settings_sources_unchanged"] == true
            && report["only_output_selection_added"] == true
            && report["source_records"] == 34
            && report["unique_source_profiles"] == 27
            && report["exact_ondo_vendored_dependencies"] == 15
            && report["unique_exact_upstream_proxy_dependencies"] == 7
            && report["custom_primary_gaps"] == 5
            && report["custom_gap"] == PRIMARY_GAP
            && report["history_limit"] == HISTORY_LIMIT
            && report["network_chain_calls"] == 0
            && report["fresh_public_source_requests_only"] == true
            && report["license_inventory_sha256"] == sha(&fs::read(root.join("source-licenses.json"))?),
        "source compiler report scope"
    );
    ensure!(
        report["compiled"].as_array().context("compiled records")?.len() == 3,
        "complete compiler records"
    );
    for (i, (c, p)) in captures.iter().zip(&CAPTURES).enumerate() {
        let original = read(&format!("{}-compiler-input-original.json", p.label))?;
        let mut used = read(&format!("{}-compiler-input.json", p.label))?;
        ensure!(
            original == c["stdJsonInput"]
                && used["settings"]["outputSelection"]
                    == json!({"*":{"*":["abi","metadata","devdoc","userdoc","storageLayout","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}}),
            "original and selected compiler input"
        );
        used["settings"].as_object_mut().context("settings")?.remove("outputSelection");
        ensure!(used == original, "outputSelection only");
        let raw = fs::read(root.join(format!("{}-compiler-output.json", p.label)))?;
        let output = verify_compiled(c, &raw, p)?;
        let selected = &output["contracts"][p.source][p.name];
        let expected = json!({"capture":p.label,"capture_sha256":p.sha,"files":p.files,"compiler_input_sha256":sha(&fs::read(root.join(format!("{}-compiler-input.json",p.label)))?),"compiler_output_sha256":sha(&raw),"runtime_bytes":p.runtime_bytes,"captured_runtime_keccak256":p.runtime_hash,"compiler_runtime_keccak256":kh(&bytes(&c["runtimeBytecode"]["recompiledBytecode"])?),"creation_compiler_bytes":p.creation_bytes,"creation_match":null,"creation_onchain_bytes":null,"runtime_transformations":c["runtimeBytecode"]["transformations"],"runtime_transformation_values":c["runtimeBytecode"]["transformationValues"],"fresh_metadata_raw_sha256":sha(selected["metadata"].as_str().context("metadata")?.as_bytes()),"saved_metadata_raw_sha256":sha(c["stdJsonOutput"]["contracts"][p.source][p.name]["metadata"].as_str().context("saved metadata")?.as_bytes())});
        ensure!(report["compiled"][i] == expected, "complete compiler report record {}", p.label);
    }
    Ok(())
}
