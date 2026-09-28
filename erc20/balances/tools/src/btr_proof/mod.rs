//! Independent BTR host operation proof; no ingestion rule or candidate.
pub mod cases;
pub use crate::ptoken_proof::{bytes, kh, sha};
pub use crate::ptoken_proof::{source_map, vm};
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde_json::{json, Value};
pub const IMPLEMENTATION: &str = "0xc8b5a0c5453c15157328b6cc1f1452be032a41f1";
pub const PROXY: &str = "0xfed13d0c40790220fbde712987079eda1ed75c51";
pub const SOURCE: &str = "contracts/BTRToken.sol";
pub const NAME: &str = "BTRToken";
pub const SOLC_PIN: &str = "16a99b8c26ed33a91796e209ff6797ee7baf2b0d";
pub const SOLC_SHA: &str = "cc2d44c706905ccc382f484625dff61d741e0c24232d226f139a6835fc644f3f";
pub const SOLC_KECCAK: &str = "0xf8c5313dfe054e7f3c208905ee6bb4097a1c7a1dd360050f90fab4b182ac1c24";
pub const SOLC_VERSION: &str = "0.8.24+commit.e11b9ed9";
pub const TOKEN_SHA: &str = "f6b70c8e72aa4519db417da71e4b827e216ddf5f3eaa2867d428d8db026c42aa";
pub const PRIMARY_GAP: &str = "The exact UNLICENSED BTRToken custom source has no recovered independent public primary revision. The official bitlayer-contracts basic/BTR.sol is a different non-upgradeable distribution token. Exact dependency matches and captured/compiler/runtime binding do not resolve that token gap.";
pub const FIXTURE: &str = "erc20/balances/tests/fixtures/btr-operation-proof";
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
    pub oz_repo: &'static str,
    pub oz_pin: &'static str,
    pub prefix: &'static str,
    pub compiled_sha: &'static str,
}
pub const CAPTURES: [Capture; 2] = [
    Capture {
        label: "implementation",
        address: IMPLEMENTATION,
        sha: "b45110e73361a6bdaf50181ec265b0c9cc00cab6dbafbe1c76f1262a57139b21",
        source: SOURCE,
        name: NAME,
        files: 31,
        runtime_bytes: 15308,
        creation_bytes: 15595,
        runtime_hash: "0x44d6248ac6cfc67512326547ee54cf5e5d5a5573d4d65a8e94b29cb2f8e42949",
        oz_repo: "OpenZeppelin/openzeppelin-contracts-upgradeable",
        oz_pin: "3d4c0d5741b131c231e558d7a6213392ab3672a5",
        prefix: "@openzeppelin/contracts-upgradeable/",
        compiled_sha: "c72916720a301b5eedf94f100b9c7cca4e35000eb77a252d5af68e1d7c53fcec",
    },
    Capture {
        label: "proxy",
        address: PROXY,
        sha: "9c9fcd09b032a0ac632041217f5ce4000a20492c555e30e5ca5896fb14b6b320",
        source: "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol",
        name: "ERC1967Proxy",
        files: 8,
        runtime_bytes: 183,
        creation_bytes: 1047,
        runtime_hash: "0xbd576604a6f2b4dac58ff676c3912bd2ef37d52f93640e2d20c8d47de43d3418",
        oz_repo: "OpenZeppelin/openzeppelin-contracts",
        oz_pin: "acd4ff74de833399287ed6b31b4debf6b2b35527",
        prefix: "@openzeppelin/contracts/",
        compiled_sha: "6d93cd857f4bf15d98f3ed89bbfa154bf399957cd1f0df406ac4bc94f6b98a05",
    },
];
pub fn address(s: &str) -> U256 {
    U256::from_big_endian(&hex::decode(&s[2..]).unwrap())
}
pub fn initializer_arguments() -> Vec<u8> {
    let mut out = erc20_balances::hash(b"initialize(string,string,address,address,uint256)")[..4].to_vec();
    for v in [
        160.into(),
        224.into(),
        address("0x27e939c9a85afd8d643eeda0bdecb193683bcda5"),
        address("0x27e939c9a85afd8d643eeda0bdecb193683bcda5"),
        U256::from(500_000_000u64) * U256::exp10(18),
    ] {
        out.extend(vm::word(v));
    }
    for s in [b"BTR token".as_slice(), b"BTR".as_slice()] {
        out.extend(vm::word(s.len().into()));
        out.extend(s);
        // The four-byte selector is outside the argument tuple.
        out.resize(4 + (out.len() - 4).div_ceil(32) * 32, 0);
    }
    out
}
pub fn constructor_arguments(p: &Capture) -> Vec<u8> {
    if p.label == "implementation" {
        return vec![];
    }
    let data = initializer_arguments();
    let mut out = vm::word(address(IMPLEMENTATION)).to_vec();
    out.extend(vm::word(64.into()));
    out.extend(vm::word(data.len().into()));
    out.extend(data);
    out.resize(out.len().div_ceil(32) * 32, 0);
    out
}
pub fn runtime(capture: &Value) -> Result<Vec<u8>> {
    bytes(&capture["runtimeBytecode"]["onchainBytecode"])
}
pub fn primary_url(p: &Capture, path: &str) -> Result<String> {
    Ok(format!(
        "https://raw.githubusercontent.com/{}/{}/contracts/{}",
        p.oz_repo,
        p.oz_pin,
        path.strip_prefix(p.prefix).context("exact dependency prefix")?
    ))
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
        ["match", "runtimeMatch", "creationMatch"].iter().all(|f| v[f] == "exact_match"),
        "original labels"
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
    let settings = json!({"metadata":{"bytecodeHash":"ipfs"},"libraries":{},"optimizer":{"runs":1000,"enabled":true},"evmVersion":"paris","remappings":[]});
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
            && sources.keys().eq(v["sourceIds"].as_object().context("IDs")?.keys()),
        "complete selected source sets"
    );
    for (name, s) in sources {
        ensure!(
            v["metadata"]["sources"][name]["keccak256"] == kh(s["content"].as_str().context("content")?.as_bytes()),
            "source Keccak {name}"
        );
    }
    let output = &v["stdJsonOutput"]["contracts"][p.source][p.name];
    let metadata: Value = serde_json::from_str(output["metadata"].as_str().context("metadata string")?)?;
    ensure!(metadata == v["metadata"], "complete saved metadata");
    for f in ["abi", "devdoc", "userdoc", "storageLayout", "transientStorageLayout"] {
        ensure!(output[f] == v[f], "saved {f}");
    }
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
    }
    let imm = if p.label == "implementation" {
        json!({"1168":[{"start":3275,"length":32},{"start":3408,"length":32},{"start":3939,"length":32},{"start":4072,"length":32},{"start":4302,"length":32}]})
    } else {
        json!({})
    };
    let r = &v["runtimeBytecode"];
    ensure!(
        r["immutableReferences"] == imm && output["evm"]["deployedBytecode"]["immutableReferences"] == imm,
        "exact immutable schema"
    );
    let mut code = bytes(&r["recompiledBytecode"])?;
    let transforms = if p.label == "implementation" {
        let value = vm::word(address(IMPLEMENTATION));
        ensure!(
            r["transformationValues"] == json!({"immutables":{"1168":format!("0x{}",hex::encode(value))}}),
            "implementation self binding"
        );
        for site in [3275, 3408, 3939, 4072, 4302] {
            ensure!(code[site..site + 32] == [0; 32], "zero self placeholder");
            code[site..site + 32].copy_from_slice(&value);
        }
        json!([3275, 3408, 3939, 4072, 4302]
            .iter()
            .map(|site| json!({"id":"1168","type":"replace","offset":site,"reason":"immutable"}))
            .collect::<Vec<_>>())
    } else {
        ensure!(r["transformationValues"] == json!({}), "no proxy values");
        json!([])
    };
    ensure!(
        r["transformations"] == transforms && code == runtime(v)? && kh(&code) == p.runtime_hash,
        "full runtime reconstruction/hash"
    );
    let creation = &v["creationBytecode"];
    let args = constructor_arguments(p);
    if p.label == "implementation" {
        ensure!(
            creation["transformations"] == json!([]) && creation["transformationValues"] == json!({}),
            "no implementation append"
        );
    } else {
        ensure!(
            creation["transformations"] == json!([{"type":"insert","offset":1047,"reason":"constructorArguments"}])
                && creation["transformationValues"] == json!({"constructorArguments":format!("0x{}",hex::encode(&args))}),
            "independent proxy constructor encoding"
        );
    }
    ensure!(
        [bytes(&creation["recompiledBytecode"])?, args].concat() == bytes(&creation["onchainBytecode"])?,
        "full creation bytes"
    );
    let (cbor, runoff, createoff) = if p.label == "implementation" {
        (
            "a2646970667358221220a4c6fc181ad7fb7979466656e71a5dd83be36f4da16322e3c939a0cf9f3a95b864736f6c63430008180033",
            15255,
            15542,
        )
    } else {
        (
            "a2646970667358221220871ef2df500bd03abb29c4e5e73d5ca9af7ff3b724ed00cb69905e7b4bc1099364736f6c63430008180033",
            130,
            994,
        )
    };
    for (b, off) in [(r, runoff), (creation, createoff)] {
        ensure!(
            b["cborAuxdata"] == json!({"1":{"value":format!("0x{cbor}"),"offset":off}}),
            "exact CBOR metadata"
        );
        ensure!(hex::encode(&bytes(&b["recompiledBytecode"])?[off..]) == cbor, "CBOR bytes");
    }
    if p.label == "implementation" {
        verify_layout(v)?;
        ensure!(
            sha(v["sources"][SOURCE]["content"].as_str().unwrap().as_bytes()) == TOKEN_SHA,
            "exact custom token"
        );
    } else {
        ensure!(v["storageLayout"] == json!({"storage":[],"types":null}), "stateless proxy layout");
    }
    Ok(())
}
pub fn verify_layout(v: &Value) -> Result<()> {
    let storage = v["storageLayout"]["storage"].as_array().context("layout")?;
    for (label, slot) in [
        ("_roles", "101"),
        ("_roleMembers", "151"),
        ("_balances", "201"),
        ("_allowances", "202"),
        ("_nonces", "303"),
        ("_paused", "404"),
        ("mintQuota", "554"),
        ("whitelisted", "555"),
    ] {
        let rows: Vec<_> = storage.iter().filter(|x| x["label"] == label).collect();
        ensure!(rows.len() == 1 && rows[0]["slot"] == slot && rows[0]["offset"] == 0, "exact layout {label}");
    }
    for (name, labels) in [
        ("struct AccessControlUpgradeable.RoleData", vec![("members", "0"), ("adminRole", "1")]),
        ("struct EnumerableSetUpgradeable.Set", vec![("_values", "0"), ("_indexes", "1")]),
    ] {
        let t: Vec<_> = v["storageLayout"]["types"]
            .as_object()
            .context("types")?
            .values()
            .filter(|t| t["label"] == name)
            .collect();
        ensure!(t.len() == 1 && t[0]["numberOfBytes"] == "64", "exact struct width");
        let members = t[0]["members"].as_array().context("members")?;
        ensure!(
            members.len() == labels.len()
                && members
                    .iter()
                    .zip(labels)
                    .all(|(m, (l, s))| m["label"] == l && m["slot"] == s && m["offset"] == 0),
            "struct fields"
        );
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
pub fn verify_primary(captures: &[Value], primary: &Value) -> Result<()> {
    ensure!(captures.len() == 2 && primary["token_gap"] == PRIMARY_GAP, "scope/source gap");
    let entries = primary["sources"].as_array().context("primary sources")?;
    ensure!(entries.len() == 38, "all dependency bodies");
    let mut seen = std::collections::BTreeSet::new();
    for s in entries {
        let i = CAPTURES.iter().position(|p| s["capture"] == p.label).context("known capture")?;
        let p = &CAPTURES[i];
        let name = s["path"].as_str().context("path")?;
        ensure!(seen.insert((i, name)) && name != SOURCE, "no duplicate/custom primary");
        let content = s["content"].as_str().context("primary body")?;
        ensure!(
            s["url"] == primary_url(p, name)? && s["sha256"] == sha(content.as_bytes()) && captures[i]["sources"][name]["content"] == content,
            "exact primary binding"
        );
    }
    for (i, v) in captures.iter().enumerate() {
        for name in v["sources"].as_object().unwrap().keys().filter(|n| n.as_str() != SOURCE) {
            ensure!(seen.contains(&(i, name.as_str())), "complete primary inventory");
        }
    }
    Ok(())
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
        ("cases.rs", include_str!("cases.rs")),
        ("../../tests/btr_operation_cases.rs", include_str!("../../tests/btr_operation_cases.rs")),
        ("../../tests/btr_operation_binding.rs", include_str!("../../tests/btr_operation_binding.rs")),
        ("../bin/build_btr_proof.rs", include_str!("../bin/build_btr_proof.rs")),
        ("../bin/execute_btr_proof.rs", include_str!("../bin/execute_btr_proof.rs")),
        ("../ptoken_proof/vm.rs", include_str!("../ptoken_proof/vm.rs")),
        ("../ptoken_proof/vm_tests.rs", include_str!("../ptoken_proof/vm_tests.rs")),
        ("../ptoken_proof/source_map.rs", include_str!("../ptoken_proof/source_map.rs")),
    ];
    fs::create_dir(out.join("as-run"))?;
    for (name, text) in own {
        let path = Path::new("erc20/balances/tools/src/btr_proof").join(name);
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
    let raw = serde_json::to_vec_pretty(&json!({"scope":"BTR host operation proof and local host dependencies; no chain calls","files":files}))?;
    fs::write(out.join("source-inventory.json"), &raw)?;
    Ok(sha(&raw))
}
