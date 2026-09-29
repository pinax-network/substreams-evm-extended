//! Selected ERC20TokenX source/runtime proof. Host evidence, never admission.
pub mod cases;
pub use crate::ptoken_proof::{bytes, kh, sha, source_map, vm};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};
pub const SOURCE: &str = "contracts/core/ERC20.sol";
pub const NAME: &str = "ERC20TokenX";
pub const ORI: &str = "0xda033999bb6165e64db01bd9be14b40f5653092e";
pub const FNA: &str = "0x08332a515cb2a57884176e887b682e7da2eb114e";
pub const PHI: &str = "0xa71add46ea4fbf0058b36e6baa39530f8e48b103";
pub const RUNTIME_HASH: &str = "0xbaaa46c4d0133a30bd1020c46f86c0ea4ac0609d18317a95fc923efb5b22afdf";
pub const SOLC_VERSION: &str = "0.7.5+commit.eb77ed08";
pub const SOLC_PIN: &str = "16a99b8c26ed33a91796e209ff6797ee7baf2b0d";
pub const SOLC_SHA: &str = "1c100ce86a3167fd4c194290aafec0d3d94fe86c7a1aa0837c1346cc93d8b6ce";
pub const SOLC_KECCAK: &str = "0xff52724ea7d3e0913219c765b40f311f72fe1e5c95389020a165a1959e57e24f";
pub const MANIFEST_SHA: &str = "22e8ba1c7c8d0fc5eb60964083b238d267ea2c1afec9757521fea20c45b206af";
pub const OZ_PIN: &str = "8e0296096449d9b1cd7c5631e917330635244c37";
pub const COMPILED_SHA: &str = "ddc187357403c4cd7dd9d70cdfc63603a36178fe33e58cf986c4af585318aa61";
pub const FIXTURE: &str = "erc20/balances/tests/fixtures/erc20tokenx-operation-proof";
pub const PRIMARY_GAP: &str = "The four complete OpenZeppelin dependencies exactly match 3.4.2. No independent public revision for custom contracts/core/ERC20.sol is established in reviewed material. PHI has no individual source/creation/deployment record; attribution uses only its separately captured exact whole runtime. No live qualification, deployed initial-set coherence or producer visibility is established.";
pub const CAPTURES: [(&str, &str, &str); 2] = [
    ("ori", ORI, "dc3274a3ed22c01d0e47a84efd72f419a5123bdb56e6be2aca9df3dd51039073"),
    ("fna", FNA, "24f4fa1fc512e62d50384358f58099318d2975a95025ee812abd821639bfee43"),
];
pub fn address(s: &str) -> primitive_types::U256 {
    primitive_types::U256::from_str_radix(s.trim_start_matches("0x"), 16).unwrap()
}
pub fn runtime(v: &Value) -> Result<Vec<u8>> {
    bytes(&v["runtimeBytecode"]["onchainBytecode"])
}
pub fn verify_capture(raw: &[u8], index: usize) -> Result<Value> {
    let spec = CAPTURES.get(index).context("capture index")?;
    ensure!(sha(raw) == spec.2, "complete original {} capture digest", spec.0);
    let v: Value = serde_json::from_slice(raw)?;
    verify_components(&v, index)?;
    Ok(v)
}
pub fn verify_components(v: &Value, index: usize) -> Result<()> {
    // Complete frozen records are independently SHA-bound above, including all
    // deployment fields and constructor arguments. No subset can be relabeled.
    let raw: &[u8] = match index {
        0 => include_bytes!("../../../tests/fixtures/erc20tokenx-operation-proof/ori-capture.json"),
        1 => include_bytes!("../../../tests/fixtures/erc20tokenx-operation-proof/fna-capture.json"),
        _ => anyhow::bail!("capture index"),
    };
    ensure!(
        sha(raw) == CAPTURES[index].2 && *v == serde_json::from_slice::<Value>(raw)?,
        "exact complete selected record"
    );
    ensure!(
        v["chainId"] == "56" && address(v["address"].as_str().context("address")?) == address(CAPTURES[index].1),
        "network/address"
    );
    for field in ["match", "runtimeMatch", "creationMatch"] {
        ensure!(v[field] == "exact_match", "match label");
    }
    ensure!(
        v["compilation"]["compilerVersion"] == SOLC_VERSION && v["metadata"]["compiler"]["version"] == SOLC_VERSION,
        "compiler"
    );
    let settings = json!({"libraries":{},"optimizer":{"enabled":true,"runs":200}});
    ensure!(
        v["compilation"]["compilerSettings"] == settings && v["stdJsonInput"]["settings"] == settings,
        "original settings, default Istanbul"
    );
    ensure!(
        v["compilation"]["fullyQualifiedName"] == format!("{SOURCE}:{NAME}") && v["sources"] == v["stdJsonInput"]["sources"],
        "target/sources"
    );
    ensure!(
        v["sources"].as_object().context("sources")?.len() == 5 && v["sourceIds"] == v["stdJsonOutput"]["sources"],
        "complete source IDs"
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
        code.len() == 7896 && kh(&code) == RUNTIME_HASH && code == bytes(&r["recompiledBytecode"])?,
        "whole runtime exact without substitution"
    );
    ensure!(
        r["transformations"] == json!([])
            && r["transformationValues"] == json!({})
            && r["immutableReferences"] == json!({})
            && r["linkReferences"] == json!({}),
        "no runtime replacements/links"
    );
    let creation = bytes(&v["creationBytecode"]["recompiledBytecode"])?;
    let args = bytes(&v["creationBytecode"]["transformationValues"]["constructorArguments"])?;
    ensure!(
        creation.len() == 9347 && v["creationBytecode"]["transformations"] == json!([{"type":"insert","offset":9347,"reason":"constructorArguments"}]),
        "only exact constructor append"
    );
    ensure!(
        [creation, args].concat() == bytes(&v["creationBytecode"]["onchainBytecode"])?,
        "whole captured creation including append"
    );
    let fields: Vec<_> = v["storageLayout"]["storage"]
        .as_array()
        .context("layout")?
        .iter()
        .map(|s| (s["label"].as_str().unwrap(), s["slot"].as_str().unwrap()))
        .collect();
    ensure!(
        fields
            == vec![
                ("_balances", "0"),
                ("_allowances", "1"),
                ("_totalSupply", "2"),
                ("_name", "3"),
                ("_symbol", "4"),
                ("_decimals", "5"),
                ("_nonces", "6"),
                ("DOMAIN_SEPARATOR", "7"),
                ("_roles", "8"),
                ("mainPair", "9"),
                ("feeReceiver", "10"),
                ("sellFeeRatio", "11"),
                ("buyFeeRatio", "12")
            ],
        "full selected linear layout"
    );
    Ok(())
}
pub fn verify_phi(path: &Path, expected: &[u8]) -> Result<()> {
    for (name, digest) in [
        ("phi-source-request.json", "22e72c35e6291c2c849d2c55740d8f978495be80ac62c37e8d5098e91f2bbed1"),
        ("phi-runtime.hex", "7b959b3716c57ba670cbdd1f3ab90970c1e52309d311a16bdbb868daa6b62600"),
        ("phi-runtime-report.json", "0d6726eac4c0923fa6b8c40e44fe4d639f6f79fb10fb5b5942482e4f64ba7f45"),
    ] {
        ensure!(sha(&fs::read(path.join(name))?) == digest, "exact historical PHI {name}");
    }
    let source: Value = serde_json::from_slice(&fs::read(path.join("phi-source-request.json"))?)?;
    ensure!(
        source.as_object().context("PHI response")?.len() == 5
            && source["match"].is_null()
            && source.get("sources").is_none()
            && source.get("deployment").is_none(),
        "PHI missing source remains explicit"
    );
    let code = fs::read_to_string(path.join("phi-runtime.hex"))?;
    ensure!(
        hex::decode(code.trim().trim_start_matches("0x"))? == expected,
        "PHI exact entire runtime attribution"
    );
    let report: Value = serde_json::from_slice(&fs::read(path.join("phi-runtime-report.json"))?)?;
    ensure!(
        report["contract"] == PHI && report["runtime_keccak256"] == RUNTIME_HASH && report["block"] == 122288067,
        "historical PHI identity/block"
    );
    Ok(())
}
pub fn primary_url(path: &str) -> Result<Option<String>> {
    if path == SOURCE {
        return Ok(None);
    }
    let tail = path.strip_prefix("@openzeppelin/contracts/").context("known dependency prefix")?;
    ensure!(
        ["access/AccessControl.sol", "utils/Address.sol", "utils/Context.sol", "utils/EnumerableSet.sol"].contains(&tail),
        "known dependency"
    );
    Ok(Some(format!(
        "https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{OZ_PIN}/contracts/{tail}"
    )))
}
pub fn verify_primary(capture: &Value, primary: &Value) -> Result<()> {
    ensure!(primary["source_gap"] == PRIMARY_GAP, "exact provenance boundary");
    let records = primary["sources"].as_array().context("primary records")?;
    ensure!(records.len() == 5, "all primary classifications");
    for ((name, s), p) in capture["sources"].as_object().context("sources")?.iter().zip(records) {
        let body = s["content"].as_str().context("body")?;
        let url = primary_url(name)?;
        ensure!(
            p["path"] == *name && p["capture_sha256"] == sha(body.as_bytes()) && p["url"] == json!(url),
            "primary identity"
        );
        if url.is_some() {
            ensure!(
                p["classification"] == "exact" && p["content"] == body && p["sha256"] == sha(body.as_bytes()),
                "exact pinned dependency"
            );
        } else {
            ensure!(
                p["classification"] == "unestablished" && p["content"].is_null() && p["sha256"].is_null(),
                "custom source gap preserved"
            );
        }
    }
    Ok(())
}
pub fn input(capture: &Value) -> Result<Value> {
    let mut v = capture["stdJsonInput"].clone();
    ensure!(v["settings"].get("outputSelection").is_none(), "original output selection absent");
    v["settings"]["outputSelection"] =
        json!({"*":{"*":["abi","metadata","devdoc","userdoc","storageLayout","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}});
    Ok(v)
}
pub fn verify_auxiliary(path: &Path, capture: &Value) -> Result<()> {
    let read = |n: &str| -> Result<Value> { Ok(serde_json::from_slice(&fs::read(path.join(n))?)?) };
    ensure!(
        read("compiler-input-original.json")? == capture["stdJsonInput"] && read("compiler-input.json")? == input(capture)?,
        "original sources/settings retained"
    );
    ensure!(
        sha(&fs::read(path.join("solc-list.json"))?) == MANIFEST_SHA,
        "official complete compiler manifest"
    );
    ensure!(
        fs::read_to_string(path.join("compiler-version.txt"))?
            == "solc, the solidity compiler commandline interface\nVersion: 0.7.5+commit.eb77ed08.Darwin.appleclang\n",
        "exact compiler version"
    );
    ensure!(
        sha(&fs::read(path.join("LICENSE-openzeppelin"))?) == "5d77daf99ea6e8033e57b449761ab4e9b486c45ef578c387d2805fe98e560f46",
        "dependency notice"
    );
    Ok(())
}
pub fn check_compiled(capture: &Value, raw: &[u8]) -> Result<Value> {
    let out: Value = serde_json::from_slice(raw)?;
    ensure!(
        !out["errors"].as_array().is_some_and(|a| a.iter().any(|e| e["severity"] == "error")),
        "compiler diagnostics"
    );
    ensure!(out["sources"] == capture["sourceIds"], "fresh source IDs");
    let a = &out["contracts"][SOURCE][NAME];
    let b = &capture["stdJsonOutput"]["contracts"][SOURCE][NAME];
    for field in ["abi", "metadata", "devdoc", "userdoc", "storageLayout"] {
        ensure!(a[field] == b[field], "fresh exact {field}");
    }
    for (field, kind) in [("runtimeBytecode", "deployedBytecode"), ("creationBytecode", "bytecode")] {
        ensure!(
            bytes(&a["evm"][kind]["object"])? == bytes(&capture[field]["recompiledBytecode"])?
                && a["evm"][kind]["sourceMap"] == capture[field]["sourceMap"]
                && a["evm"][kind]["linkReferences"] == json!({}),
            "fresh whole code/map/links"
        );
        mapped_sources(capture, &out, kind)?;
    }
    ensure!(a["evm"]["deployedBytecode"]["immutableReferences"] == json!({}), "no immutables");
    Ok(out)
}
pub fn verify_compiled(capture: &Value, raw: &[u8]) -> Result<Value> {
    ensure!(sha(raw) == COMPILED_SHA, "complete fresh compiler output digest");
    check_compiled(capture, raw)
}
pub fn mapped_sources(capture: &Value, compiled: &Value, kind: &str) -> Result<BTreeMap<i64, (String, String)>> {
    let mut result = BTreeMap::new();
    for (path, info) in compiled["sources"].as_object().context("source IDs")? {
        ensure!(
            result
                .insert(
                    info["id"].as_i64().context("source ID")?,
                    (path.clone(), capture["sources"][path]["content"].as_str().context("body")?.into())
                )
                .is_none(),
            "duplicate source ID"
        );
    }
    ensure!(
        compiled["contracts"][SOURCE][NAME]["evm"][kind]["generatedSources"] == json!([]),
        "no generated sources in selected 0.7.5 output"
    );
    Ok(result)
}
pub fn snapshot_sources(out: &Path) -> Result<String> {
    let own = [
        ("erc20tokenx_proof/mod.rs", include_str!("mod.rs")),
        ("erc20tokenx_proof/cases.rs", include_str!("cases.rs")),
        ("bin/build_erc20tokenx_proof.rs", include_str!("../bin/build_erc20tokenx_proof.rs")),
        ("bin/execute_erc20tokenx_proof.rs", include_str!("../bin/execute_erc20tokenx_proof.rs")),
        (
            "../tests/erc20tokenx_operation_binding.rs",
            include_str!("../../tests/erc20tokenx_operation_binding.rs"),
        ),
        (
            "../tests/erc20tokenx_operation_cases.rs",
            include_str!("../../tests/erc20tokenx_operation_cases.rs"),
        ),
        ("ptoken_proof/vm.rs", include_str!("../ptoken_proof/vm.rs")),
        ("ptoken_proof/vm_tests.rs", include_str!("../ptoken_proof/vm_tests.rs")),
        ("ptoken_proof/source_map.rs", include_str!("../ptoken_proof/source_map.rs")),
        ("ptoken_proof/cases.rs", include_str!("../ptoken_proof/cases.rs")),
    ];
    fs::create_dir(out.join("as-run"))?;
    for (name, body) in own {
        let p = Path::new("erc20/balances/tools/src").join(name);
        ensure!(fs::read(&p)? == body.as_bytes(), "stale executable source {}", p.display());
        let dest = out.join("as-run").join(name.replace('/', "_"));
        fs::write(dest, body)?;
    }
    fn walk(p: &Path, files: &mut BTreeMap<String, Value>) -> Result<()> {
        if p.is_dir() {
            for e in fs::read_dir(p)? {
                walk(&e?.path(), files)?;
            }
        } else if p.extension().is_some_and(|e| e == "rs" || e == "toml" || e == "lock") {
            let b = fs::read(p)?;
            files.insert(p.to_string_lossy().into(), json!({"sha256":sha(&b),"bytes":b.len()}));
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
        files.insert(p.into(), json!({"sha256":sha(&raw),"bytes":raw.len()}));
    }
    let raw = serde_json::to_vec_pretty(&json!({"scope":"ERC20TokenX host source/runtime proof; no chain calls","files":files}))?;
    fs::write(out.join("source-inventory.json"), &raw)?;
    Ok(sha(&raw))
}
