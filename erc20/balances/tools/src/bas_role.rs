//! One unqualified BAS membership path and one fixed constructor-admin word.
//! No network access, source normalization, generic admin paths or promotion.
use anyhow::{ensure, Context, Result};
use erc20_balances::hash;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const CONTRACT: &str = "0x0f0df6cb17ee5e883eddfef9153fc6036bdb4e37";
pub const CAPTURE: &str = "8fe36b7af10f037925e8a777e45ac5647d82779c606d99790f467c52e0d8cdd0";
pub const RUNTIME: &str = "0x957bf9d2d5b267f6e4f01ddcbda34c4b3cf9d89f3af6c07c98056ca60fb9733e";
pub const BASELINE: &str = "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468";
pub const FIXTURE: &str = "tests/fixtures/bas-role-candidate";
pub const TOKEN: &str = "contracts/bas/BAS.sol";
pub const TOKEN_SHA: &str = "08beba1cb6911308c84bb235219eead860f1246e8913a7652f232ea73495c8c1";
pub const OZ_PIN: &str = "dc44c9f1a4c3b10af99492eed84f83ed244203f6";
pub const PAUSER_ROLE: &str = "0x65d7a28e3265b37a6474929f336521b332c1681b933f6cb9f3376673440d862a";
pub const ADMIN_WORD: &str = "0xe09f975e15f8f53f24cbbc282b13c40b84df485fcdb8d3997fa103dc5a4ef842";
pub const CAP_WORD: &str = "0x0000000000000000000000000000000000000000204fce5e3e25026110000000";
pub const ADMIN: &str = "0x9d8796b0ac1064ede1378d785df96970eaf5a2b9";
pub const SOURCE_GAP: &str = "None of the six bounded public BAS.sol revisions matches the captured token source. Public head lacks the captured ERC20Recovered event declaration and emit. No normalization is allowed; exact OpenZeppelin dependencies do not resolve the token-source gap.";
pub const TOKEN_REVISIONS: [(&str, &str); 6] = [
    (
        "2a988ca00a4b8301cb2d5d90f773d2fa402054cf",
        "7741dd1dfe11255183a90a1ee8e5f56ff208777ec76a05edc8b35e117fd6a940",
    ),
    (
        "7d47f1724ae4e723b48be0608f1f4f4ff9d76dbf",
        "fb47f2afd4ebd9e8f170d17fa060c35cd923f581879cab7593355be3cae88b80",
    ),
    (
        "6d139517aec4295aa6312893c3e7ff1b97a896e4",
        "6c4b460d48a288398fdff76dba692b6fbabe8d1001c0ed51a6fe1f3272cb2f94",
    ),
    (
        "870c62f27382b4f0914cf13a1a0607d3e23cbb3a",
        "3ed80914d0242e3166b7356e215cee7e653e6f41ec24fd8d4b13ebec4c23a1b5",
    ),
    (
        "3c2ca4c3b44a4834ddadd14c657263a0fa440115",
        "d692f571337637862a7b43fcf6e2071ce1c1b228d8f89f703aacde35f794acff",
    ),
    (
        "fb2fa857106fe6e630af97e88d5a2b4bb2d5599c",
        "ca3d4a683fcdea2bda23e12e312682aca8b73402bd7f31eebc6fb5110b0df1b6",
    ),
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
fn word(n: u64) -> [u8; 32] {
    let mut w = [0; 32];
    w[24..].copy_from_slice(&n.to_be_bytes());
    w
}
pub fn fixed_admin_word() -> String {
    let role = hash(b"PAUSER_ROLE");
    let mut slot = hash(&[role.as_slice(), word(6).as_slice()].concat());
    for byte in slot.iter_mut().rev() {
        let (next, carry) = byte.overflowing_add(1);
        *byte = next;
        if !carry {
            break;
        }
    }
    format!("0x{}", hex::encode(slot))
}
/// Independent ABI encoding of the five recorded constructor arguments.
pub fn constructor_arguments() -> Vec<u8> {
    let mut args = Vec::new();
    args.extend(word(160));
    args.extend(word(224));
    let cap = primitive_types::U256::from_dec_str("10000000000000000000000000000").unwrap();
    let mut cap_word = [0; 32];
    cap.to_big_endian(&mut cap_word);
    assert_eq!(format!("0x{}", hex::encode(cap_word)), CAP_WORD);
    args.extend(cap_word);
    let mut address = [0; 32];
    address[12..].copy_from_slice(&hex::decode(&ADMIN[2..]).unwrap());
    args.extend(address);
    args.extend(address);
    for value in ["BNB Attestation", "BAS"] {
        args.extend(word(value.len() as u64));
        let mut data = [0; 32];
        data[..value.len()].copy_from_slice(value.as_bytes());
        args.extend(data);
    }
    args
}
pub fn verify_capture(raw: &[u8]) -> Result<Value> {
    ensure!(sha(raw) == CAPTURE, "complete original BAS capture digest");
    let v = serde_json::from_slice(raw)?;
    verify_components(&v)?;
    Ok(v)
}
fn verify_components(v: &Value) -> Result<()> {
    ensure!(
        v["chainId"] == "56" && bytes(&v["address"])? == hex::decode(&CONTRACT[2..])?,
        "network/address binding"
    );
    ensure!(
        ["match", "runtimeMatch", "creationMatch"].iter().all(|key| v[key] == "match"),
        "preserve original match labels"
    );
    ensure!(
        v["compilation"]["compiler"] == "solc"
            && v["compilation"]["language"] == "Solidity"
            && v["compilation"]["compilerVersion"] == "0.8.26+commit.8a97fa7a"
            && v["metadata"]["compiler"]["version"] == "0.8.26+commit.8a97fa7a",
        "compiler identity"
    );
    ensure!(
        v["compilation"]["name"] == "BASToken"
            && v["compilation"]["fullyQualifiedName"] == format!("{TOKEN}:BASToken")
            && v["metadata"]["settings"]["compilationTarget"] == json!({TOKEN:"BASToken"}),
        "compilation target"
    );
    ensure!(
        v["compilation"]["compilerSettings"]
            == json!({"viaIR":true,"metadata":{"bytecodeHash":"none"},"libraries":{},"optimizer":{"runs":1000000,"enabled":true},"evmVersion":"paris","remappings":[]}),
        "exact compiler settings"
    );
    ensure!(
        v["stdJsonInput"]["language"] == "Solidity" && v["stdJsonInput"]["settings"] == v["compilation"]["compilerSettings"],
        "compiler input settings"
    );
    let sources = v["sources"].as_object().context("sources")?;
    let metadata = v["metadata"]["sources"].as_object().context("metadata sources")?;
    ensure!(
        sources.len() == 14 && sources.keys().eq(metadata.keys()) && v["sources"] == v["stdJsonInput"]["sources"],
        "complete fourteen-source input"
    );
    ensure!(
        v["sourceIds"] == v["stdJsonOutput"]["sources"] && sources.keys().eq(v["sourceIds"].as_object().context("source ids")?.keys()),
        "complete source IDs"
    );
    for (name, source) in sources {
        ensure!(name == TOKEN || name.starts_with("@openzeppelin/contracts/"), "source path");
        ensure!(
            format!("0x{}", hex::encode(hash(source["content"].as_str().context("source content")?.as_bytes()))) == metadata[name]["keccak256"],
            "source content digest {name}"
        );
    }
    let token = sources[TOKEN]["content"].as_str().context("token source")?;
    ensure!(sha(token.as_bytes()) == TOKEN_SHA, "exact captured token body");
    ensure!(
        token.matches("_setRoleAdmin").count() == 1 && token.contains("_setRoleAdmin(PAUSER_ROLE, PAUSER_ROLE);"),
        "sole reviewed token role-admin call"
    );
    ensure!(
        fixed_admin_word() == ADMIN_WORD && format!("0x{}", hex::encode(hash(b"PAUSER_ROLE"))) == PAUSER_ROLE,
        "fixed admin location derivation"
    );
    let output = &v["stdJsonOutput"]["contracts"][TOKEN]["BASToken"];
    let compiled_metadata: Value = serde_json::from_str(output["metadata"].as_str().context("compiler metadata")?)?;
    ensure!(compiled_metadata == v["metadata"], "compiler metadata binding");
    for field in ["abi", "devdoc", "userdoc", "storageLayout", "transientStorageLayout"] {
        ensure!(output[field] == v[field], "compiler output {field}");
    }
    let runtime = &v["runtimeBytecode"];
    ensure!(
        runtime["immutableReferences"] == json!({"1169":[{"start":3249,"length":32},{"start":4710,"length":32}]}),
        "single cap immutable schema"
    );
    ensure!(
        runtime["transformations"]
            == json!([{"id":"1169","type":"replace","offset":3249,"reason":"immutable"},{"id":"1169","type":"replace","offset":4710,"reason":"immutable"}]),
        "only two immutable replacements"
    );
    ensure!(
        runtime["transformationValues"] == json!({"immutables":{"1169":CAP_WORD}}) && runtime["linkReferences"] == json!({}),
        "cap value and no links"
    );
    ensure!(
        runtime["cborAuxdata"] == json!({"1":{"value":"0xa164736f6c634300081a000a","offset":9640}}),
        "unchanged runtime metadata"
    );
    for field in ["immutableReferences", "linkReferences", "sourceMap"] {
        ensure!(runtime[field] == output["evm"]["deployedBytecode"][field], "compiler runtime {field}");
    }
    let compiled = bytes(&runtime["recompiledBytecode"])?;
    ensure!(
        compiled.len() == 9652 && compiled == bytes(&output["evm"]["deployedBytecode"]["object"])?,
        "saved compiler runtime"
    );
    ensure!(compiled[9640..] == hex::decode("a164736f6c634300081a000a")?, "original runtime CBOR bytes");
    let mut patched = compiled;
    for offset in [3249, 4710] {
        ensure!(patched[offset..offset + 32] == [0; 32], "zero immutable placeholder");
        patched[offset..offset + 32].copy_from_slice(&hex::decode(&CAP_WORD[2..])?);
    }
    ensure!(
        patched == bytes(&runtime["onchainBytecode"])? && format!("0x{}", hex::encode(hash(&patched))) == RUNTIME,
        "exact saved runtime reconstruction"
    );
    verify_constructor(v, output)?;
    verify_layout(v)?;
    Ok(())
}
fn verify_constructor(v: &Value, output: &Value) -> Result<()> {
    let c = &v["creationBytecode"];
    ensure!(
        c["linkReferences"] == json!({})
            && c["cborAuxdata"] == json!({})
            && c["transformations"] == json!([{"type":"insert","offset":11365,"reason":"constructorArguments"}]),
        "only constructor append"
    );
    let args = constructor_arguments();
    ensure!(
        args.len() == 288 && c["transformationValues"] == json!({"constructorArguments":format!("0x{}",hex::encode(&args))}),
        "canonical five-argument ABI append"
    );
    let mut compiled = bytes(&c["recompiledBytecode"])?;
    ensure!(
        compiled.len() == 11365 && compiled == bytes(&output["evm"]["bytecode"]["object"])?,
        "saved compiler creation object"
    );
    for field in ["linkReferences", "sourceMap"] {
        ensure!(c[field] == output["evm"]["bytecode"][field], "compiler creation {field}");
    }
    compiled.extend(args);
    ensure!(
        compiled.len() == 11653 && compiled == bytes(&c["onchainBytecode"])?,
        "exact saved creation reconstruction"
    );
    let ctors: Vec<_> = v["abi"]
        .as_array()
        .context("ABI")?
        .iter()
        .filter(|entry| entry["type"] == "constructor")
        .collect();
    ensure!(
        ctors.len() == 1
            && ctors[0]
                == &json!({"type":"constructor","stateMutability":"nonpayable","inputs":[{"name":"name","type":"string","internalType":"string"},{"name":"symbol","type":"string","internalType":"string"},{"name":"cap","type":"uint256","internalType":"uint256"},{"name":"admin","type":"address","internalType":"address"},{"name":"pauser","type":"address","internalType":"address"}]}),
        "exact constructor ABI"
    );
    ensure!(
        v["deployment"]["blockNumber"] == "53994920"
            && v["deployment"]["transactionIndex"] == "117"
            && v["deployment"]["transactionHash"] == "0x5368d4291e1b1427f0f61abe80ff515c5814e88f573a35dd71f4754accfd1c72"
            && bytes(&v["deployment"]["deployer"])? == hex::decode(&ADMIN[2..])?,
        "recorded deployment identity"
    );
    Ok(())
}
fn verify_layout(v: &Value) -> Result<()> {
    let layout = &v["storageLayout"];
    let fields = layout["storage"].as_array().context("storage fields")?;
    let names = [
        "_paused",
        "_balances",
        "_allowances",
        "_totalSupply",
        "_name",
        "_symbol",
        "_roles",
        "_isWhitelisted",
    ];
    ensure!(fields.len() == names.len(), "complete layout");
    for (i, name) in names.iter().enumerate() {
        ensure!(
            fields[i]["label"] == *name
                && fields[i]["slot"].as_str() == Some(["0", "1", "2", "3", "4", "5", "6", "7"][i])
                && fields[i]["offset"] == 0
                && fields[i]["contract"] == format!("{TOKEN}:BASToken"),
            "layout slot/name/offset/contract"
        );
    }
    let types = &layout["types"];
    let role = &types[fields[6]["type"].as_str().context("role type")?];
    ensure!(role["encoding"] == "mapping" && role["key"] == "t_bytes32", "outer role mapping");
    let record = &types[role["value"].as_str().context("role record")?];
    let members = record["members"].as_array().context("role members")?;
    ensure!(
        record["numberOfBytes"] == "64"
            && members.len() == 2
            && members[0]["label"] == "members"
            && members[0]["slot"] == "0"
            && members[0]["offset"] == 0
            && members[1]["label"] == "adminRole"
            && members[1]["slot"] == "1"
            && members[1]["offset"] == 0
            && members[1]["type"] == "t_bytes32",
        "role record shape"
    );
    let inner = &types[members[0]["type"].as_str().context("membership type")?];
    ensure!(
        inner["encoding"] == "mapping" && inner["key"] == "t_address" && inner["value"] == "t_bool" && types["t_bool"]["numberOfBytes"] == "1",
        "address/bool membership"
    );
    for (i, kind) in [
        (0, "t_bool"),
        (1, "t_mapping(t_address,t_uint256)"),
        (2, "t_mapping(t_address,t_mapping(t_address,t_uint256))"),
        (3, "t_uint256"),
        (4, "t_string_storage"),
        (5, "t_string_storage"),
        (7, "t_mapping(t_address,t_bool)"),
    ] {
        ensure!(fields[i]["type"] == kind, "unchanged non-role field type");
    }
    Ok(())
}
pub fn primary_url(name: &str) -> Result<String> {
    let suffix = name.strip_prefix("@openzeppelin/contracts/").context("OZ source prefix")?;
    Ok(format!(
        "https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{OZ_PIN}/contracts/{suffix}"
    ))
}
pub fn token_url(pin: &str) -> String {
    format!("https://raw.githubusercontent.com/bnb-attestation-service/bas-erc20/{pin}/BAS.sol")
}
pub fn verify_primary(capture: &Value, primary: &Value) -> Result<()> {
    ensure!(
        primary["qualified"] == false && primary["token_source_gap"] == SOURCE_GAP && primary["oz_pin"] == OZ_PIN,
        "explicit primary-source gap"
    );
    let saved = primary["sources"].as_object().context("primary OZ sources")?;
    let sources = capture["sources"].as_object().context("captured sources")?;
    ensure!(
        saved.len() == 13 && saved.keys().eq(sources.keys().filter(|name| name.as_str() != TOKEN)),
        "all thirteen exact OZ files"
    );
    for (name, v) in saved {
        let content = v["content"].as_str().context("primary content")?;
        ensure!(
            v["url"] == primary_url(name)? && v["sha256"] == sha(content.as_bytes()) && sources[name]["content"] == content,
            "exact primary source {name}"
        );
    }
    let revisions = primary["token_revisions"].as_array().context("token revisions")?;
    ensure!(revisions.len() == 6, "bounded six token revisions");
    for (saved, (pin, digest)) in revisions.iter().zip(TOKEN_REVISIONS) {
        let content = saved["content"].as_str().context("revision content")?;
        ensure!(
            saved["pin"] == pin
                && saved["url"] == token_url(pin)
                && saved["sha256"] == digest
                && sha(content.as_bytes()) == digest
                && saved["exact_match"] == false
                && sources[TOKEN]["content"] != content,
            "unchanged nonmatching primary revision {pin}"
        );
    }
    Ok(())
}
pub fn candidate(baseline_raw: &[u8]) -> Result<Value> {
    ensure!(sha(baseline_raw) == BASELINE, "unchanged historical431 baseline");
    let baseline: Value = serde_json::from_slice(baseline_raw)?;
    let mut c = baseline
        .as_array()
        .context("baseline")?
        .iter()
        .find(|p| p["contract"] == CONTRACT)
        .context("BAS profile")?
        .clone();
    ensure!(
        c["code_hash"] == RUNTIME && c["balance_slot"] == root(1) && c.get("deployment").is_none() && c.get("other_mapping_paths").is_none(),
        "baseline runtime/balance/guard scope"
    );
    ensure!(
        c["other_mapping_words"].as_object_mut().context("legacy role width")?.remove(&root(6)) == Some(json!(2)),
        "legacy root6 width2"
    );
    c["other_mapping_paths"] = json!([{"root":root(6),"key_types":["bytes32","address"],"offset":0,"words":1}]);
    let slots = c["other_slots"].as_array_mut().context("scalar slots")?;
    ensure!(
        !slots.contains(&json!(ADMIN_WORD)) && fixed_admin_word() == ADMIN_WORD,
        "new exact fixed admin word"
    );
    slots.push(json!(ADMIN_WORD));
    Ok(json!([c]))
}
pub fn verify_candidate(baseline: &[u8], candidate_json: &Value) -> Result<()> {
    ensure!(
        candidate(baseline)? == *candidate_json,
        "only BAS exact membership and fixed PAUSER admin slot may change"
    );
    Ok(())
}
pub fn review(capture: &Value) -> Result<Value> {
    verify_components(capture)?;
    let sources: Vec<_> = capture["sources"].as_object().unwrap().iter().map(|(name,s)| json!({"path":name,"sha256":sha(s["content"].as_str().unwrap().as_bytes()),"keccak256":capture["metadata"]["sources"][name]["keccak256"]})).collect();
    Ok(
        json!({"qualified":false,"contract":CONTRACT,"chain_id":56,"source_capture_sha256":CAPTURE,"source_files":sources,"runtime_hash":RUNTIME,"original_match_labels":{"match":capture["match"],"runtimeMatch":capture["runtimeMatch"],"creationMatch":capture["creationMatch"]},"runtime_reconstructed_from_saved_compiler_bytes":true,"fresh_compilation":false,"immutable_cap":"10000000000000000000000000000","immutable_id":"1169","immutable_sites":[3249,4710],"constructor_abi_append_bytes":288,"constructor":{"name":"BNB Attestation","symbol":"BAS","admin":ADMIN,"pauser":ADMIN},"recorded_deployment":capture["deployment"],"current_admin_or_deployment_qualification":false,"membership_root":root(6),"pauser_role":PAUSER_ROLE,"fixed_pauser_admin_word":ADMIN_WORD,"role_admin_callsite":"constructor-only _setRoleAdmin(PAUSER_ROLE, PAUSER_ROLE)","token_primary_source_gap":SOURCE_GAP,"scope":"Exact captured source/compiler/runtime/constructor binding only; synthetic shapes and saved replay do not qualify a deployment or replacement package. No generic role-admin rule or constructor admission."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    const RAW: &[u8] = include_bytes!("../../tests/fixtures/bas-role-candidate/source-capture.json");
    #[test]
    fn capture_compiler_sources_layout_immutables_and_match_labels_are_bound() {
        let good = verify_capture(RAW).unwrap();
        let mut raw = RAW.to_vec();
        raw.push(b' ');
        assert!(verify_capture(&raw).is_err());
        for (pointer, value) in [
            ("/chainId", json!("1")),
            ("/address", json!("0x01")),
            ("/match", json!("exact_match")),
            ("/runtimeMatch", json!("exact_match")),
            ("/creationMatch", json!("exact_match")),
            ("/sources", json!({})),
            ("/stdJsonInput/sources", json!({})),
            ("/sourceIds", json!({})),
            ("/stdJsonInput/settings/optimizer/runs", json!(200)),
            ("/compilation/compilerSettings/viaIR", json!(false)),
            ("/metadata/compiler/version", json!("changed")),
            ("/runtimeBytecode/linkReferences", json!({"new":[]})),
            ("/runtimeBytecode/immutableReferences/1169/0/start", json!(3250)),
            ("/runtimeBytecode/immutableReferences/1169/1/length", json!(31)),
            ("/runtimeBytecode/transformationValues/immutables/1169", json!(root(1))),
            ("/runtimeBytecode/transformations/0/offset", json!(3250)),
            ("/runtimeBytecode/cborAuxdata/1/offset", json!(0)),
            ("/runtimeBytecode/onchainBytecode", json!("0x00")),
            ("/runtimeBytecode/recompiledBytecode", json!("0x00")),
            ("/storageLayout/storage/6/slot", json!("7")),
            ("/storageLayout/types/t_mapping(t_address,t_bool)/value", json!("t_uint256")),
        ] {
            let mut bad = good.clone();
            *bad.pointer_mut(pointer).expect(pointer) = value;
            assert!(verify_components(&bad).is_err(), "{pointer}");
        }
        for name in good["sources"].as_object().unwrap().keys() {
            let mut bad = good.clone();
            bad["sources"][name]["content"] = json!("tampered");
            assert!(verify_components(&bad).is_err(), "{name}");
        }
        let mut bad = good.clone();
        bad["stdJsonOutput"]["contracts"][TOKEN]["BASToken"]["evm"]["deployedBytecode"]["object"] = json!("00");
        assert!(verify_components(&bad).is_err());
        // Even self-consistent saved compiler objects cannot repurpose the cap sites.
        let mut bad = good.clone();
        let mut compiled = bytes(&bad["runtimeBytecode"]["recompiledBytecode"]).unwrap();
        compiled[3249] = 1;
        bad["runtimeBytecode"]["recompiledBytecode"] = json!(format!("0x{}", hex::encode(&compiled)));
        bad["stdJsonOutput"]["contracts"][TOKEN]["BASToken"]["evm"]["deployedBytecode"]["object"] = json!(hex::encode(compiled));
        assert!(verify_components(&bad).is_err());
    }
    #[test]
    fn constructor_abi_append_cap_addresses_and_saved_creation_are_exact() {
        let good = verify_capture(RAW).unwrap();
        let output = &good["stdJsonOutput"]["contracts"][TOKEN]["BASToken"];
        verify_constructor(&good, output).unwrap();
        let args = constructor_arguments();
        assert_eq!(args.len(), 288);
        // Dynamic offsets, cap, address padding/values, string sizes/data/padding.
        for offset in [31, 63, 95, 96, 127, 159, 191, 192, 207, 223, 255, 256, 259, 287] {
            let mut changed = args.clone();
            changed[offset] ^= 1;
            let mut bad = good.clone();
            bad["creationBytecode"]["transformationValues"]["constructorArguments"] = json!(format!("0x{}", hex::encode(&changed)));
            let mut creation = bytes(&bad["creationBytecode"]["recompiledBytecode"]).unwrap();
            creation.extend(changed);
            bad["creationBytecode"]["onchainBytecode"] = json!(format!("0x{}", hex::encode(creation)));
            assert!(verify_constructor(&bad, output).is_err(), "constructor byte {offset}");
        }
        for (pointer, value) in [
            ("/creationBytecode/transformations/0/offset", json!(11364)),
            ("/creationBytecode/cborAuxdata", json!({"1":{}})),
            ("/creationBytecode/recompiledBytecode", json!("0x00")),
            ("/creationBytecode/onchainBytecode", json!("0x00")),
            ("/deployment/blockNumber", json!("53994921")),
            ("/deployment/transactionHash", json!("0x00")),
            ("/deployment/deployer", json!("0x00")),
        ] {
            let mut bad = good.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(verify_constructor(&bad, output).is_err(), "{pointer}");
        }
        let mut bad = good.clone();
        bad["abi"].as_array_mut().unwrap().iter_mut().find(|v| v["type"] == "constructor").unwrap()["inputs"][4]["type"] = json!("bytes32");
        assert!(verify_constructor(&bad, output).is_err());
    }
    #[test]
    fn exact_oz_sources_do_not_normalize_the_six_nonmatching_token_revisions() {
        let capture = verify_capture(RAW).unwrap();
        let primary: Value = serde_json::from_str(include_str!("../../tests/fixtures/bas-role-candidate/primary-sources.json")).unwrap();
        verify_primary(&capture, &primary).unwrap();
        for (pointer, value) in [
            ("/qualified", json!(true)),
            ("/token_source_gap", json!("resolved")),
            ("/oz_pin", json!("main")),
            ("/sources", json!({})),
            ("/token_revisions", json!([])),
        ] {
            let mut bad = primary.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(verify_primary(&capture, &bad).is_err(), "{pointer}");
        }
        for name in primary["sources"].as_object().unwrap().keys() {
            let mut bad = primary.clone();
            bad["sources"][name]["content"] = json!("changed");
            bad["sources"][name]["sha256"] = json!(sha(b"changed"));
            assert!(verify_primary(&capture, &bad).is_err(), "{name}");
        }
        for i in 0..6 {
            for (field, value) in [
                ("exact_match", json!(true)),
                ("pin", json!("main")),
                ("url", json!(token_url("main"))),
                ("content", capture["sources"][TOKEN]["content"].clone()),
            ] {
                let mut bad = primary.clone();
                bad["token_revisions"][i][field] = value;
                assert!(verify_primary(&capture, &bad).is_err(), "revision{i}/{field}");
            }
        }
    }
    #[test]
    fn candidate_and_review_change_only_membership_and_one_fixed_admin_location() {
        let capture = verify_capture(RAW).unwrap();
        let saved: Value = serde_json::from_str(include_str!("../../tests/fixtures/bas-role-candidate/source-review.json")).unwrap();
        assert_eq!(review(&capture).unwrap(), saved);
        let baseline = include_bytes!("../../tests/fixtures/bsc-refined450-layouts.json");
        let candidate: Value = serde_json::from_str(include_str!("../../tests/fixtures/bas-role-candidate/layouts.json")).unwrap();
        verify_candidate(baseline, &candidate).unwrap();
        assert_eq!(fixed_admin_word(), ADMIN_WORD);
        for (field, value) in [
            ("code_hash", json!("0x00")),
            ("balance_slot", json!(root(0))),
            ("other_slots", json!([])),
            ("other_mapping_paths", json!([])),
            ("other_mapping_slots", json!([])),
            ("deployment", json!({})),
        ] {
            let mut bad = candidate.clone();
            bad[0][field] = value;
            assert!(verify_candidate(baseline, &bad).is_err(), "{field}");
        }
        let mut bad = candidate.clone();
        bad[0]["other_mapping_paths"]
            .as_array_mut()
            .unwrap()
            .push(json!({"root":root(6),"key_types":["bytes32"],"offset":1,"words":1}));
        assert!(verify_candidate(baseline, &bad).is_err());
        let mut raw = baseline.to_vec();
        raw.push(b' ');
        assert!(verify_candidate(&raw, &candidate).is_err());
    }
}
