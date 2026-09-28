//! Two bounded, unqualified source-bound role candidates. No network access.
use anyhow::{ensure, Context, Result};
use erc20_balances::hash;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const BASELINE: &str = "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468";
pub const FIXTURE: &str = "tests/fixtures/fhe-b2-role-candidates";
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
    pub token_name: &'static str,
    pub cache: &'static str,
}
pub const PROFILES: [Profile; 2] = [
    Profile {
        name: "FHE",
        contract: "0xd55c9fb62e176a8eb6968f32958fefdd0962727e",
        capture: "a71797c05722783295bb7dc0dc5a13956e1bb6eb5295b4c11766e693d81f4ecf",
        runtime: "0x652b7c5764581dd0fac2d759383df2c06101f0b7edf7f4194737f5b392dcc20b",
        compiler: "0.8.24+commit.e11b9ed9",
        source: "contracts/other-evm/FHE.sol",
        files: 25,
        role_root: 5,
        balance_root: 0,
        token_name: "MindNetwork FHE Token",
        cache: "out/ranks351-400-source-review/0xd55c9fb62e176a8eb6968f32958fefdd0962727e.json",
    },
    Profile {
        name: "B2Token",
        contract: "0x783c3f003f172c6ac5ac700218a357d2d66ee2a2",
        capture: "c468c616cfc867dab05817ab1af80e31b05575265eb1aaf7c5125dbe7ec6eb99",
        runtime: "0xb616979feee87559683543b45de4e46f31244303ab158bb343f1759a5f1ce5ab",
        compiler: "0.8.22+commit.4fc1097e",
        source: "contracts/B2Token.sol",
        files: 26,
        role_root: 9,
        balance_root: 0,
        token_name: "BSquared Token",
        cache: "out/ranks201-250-source-review/0x783c3f003f172c6ac5ac700218a357d2d66ee2a2.json",
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
    verify_runtime(v, p)?;
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
        "_roles[role].hasRole[account] = true;",
        "_roles[role].hasRole[account] = false;",
        "_roles[role].adminRole = adminRole;",
        "_grantRole(role, account);",
        "_revokeRole(role, account);",
    ] {
        ensure!(access.contains(exact), "reviewed membership/admin source statement");
    }
    Ok(())
}
#[derive(Clone)]
struct Patch {
    id: &'static str,
    offset: usize,
    meaning: &'static str,
    value: [u8; 32],
}
fn word(n: u128) -> [u8; 32] {
    let mut w = [0; 32];
    w[16..].copy_from_slice(&n.to_be_bytes());
    w
}
fn short_string(s: &str) -> [u8; 32] {
    assert!(s.len() < 32);
    let mut w = [0; 32];
    w[..s.len()].copy_from_slice(s.as_bytes());
    w[31] = s.len() as u8;
    w
}
fn patches(p: &Profile) -> Vec<Patch> {
    let mut address = [0; 32];
    address[12..].copy_from_slice(&hex::decode(&p.contract[2..]).expect("pinned address"));
    let name = hash(p.token_name.as_bytes());
    let version = hash(b"1");
    let chain = word(56);
    let domain = hash(
        &[
            hash(b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)").as_slice(),
            &name,
            &version,
            &chain,
            &address,
        ]
        .concat(),
    );
    let ids: [(&str, usize); 7] = if p.name == "FHE" {
        [
            ("3455", 4406),
            ("3457", 4365),
            ("3459", 4279),
            ("3461", 6451),
            ("3463", 6484),
            ("3466", 5043),
            ("3469", 5102),
        ]
    } else {
        [
            ("2694", 4209),
            ("2696", 4168),
            ("2698", 4082),
            ("2700", 5919),
            ("2702", 5952),
            ("2705", 5044),
            ("2708", 5103),
        ]
    };
    let mut out = Vec::new();
    if p.name == "B2Token" {
        out.push(Patch {
            id: "1188",
            offset: 1850,
            meaning: "ERC20Capped cap",
            value: word(210_000_000u128 * 10u128.pow(18)),
        });
    }
    for ((id, offset), (meaning, value)) in ids.into_iter().zip([
        ("cached EIP712 domain separator", domain),
        ("cached chain id", chain),
        ("cached contract address", address),
        ("hashed name", name),
        ("hashed version", version),
        ("ShortString name", short_string(p.token_name)),
        ("ShortString version", short_string("1")),
    ]) {
        out.push(Patch { id, offset, meaning, value });
    }
    out
}
fn verify_runtime(v: &Value, p: &Profile) -> Result<()> {
    let runtime = &v["runtimeBytecode"];
    let compiler = &v["stdJsonOutput"]["contracts"][p.source][p.name]["evm"]["deployedBytecode"];
    let patches = patches(p);
    let mut refs = serde_json::Map::new();
    let mut values = serde_json::Map::new();
    let mut transformations = Vec::new();
    for patch in &patches {
        refs.insert(patch.id.into(), json!([{"start":patch.offset,"length":32}]));
        values.insert(patch.id.into(), json!(format!("0x{}", hex::encode(patch.value))));
        transformations.push(json!({"id":patch.id,"offset":patch.offset,"reason":"immutable","type":"replace"}));
    }
    ensure!(
        runtime["immutableReferences"] == json!(refs) && compiler["immutableReferences"] == json!(refs),
        "exact immutable ID/start/length set"
    );
    ensure!(
        runtime["transformationValues"] == json!({"immutables":values}) && runtime["transformations"] == json!(transformations),
        "exact recomputed immutable values/transformation set"
    );
    ensure!(
        runtime["linkReferences"] == json!({}) && compiler["linkReferences"] == json!({}),
        "no linked libraries"
    );
    ensure!(runtime["sourceMap"] == compiler["sourceMap"], "compiler runtime source map");
    let mut compiled = bytes(&runtime["recompiledBytecode"])?;
    ensure!(compiled == bytes(&compiler["object"])?, "saved original compiler bytes");
    let (length, offset, trailer) = if p.name == "FHE" {
        (
            10188,
            10135,
            "0xa2646970667358221220c69bd540082aa8ab872db2156c0b38ff59b804ce84779cce89ec10d40796be0564736f6c63430008180033",
        )
    } else {
        (
            10457,
            10404,
            "0xa2646970667358221220e5f7a8d4f8a2d6631efcf30b7b906a0a9bd4211ea7d85a79702d51b3d0cb822f64736f6c63430008160033",
        )
    };
    ensure!(
        compiled.len() == length
            && runtime["cborAuxdata"] == json!({"1":{"offset":offset,"value":trailer}})
            && compiled.get(offset..) == Some(bytes(&json!(trailer))?.as_slice()),
        "exact original metadata trailer and runtime length"
    );
    let mut occupied = std::collections::BTreeSet::new();
    for patch in patches {
        let end = patch.offset.checked_add(32).context("immutable range overflow")?;
        ensure!(
            end <= offset && (patch.offset..end).all(|i| occupied.insert(i)),
            "immutable range bounds/nonoverlap"
        );
        let target = compiled.get_mut(patch.offset..end).context("immutable runtime bounds")?;
        ensure!(target == [0; 32], "original compiler immutable placeholder");
        target.copy_from_slice(&patch.value);
    }
    ensure!(
        compiled == bytes(&runtime["onchainBytecode"])? && format!("0x{}", hex::encode(hash(&compiled))) == p.runtime,
        "whole saved runtime equality and hash after exact immutable patches"
    );
    Ok(())
}

fn verify_layout(v: &Value, p: &Profile) -> Result<()> {
    let layout = &v["storageLayout"];
    let fields = layout["storage"].as_array().context("layout")?;
    let names: &[&str] = if p.name == "FHE" {
        &[
            "_balances",
            "_allowances",
            "_totalSupply",
            "_name",
            "_symbol",
            "_roles",
            "_nameFallback",
            "_versionFallback",
            "_nonces",
            "ccipAdmin",
        ]
    } else {
        &[
            "_balances",
            "_allowances",
            "_totalSupply",
            "_name",
            "_symbol",
            "_nameFallback",
            "_versionFallback",
            "_nonces",
            "_paused",
            "_roles",
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
            && members[0]["label"] == "hasRole"
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
        Ok(json!({"contract":p.contract,"name":p.name,"chain_id":56,"source_capture_sha256":p.capture,"compiler":p.compiler,"source_files":sources,"runtime_hash":p.runtime,"role_root":root(p.role_root),"balance_root":root(p.balance_root),"runtime_transformations":patches(p).len(),"immutable_patches":patches(p).iter().map(|x|json!({"id":x.id,"offset":x.offset,"length":32,"meaning":x.meaning,"value":format!("0x{}",hex::encode(x.value))})).collect::<Vec<_>>(),"runtime_reconstructed_from_saved_compiler_bytes":true,"fresh_compilation":false,"role_admin_identifier_references":1,"role_admin_callsite":"none_beyond_declaration_in_complete_pinned_capture","qualified":false}))
    }).collect::<Result<Vec<_>>>()?;
    Ok(
        json!({"qualified":false,"scope":"Saved source/compiler/runtime binding and synthetic storage shapes only; not deployment or replacement-package qualification.","profiles":profiles}),
    )
}
pub const FHE_PIN: &str = "e3b9112918304a7a00d210609eec34d5390ef2a1";
pub const FHE_OZ_PIN: &str = "acd4ff74de833399287ed6b31b4debf6b2b35527";
pub const B2_PIN: &str = "8369fb6537f85ad746cef56ae616b5cb581c2c0b";
pub const B2_OZ_PIN: &str = "dbb6104ce834628e473d2173bbc9d47f81a9eec3";
pub fn primary_url(p: &Profile, name: &str) -> Result<String> {
    if name == p.source {
        let (repo, pin) = if p.name == "FHE" {
            ("mind-network/mind-token-contracts", FHE_PIN)
        } else {
            ("b2network/b2-token-contract", B2_PIN)
        };
        return Ok(format!("https://raw.githubusercontent.com/{repo}/{pin}/{name}"));
    }
    let path = name.strip_prefix("@openzeppelin/contracts/").context("reviewed primary source prefix")?;
    let pin = if p.name == "FHE" { FHE_OZ_PIN } else { B2_OZ_PIN };
    Ok(format!(
        "https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{pin}/contracts/{path}"
    ))
}
pub fn verify_primary(captures: &[Value], primary: &Value) -> Result<()> {
    ensure!(primary["qualified"] == false, "unqualified primary evidence");
    let saved = primary["profiles"].as_array().context("primary profiles")?;
    ensure!(saved.len() == 2 && captures.len() == 2, "exact primary scope");
    for ((capture, p), saved) in captures.iter().zip(&PROFILES).zip(saved) {
        ensure!(saved["contract"] == p.contract, "primary profile binding");
        let sources = capture["sources"].as_object().context("sources")?;
        ensure!(
            sources.keys().eq(saved["sources"].as_object().context("primary sources")?.keys()),
            "complete primary source set"
        );
        for (name, source) in sources {
            let s = &saved["sources"][name];
            let content = s["content"].as_str().context("primary content")?;
            ensure!(
                s["url"] == primary_url(p, name)? && s["sha256"] == sha(content.as_bytes()) && source["content"] == content,
                "exact primary source bytes/URL/hash {name}"
            );
        }
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
                if p.name == "FHE" {
                    let broad = c["other_mapping_slots"].as_array_mut().context("broad mapping list")?;
                    let role = json!(root(p.role_root));
                    ensure!(
                        broad.iter().filter(|v| **v == role).count() == 1,
                        "exact duplicate broad FHE role rule required"
                    );
                    broad.retain(|v| *v != role);
                }
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
        include_bytes!("../../tests/fixtures/fhe-b2-role-candidates/FHE.json"),
        include_bytes!("../../tests/fixtures/fhe-b2-role-candidates/B2Token.json"),
    ];
    fn captures() -> Vec<Value> {
        RAW.iter().zip(&PROFILES).map(|(raw, p)| verify_capture(raw, p).unwrap()).collect()
    }
    #[test]
    fn complete_capture_compiler_source_layout_and_runtime_bindings() {
        for (raw, p) in RAW.iter().zip(&PROFILES) {
            let good = verify_capture(raw, p).unwrap();
            let mut raw = raw.to_vec();
            raw.push(b' ');
            assert!(verify_capture(&raw, p).is_err());
            for (pointer, value) in [
                ("/chainId", json!("1")),
                ("/address", json!("0x01")),
                ("/sources", json!({})),
                ("/stdJsonInput/sources", json!({})),
                ("/stdJsonInput/settings/optimizer/runs", json!(201)),
                ("/metadata/compiler/version", json!("changed")),
                ("/sourceIds", json!({})),
                ("/runtimeMatch", json!("partial")),
                ("/runtimeBytecode/onchainBytecode", json!("0x00")),
                ("/runtimeBytecode/recompiledBytecode", json!("0x00")),
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
                bad["stdJsonInput"]["sources"][name]["content"] = json!("tampered");
                assert!(verify_components(&bad, p).is_err(), "source hash {name}");
            }
        }
    }
    #[test]
    fn every_immutable_axis_and_patch_set_is_exact() {
        for (good, p) in captures().iter().zip(&PROFILES) {
            let expected = patches(p);
            for (i, patch) in expected.iter().enumerate() {
                for (axis, value) in [
                    ("start", json!(patch.offset + 1)),
                    ("start", json!(u64::MAX)),
                    ("length", json!(31)),
                    ("length", json!(33)),
                ] {
                    let mut bad = good.clone();
                    bad["runtimeBytecode"]["immutableReferences"][patch.id][0][axis] = value;
                    assert!(verify_runtime(&bad, p).is_err(), "{} {} {axis}", p.name, patch.id);
                }
                let mut bad = good.clone();
                bad["runtimeBytecode"]["transformationValues"]["immutables"][patch.id] = json!("0x00");
                assert!(verify_runtime(&bad, p).is_err());
                let mut bad = good.clone();
                let mut value = patch.value;
                value[0] ^= 1;
                bad["runtimeBytecode"]["transformationValues"]["immutables"][patch.id] = json!(format!("0x{}", hex::encode(value)));
                assert!(verify_runtime(&bad, p).is_err());
                let mut bad = good.clone();
                bad["runtimeBytecode"]["immutableReferences"].as_object_mut().unwrap().remove(patch.id);
                assert!(verify_runtime(&bad, p).is_err());
                let mut bad = good.clone();
                bad["runtimeBytecode"]["transformations"].as_array_mut().unwrap().remove(i);
                assert!(verify_runtime(&bad, p).is_err());
                let mut bad = good.clone();
                let duplicate = bad["runtimeBytecode"]["transformations"][i].clone();
                bad["runtimeBytecode"]["transformations"].as_array_mut().unwrap().push(duplicate);
                assert!(verify_runtime(&bad, p).is_err());
                // Coherently changing the compiler and reported range cannot
                // authorize an overlapping/out-of-bounds substitution either.
                for offset in [expected[(i + 1) % expected.len()].offset as u64, u64::MAX] {
                    let mut bad = good.clone();
                    bad["runtimeBytecode"]["immutableReferences"][patch.id][0]["start"] = json!(offset);
                    bad["stdJsonOutput"]["contracts"][p.source][p.name]["evm"]["deployedBytecode"]["immutableReferences"][patch.id][0]["start"] = json!(offset);
                    bad["runtimeBytecode"]["transformations"][i]["offset"] = json!(offset);
                    assert!(verify_runtime(&bad, p).is_err());
                }
            }
            for (key, value) in [
                ("immutableReferences", json!({})),
                ("transformationValues", json!({"immutables":{}})),
                ("transformations", json!([])),
                ("linkReferences", json!({"unreviewed":[]})),
                ("cborAuxdata", json!({})),
            ] {
                let mut bad = good.clone();
                bad["runtimeBytecode"][key] = value;
                assert!(verify_runtime(&bad, p).is_err(), "{} {key}", p.name);
            }
            let mut bad = good.clone();
            bad["runtimeBytecode"]["immutableReferences"]["extra"] = json!([{"start":0,"length":32}]);
            bad["runtimeBytecode"]["transformationValues"]["immutables"]["extra"] = json!(root(0));
            bad["runtimeBytecode"]["transformations"]
                .as_array_mut()
                .unwrap()
                .push(json!({"id":"extra","offset":0,"reason":"immutable","type":"replace"}));
            assert!(verify_runtime(&bad, p).is_err());
        }
    }
    #[test]
    fn undeclared_runtime_and_cbor_edits_cannot_be_hidden_by_agreeing_saved_fields() {
        for (good, p) in captures().iter().zip(&PROFILES) {
            for offset in [0, bytes(&good["runtimeBytecode"]["onchainBytecode"]).unwrap().len() - 1] {
                let mut bad = good.clone();
                let mut deployed = bytes(&bad["runtimeBytecode"]["onchainBytecode"]).unwrap();
                deployed[offset] ^= 1;
                let mut compiled = bytes(&bad["runtimeBytecode"]["recompiledBytecode"]).unwrap();
                compiled[offset] ^= 1;
                bad["runtimeBytecode"]["onchainBytecode"] = json!(format!("0x{}", hex::encode(deployed)));
                bad["runtimeBytecode"]["recompiledBytecode"] = json!(format!("0x{}", hex::encode(&compiled)));
                bad["stdJsonOutput"]["contracts"][p.source][p.name]["evm"]["deployedBytecode"]["object"] = json!(hex::encode(compiled));
                assert!(verify_runtime(&bad, p).is_err());
            }
        }
    }
    #[test]
    fn primary_pins_require_all_exact_sources_and_reject_wrong_b2_main() {
        let captures = captures();
        let primary: Value = serde_json::from_str(include_str!("../../tests/fixtures/fhe-b2-role-candidates/primary-sources.json")).unwrap();
        verify_primary(&captures, &primary).unwrap();
        for (pointer, value) in [
            ("/qualified", json!(true)),
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
        let mut bad = primary.clone();
        bad["profiles"][1]["sources"][PROFILES[1].source]["url"] =
            json!("https://raw.githubusercontent.com/b2network/b2-token-contract/72dc2b060eb986b3fc591bde55a685354a3cbcd4/contracts/B2Token.sol");
        assert!(verify_primary(&captures, &bad).is_err());
    }
    #[test]
    fn generated_review_and_candidates_preserve_every_unrelated_baseline_field() {
        let saved: Value = serde_json::from_str(include_str!("../../tests/fixtures/fhe-b2-role-candidates/source-review.json")).unwrap();
        assert_eq!(review(&captures()).unwrap(), saved);
        let baseline = include_bytes!("../../tests/fixtures/bsc-refined450-layouts.json");
        let candidate: Value = serde_json::from_str(include_str!("../../tests/fixtures/fhe-b2-role-candidates/layouts.json")).unwrap();
        verify_candidate(baseline, &candidate).unwrap();
        for i in 0..2 {
            for (field, value) in [
                ("code_hash", json!("0x00")),
                ("balance_slot", json!(root(10))),
                ("other_mapping_paths", json!([])),
                ("other_mapping_slots", json!([])),
                ("other_slots", json!([])),
            ] {
                let mut bad = candidate.clone();
                bad[i][field] = value;
                assert!(verify_candidate(baseline, &bad).is_err());
            }
        }
        let mut bad = candidate.clone();
        bad[0]["other_mapping_slots"].as_array_mut().unwrap().insert(1, json!(root(5)));
        assert!(verify_candidate(baseline, &bad).is_err());
        let mut raw = baseline.to_vec();
        raw.push(b' ');
        assert!(verify_candidate(&raw, &candidate).is_err());
    }
}
