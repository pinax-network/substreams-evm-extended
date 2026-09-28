//! Host-only proof for one exact PTokenV2 capture. No ingestion configuration.
pub mod cases;
pub mod source_map;
pub mod vm;
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
pub const CONTRACT: &str = "0xc63961bf9a7d6bb5844524851d04fbd76dbe915b";
pub const CAPTURE: &str = "14d5dcc50d8dd1b04fc6283e0d1fafad66fd7fe0bfafdff224a9ba6eda203f9c";
pub const SOURCE_SHA: &str = "cb4a65707a1304bc98e870bb7b7fa72f757e4a249948415ef63a92f1eeec1f1e";
pub const SOURCE: &str = "src/v2/PToken.sol";
pub const NAME: &str = "PTokenV2";
pub const RUNTIME: &str = "0x60f53552d1ab18923098e47b0d7952ae7a4a48d8bece60b20d3d41734832c15c";
pub const SOLC_PIN: &str = "16a99b8c26ed33a91796e209ff6797ee7baf2b0d";
pub const SOLC_SHA: &str = "81515b0e53deaa266d549545ccaac0a5a96e6d4e8201c77f673b2c710976d9ea";
pub const SOLC_VERSION: &str = "0.8.28+commit.7893614a";
pub const OZ_PIN: &str = "c64a1edb67b6e3f4a15cca8909c9482ad33a02b0";
pub const PRIMARY_GAP:&str="No exact independent public token repository revision was recovered for src/v2/PToken.sol. The twenty dependency sources have exact OpenZeppelin v5.4.0 pins; they do not establish the token's primary repository.";
pub fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}
pub fn bytes(v: &Value) -> Result<Vec<u8>> {
    Ok(hex::decode(v.as_str().context("hex string")?.trim_start_matches("0x"))?)
}
pub fn kh(b: &[u8]) -> String {
    format!("0x{}", hex::encode(erc20_balances::hash(b)))
}
pub fn verify_capture(raw: &[u8]) -> Result<Value> {
    ensure!(sha(raw) == CAPTURE, "complete original capture digest");
    let v: Value = serde_json::from_slice(raw)?;
    ensure!(v["chainId"] == "56" && bytes(&v["address"])? == hex::decode(&CONTRACT[2..])?, "network/address");
    for field in ["match", "runtimeMatch", "creationMatch"] {
        ensure!(v[field] == "match", "original match label");
    }
    ensure!(
        v["compilation"]["compilerVersion"] == SOLC_VERSION && v["metadata"]["compiler"]["version"] == SOLC_VERSION,
        "compiler identity"
    );
    ensure!(
        v["sources"] == v["stdJsonInput"]["sources"] && v["sources"].as_object().context("sources")?.len() == 21,
        "complete source/input set"
    );
    ensure!(v["sourceIds"] == v["stdJsonOutput"]["sources"], "source IDs/output");
    let sources = v["sources"].as_object().unwrap();
    ensure!(
        sources.keys().eq(v["metadata"]["sources"].as_object().context("source metadata")?.keys())
            && sources.keys().eq(v["sourceIds"].as_object().context("source IDs")?.keys()),
        "complete source sets"
    );
    for (name, s) in sources {
        ensure!(
            v["metadata"]["sources"][name]["keccak256"] == kh(s["content"].as_str().context("content")?.as_bytes()),
            "source hash {name}"
        );
    }
    ensure!(
        sha(v["sources"][SOURCE]["content"].as_str().context("token source")?.as_bytes()) == SOURCE_SHA,
        "exact token source"
    );
    ensure!(
        v["stdJsonInput"]["settings"] == v["compilation"]["compilerSettings"],
        "original compiler input settings"
    );
    let output = &v["stdJsonOutput"]["contracts"][SOURCE][NAME];
    let metadata: Value = serde_json::from_str(output["metadata"].as_str().context("output metadata")?)?;
    ensure!(metadata == v["metadata"], "saved output metadata");
    for f in ["abi", "devdoc", "userdoc", "storageLayout", "transientStorageLayout"] {
        ensure!(output[f] == v[f], "saved output {f}");
    }
    let runtime = &v["runtimeBytecode"];
    for f in ["immutableReferences", "linkReferences", "cborAuxdata", "transformationValues"] {
        ensure!(runtime[f] == json!({}), "no runtime {f}");
    }
    ensure!(runtime["transformations"] == json!([]), "no runtime transformations");
    let code = bytes(&runtime["onchainBytecode"])?;
    ensure!(
        code.len() == 6065
            && kh(&code) == RUNTIME
            && code == bytes(&runtime["recompiledBytecode"])?
            && code == bytes(&output["evm"]["deployedBytecode"]["object"])?,
        "whole runtime equality/hash"
    );
    let creation = &v["creationBytecode"];
    ensure!(
        creation["linkReferences"] == json!({}) && creation["cborAuxdata"] == json!({}),
        "no creation links/CBOR"
    );
    ensure!(
        creation["transformations"] == json!([{"type":"insert","offset":7220,"reason":"constructorArguments"}]),
        "sole constructor append"
    );
    let args = constructor_arguments();
    ensure!(
        creation["transformationValues"] == json!({"constructorArguments":format!("0x{}",hex::encode(&args))}),
        "independent constructor ABI encoding"
    );
    let compiled = bytes(&creation["recompiledBytecode"])?;
    ensure!(
        compiled.len() == 7220 && compiled == bytes(&output["evm"]["bytecode"]["object"])?,
        "creation compiler binding"
    );
    let saved = [compiled, args].concat();
    ensure!(
        saved == bytes(&creation["onchainBytecode"])? && kh(&saved) == "0x0486348dd8630803fb8906c5015b6264f826dba7f180d35a129920edab2ce6b4",
        "whole creation binding"
    );
    Ok(v)
}
/// Exact stdout from the saved official 0.8.28 invocation; this also binds
/// generated Yul sources absent from the original reduced-output capture.
pub const COMPILED_SHA: &str = "c85b99ca52a3ef59a6aeeefe10afd2c7c95ab852754293ab16b1489e199da891";
pub fn verify_compiled(capture: &Value, raw: &[u8]) -> Result<Value> {
    ensure!(sha(raw) == COMPILED_SHA, "complete fresh compiler output digest");
    let compiled: Value = serde_json::from_slice(raw)?;
    ensure!(compiled["sources"] == capture["sourceIds"], "complete fresh source IDs");
    let actual = &compiled["contracts"][SOURCE][NAME];
    let saved = &capture["stdJsonOutput"]["contracts"][SOURCE][NAME];
    for field in ["abi", "devdoc", "userdoc", "storageLayout"] {
        ensure!(actual[field] == saved[field], "fresh {field}");
    }
    let metadata: Value = serde_json::from_str(actual["metadata"].as_str().context("fresh metadata")?)?;
    ensure!(metadata == capture["metadata"], "parsed metadata equality");
    for (field, kind) in [("runtimeBytecode", "deployedBytecode"), ("creationBytecode", "bytecode")] {
        ensure!(
            bytes(&actual["evm"][kind]["object"])? == bytes(&capture[field]["recompiledBytecode"])?,
            "exact fresh bytecode"
        );
        ensure!(actual["evm"][kind]["sourceMap"] == capture[field]["sourceMap"], "exact fresh source map");
    }
    Ok(compiled)
}
pub fn constructor_arguments() -> Vec<u8> {
    use primitive_types::U256;
    let mut b = vec![];
    for offset in [128u64, 192] {
        b.extend(vm::word(offset.into()));
    }
    let address = U256::from_big_endian(&hex::decode("bf29536989200077547c3270e3f48e729f7880f8").unwrap());
    b.extend(vm::word(address));
    b.extend(vm::word(address));
    for name in [b"ALPHEA Point Token".as_slice(), b"APT".as_slice()] {
        b.extend(vm::word(name.len().into()));
        b.extend(name);
        b.resize(b.len().div_ceil(32) * 32, 0);
    }
    b
}

#[cfg(test)]
mod vm_tests;

/// Preserve the compiler-embedded proof sources before attempting any operation.
/// These must match the checkout, so a stale executable cannot label new code as run.
pub fn snapshot_sources(out: &std::path::Path) -> Result<String> {
    use std::{collections::BTreeMap, fs, path::Path};
    let own = [
        ("mod.rs", include_str!("mod.rs")),
        ("vm.rs", include_str!("vm.rs")),
        ("vm_tests.rs", include_str!("vm_tests.rs")),
        ("cases.rs", include_str!("cases.rs")),
        ("source_map.rs", include_str!("source_map.rs")),
        ("../../tests/ptoken_operation_proof.rs", include_str!("../../tests/ptoken_operation_proof.rs")),
        ("../bin/build_ptoken_proof.rs", include_str!("../bin/build_ptoken_proof.rs")),
        ("../bin/execute_ptoken_proof.rs", include_str!("../bin/execute_ptoken_proof.rs")),
    ];
    fs::create_dir(out.join("as-run"))?;
    for (name, text) in own {
        let path = Path::new("erc20/balances/tools/src/ptoken_proof").join(name);
        ensure!(fs::read(&path)? == text.as_bytes(), "stale executable source {}", path.display());
        fs::write(out.join("as-run").join(Path::new(name).file_name().unwrap()), text)?;
    }
    fn walk(path: &Path, files: &mut BTreeMap<String, Value>) -> Result<()> {
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                walk(&entry?.path(), files)?;
            }
        } else if path.extension().is_some_and(|s| s == "rs" || s == "toml" || s == "lock") {
            let raw = fs::read(path)?;
            files.insert(path.to_string_lossy().into(), json!({"sha256":sha(&raw),"bytes":raw.len()}));
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    for path in [
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
        walk(Path::new(path), &mut files)?;
    }
    let raw = serde_json::to_vec_pretty(&json!({"scope":"host tool and local host dependency source inventory; no chain calls","files":files}))?;
    fs::write(out.join("source-inventory.json"), &raw)?;
    Ok(sha(&raw))
}
