//! Exact Mai source/compiler binding and synthetic host proof, never admission.
pub mod cases;
pub use crate::ptoken_proof::{bytes, kh, sha, source_map, vm};
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};

pub const ADDRESS: &str = "0x35803e77c3163fed8a942536c1c8e0d5bf90f906";
pub const SOURCE: &str = "contracts/Mai.sol";
pub const NAME: &str = "Mai";
pub const FIXTURE: &str = "erc20/balances/tests/fixtures/mai-operation-proof";
pub const CAPTURE_SHA: &str = "e5fb99260765d58a5fecdd4d2eccd61c84e6f0032821fc75402f00e93535386b";
pub const TOKEN_SHA: &str = "fee53b965e3b7929b2f863e2a833b4b68bfb3783a5593cfab2722d4028a4948b";
pub const RUNTIME_HASH: &str = "0x1134938644214395aba790bf7d50e13027c30b74b4bd09dcbe167c0ccc2cc9e5";
pub const COMPILED_SHA: &str = "18a24ae3680b1b25d1b47c0d0c9a9b9d53dbd5a90c6bd6c003e6a0f5a5a20764";
pub const SOLC_VERSION: &str = "0.8.9+commit.e5eed63a";
pub const SOLC_PIN: &str = "16a99b8c26ed33a91796e209ff6797ee7baf2b0d";
pub const SOLC_SHA: &str = "d619d4f5d8fd988bc63262407e749e905ccc8d8ab1ccf0280da1d12b918894ce";
pub const SOLC_KECCAK: &str = "0x0dec1d0015c882a98259d9ed1c7a157c77cb4fb05dfe0e2b74484957501dce7c";
pub const MANIFEST_SHA: &str = "22e8ba1c7c8d0fc5eb60964083b238d267ea2c1afec9757521fea20c45b206af";
pub const OZ_PIN: &str = "8c49ad74eae76ee389d038780d407cf90b4ae1de";
pub const LICENSE_SHA: &str = "8c21a3d62814a86c6e6db50768e5a885a1f586873628e9be79a93f4eb6c18820";
pub const PRIMARY_GAP: &str = "Fourteen complete dependencies match the exact OpenZeppelin 4.7.0 revision. The UNLICENSED custom contracts/Mai.sol has no independently established maintainer revision in the reviewed material. Captured source/compiler/runtime equality does not resolve that source gap or qualify actual deployment, initial coherence, producer visibility or live package/getter/holder behavior.";
pub const LIMITS: &str = "Host-only execution of exact captured Mai runtime and no-argument creation in synthetic local state. No production rule/template/candidate, external-call emulator, VM extension, dependency or schema change. No chain calls or deployment qualification. Unsupported outcomes remain failures unless explicitly measured and named.";
pub fn captured() -> &'static [u8] {
    include_bytes!("../../../tests/fixtures/mai-operation-proof/capture.json")
}
pub fn account() -> U256 {
    U256::from_str_radix(&ADDRESS[2..], 16).unwrap()
}
pub fn cap() -> U256 {
    // The independently read constructor literal is 1000_0000_0000e18.
    U256::from(100_000_000_000u64) * U256::exp10(18)
}
pub fn runtime(v: &Value) -> Result<Vec<u8>> {
    bytes(&v["runtimeBytecode"]["onchainBytecode"])
}
pub fn verify_capture(raw: &[u8]) -> Result<Value> {
    ensure!(sha(raw) == CAPTURE_SHA, "complete original capture digest");
    let v = serde_json::from_slice(raw)?;
    verify_components(&v)?;
    Ok(v)
}
pub fn verify_components(v: &Value) -> Result<()> {
    ensure!(
        sha(captured()) == CAPTURE_SHA && *v == serde_json::from_slice::<Value>(captured())?,
        "exact complete original record"
    );
    ensure!(v["chainId"] == "56" && bytes(&v["address"])? == hex::decode(&ADDRESS[2..])?, "chain/address");
    for f in ["match", "runtimeMatch", "creationMatch"] {
        ensure!(v[f] == "exact_match", "original match labels");
    }
    let settings = json!({"libraries":{},"optimizer":{"enabled":false,"runs":200}});
    ensure!(
        v["compilation"]["compilerVersion"] == SOLC_VERSION && v["metadata"]["compiler"]["version"] == SOLC_VERSION,
        "compiler identity"
    );
    ensure!(
        v["compilation"]["compilerSettings"] == settings && v["stdJsonInput"]["settings"] == settings,
        "exact original settings"
    );
    ensure!(
        v["compilation"]["fullyQualifiedName"] == format!("{SOURCE}:{NAME}") && v["sources"] == v["stdJsonInput"]["sources"],
        "target/source equality"
    );
    let sources = v["sources"].as_object().context("sources")?;
    ensure!(
        sources.len() == 15
            && sources.keys().eq(v["metadata"]["sources"].as_object().context("metadata sources")?.keys())
            && v["sourceIds"] == v["stdJsonOutput"]["sources"],
        "complete source sets/IDs"
    );
    for (path, s) in sources {
        ensure!(
            v["metadata"]["sources"][path]["keccak256"] == kh(s["content"].as_str().context("content")?.as_bytes()),
            "source metadata {path}"
        );
    }
    ensure!(
        sha(sources[SOURCE]["content"].as_str().context("token")?.as_bytes()) == TOKEN_SHA,
        "custom token pin"
    );
    let c = &v["creationBytecode"];
    let creation = bytes(&c["recompiledBytecode"])?;
    ensure!(
        creation.len() == 12772
            && creation == bytes(&c["onchainBytecode"])?
            && c["transformations"] == json!([])
            && c["transformationValues"] == json!({})
            && c["linkReferences"] == json!({}),
        "exact whole creation/no arguments"
    );
    let r = &v["runtimeBytecode"];
    let references = json!({"1258":[{"start":1767,"length":32}]});
    ensure!(
        r["immutableReferences"] == references && r["linkReferences"] == json!({}),
        "exact single immutable site"
    );
    let value = vm::word(cap());
    ensure!(
        r["transformationValues"] == json!({"immutables":{"1258":format!("0x{}",hex::encode(value))}})
            && r["transformations"] == json!([{"id":"1258","type":"replace","offset":1767,"reason":"immutable"}]),
        "only source-derived cap replacement"
    );
    let mut code = bytes(&r["recompiledBytecode"])?;
    ensure!(code.len() == 11296 && code[1767..1799] == [0; 32], "cap zero placeholder");
    code[1767..1799].copy_from_slice(&value);
    ensure!(code == runtime(v)? && kh(&code) == RUNTIME_HASH, "complete independently reconstructed runtime");
    let output = &v["stdJsonOutput"]["contracts"][SOURCE][NAME];
    for field in ["abi", "devdoc", "userdoc", "storageLayout"] {
        ensure!(output[field] == v[field], "saved {field}");
    }
    ensure!(
        serde_json::from_str::<Value>(output["metadata"].as_str().context("metadata")?)? == v["metadata"],
        "complete metadata"
    );
    for (field, kind) in [("runtimeBytecode", "deployedBytecode"), ("creationBytecode", "bytecode")] {
        ensure!(
            bytes(&output["evm"][kind]["object"])? == bytes(&v[field]["recompiledBytecode"])?
                && output["evm"][kind]["sourceMap"] == v[field]["sourceMap"]
                && output["evm"][kind]["linkReferences"] == json!({}),
            "saved code/map/links"
        );
    }
    ensure!(output["evm"]["deployedBytecode"]["immutableReferences"] == references, "saved cap schema");
    verify_layout(&v["storageLayout"])
}
pub fn verify_layout(v: &Value) -> Result<()> {
    let fields = v["storage"].as_array().context("storage")?;
    let names = ["_roles", "_roleMembers", "_balances", "_allowances", "_totalSupply", "_name", "_symbol"];
    ensure!(fields.len() == names.len(), "complete storage declarations");
    for (i, (f, name)) in fields.iter().zip(names).enumerate() {
        let slot = i.to_string();
        ensure!(f["label"] == name && f["slot"] == slot && f["offset"] == 0, "exact field {name}");
    }
    let types = &v["types"];
    let mt = &types[fields[0]["type"].as_str().context("membership type")?];
    let role = &types[mt["value"].as_str().context("role type")?];
    ensure!(
        mt["key"] == "t_bytes32"
            && role["members"][0]["label"] == "members"
            && role["members"][0]["slot"] == "0"
            && role["members"][0]["type"] == "t_mapping(t_address,t_bool)"
            && role["members"][1]["label"] == "adminRole"
            && role["members"][1]["slot"] == "1",
        "membership/admin layout"
    );
    let st = &types[fields[1]["type"].as_str().context("set mapping")?];
    let wrapper = &types[st["value"].as_str().context("wrapper")?];
    let inner = &types[wrapper["members"][0]["type"].as_str().context("inner")?];
    ensure!(
        st["key"] == "t_bytes32"
            && wrapper["numberOfBytes"] == "64"
            && inner["members"][0]["label"] == "_values"
            && inner["members"][0]["slot"] == "0"
            && inner["members"][1]["label"] == "_indexes"
            && inner["members"][1]["slot"] == "1",
        "separate two-word set layout"
    );
    Ok(())
}
pub fn primary_url(path: &str) -> Result<Option<String>> {
    if path == SOURCE {
        return Ok(None);
    }
    let suffix = path.strip_prefix("@openzeppelin/contracts/").context("dependency prefix")?;
    Ok(Some(format!(
        "https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{OZ_PIN}/contracts/{suffix}"
    )))
}
pub fn verify_primary(c: &Value, p: &Value) -> Result<()> {
    ensure!(p["source_gap"] == PRIMARY_GAP, "explicit primary gap");
    let records = p["sources"].as_array().context("primary records")?;
    ensure!(records.len() == 15, "complete primary inventory");
    for ((path, s), r) in c["sources"].as_object().context("sources")?.iter().zip(records) {
        let body = s["content"].as_str().context("body")?;
        let url = primary_url(path)?;
        ensure!(
            r["path"] == *path && r["url"] == json!(url) && r["capture_sha256"] == sha(body.as_bytes()),
            "primary identity"
        );
        if url.is_some() {
            ensure!(
                r["classification"] == "exact" && r["content"] == body && r["sha256"] == sha(body.as_bytes()),
                "exact dependency"
            );
        } else {
            ensure!(
                r["classification"] == "unestablished" && r["content"].is_null() && r["sha256"].is_null(),
                "custom gap"
            );
        }
    }
    Ok(())
}
pub fn input(c: &Value) -> Result<Value> {
    let mut v = c["stdJsonInput"].clone();
    ensure!(v["settings"].get("outputSelection").is_none(), "original output selection absent");
    v["settings"]["outputSelection"] =
        json!({"*":{"*":["abi","metadata","devdoc","userdoc","storageLayout","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"],"":["ast"]}});
    Ok(v)
}
fn find(ast: &Value, id: u64) -> Option<&Value> {
    match ast {
        Value::Object(m) => {
            if ast["id"] == id {
                Some(ast)
            } else {
                m.values().find_map(|v| find(v, id))
            }
        }
        Value::Array(a) => a.iter().find_map(|v| find(v, id)),
        _ => None,
    }
}
pub fn check_compiled(c: &Value, raw: &[u8]) -> Result<Value> {
    let out: Value = serde_json::from_slice(raw)?;
    ensure!(
        !out["errors"].as_array().is_some_and(|a| a.iter().any(|e| e["severity"] == "error")),
        "compiler errors"
    );
    for (name, id) in c["sourceIds"].as_object().context("source IDs")? {
        ensure!(
            out["sources"][name]["id"] == id["id"] && out["sources"][name]["ast"].is_object(),
            "fresh ID/AST {name}"
        );
    }
    let node = find(&out["sources"]["@openzeppelin/contracts/token/ERC20/extensions/ERC20Capped.sol"]["ast"], 1258).context("cap AST ID")?;
    ensure!(
        node["nodeType"] == "VariableDeclaration"
            && node["name"] == "_cap"
            && node["mutability"] == "immutable"
            && node["typeDescriptions"]["typeString"] == "uint256",
        "source-bound cap declaration"
    );
    let literal = find(&out["sources"][SOURCE]["ast"], 2249).context("constructor cap literal AST")?;
    ensure!(
        literal["nodeType"] == "Literal" && literal["kind"] == "number" && literal["value"] == "1000_0000_0000e18" && literal["src"] == "541:17:14",
        "exact constructor cap literal/span"
    );
    ensure!(
        c["sources"][SOURCE]["content"].as_str().context("custom source")?.as_bytes().get(541..558) == Some(b"1000_0000_0000e18".as_slice())
            && literal["typeDescriptions"]["typeString"] == format!("int_const {}", cap()),
        "independent source literal and compiler rational agree"
    );
    let a = &out["contracts"][SOURCE][NAME];
    let b = &c["stdJsonOutput"]["contracts"][SOURCE][NAME];
    for field in ["abi", "metadata", "devdoc", "userdoc", "storageLayout"] {
        ensure!(a[field] == b[field], "fresh exact {field}");
    }
    verify_layout(&a["storageLayout"])?;
    for (field, kind) in [("runtimeBytecode", "deployedBytecode"), ("creationBytecode", "bytecode")] {
        ensure!(
            bytes(&a["evm"][kind]["object"])? == bytes(&c[field]["recompiledBytecode"])?
                && a["evm"][kind]["sourceMap"] == c[field]["sourceMap"]
                && a["evm"][kind]["linkReferences"] == json!({}),
            "whole fresh code/map/links"
        );
        mapped_sources(c, &out, kind)?;
    }
    ensure!(
        a["evm"]["deployedBytecode"]["immutableReferences"] == c["runtimeBytecode"]["immutableReferences"],
        "fresh single cap site"
    );
    Ok(out)
}
pub fn verify_compiled(c: &Value, raw: &[u8]) -> Result<Value> {
    ensure!(sha(raw) == COMPILED_SHA, "complete fresh compiler output pin");
    check_compiled(c, raw)
}
pub fn mapped_sources(c: &Value, out: &Value, kind: &str) -> Result<BTreeMap<i64, (String, String)>> {
    let mut m = BTreeMap::new();
    for (path, id) in out["sources"].as_object().context("sources")? {
        ensure!(
            m.insert(
                id["id"].as_i64().context("ID")?,
                (path.clone(), c["sources"][path]["content"].as_str().context("body")?.into())
            )
            .is_none(),
            "duplicate source ID"
        );
    }
    for g in out["contracts"][SOURCE][NAME]["evm"][kind]["generatedSources"]
        .as_array()
        .context("generated sources")?
    {
        ensure!(
            m.insert(
                g["id"].as_i64().context("generated ID")?,
                (
                    g["name"].as_str().context("generated name")?.into(),
                    g["contents"].as_str().context("generated body")?.into()
                )
            )
            .is_none(),
            "duplicate generated ID"
        );
    }
    Ok(m)
}
pub fn verify_auxiliary(dir: &Path, c: &Value) -> Result<()> {
    let read = |n: &str| -> Result<Value> { Ok(serde_json::from_slice(&fs::read(dir.join(n))?)?) };
    ensure!(
        read("compiler-input-original.json")? == c["stdJsonInput"] && read("compiler-input.json")? == input(c)?,
        "exact original and outputSelection-only inputs"
    );
    let md = c["stdJsonOutput"]["contracts"][SOURCE][NAME]["metadata"].as_str().context("metadata")?;
    ensure!(
        fs::read(dir.join("metadata-original.json"))? == md.as_bytes() && fs::read(dir.join("metadata-fresh.json"))? == md.as_bytes(),
        "both raw metadata"
    );
    verify_primary(c, &read("primary-sources.json")?)
}
pub fn verify_source_directory(dir: &Path) -> Result<(Value, Value)> {
    ensure!(
        sha(&fs::read(dir.join("solc-list.json"))?) == MANIFEST_SHA && sha(&fs::read(dir.join("LICENSE-openzeppelin"))?) == LICENSE_SHA,
        "manifest/license pins"
    );
    ensure!(
        fs::read_to_string(dir.join("compiler-version.txt"))?
            == "solc, the solidity compiler commandline interface\nVersion: 0.8.9+commit.e5eed63a.Darwin.appleclang\n",
        "exact compiler version output"
    );
    let c = verify_capture(&fs::read(dir.join("capture.json"))?)?;
    verify_auxiliary(dir, &c)?;
    let out = verify_compiled(&c, &fs::read(dir.join("compiler-output.json"))?)?;
    Ok((c, out))
}

pub fn snapshot_sources(out: &Path) -> Result<String> {
    fn walk(p: &Path, files: &mut BTreeMap<String, Value>) -> Result<()> {
        if p.is_dir() {
            for x in fs::read_dir(p)? {
                walk(&x?.path(), files)?;
            }
        } else if p.extension().is_some_and(|e| e == "rs" || e == "toml" || e == "lock") {
            let b = fs::read(p)?;
            files.insert(p.to_string_lossy().into(), json!({"bytes":b.len(),"sha256":sha(&b)}));
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    for p in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "erc20/balances/Cargo.toml",
        "erc20/balances/src",
        "erc20/balances/tools/Cargo.toml",
        "erc20/balances/tools/src",
        "erc20/balances/tools/tests",
        "proto",
        "common/persist",
        "common/retention",
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
        files.insert(p.into(), json!({"bytes":raw.len(),"sha256":sha(&raw)}));
    }
    fn fixtures(p: &Path, files: &mut BTreeMap<String, Value>) -> Result<()> {
        for e in fs::read_dir(p)? {
            let p = e?.path();
            if p.is_dir() {
                fixtures(&p, files)?;
            } else if p.file_name().is_none_or(|n| n != "README.md") {
                let b = fs::read(&p)?;
                files.insert(p.to_string_lossy().into(), json!({"bytes":b.len(),"sha256":sha(&b)}));
            }
        }
        Ok(())
    }
    fixtures(Path::new(FIXTURE), &mut files)?;
    fs::create_dir(out.join("as-run"))?;
    for (name, body) in [
        ("mai_proof/mod.rs", include_str!("mod.rs")),
        ("mai_proof/cases.rs", include_str!("cases.rs")),
        ("bin/execute_mai_proof.rs", include_str!("../bin/execute_mai_proof.rs")),
        ("bin/build_mai_proof.rs", include_str!("../bin/build_mai_proof.rs")),
        ("ptoken_proof/vm.rs", include_str!("../ptoken_proof/vm.rs")),
        ("ptoken_proof/source_map.rs", include_str!("../ptoken_proof/source_map.rs")),
        ("ptoken_proof/cases.rs", include_str!("../ptoken_proof/cases.rs")),
        ("ptoken_proof/vm_tests.rs", include_str!("../ptoken_proof/vm_tests.rs")),
        ("../tests/mai_operation_binding.rs", include_str!("../../tests/mai_operation_binding.rs")),
        ("../tests/mai_operation_cases.rs", include_str!("../../tests/mai_operation_cases.rs")),
    ] {
        ensure!(
            fs::read(Path::new("erc20/balances/tools/src").join(name))? == body.as_bytes(),
            "stale executable {name}"
        );
        fs::write(out.join("as-run").join(name.replace('/', "_")), body)?;
    }
    let raw = serde_json::to_vec_pretty(&json!({"scope":LIMITS,"files":files}))?;
    fs::write(out.join("source-inventory.json"), &raw)?;
    Ok(sha(&raw))
}
