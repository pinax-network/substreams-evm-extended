//! One unqualified Artx proxy membership candidate; complete saved bindings only.
use anyhow::{ensure, Context, Result};
use erc20_balances::hash;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const CONTRACT: &str = "0x8105743e8a19c915a604d7d9e7aa3a060a4c2c32";
pub const IMPLEMENTATION: &str = "0xc401796ec909f7774703528571c3590748ebc563";
pub const IMPLEMENTATION_SLOT: &str = "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc";
pub const BASELINE: &str = "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468";
pub const FIXTURE: &str = "tests/fixtures/artx-role-candidate";
pub const PRIMARY: &str = "57adbbb35d47187a3edada8f43da5471274f27114652490bb5736528aeaf9bb1";
pub const TOKEN: &str = "contracts/ArtxToken.sol";
pub const TOKEN_SHA: &str = "ba971c5212b7cc1dd5a679bf6501fb4588540fb8a1f2c42325d72248575693d8";
pub const SOURCE_GAP: &str = "The independent public ArtxToken source pin remains unresolved after the bounded ULTILAND/public-code search. Exact dependency pins do not resolve this token-source gap; no source normalization or inferred token pin.";
const DEPLOYER: &str = "0x030b08e1f870362453062d2920fb651c25b96ea1";
const PROXY_SELECTED: [&str; 8] = [
    "@openzeppelin/contracts/interfaces/IERC1967.sol",
    "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol",
    "@openzeppelin/contracts/proxy/ERC1967/ERC1967Utils.sol",
    "@openzeppelin/contracts/proxy/Proxy.sol",
    "@openzeppelin/contracts/proxy/beacon/IBeacon.sol",
    "@openzeppelin/contracts/utils/Address.sol",
    "@openzeppelin/contracts/utils/Errors.sol",
    "@openzeppelin/contracts/utils/StorageSlot.sol",
];
#[derive(Clone, Copy)]
pub struct Capture {
    pub label: &'static str,
    pub contract: &'static str,
    pub capture: &'static str,
    pub cache: &'static str,
    pub source: &'static str,
    pub name: &'static str,
    pub compiler: &'static str,
    pub runtime: &'static str,
    pub creation: &'static str,
    pub runtime_len: usize,
    pub creation_len: usize,
    pub cbor: &'static str,
    pub pin: &'static str,
    pub prefix: &'static str,
    pub repo: &'static str,
    pub block: &'static str,
    pub tx_index: &'static str,
    pub tx: &'static str,
}
pub const CAPTURES: [Capture; 2] = [
    Capture {
        label: "proxy",
        contract: CONTRACT,
        capture: "22e604d6c46a14825b5a95f686ffb1875475ad0725fe63d7d77747bb0931d9ab",
        cache: "out/ranks151-200-source-review/0x8105743e8a19c915a604d7d9e7aa3a060a4c2c32.json",
        source: "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol",
        name: "ERC1967Proxy",
        compiler: "0.8.29+commit.ab55807c",
        runtime: "0x864cc9ad53b338b82da1f7cab85ab0b3d5c8861acb422b6fec63cf36234f36a6",
        creation: "0xe8a46410e3efa2bfef8c9d29bbc3c99cf959b5825df1d9c898b2dc505d142a2d",
        runtime_len: 170,
        creation_len: 1040,
        cbor: "0xa264697066735822122051da5b51f2e43cd956e2b3e18d302642bb371a72f196cc4a9776ab84ef5e725a64736f6c634300081d0033",
        pin: "acd4ff74de833399287ed6b31b4debf6b2b35527",
        prefix: "@openzeppelin/contracts/",
        repo: "openzeppelin-contracts",
        block: "67828331",
        tx_index: "141",
        tx: "0x83c78916921771e158adceb8fd66ced6914936fd779a85a6a648f3d2bdb27307",
    },
    Capture {
        label: "implementation",
        contract: IMPLEMENTATION,
        capture: "f01037c490dd41a7e656ad3dcf8b7cfd95ee125f3378ceac81d663933a5d7e7e",
        cache: "out/ranks151-200-pending-proxies/0xc401796ec909f7774703528571c3590748ebc563-source.json",
        source: TOKEN,
        name: "ArtxToken",
        compiler: "0.8.20+commit.a1b79de6",
        runtime: "0xf8ef8aafe948cf5240fb04631cd152673a48a13c9c6be2e638e2642250d4f03c",
        creation: "0xc23c8191323365eee633715e963eb0f4df6fa1d0d1377808532c4d0552bce19b",
        runtime_len: 10348,
        creation_len: 10574,
        cbor: "0xa2646970667358221220f64a8751c03862f77d0e1a6c3e37707fc11336fde88bbb8bd1cd3742c458bcf864736f6c63430008140033",
        pin: "2d081f24cac1a867f6f73d512f2022e1fa987854",
        prefix: "@openzeppelin/contracts-upgradeable/",
        repo: "openzeppelin-contracts-upgradeable",
        block: "67828310",
        tx_index: "162",
        tx: "0x2eafae3779840d4033cc4c34aef22d892ba438947dbdbbfb080d03d144bc4d5b",
    },
];
pub fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
pub fn root(n: u64) -> String {
    format!("0x{n:064x}")
}
fn bytes(v: &Value) -> Result<Vec<u8>> {
    Ok(hex::decode(v.as_str().context("hex string")?.trim_start_matches("0x"))?)
}
fn word(n: u64) -> [u8; 32] {
    let mut w = [0; 32];
    w[24..].copy_from_slice(&n.to_be_bytes());
    w
}
fn implementation_word() -> [u8; 32] {
    let mut w = [0; 32];
    w[12..].copy_from_slice(&hex::decode(&IMPLEMENTATION[2..]).unwrap());
    w
}
/// Independently encode the exact saved initializer call, not a seed or execution.
pub fn initializer_calldata() -> Vec<u8> {
    let mut v = hash(b"__ArtxToken_init(string,string,uint256,address[],uint256[])")[..4].to_vec();
    assert_eq!(hex::encode(&v), "1e5c013d");
    for n in [160, 224, 0, 288, 320] {
        v.extend(word(n));
    }
    for s in ["Ultiland", "ARTX"] {
        v.extend(word(s.len() as u64));
        let mut w = [0; 32];
        w[..s.len()].copy_from_slice(s.as_bytes());
        v.extend(w);
    }
    v.extend(word(0));
    v.extend(word(0));
    assert_eq!(v.len(), 356);
    v
}
pub fn constructor_arguments() -> Vec<u8> {
    let data = initializer_calldata();
    let mut v = implementation_word().to_vec();
    v.extend(word(64));
    v.extend(word(data.len() as u64));
    v.extend(data);
    v.resize(480, 0);
    v
}
pub fn verify_capture(raw: &[u8], p: &Capture) -> Result<Value> {
    ensure!(sha(raw) == p.capture, "complete original {} capture digest", p.label);
    let v = serde_json::from_slice(raw)?;
    verify_components(&v, p)?;
    Ok(v)
}
fn verify_components(v: &Value, p: &Capture) -> Result<()> {
    let proxy = p.label == "proxy";
    ensure!(
        v["chainId"] == "56" && bytes(&v["address"])? == hex::decode(&p.contract[2..])?,
        "network/address identity"
    );
    ensure!(
        ["match", "runtimeMatch", "creationMatch"].iter().all(|k| v[k] == "exact_match"),
        "original exact_match labels"
    );
    let settings = if proxy {
        json!({"metadata":{"bytecodeHash":"ipfs"},"libraries":{},"optimizer":{"runs":200,"enabled":true},"evmVersion":"paris","remappings":[]})
    } else {
        json!({"viaIR":true,"optimizer":{"runs":1,"enabled":true},"evmVersion":"paris"})
    };
    ensure!(
        v["compilation"]["compiler"] == "solc"
            && v["compilation"]["language"] == "Solidity"
            && v["compilation"]["compilerVersion"] == p.compiler
            && v["metadata"]["compiler"]["version"] == p.compiler
            && v["compilation"]["name"] == p.name
            && v["compilation"]["fullyQualifiedName"] == format!("{}:{}", p.source, p.name)
            && v["metadata"]["settings"]["compilationTarget"] == json!({p.source:p.name}),
        "compiler/target identity"
    );
    ensure!(
        v["compilation"]["compilerSettings"] == settings && v["stdJsonInput"]["settings"] == settings && v["stdJsonInput"]["language"] == "Solidity",
        "exact compiler input settings"
    );
    let sources = v["sources"].as_object().context("source set")?;
    let selected = v["sourceIds"].as_object().context("selected source IDs")?;
    let metadata = v["metadata"]["sources"].as_object().context("metadata sources")?;
    ensure!(
        sources.len() == if proxy { 14 } else { 21 } && v["sources"] == v["stdJsonInput"]["sources"],
        "complete original input source set"
    );
    ensure!(
        v["sourceIds"] == v["stdJsonOutput"]["sources"] && selected.keys().eq(metadata.keys()),
        "selected compiler output/metadata source binding"
    );
    if proxy {
        ensure!(selected.keys().map(String::as_str).eq(PROXY_SELECTED), "exact eight selected proxy sources");
    } else {
        ensure!(sources.keys().eq(selected.keys()), "all twenty-one implementation sources selected");
    }
    for (name, s) in sources {
        ensure!(name == TOKEN || name.starts_with(p.prefix), "reviewed source prefix");
        if selected.contains_key(name) {
            ensure!(
                format!("0x{}", hex::encode(hash(s["content"].as_str().context("source content")?.as_bytes()))) == metadata[name]["keccak256"],
                "metadata source digest {name}"
            );
        }
    }
    let output = &v["stdJsonOutput"]["contracts"][p.source][p.name];
    let compiled_metadata: Value = serde_json::from_str(output["metadata"].as_str().context("compiler metadata")?)?;
    ensure!(compiled_metadata == v["metadata"], "compiler metadata object");
    for field in ["abi", "userdoc", "devdoc", "storageLayout", "transientStorageLayout"] {
        ensure!(output[field] == v[field], "compiler output {field}");
    }
    if !proxy {
        let token = sources[TOKEN]["content"].as_str().context("token body")?;
        ensure!(
            sha(token.as_bytes()) == TOKEN_SHA
                && token.contains("_disableInitializers();")
                && token.contains("_setupRole(OPERATOR_ROLE, msg.sender);")
                && token.contains("onlyOwner"),
            "exact token body and reviewed controls"
        );
        let ids: Vec<_> = sources.values().flat_map(|s| identifiers(s["content"].as_str().unwrap())).collect();
        ensure!(
            ids.iter().filter(|s| s.as_str() == "_setRoleAdmin").count() == 1,
            "only unused inherited role-admin declaration"
        );
        ensure!(
            sources["@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol"]["content"]
                .as_str()
                .unwrap()
                .contains("address private immutable __self = address(this);"),
            "UUPS self immutable provenance"
        );
    }
    verify_bytecode(v, output, p)?;
    verify_layout(v, proxy)?;
    ensure!(
        v["deployment"]["blockNumber"] == p.block
            && v["deployment"]["transactionIndex"] == p.tx_index
            && v["deployment"]["transactionHash"] == p.tx
            && bytes(&v["deployment"]["deployer"])? == hex::decode(&DEPLOYER[2..])?,
        "recorded deployment identity only"
    );
    Ok(())
}
fn verify_bytecode(v: &Value, output: &Value, p: &Capture) -> Result<()> {
    let proxy = p.label == "proxy";
    let r = &v["runtimeBytecode"];
    let c = &v["creationBytecode"];
    for (saved, compiled, len) in [
        (r, &output["evm"]["deployedBytecode"], p.runtime_len),
        (c, &output["evm"]["bytecode"], p.creation_len),
    ] {
        ensure!(
            saved["linkReferences"] == json!({}) && saved["linkReferences"] == compiled["linkReferences"] && saved["sourceMap"] == compiled["sourceMap"],
            "compiler source map/no links"
        );
        let raw = bytes(&saved["recompiledBytecode"])?;
        ensure!(raw.len() == len && raw == bytes(&compiled["object"])?, "complete saved compiler object");
        let cbor = hex::decode(&p.cbor[2..])?;
        ensure!(
            saved["cborAuxdata"] == json!({"1":{"value":p.cbor,"offset":len-cbor.len()}}) && raw[len - cbor.len()..] == cbor,
            "unmodified compiler CBOR trailer"
        );
    }
    ensure!(
        r["immutableReferences"] == output["evm"]["deployedBytecode"]["immutableReferences"],
        "compiler immutable binding"
    );
    let mut runtime = bytes(&r["recompiledBytecode"])?;
    if proxy {
        ensure!(
            r["immutableReferences"] == json!({}) && r["transformations"] == json!([]) && r["transformationValues"] == json!({}),
            "untransformed proxy runtime"
        );
    } else {
        ensure!(
            r["immutableReferences"] == json!({"1097":[{"start":1785,"length":32},{"start":2035,"length":32},{"start":2980,"length":32}]}),
            "exact three self immutable sites"
        );
        ensure!(
            r["transformations"]
                == json!([{"id":"1097","type":"replace","offset":1785,"reason":"immutable"},{"id":"1097","type":"replace","offset":2035,"reason":"immutable"},{"id":"1097","type":"replace","offset":2980,"reason":"immutable"}])
                && r["transformationValues"] == json!({"immutables":{"1097":format!("0x{}",hex::encode(implementation_word()))}}),
            "only implementation self replacements"
        );
        for offset in [1785, 2035, 2980] {
            ensure!(runtime[offset..offset + 32] == [0; 32], "zero self placeholder");
            runtime[offset..offset + 32].copy_from_slice(&implementation_word());
        }
    }
    ensure!(
        runtime == bytes(&r["onchainBytecode"])? && format!("0x{}", hex::encode(hash(&runtime))) == p.runtime,
        "complete runtime reconstruction"
    );
    let mut creation = bytes(&c["recompiledBytecode"])?;
    let ctors: Vec<_> = v["abi"].as_array().context("ABI")?.iter().filter(|v| v["type"] == "constructor").collect();
    if proxy {
        let args = constructor_arguments();
        ensure!(
            c["transformations"] == json!([{"type":"insert","offset":1040,"reason":"constructorArguments"}])
                && c["transformationValues"] == json!({"constructorArguments":format!("0x{}",hex::encode(&args))}),
            "exact independently encoded constructor append"
        );
        ensure!(
            ctors
                == vec![
                    &json!({"type":"constructor","inputs":[{"name":"implementation","type":"address","internalType":"address"},{"name":"_data","type":"bytes","internalType":"bytes"}],"stateMutability":"payable"})
                ],
            "proxy constructor ABI"
        );
        creation.extend(args);
    } else {
        ensure!(
            c["transformations"] == json!([]) && c["transformationValues"] == json!({}),
            "untransformed implementation creation"
        );
        ensure!(
            ctors == vec![&json!({"type":"constructor","inputs":[],"stateMutability":"nonpayable"})],
            "empty implementation constructor ABI"
        );
    }
    ensure!(
        creation == bytes(&c["onchainBytecode"])? && format!("0x{}", hex::encode(hash(&creation))) == p.creation,
        "complete creation reconstruction"
    );
    Ok(())
}
fn verify_layout(v: &Value, proxy: bool) -> Result<()> {
    let layout = &v["storageLayout"];
    if proxy {
        ensure!(*layout == json!({"types":null,"storage":[]}), "empty proxy compiler layout");
        return Ok(());
    }
    let fields = layout["storage"].as_array().context("storage fields")?;
    let expected = [
        ("_initialized", "0", 0, "t_uint8"),
        ("_initializing", "0", 1, "t_bool"),
        ("__gap", "1", 0, "t_array(t_uint256)50_storage"),
        ("_balances", "51", 0, "t_mapping(t_address,t_uint256)"),
        ("_allowances", "52", 0, "t_mapping(t_address,t_mapping(t_address,t_uint256))"),
        ("_totalSupply", "53", 0, "t_uint256"),
        ("_name", "54", 0, "t_string_storage"),
        ("_symbol", "55", 0, "t_string_storage"),
        ("__gap", "56", 0, "t_array(t_uint256)45_storage"),
        ("__gap", "101", 0, "t_array(t_uint256)50_storage"),
        ("_roles", "151", 0, "t_mapping(t_bytes32,t_struct(RoleData)23_storage)"),
        ("__gap", "152", 0, "t_array(t_uint256)49_storage"),
        ("_owner", "201", 0, "t_address"),
        ("__gap", "202", 0, "t_array(t_uint256)49_storage"),
        ("__gap", "251", 0, "t_array(t_uint256)50_storage"),
        ("__gap", "301", 0, "t_array(t_uint256)50_storage"),
        ("totalMinted", "351", 0, "t_uint256"),
        ("totalBurned", "352", 0, "t_uint256"),
    ];
    ensure!(fields.len() == expected.len(), "complete layout including gaps");
    for (f, (name, slot, offset, kind)) in fields.iter().zip(expected) {
        ensure!(
            f["label"] == name && f["slot"] == slot && f["offset"] == offset && f["type"] == kind && f["contract"] == "contracts/ArtxToken.sol:ArtxToken",
            "exact field layout"
        );
    }
    let types = &layout["types"];
    let outer = &types["t_mapping(t_bytes32,t_struct(RoleData)23_storage)"];
    ensure!(
        outer["encoding"] == "mapping" && outer["key"] == "t_bytes32" && outer["value"] == "t_struct(RoleData)23_storage",
        "outer role mapping"
    );
    let record = &types["t_struct(RoleData)23_storage"];
    ensure!(
        record["encoding"] == "inplace" && record["numberOfBytes"] == "64" && record["members"].as_array().context("role members")?.len() == 2,
        "two-word RoleData"
    );
    for (f, name, slot, kind) in [
        (&record["members"][0], "members", "0", "t_mapping(t_address,t_bool)"),
        (&record["members"][1], "adminRole", "1", "t_bytes32"),
    ] {
        ensure!(
            f["label"] == name && f["slot"] == slot && f["offset"] == 0 && f["type"] == kind,
            "exact role members/admin shape"
        );
    }
    let inner = &types["t_mapping(t_address,t_bool)"];
    ensure!(
        inner["encoding"] == "mapping" && inner["key"] == "t_address" && inner["value"] == "t_bool" && types["t_bool"]["numberOfBytes"] == "1",
        "address/bool inner membership"
    );
    Ok(())
}
// Review guard for this exact pinned source bundle, not a Solidity call graph.
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
pub fn primary_url(p: &Capture, name: &str) -> Result<String> {
    let suffix = name.strip_prefix(p.prefix).context("primary dependency prefix")?;
    Ok(format!(
        "https://raw.githubusercontent.com/OpenZeppelin/{}/{}/contracts/{suffix}",
        p.repo, p.pin
    ))
}
pub fn verify_primary_raw(raw: &[u8], captures: &[Value]) -> Result<Value> {
    ensure!(sha(raw) == PRIMARY, "original complete thirty-four-source primary evidence");
    let primary = serde_json::from_slice(raw)?;
    verify_primary(captures, &primary)?;
    Ok(primary)
}
pub fn verify_primary(captures: &[Value], primary: &Value) -> Result<()> {
    ensure!(captures.len() == 2, "exact proxy plus implementation");
    let sources = primary["sources"].as_array().context("primary files")?;
    ensure!(sources.len() == 34, "all thirty-four primary dependencies");
    let mut seen = BTreeSet::new();
    for s in sources {
        let label = s["capture"].as_str().context("capture label")?;
        let i = CAPTURES.iter().position(|p| p.label == label).context("known capture")?;
        let p = &CAPTURES[i];
        let name = s["path"].as_str().context("source path")?;
        let content = s["content"].as_str().context("source content")?;
        ensure!(seen.insert((label, name)), "no duplicate primary source");
        ensure!(
            s["url"] == primary_url(p, name)?
                && s["sha256"] == sha(content.as_bytes())
                && captures[i]["sources"][name]["content"] == content
                && s["exact_public_bytes_match_verified"] == true,
            "exact primary content/hash/URL"
        );
        ensure!(
            s["in_selected_compiler_source_ids"] == captures[i]["sourceIds"].get(name).is_some(),
            "input-only versus selected source distinction"
        );
    }
    for (v, p) in captures.iter().zip(&CAPTURES) {
        for name in v["sources"].as_object().context("source set")?.keys().filter(|name| name.as_str() != TOKEN) {
            ensure!(seen.contains(&(p.label, name.as_str())), "complete primary set {name}");
        }
    }
    Ok(())
}
pub fn candidate(baseline: &[u8]) -> Result<Value> {
    ensure!(sha(baseline) == BASELINE, "unchanged historical431 baseline");
    let all: Value = serde_json::from_slice(baseline)?;
    let mut v = all
        .as_array()
        .context("baseline array")?
        .iter()
        .find(|v| v["contract"] == CONTRACT)
        .context("Artx profile")?
        .clone();
    ensure!(
        v["balance_slot"] == root(51)
            && v["code_hash"] == CAPTURES[0].runtime
            && v["proxy"] == json!({"implementation":IMPLEMENTATION,"implementation_slot":IMPLEMENTATION_SLOT,"code_hash":CAPTURES[1].runtime})
            && v.get("deployment").is_none()
            && v.get("other_mapping_paths").is_none(),
        "existing runtime/pointer/deployment/balance guards"
    );
    ensure!(
        v["other_mapping_words"].as_object_mut().context("legacy role width")?.remove(&root(151)) == Some(json!(2)),
        "legacy root151 width2"
    );
    v["other_mapping_paths"] = json!([{"root":root(151),"key_types":["bytes32","address"],"offset":0,"words":1}]);
    Ok(json!([v]))
}
pub fn verify_candidate(baseline: &[u8], v: &Value) -> Result<()> {
    ensure!(candidate(baseline)? == *v, "only exact Artx membership path may change");
    Ok(())
}
pub fn review(captures: &[Value]) -> Result<Value> {
    ensure!(captures.len() == 2, "proxy and implementation captures");
    let profiles=captures.iter().zip(&CAPTURES).map(|(v,p)|->Result<Value>{
        verify_components(v,p)?;
        let sources:Vec<_>=v["sources"].as_object().unwrap().iter().map(|(name,s)|json!({"path":name,"sha256":sha(s["content"].as_str().unwrap().as_bytes()),"selected_output":v["sourceIds"].get(name).is_some(),"selected_metadata_keccak256":v["metadata"]["sources"][name]["keccak256"]})).collect();
        Ok(json!({"capture":p.label,"contract":p.contract,"chain_id":56,"capture_sha256":p.capture,"compiler":p.compiler,"runtime_hash":p.runtime,"creation_hash":p.creation,"source_files":sources,"recorded_deployment":v["deployment"],"original_match_labels":{"match":v["match"],"runtimeMatch":v["runtimeMatch"],"creationMatch":v["creationMatch"]}}))
    }).collect::<Result<Vec<_>>>()?;
    Ok(
        json!({"qualified":false,"contract":CONTRACT,"captures":profiles,"source_gap":SOURCE_GAP,"runtime_reconstructed_from_saved_compiler_bytes":true,"fresh_compilation":false,"proxy_constructor_append_bytes":480,"initializer_payload_bytes":356,"initializer_signature":"__ArtxToken_init(string,string,uint256,address[],uint256[])","initializer_selector":"0x1e5c013d","captured_initializer":{"name":"Ultiland","symbol":"ARTX","initial_supply":"0","recipients":[],"amounts":[]},"initial_state_inferred_or_seeded":false,"implementation_self_immutable":{"id":"1097","offsets":[1785,2035,2980],"value":format!("0x{}",hex::encode(implementation_word()))},"membership_root":root(151),"role_admin_identifier_references":1,"role_admin_callsite":"none beyond unused inherited declaration","scope":"Complete saved proxy/implementation bindings and synthetic shapes only. No current pointer/owner, initialization, deployment or replacement-package qualification."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    const RAW: [&[u8]; 2] = [
        include_bytes!("../../tests/fixtures/artx-role-candidate/proxy.json"),
        include_bytes!("../../tests/fixtures/artx-role-candidate/implementation.json"),
    ];
    fn captures() -> Vec<Value> {
        RAW.iter().zip(&CAPTURES).map(|(raw, p)| verify_capture(raw, p).unwrap()).collect()
    }
    #[test]
    fn complete_captures_compilers_layouts_and_runtime_identity_are_bound() {
        for (raw, p) in RAW.iter().zip(&CAPTURES) {
            let good = verify_capture(raw, p).unwrap();
            let mut altered = raw.to_vec();
            altered.push(b' ');
            assert!(verify_capture(&altered, p).is_err());
            for (pointer, value) in [
                ("/address", json!("0x00")),
                ("/chainId", json!("1")),
                ("/match", json!("match")),
                ("/runtimeMatch", json!("match")),
                ("/creationMatch", json!("match")),
                ("/sources", json!({})),
                ("/stdJsonInput/sources", json!({})),
                ("/sourceIds", json!({})),
                ("/metadata/sources", json!({})),
                ("/stdJsonInput/settings/optimizer/runs", json!(999)),
                ("/metadata/compiler/version", json!("changed")),
                ("/runtimeBytecode/onchainBytecode", json!("0x00")),
                ("/runtimeBytecode/recompiledBytecode", json!("0x00")),
                ("/runtimeBytecode/cborAuxdata/1/offset", json!(0)),
                ("/creationBytecode/cborAuxdata/1/offset", json!(0)),
                ("/runtimeBytecode/linkReferences", json!({"extra":[]})),
                ("/runtimeBytecode/immutableReferences", json!({"extra":[]})),
                ("/storageLayout/storage", json!([{}])),
                ("/deployment/blockNumber", json!("0")),
                ("/deployment/transactionHash", json!("0x00")),
            ] {
                let mut bad = good.clone();
                *bad.pointer_mut(pointer).expect(pointer) = value;
                assert!(verify_components(&bad, p).is_err(), "{} {pointer}", p.label);
            }
            for name in good["sources"].as_object().unwrap().keys() {
                let mut bad = good.clone();
                bad["sources"][name]["content"] = json!("changed");
                assert!(verify_components(&bad, p).is_err(), "{name}");
            }
            let mut bad = good.clone();
            bad["stdJsonOutput"]["contracts"][p.source][p.name]["abi"] = json!([]);
            assert!(verify_components(&bad, p).is_err());
        }
    }
    #[test]
    fn proxy_preserves_all_fourteen_inputs_and_exact_eight_selected_sources() {
        let all = captures();
        let good = &all[0];
        let p = &CAPTURES[0];
        let selected = good["sourceIds"].as_object().unwrap();
        assert_eq!(selected.len(), 8);
        assert_eq!(good["sources"].as_object().unwrap().len(), 14);
        for name in good["sources"].as_object().unwrap().keys().filter(|n| !selected.contains_key(*n)) {
            let mut bad = good.clone();
            bad["sources"].as_object_mut().unwrap().remove(name);
            bad["stdJsonInput"]["sources"].as_object_mut().unwrap().remove(name);
            assert!(verify_components(&bad, p).is_err(), "cannot drop input-only {name}");
            let mut bad = good.clone();
            bad["sourceIds"][name] = json!({"id":99});
            bad["stdJsonOutput"]["sources"] = bad["sourceIds"].clone();
            bad["metadata"]["sources"][name] =
                json!({"keccak256":format!("0x{}",hex::encode(hash(good["sources"][name]["content"].as_str().unwrap().as_bytes())))});
            assert!(verify_components(&bad, p).is_err(), "cannot invent selected metadata {name}");
            // Input-only files have no selected metadata Keccak. Their raw-capture
            // digest and full primary bytes still bind their exact content.
            let mut altered = all.clone();
            altered[0]["sources"][name]["content"] = json!("changed");
            altered[0]["stdJsonInput"]["sources"] = altered[0]["sources"].clone();
            let primary: Value = serde_json::from_slice(include_bytes!("../../tests/fixtures/artx-role-candidate/primary-sources.json")).unwrap();
            assert!(verify_primary(&altered, &primary).is_err());
        }
    }
    #[test]
    fn implementation_self_immutable_replacements_cannot_be_rebound() {
        let good = &captures()[1];
        let p = &CAPTURES[1];
        for (pointer, value) in [
            ("/runtimeBytecode/immutableReferences/1097/0/start", json!(1786)),
            ("/runtimeBytecode/immutableReferences/1097/1/length", json!(31)),
            ("/runtimeBytecode/transformationValues/immutables/1097", json!(root(0))),
            ("/runtimeBytecode/transformations/2/offset", json!(2981)),
        ] {
            let mut bad = good.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(verify_components(&bad, p).is_err(), "{pointer}");
        }
        let mut bad = good.clone();
        let mut runtime = bytes(&bad["runtimeBytecode"]["recompiledBytecode"]).unwrap();
        runtime[1785] = 1;
        bad["runtimeBytecode"]["recompiledBytecode"] = json!(format!("0x{}", hex::encode(&runtime)));
        bad["stdJsonOutput"]["contracts"][p.source][p.name]["evm"]["deployedBytecode"]["object"] = json!(hex::encode(runtime));
        assert!(verify_components(&bad, p).is_err());
    }
    #[test]
    fn complete_constructor_payload_offsets_strings_arrays_and_creation_bytes_are_exact() {
        let all = captures();
        let good = &all[0];
        let p = &CAPTURES[0];
        let args = constructor_arguments();
        assert_eq!(args.len(), 480);
        assert_eq!(initializer_calldata().len(), 356);
        // Implementation word, bytes offset/length, selector, five ABI heads,
        // string lengths/content/padding, both empty arrays and tail padding.
        for offset in [
            0, 31, 63, 95, 96, 99, 131, 163, 195, 227, 259, 291, 292, 300, 323, 355, 356, 361, 387, 419, 451, 479,
        ] {
            let mut changed = args.clone();
            changed[offset] ^= 1;
            let mut bad = good.clone();
            bad["creationBytecode"]["transformationValues"]["constructorArguments"] = json!(format!("0x{}", hex::encode(&changed)));
            let mut creation = bytes(&good["creationBytecode"]["recompiledBytecode"]).unwrap();
            creation.extend(changed);
            bad["creationBytecode"]["onchainBytecode"] = json!(format!("0x{}", hex::encode(creation)));
            assert!(verify_components(&bad, p).is_err(), "constructor byte {offset}");
        }
        for (good, p) in all.iter().zip(&CAPTURES) {
            for (pointer, value) in [
                ("/creationBytecode/transformations", json!([{}])),
                ("/creationBytecode/transformationValues", json!({"extra":"0x00"})),
                ("/creationBytecode/onchainBytecode", json!("0x00")),
                ("/creationBytecode/recompiledBytecode", json!("0x00")),
            ] {
                let mut bad = good.clone();
                *bad.pointer_mut(pointer).unwrap() = value;
                assert!(verify_components(&bad, p).is_err(), "{} {pointer}", p.label);
            }
        }
    }
    #[test]
    fn all_thirty_four_primary_dependencies_are_exact_without_inventing_token_pin() {
        let all = captures();
        let raw = include_bytes!("../../tests/fixtures/artx-role-candidate/primary-sources.json");
        let primary = verify_primary_raw(raw, &all).unwrap();
        let mut changed = raw.to_vec();
        changed.push(b' ');
        assert!(verify_primary_raw(&changed, &all).is_err());
        for i in 0..34 {
            for (field, value) in [
                ("url", json!("https://example.invalid/main")),
                ("content", json!("changed")),
                ("sha256", json!("00")),
                ("capture", json!("token")),
            ] {
                let mut bad = primary.clone();
                bad["sources"][i][field] = value;
                assert!(verify_primary(&all, &bad).is_err(), "{i}/{field}");
            }
            let mut bad = primary.clone();
            bad["sources"][i]["in_selected_compiler_source_ids"] = json!(!primary["sources"][i]["in_selected_compiler_source_ids"].as_bool().unwrap());
            assert!(verify_primary(&all, &bad).is_err());
        }
        let mut bad = primary.clone();
        bad["sources"].as_array_mut().unwrap().pop();
        assert!(verify_primary(&all, &bad).is_err());
        let mut bad = primary.clone();
        bad["sources"][1] = bad["sources"][0].clone();
        assert!(verify_primary(&all, &bad).is_err());
        let mut bad = primary.clone();
        bad["sources"]
            .as_array_mut()
            .unwrap()
            .push(json!({"capture":"implementation","path":TOKEN,"content":all[1]["sources"][TOKEN]["content"]}));
        assert!(verify_primary(&all, &bad).is_err());
    }
    #[test]
    fn candidate_and_review_preserve_every_proxy_guard_and_unrelated_field() {
        let all = captures();
        let saved: Value = serde_json::from_str(include_str!("../../tests/fixtures/artx-role-candidate/source-review.json")).unwrap();
        assert_eq!(review(&all).unwrap(), saved);
        let baseline = include_bytes!("../../tests/fixtures/bsc-refined450-layouts.json");
        let candidate: Value = serde_json::from_str(include_str!("../../tests/fixtures/artx-role-candidate/layouts.json")).unwrap();
        verify_candidate(baseline, &candidate).unwrap();
        for (field, value) in [
            ("balance_slot", json!(root(0))),
            ("code_hash", json!("0x00")),
            ("proxy", json!({})),
            ("other_slots", json!([])),
            ("other_mapping_slots", json!([])),
            ("deployment", json!({})),
            ("other_mapping_paths", json!([])),
        ] {
            let mut bad = candidate.clone();
            bad[0][field] = value;
            assert!(verify_candidate(baseline, &bad).is_err(), "{field}");
        }
        let mut bad = candidate.clone();
        bad[0]["other_mapping_paths"]
            .as_array_mut()
            .unwrap()
            .push(json!({"root":root(151),"key_types":["bytes32"],"offset":1,"words":1}));
        assert!(verify_candidate(baseline, &bad).is_err());
        let mut raw = baseline.to_vec();
        raw.push(b' ');
        assert!(verify_candidate(&raw, &candidate).is_err());
        assert_eq!(
            identifiers("// _setRoleAdmin\n\"_setRoleAdmin\" /* _setRoleAdmin */ function _setRoleAdmin()"),
            vec!["function", "_setRoleAdmin"]
        );
    }
}
