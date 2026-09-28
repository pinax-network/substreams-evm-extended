//! Two bounded, unqualified source-bound role candidates. No network access.
use anyhow::{ensure, Context, Result};
use erc20_balances::hash;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const BASELINE: &str = "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468";
pub const FIXTURE: &str = "tests/fixtures/point-bedrock-role-candidates";
pub struct Profile {
    pub name: &'static str,
    pub contract: &'static str,
    pub capture: &'static str,
    pub runtime: &'static str,
    pub compiler: &'static str,
    pub source: &'static str,
    pub files: usize,
    pub role_root: u64,
    pub balance_root: u64,
}
pub const PROFILES: [Profile; 2] = [
    Profile {
        name: "Point",
        contract: "0x826923122a8521be36358bdc53d3b4362b6f46e5",
        capture: "379a18f2648c684062838d90735bc4bfee90187d62f15e64e18c5ce6b1f1ae88",
        runtime: "0xa099dede5abb0391e457e8f49f4b61061005b3aea81cd5be261380d99b5659ec",
        compiler: "0.8.9+commit.e5eed63a",
        source: "contracts/Point.sol",
        files: 11,
        role_root: 0,
        balance_root: 1,
    },
    Profile {
        name: "Bedrock",
        contract: "0xff7d6a96ae471bbcd7713af9cb1feeb16cf56b41",
        capture: "4a1d4290e378ee9bf416f83b60097b9bed9ac1c43a22e42137d3874d9c936c0a",
        runtime: "0xf5a9b1fe6edbc5caf0c7e395f1fca9d2bb67f7d232ca90e149507a8b842256c4",
        compiler: "0.8.17+commit.8df45f5f",
        source: "contracts/BR.sol",
        files: 12,
        role_root: 5,
        balance_root: 0,
    },
];
pub fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn bytes(v: &Value) -> Result<Vec<u8>> {
    Ok(hex::decode(v.as_str().context("hex string")?.trim_start_matches("0x"))?)
}
pub fn root(n: u64) -> String {
    format!("0x{n:064x}")
}

pub fn verify_capture(raw: &[u8], p: &Profile) -> Result<Value> {
    ensure!(sha(raw) == p.capture, "{} complete capture hash", p.name);
    let v = serde_json::from_slice(raw)?;
    verify_components(&v, p)?;
    Ok(v)
}
fn verify_components(v: &Value, p: &Profile) -> Result<()> {
    ensure!(
        v["chainId"] == "56" && bytes(&v["address"])? == hex::decode(&p.contract[2..])?,
        "network/address binding"
    );
    ensure!(v["runtimeMatch"] == "exact_match", "runtime match status");
    ensure!(
        v["compilation"]["compiler"] == "solc"
            && v["compilation"]["language"] == "Solidity"
            && v["compilation"]["compilerVersion"] == p.compiler
            && v["metadata"]["compiler"]["version"] == p.compiler,
        "compiler identity"
    );
    ensure!(
        v["compilation"]["name"] == p.name
            && v["compilation"]["fullyQualifiedName"] == format!("{}:{}", p.source, p.name)
            && v["metadata"]["settings"]["compilationTarget"] == json!({p.source:p.name}),
        "compilation target"
    );
    ensure!(
        v["stdJsonInput"]["language"] == "Solidity" && v["stdJsonInput"]["settings"] == v["compilation"]["compilerSettings"],
        "compiler input settings"
    );
    let sources = v["sources"].as_object().context("sources")?;
    let metadata = v["metadata"]["sources"].as_object().context("source metadata")?;
    ensure!(
        sources.len() == p.files && sources.keys().eq(metadata.keys()) && v["sources"] == v["stdJsonInput"]["sources"],
        "complete source/input/metadata set"
    );
    ensure!(
        v["sourceIds"] == v["stdJsonOutput"]["sources"] && sources.keys().eq(v["sourceIds"].as_object().context("source ids")?.keys()),
        "complete compiler source IDs"
    );
    for (name, source) in sources {
        let content = source["content"].as_str().context("source content")?;
        ensure!(
            format!("0x{}", hex::encode(hash(content.as_bytes()))) == metadata[name]["keccak256"],
            "source hash {name}"
        );
    }
    let output = &v["stdJsonOutput"]["contracts"][p.source][p.name];
    let output_metadata: Value = serde_json::from_str(output["metadata"].as_str().context("compiler metadata")?)?;
    ensure!(output_metadata == v["metadata"], "saved compiler metadata binding");
    for field in ["abi", "devdoc", "userdoc", "storageLayout", "transientStorageLayout"] {
        ensure!(output[field] == v[field], "saved compiler output {field}");
    }
    let runtime = &v["runtimeBytecode"];
    ensure!(
        runtime["immutableReferences"] == json!({})
            && runtime["linkReferences"] == json!({})
            && runtime["transformations"] == json!([])
            && runtime["transformationValues"] == json!({}),
        "no runtime transformations/dependencies allowed"
    );
    let compiled = bytes(&runtime["recompiledBytecode"])?;
    ensure!(
        compiled == bytes(&runtime["onchainBytecode"])?
            && compiled == bytes(&output["evm"]["deployedBytecode"]["object"])?
            && format!("0x{}", hex::encode(hash(&compiled))) == p.runtime,
        "exact untransformed saved compiler/runtime binding"
    );
    for field in ["immutableReferences", "linkReferences", "sourceMap"] {
        ensure!(runtime[field] == output["evm"]["deployedBytecode"][field], "compiler runtime {field}");
    }
    // Metadata is checked in place, never stripped or replaced to force a match.
    let aux = runtime["cborAuxdata"].as_object().context("CBOR auxdata")?;
    ensure!(aux.len() == 1, "one original metadata trailer");
    for data in aux.values() {
        let offset = data["offset"].as_u64().context("metadata offset")? as usize;
        ensure!(compiled.get(offset..) == Some(bytes(&data["value"])?.as_slice()), "metadata trailer bytes");
    }
    verify_layout(v, p)?;
    // This bounded lexical check excludes comments/literals; it is not a general
    // Solidity call graph. Full source capture pins make the reviewed set exact.
    let identifiers: Vec<_> = sources.values().flat_map(|s| identifiers(s["content"].as_str().unwrap())).collect();
    ensure!(
        identifiers.iter().filter(|id| id.as_str() == "_setRoleAdmin").count() == 1,
        "unreviewed role-admin callsite"
    );
    let access = sources
        .iter()
        .find(|(name, _)| name.ends_with("/access/AccessControl.sol"))
        .context("AccessControl source")?
        .1["content"]
        .as_str()
        .unwrap();
    for exact in [
        "_roles[role].members[account] = true;",
        "_roles[role].members[account] = false;",
        "_roles[role].adminRole = adminRole;",
        "_grantRole(role, account);",
        "_revokeRole(role, account);",
    ] {
        ensure!(access.contains(exact), "reviewed membership/admin source statement");
    }
    Ok(())
}
fn verify_layout(v: &Value, p: &Profile) -> Result<()> {
    let layout = &v["storageLayout"];
    let fields = layout["storage"].as_array().context("layout")?;
    let names: &[&str] = if p.role_root == 0 {
        &["_roles", "_balances", "_allowances", "_totalSupply", "_name", "_symbol"]
    } else {
        &[
            "_balances",
            "_allowances",
            "_totalSupply",
            "_name",
            "_symbol",
            "_roles",
            "freezeToRecipient",
            "frozenUsers",
        ]
    };
    ensure!(fields.len() == names.len(), "full layout coverage");
    for (i, name) in names.iter().enumerate() {
        let expected_slot = i.to_string();
        ensure!(
            fields[i]["slot"] == expected_slot
                && fields[i]["offset"] == 0
                && fields[i]["label"] == *name
                && fields[i]["contract"] == format!("{}:{}", p.source, p.name),
            "layout slot/name/offset/contract"
        );
    }
    let types = &layout["types"];
    let role = &types[fields[p.role_root as usize]["type"].as_str().context("role type")?];
    ensure!(role["encoding"] == "mapping" && role["key"] == "t_bytes32", "outer bytes32 mapping");
    let record = &types[role["value"].as_str().context("role record")?];
    let members = record["members"].as_array().context("role members")?;
    ensure!(
        record["numberOfBytes"] == "64"
            && members.len() == 2
            && members[0]["slot"] == "0"
            && members[0]["offset"] == 0
            && members[0]["label"] == "members"
            && members[1]["slot"] == "1"
            && members[1]["offset"] == 0
            && members[1]["label"] == "adminRole"
            && members[1]["type"] == "t_bytes32",
        "role struct layout"
    );
    let member = &types[members[0]["type"].as_str().context("member type")?];
    ensure!(
        member["encoding"] == "mapping" && member["key"] == "t_address" && member["value"] == "t_bool" && types["t_bool"]["numberOfBytes"] == "1",
        "address/bool membership"
    );
    ensure!(fields[p.balance_root as usize]["type"] == "t_mapping(t_address,t_uint256)", "balance mapping");
    Ok(())
}
fn identifiers(source: &str) -> Vec<String> {
    let b = source.as_bytes();
    let mut at = 0;
    let mut out = Vec::new();
    while at < b.len() {
        if b[at..].starts_with(b"//") {
            while at < b.len() && b[at] != b'\n' {
                at += 1;
            }
        } else if b[at..].starts_with(b"/*") {
            at += 2;
            while at < b.len() && !b[at..].starts_with(b"*/") {
                at += 1;
            }
            at = (at + 2).min(b.len());
        } else if matches!(b[at], b'\'' | b'"') {
            let q = b[at];
            at += 1;
            while at < b.len() {
                if b[at] == b'\\' {
                    at = (at + 2).min(b.len());
                } else {
                    let end = b[at] == q;
                    at += 1;
                    if end {
                        break;
                    }
                }
            }
        } else if b[at].is_ascii_alphabetic() || b[at] == b'_' {
            let start = at;
            at += 1;
            while at < b.len() && (b[at].is_ascii_alphanumeric() || b[at] == b'_') {
                at += 1;
            }
            out.push(source[start..at].into());
        } else {
            at += 1;
        }
    }
    out
}
pub fn review(captures: &[Value]) -> Result<Value> {
    ensure!(captures.len() == PROFILES.len(), "exact two captures");
    let profiles = captures.iter().zip(&PROFILES).map(|(v,p)| -> Result<Value> {
        verify_components(v,p)?;
        let sources: Vec<_> = v["sources"].as_object().unwrap().iter().map(|(name,s)| json!({"path":name,"sha256":sha(s["content"].as_str().unwrap().as_bytes()),"keccak256":v["metadata"]["sources"][name]["keccak256"]})).collect();
        Ok(json!({"contract":p.contract,"name":p.name,"chain_id":56,"source_capture_sha256":p.capture,"compiler":p.compiler,"source_files":sources,"runtime_hash":p.runtime,"role_root":root(p.role_root),"balance_root":root(p.balance_root),"runtime_transformations":0,"runtime_reconstructed_from_saved_compiler_bytes":true,"fresh_compilation":false,"role_admin_identifier_references":1,"role_admin_callsite":"none_beyond_declaration_in_complete_pinned_capture","qualified":false}))
    }).collect::<Result<Vec<_>>>()?;
    Ok(
        json!({"qualified":false,"scope":"Saved source/compiler/runtime binding and synthetic storage shapes only; not deployment or replacement-package qualification.","profiles":profiles}),
    )
}
pub const BEDROCK_PIN: &str = "48f3eb27d953a00ac7d2b6bc2719052b95ad18bd";
pub const BEDROCK_OZ_PIN: &str = "0a25c1940ca220686588c4af3ec526f725fe2582";
pub const POINT_OZ_PIN: &str = "8c49ad74eae76ee389d038780d407cf90b4ae1de";
pub const POINT_SOURCE_GAP: &str = "No independently pinned public repository for contracts/Point.sol was recovered; its exact saved source/compiler/runtime binding remains available. OpenZeppelin dependencies alone do not establish the token's primary repository.";
pub fn primary_url(p: &Profile, name: &str) -> Result<Option<String>> {
    if name == p.source {
        return Ok(if p.name == "Point" {
            None
        } else {
            Some(format!(
                "https://raw.githubusercontent.com/Bedrock-Technology/BR/{BEDROCK_PIN}/contracts/BR.sol"
            ))
        });
    }
    let (prefix, pin) = if p.name == "Point" {
        ("@openzeppelin/contracts/", POINT_OZ_PIN)
    } else {
        ("lib/OpenZeppelin/openzeppelin-contracts@4.8.3/contracts/", BEDROCK_OZ_PIN)
    };
    let path = name.strip_prefix(prefix).context("reviewed primary source prefix")?;
    Ok(Some(format!(
        "https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{pin}/contracts/{path}"
    )))
}
pub fn verify_primary(captures: &[Value], primary: &Value) -> Result<()> {
    ensure!(
        primary["qualified"] == false && primary["point_token_source_gap"] == POINT_SOURCE_GAP,
        "explicit primary-source limitation"
    );
    let saved = primary["profiles"].as_array().context("primary profiles")?;
    ensure!(saved.len() == 2 && captures.len() == 2, "exact primary profile scope");
    for ((capture, p), saved) in captures.iter().zip(&PROFILES).zip(saved) {
        ensure!(saved["contract"] == p.contract, "primary profile binding");
        let mut expected = serde_json::Map::new();
        for (name, source) in capture["sources"].as_object().context("source set")? {
            if let Some(url) = primary_url(p, name)? {
                let s = &saved["sources"][name];
                let content = s["content"].as_str().context("primary content")?;
                ensure!(
                    s["url"] == url && s["sha256"] == sha(content.as_bytes()) && source["content"] == content,
                    "primary source exact bytes/URL/hash {name}"
                );
                expected.insert(name.clone(), s.clone());
            }
        }
        ensure!(
            saved["sources"] == json!(expected),
            "complete primary source set, without invented Point source"
        );
    }
    Ok(())
}

pub fn candidate(baseline_raw: &[u8]) -> Result<Value> {
    ensure!(sha(baseline_raw) == BASELINE, "unchanged baseline digest");
    let baseline: Value = serde_json::from_slice(baseline_raw)?;
    Ok(Value::Array(
        PROFILES
            .iter()
            .map(|p| -> Result<Value> {
                let mut c = baseline
                    .as_array()
                    .context("baseline array")?
                    .iter()
                    .find(|c| c["contract"] == p.contract)
                    .context("baseline profile")?
                    .clone();
                ensure!(
                    c["code_hash"] == p.runtime && c["balance_slot"] == root(p.balance_root),
                    "baseline runtime/balance binding"
                );
                ensure!(
                    c["other_mapping_words"].as_object_mut().context("legacy mapping")?.remove(&root(p.role_root)) == Some(json!(2)),
                    "legacy width two required"
                );
                ensure!(c.get("other_mapping_paths").is_none(), "preexisting paths");
                c["other_mapping_paths"] = json!([{"root":root(p.role_root),"key_types":["bytes32","address"],"offset":0,"words":1}]);
                Ok(c)
            })
            .collect::<Result<Vec<_>>>()?,
    ))
}
pub fn verify_candidate(baseline: &[u8], v: &Value) -> Result<()> {
    ensure!(candidate(baseline)? == *v, "only two reviewed membership rules may change");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const RAW: [&[u8]; 2] = [
        include_bytes!("../../tests/fixtures/point-bedrock-role-candidates/Point.json"),
        include_bytes!("../../tests/fixtures/point-bedrock-role-candidates/Bedrock.json"),
    ];
    #[test]
    fn complete_capture_compiler_source_layout_and_runtime_bindings() {
        for (raw, p) in RAW.iter().zip(&PROFILES) {
            let good = verify_capture(raw, p).unwrap();
            let mut bytes = raw.to_vec();
            bytes.push(b' ');
            assert!(verify_capture(&bytes, p).is_err());
            for (pointer, value) in [
                ("/chainId", json!("1")),
                ("/address", json!("0x01")),
                ("/sources", json!({})),
                ("/stdJsonInput/sources", json!({})),
                ("/stdJsonInput/settings/optimizer/runs", json!(201)),
                ("/metadata/compiler/version", json!("changed")),
                ("/sourceIds", json!({})),
                ("/runtimeMatch", json!("partial")),
                ("/runtimeBytecode/transformations", json!([{}])),
                ("/runtimeBytecode/immutableReferences", json!({"1":[]})),
                ("/runtimeBytecode/onchainBytecode", json!("0x00")),
                ("/runtimeBytecode/recompiledBytecode", json!("0x00")),
                ("/runtimeBytecode/cborAuxdata/1/offset", json!(0)),
                ("/storageLayout/storage/0/slot", json!("99")),
                ("/storageLayout/types/t_mapping(t_address,t_bool)/key", json!("t_bytes32")),
            ] {
                let mut bad = good.clone();
                *bad.pointer_mut(pointer).expect(pointer) = value;
                assert!(verify_components(&bad, p).is_err(), "{} {pointer}", p.name);
            }
            for name in good["sources"].as_object().unwrap().keys() {
                let mut bad = good.clone();
                bad["sources"][name]["content"] = json!("tampered");
                assert!(verify_components(&bad, p).is_err(), "{name}");
            }
            let mut bad = good.clone();
            bad["stdJsonOutput"]["contracts"][p.source][p.name]["evm"]["deployedBytecode"]["object"] = json!("00");
            assert!(verify_components(&bad, p).is_err());
        }
    }
    #[test]
    fn primary_pins_require_exact_bytes_and_preserve_point_token_source_gap() {
        let captures: Vec<_> = RAW.iter().zip(&PROFILES).map(|(raw, p)| verify_capture(raw, p).unwrap()).collect();
        let primary: Value = serde_json::from_str(include_str!("../../tests/fixtures/point-bedrock-role-candidates/primary-sources.json")).unwrap();
        verify_primary(&captures, &primary).unwrap();
        for (pointer, value) in [
            ("/qualified", json!(true)),
            ("/point_token_source_gap", json!("resolved")),
            ("/profiles/0/sources", json!({})),
            ("/profiles/1/sources", json!({})),
        ] {
            let mut bad = primary.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(verify_primary(&captures, &bad).is_err());
        }
        for i in 0..2 {
            for name in primary["profiles"][i]["sources"].as_object().unwrap().keys() {
                let mut bad = primary.clone();
                bad["profiles"][i]["sources"][name]["content"] = json!("tampered");
                bad["profiles"][i]["sources"][name]["sha256"] = json!(sha(b"tampered"));
                assert!(verify_primary(&captures, &bad).is_err(), "{name}");
            }
        }
    }
    #[test]
    fn generated_review_and_candidates_preserve_baseline_and_all_pins() {
        let captures: Vec<_> = RAW.iter().zip(&PROFILES).map(|(raw, p)| verify_capture(raw, p).unwrap()).collect();
        let saved: Value = serde_json::from_str(include_str!("../../tests/fixtures/point-bedrock-role-candidates/source-review.json")).unwrap();
        assert_eq!(review(&captures).unwrap(), saved);
        let baseline = include_bytes!("../../tests/fixtures/bsc-refined450-layouts.json");
        let candidate: Value = serde_json::from_str(include_str!("../../tests/fixtures/point-bedrock-role-candidates/layouts.json")).unwrap();
        verify_candidate(baseline, &candidate).unwrap();
        for i in 0..2 {
            for (field, value) in [
                ("code_hash", json!("0x00")),
                ("balance_slot", json!("0x09")),
                ("other_mapping_paths", json!([])),
            ] {
                let mut bad = candidate.clone();
                bad[i][field] = value;
                assert!(verify_candidate(baseline, &bad).is_err());
            }
        }
        let mut raw = baseline.to_vec();
        raw.push(b' ');
        assert!(verify_candidate(&raw, &candidate).is_err());
    }
}
