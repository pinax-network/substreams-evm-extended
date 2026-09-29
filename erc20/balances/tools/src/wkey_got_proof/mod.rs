//! Two exact legacy runtimes in synthetic host state. Never production admission.
pub mod cases;
pub use crate::ptoken_proof::{bytes, kh, sha, source_map, vm};
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};

pub const SOURCE: &str = "contracts/ERC20.sol";
pub const WKEYDAO: &str = "0x194b302a4b0a79795fb68e2adf1b8c9ec5ff8d1f";
pub const GOT: &str = "0x701add4311e85c1f9c1549319fe2c476bc8a1b8b";
pub const FIXTURE: &str = "erc20/balances/tests/fixtures/wkey-got-operation-proof";
pub const SOLC_VERSION: &str = "0.7.5+commit.eb77ed08";
pub const SOLC_PIN: &str = "16a99b8c26ed33a91796e209ff6797ee7baf2b0d";
pub const SOLC_SHA: &str = "1c100ce86a3167fd4c194290aafec0d3d94fe86c7a1aa0837c1346cc93d8b6ce";
pub const SOLC_KECCAK: &str = "0xff52724ea7d3e0913219c765b40f311f72fe1e5c95389020a165a1959e57e24f";
pub const MANIFEST_SHA: &str = "22e8ba1c7c8d0fc5eb60964083b238d267ea2c1afec9757521fea20c45b206af";
pub const OZ_PIN: &str = "8e0296096449d9b1cd7c5631e917330635244c37";
pub const LICENSE_SHA: &str = "5d77daf99ea6e8033e57b449761ab4e9b486c45ef578c387d2805fe98e560f46";
pub const PRIMARY_GAP: &str = "WKEYDAO has four and GOT six complete exact OpenZeppelin 3.4.2 dependency bodies. Neither distinct custom contracts/ERC20.sol has an independent public maintainer revision established in reviewed material. Exact captured source/compiler/runtime bindings do not qualify current deployment, initial sets, producer visibility or live package/getter/holder behavior.";
pub const LIMITS: &str = "Host-only independent execution of two whole captured runtimes and bounded constructor paths in synthetic local state. No production validator/candidate, external-call emulator, signature precompile or new VM opcode. Unsupported contexts remain HarnessFailure, separate from source REVERT/INVALID. No chain calls or deployment qualification.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Wkeydao,
    Got,
}
impl Target {
    pub const ALL: [Self; 2] = [Self::Wkeydao, Self::Got];
    pub fn label(self) -> &'static str {
        match self {
            Self::Wkeydao => "wkeydao",
            Self::Got => "got",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Wkeydao => "WKEYDAO",
            Self::Got => "GOT",
        }
    }
    pub fn address(self) -> &'static str {
        match self {
            Self::Wkeydao => WKEYDAO,
            Self::Got => GOT,
        }
    }
    pub fn account(self) -> U256 {
        address(self.address())
    }
    pub fn role_root(self) -> U256 {
        match self {
            Self::Wkeydao => 9.into(),
            Self::Got => 8.into(),
        }
    }
    pub fn runtime_len(self) -> usize {
        match self {
            Self::Wkeydao => 7991,
            Self::Got => 8440,
        }
    }
    pub fn creation_len(self) -> usize {
        match self {
            Self::Wkeydao => 9180,
            Self::Got => 9572,
        }
    }
    pub fn capture_sha(self) -> &'static str {
        match self {
            Self::Wkeydao => "e74c8d858f361712037871c6e5a94c0fb4a81064861f423e2df929b62e02108d",
            Self::Got => "cfbda1cef9b3fb83fb92c388f9a3b0d92b7975378050cc90376028b81f0e6ceb",
        }
    }
    pub fn runtime_hash(self) -> &'static str {
        match self {
            Self::Wkeydao => "0x84d1cbfc7b7c569181930ce930f0dbe6edb8e8df5631b0a066bd0197d109b9f3",
            Self::Got => "0x8f10d493bbd10ba2062c25efaa1cfe1035b392f339a485fce3faed0c0768dc9c",
        }
    }
    pub fn compiled_sha(self) -> &'static str {
        match self {
            Self::Wkeydao => "73a1801e4de6ec8a6c6d22c649de3782996da063b104edbccde843718b12633c",
            Self::Got => "d6a4e5151228a4e89545f1193e349c60ad9e1f1e7c628abda7b6cbf2fae76552",
        }
    }
    pub fn captured(self) -> &'static [u8] {
        match self {
            Self::Wkeydao => include_bytes!("../../../tests/fixtures/wkey-got-operation-proof/wkeydao-capture.json"),
            Self::Got => include_bytes!("../../../tests/fixtures/wkey-got-operation-proof/got-capture.json"),
        }
    }
}
pub fn address(s: &str) -> U256 {
    U256::from_str_radix(s.trim_start_matches("0x"), 16).unwrap()
}
pub fn runtime(capture: &Value) -> Result<Vec<u8>> {
    bytes(&capture["runtimeBytecode"]["onchainBytecode"])
}
pub fn verify_capture(raw: &[u8], target: Target) -> Result<Value> {
    ensure!(sha(raw) == target.capture_sha(), "complete original capture digest");
    let v = serde_json::from_slice(raw)?;
    verify_components(&v, target)?;
    Ok(v)
}
pub fn verify_components(v: &Value, t: Target) -> Result<()> {
    ensure!(
        sha(t.captured()) == t.capture_sha() && *v == serde_json::from_slice::<Value>(t.captured())?,
        "exact complete selected record"
    );
    ensure!(
        v["chainId"] == "56" && address(v["address"].as_str().context("address")?) == t.account(),
        "network/address"
    );
    for f in ["match", "runtimeMatch", "creationMatch"] {
        ensure!(v[f] == "exact_match", "original match label");
    }
    let settings = json!({"libraries":{},"optimizer":{"enabled":true,"runs":200}});
    ensure!(
        v["compilation"]["compilerVersion"] == SOLC_VERSION && v["metadata"]["compiler"]["version"] == SOLC_VERSION,
        "compiler version"
    );
    ensure!(
        v["compilation"]["compilerSettings"] == settings && v["stdJsonInput"]["settings"] == settings && v["metadata"]["settings"]["evmVersion"] == "istanbul",
        "exact settings/default Istanbul"
    );
    ensure!(
        v["compilation"]["fullyQualifiedName"] == format!("{SOURCE}:{}", t.name()) && v["sources"] == v["stdJsonInput"]["sources"],
        "exact target/source input"
    );
    let count = if t == Target::Wkeydao { 5 } else { 7 };
    ensure!(
        v["sources"].as_object().context("source set")?.len() == count
            && v["metadata"]["sources"].as_object().context("metadata sources")?.len() == count
            && v["sourceIds"] == v["stdJsonOutput"]["sources"],
        "complete source sets/IDs"
    );
    for (path, s) in v["sources"].as_object().unwrap() {
        ensure!(
            v["metadata"]["sources"][path]["keccak256"] == kh(s["content"].as_str().context("body")?.as_bytes()),
            "source metadata {path}"
        );
    }
    let r = &v["runtimeBytecode"];
    let code = runtime(v)?;
    ensure!(
        code.len() == t.runtime_len() && kh(&code) == t.runtime_hash() && code == bytes(&r["recompiledBytecode"])?,
        "whole runtime without substitutions"
    );
    ensure!(
        r["transformations"] == json!([])
            && r["transformationValues"] == json!({})
            && r["immutableReferences"] == json!({})
            && r["linkReferences"] == json!({}),
        "no runtime patches/links"
    );
    let creation = bytes(&v["creationBytecode"]["recompiledBytecode"])?;
    let args = bytes(&v["creationBytecode"]["transformationValues"]["constructorArguments"])?;
    ensure!(
        creation.len() == t.creation_len()
            && args.len() == 96
            && v["creationBytecode"]["transformations"] == json!([{"type":"insert","offset":t.creation_len(),"reason":"constructorArguments"}])
            && v["creationBytecode"]["linkReferences"] == json!({}),
        "only exact constructor append"
    );
    ensure!(
        [creation, args].concat() == bytes(&v["creationBytecode"]["onchainBytecode"])?,
        "whole appended creation"
    );
    verify_layout(&v["storageLayout"], t)?;
    Ok(())
}
pub fn verify_layout(layout: &Value, t: Target) -> Result<()> {
    let mut names = vec![
        "_balances",
        "_allowances",
        "_totalSupply",
        "_name",
        "_symbol",
        "_decimals",
        "MaxSupply",
        "_nonces",
    ];
    if t == Target::Wkeydao {
        names.push("DOMAIN_SEPARATOR");
    }
    names.extend(["_roles", "mainPair", "feeReceiver", "buyFeeReceiver", "feeRatio", "buyFeeRatio"]);
    let fields = layout["storage"].as_array().context("layout")?;
    ensure!(fields.len() == names.len(), "complete storage declaration set");
    for (i, (f, name)) in fields.iter().zip(names).enumerate() {
        let slot = i.to_string();
        ensure!(f["label"] == name && f["slot"] == slot && f["offset"] == 0, "exact declaration {name}");
    }
    let rt = fields[t.role_root().as_usize()]["type"].as_str().context("role type")?;
    let types = &layout["types"];
    let role_type = &types[types[rt]["value"].as_str().context("role struct")?];
    ensure!(
        types[rt]["key"] == "t_bytes32" && role_type["numberOfBytes"] == "96",
        "role bytes32/three-word type"
    );
    let members = role_type["members"].as_array().context("role members")?;
    ensure!(
        members.len() == 2 && members[0]["label"] == "members" && members[0]["slot"] == "0" && members[1]["label"] == "adminRole" && members[1]["slot"] == "2",
        "set/admin layout"
    );
    // The whole original layout and fresh compiler layout are additionally exact-bound.
    Ok(())
}
pub fn primary_url(path: &str, t: Target) -> Result<Option<String>> {
    if path == SOURCE {
        return Ok(None);
    }
    let tail = path.strip_prefix("@openzeppelin/contracts/").context("dependency prefix")?;
    let ordinary = ["access/AccessControl.sol", "utils/Address.sol", "utils/Context.sol", "utils/EnumerableSet.sol"];
    ensure!(
        ordinary.contains(&tail) || (t == Target::Got && ["cryptography/ECDSA.sol", "token/ERC20/IERC20.sol"].contains(&tail)),
        "selected dependency only"
    );
    Ok(Some(format!(
        "https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{OZ_PIN}/contracts/{tail}"
    )))
}
pub fn verify_primary(c: &Value, primary: &Value, t: Target) -> Result<()> {
    ensure!(primary["source_gap"] == PRIMARY_GAP && primary["target"] == t.label(), "primary scope/target");
    let records = primary["sources"].as_array().context("primary records")?;
    ensure!(
        records.len() == c["sources"].as_object().context("sources")?.len(),
        "complete primary record set"
    );
    for ((path, s), p) in c["sources"].as_object().unwrap().iter().zip(records) {
        let body = s["content"].as_str().context("source body")?;
        let url = primary_url(path, t)?;
        ensure!(
            p["path"] == *path && p["capture_sha256"] == sha(body.as_bytes()) && p["url"] == json!(url),
            "primary identity"
        );
        if url.is_some() {
            ensure!(
                p["classification"] == "exact" && p["content"] == body && p["sha256"] == sha(body.as_bytes()),
                "exact dependency body"
            );
        } else {
            ensure!(
                p["classification"] == "unestablished" && p["content"].is_null() && p["sha256"].is_null(),
                "custom source gap"
            );
        }
    }
    Ok(())
}
pub fn input(c: &Value) -> Result<Value> {
    let mut v = c["stdJsonInput"].clone();
    ensure!(v["settings"].get("outputSelection").is_none(), "original outputSelection absent");
    v["settings"]["outputSelection"] =
        json!({"*":{"*":["abi","metadata","devdoc","userdoc","storageLayout","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}});
    Ok(v)
}
pub fn check_compiled(c: &Value, raw: &[u8], t: Target) -> Result<Value> {
    let out: Value = serde_json::from_slice(raw)?;
    ensure!(
        !out["errors"].as_array().is_some_and(|a| a.iter().any(|e| e["severity"] == "error")),
        "compiler errors"
    );
    ensure!(out["sources"] == c["sourceIds"], "fresh source IDs");
    let a = &out["contracts"][SOURCE][t.name()];
    let b = &c["stdJsonOutput"]["contracts"][SOURCE][t.name()];
    for field in ["abi", "metadata", "devdoc", "userdoc", "storageLayout"] {
        ensure!(a[field] == b[field], "fresh exact {field}");
    }
    verify_layout(&a["storageLayout"], t)?;
    for (field, kind) in [("runtimeBytecode", "deployedBytecode"), ("creationBytecode", "bytecode")] {
        ensure!(
            bytes(&a["evm"][kind]["object"])? == bytes(&c[field]["recompiledBytecode"])?
                && a["evm"][kind]["sourceMap"] == c[field]["sourceMap"]
                && a["evm"][kind]["linkReferences"] == json!({}),
            "whole code/map/links"
        );
        mapped_sources(c, &out, t, kind)?;
    }
    ensure!(a["evm"]["deployedBytecode"]["immutableReferences"] == json!({}), "no fresh immutables");
    Ok(out)
}
pub fn verify_compiled(c: &Value, raw: &[u8], t: Target) -> Result<Value> {
    ensure!(sha(raw) == t.compiled_sha(), "complete per-runtime compiler output digest");
    check_compiled(c, raw, t)
}
pub fn mapped_sources(c: &Value, out: &Value, t: Target, kind: &str) -> Result<BTreeMap<i64, (String, String)>> {
    let mut result = BTreeMap::new();
    for (path, id) in out["sources"].as_object().context("source IDs")? {
        ensure!(
            result
                .insert(
                    id["id"].as_i64().context("source ID")?,
                    (path.clone(), c["sources"][path]["content"].as_str().context("source body")?.into())
                )
                .is_none(),
            "duplicate source ID"
        );
    }
    ensure!(
        out["contracts"][SOURCE][t.name()]["evm"][kind]["generatedSources"] == json!([]),
        "no generated sources in selected build"
    );
    Ok(result)
}
pub fn verify_auxiliary(dir: &Path, c: &Value, t: Target) -> Result<()> {
    let read = |name: &str| -> Result<Value> { Ok(serde_json::from_slice(&fs::read(dir.join(name))?)?) };
    ensure!(
        read("compiler-input-original.json")? == c["stdJsonInput"] && read("compiler-input.json")? == input(c)?,
        "exact compiler inputs"
    );
    let original = c["stdJsonOutput"]["contracts"][SOURCE][t.name()]["metadata"].as_str().context("metadata")?;
    ensure!(
        fs::read(dir.join("metadata-original.json"))? == original.as_bytes() && fs::read(dir.join("metadata-fresh.json"))? == original.as_bytes(),
        "both exact raw metadata strings"
    );
    verify_primary(c, &read("primary-sources.json")?, t)
}
pub fn verify_source_directory(dir: &Path) -> Result<Vec<(Target, Value, Value)>> {
    ensure!(
        sha(&fs::read(dir.join("solc-list.json"))?) == MANIFEST_SHA && sha(&fs::read(dir.join("LICENSE-openzeppelin"))?) == LICENSE_SHA,
        "compiler manifest/license pins"
    );
    ensure!(
        fs::read_to_string(dir.join("compiler-version.txt"))?
            == "solc, the solidity compiler commandline interface\nVersion: 0.7.5+commit.eb77ed08.Darwin.appleclang\n",
        "official compiler version"
    );
    let mut result = vec![];
    for t in Target::ALL {
        let d = dir.join(t.label());
        let c = verify_capture(&fs::read(d.join("capture.json"))?, t)?;
        verify_auxiliary(&d, &c, t)?;
        let out = verify_compiled(&c, &fs::read(d.join("compiler-output.json"))?, t)?;
        result.push((t, c, out));
    }
    Ok(result)
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
        ("wkey_got_proof/mod.rs", include_str!("mod.rs")),
        ("wkey_got_proof/cases.rs", include_str!("cases.rs")),
        ("bin/execute_wkey_got_proof.rs", include_str!("../bin/execute_wkey_got_proof.rs")),
        ("bin/build_wkey_got_proof.rs", include_str!("../bin/build_wkey_got_proof.rs")),
        ("ptoken_proof/vm.rs", include_str!("../ptoken_proof/vm.rs")),
        ("ptoken_proof/source_map.rs", include_str!("../ptoken_proof/source_map.rs")),
        ("ptoken_proof/cases.rs", include_str!("../ptoken_proof/cases.rs")),
        ("ptoken_proof/vm_tests.rs", include_str!("../ptoken_proof/vm_tests.rs")),
        (
            "../tests/wkey_got_operation_binding.rs",
            include_str!("../../tests/wkey_got_operation_binding.rs"),
        ),
        ("../tests/wkey_got_operation_cases.rs", include_str!("../../tests/wkey_got_operation_cases.rs")),
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
