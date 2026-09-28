//! Separate, unqualified Kgen/Deep plain-role candidates. No live chain access.
use anyhow::{ensure, Context, Result};
use erc20_balances::hash;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const KGEN: &str = "0xf3d5b4c34ed623478cc5141861776e6cf7ae3a1e";
pub const DEEP: &str = "0x9b6a1d4fa5d90e5f2d34130053978d14cd301d58";
pub const IMPLEMENTATION: &str = "0x76ec4be0109305fb4543f1d1a8a515ac3bcabd60";
pub const ORIGINAL_IMPLEMENTATION: &str = "0x563fe7f145f39e31cd7138193df6fe03f5b14253";
pub const IMPLEMENTATION_SLOT: &str = "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc";
pub const ROLE_ROOT: &str = "0x02dd7bc7dec4dceedda775e58dd541e08a116c6c53815c0bd028192f7b626800";
pub const BALANCE_ROOT: &str = "0x52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace00";
pub const BASELINE: &str = "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468";
pub const FIXTURE: &str = "tests/fixtures/oft-role-candidates";
pub const ENDPOINT: &str = "0x1a44076050125825900e736c501f859c50fe728c";
pub const FORWARDER: &str = "0xbd09506ce83927374796516ef139edd35c8aa378";
pub const KGEN_DELEGATE: &str = "0x17cfabf22aa9f67aa178b3f53e3121faac31b339";
pub const PROXY_OWNER: &str = "0x40e0400d7f4cb921635bf241b7ea0af4914d59be";
pub const VENDORED_URL: &str = "https://raw.githubusercontent.com/kgen-protocol/smartcontracts/5fc4a4250ff7039e68ee06b83ac1376ed779546b/KGeN-Token/BASE/deployments/bsc-mainnet/solcInputs/23ba6c1962a4e8042bae5691a051440d.json";
pub const VENDORED_SHA: &str = "5adf0df4ff9f8d5ffc81de036930455d23ce9ed7fc7904f4943844a799600c48";
pub const NEAR_URL: &str =
    "https://raw.githubusercontent.com/kgen-protocol/smartcontracts/5fc4a4250ff7039e68ee06b83ac1376ed779546b/KGeN-Token/BSC/contracts/KgenOFT.sol";
pub const NEAR_SHA: &str = "bc7e2713fcf3beb380dd5c19259b10acc701c2114a489d0536cbad65a6fe7eb1";
pub const INTERFACE: &str = "@layerzerolabs/lz-evm-protocol-v2/contracts/interfaces/IMessageLibManager.sol";
pub const NONMATCHING_INTERFACE_SHA: &str = "c648091e7cb079cb95778becfff1a813e901fdc07124153ba731a7d9daa29b40";
pub const OLD_INITIALIZER: &str = "f8c8765e000000000000000000000000e0e90104409c860e780ba9a702db2d45c5e5a10100000000000000000000000047c3026020448f26b015a6f2d3643a24f51745e000000000000000000000000063fa2433fb5876cb0ae929c809247d9e2b5fe1c9000000000000000000000000e0e90104409c860e780ba9a702db2d45c5e5a101";
pub const HISTORY_LIMIT: &str = "The saved proxy constructor initializes an older implementation before creating ProxyAdmin. RLP(proxy, nonce 1) matches the saved immutable but does not prove the old initializer made no CREATE calls, its semantics, deployment qualification, current pointer/owner or initialized holders. The old initializer is opaque and is not decoded as the later DeepTokenOFT implementation.";
pub const SOURCE_GAPS: &str = "Kgen public token source differs in whitespace and is not normalized; its exact protocol interface snapshot is vendored in the token repository, not an exact LayerZero upstream match. Deep has seven custom primary-source gaps plus the nonmatching upstream protocol interface. All captured source bytes remain complete and pinned.";

#[derive(Clone, Copy)]
pub struct Capture {
    pub label: &'static str,
    pub address: &'static str,
    pub digest: &'static str,
    pub cache: &'static str,
    pub source: &'static str,
    pub name: &'static str,
    pub count: usize,
    pub compiler: &'static str,
    pub runtime: &'static str,
    pub runtime_len: usize,
    pub creation_len: usize,
    pub block: &'static str,
    pub index: &'static str,
    pub tx: &'static str,
}
pub const CAPTURES: [Capture; 3] = [
    Capture {
        label: "kgen",
        address: KGEN,
        digest: "e71ba802e1b6158d14ee38e47c793fb7ef025a484f9a3221a95a70a9bd0190df",
        cache: "out/ranks101-150-source-review/0xf3d5b4c34ed623478cc5141861776e6cf7ae3a1e.json",
        source: "contracts/KgenOFT.sol",
        name: "KgenOFT",
        count: 55,
        compiler: "0.8.22+commit.4fc1097e",
        runtime: "0xfc77b3c0763583345ead87010618488c5eb5f292e12c0857b94bd200761938cd",
        runtime_len: 19452,
        creation_len: 21978,
        block: "59625960",
        index: "88",
        tx: "0x187f6f6687df866a0e4e3f428a072a35393a92708cf4250d31262dac78667de7",
    },
    Capture {
        label: "deep",
        address: IMPLEMENTATION,
        digest: "3f2b8fa6fcf517c932f4793951a3aba6ff4567ef0c23c6dd989c33d0ba102a7e",
        cache: "out/proxy400-source-review/0x76ec4be0109305fb4543f1d1a8a515ac3bcabd60.json",
        source: "contracts/DeepTokenOFT.sol",
        name: "DeepTokenOFT",
        count: 66,
        compiler: "0.8.28+commit.7893614a",
        runtime: "0x65a4e8836b6ad79fd414fbbfe50ce30aa221ef90703d21e19327bb0f8c78340c",
        runtime_len: 18925,
        creation_len: 19679,
        block: "73719448",
        index: "57",
        tx: "0xc535e44147d4699a7ab7adf6948075016b2e7de9b1aeb449d87dbba0ec4796c3",
    },
    Capture {
        label: "proxy",
        address: DEEP,
        digest: "41fbd48e0c4d9dd498b6c3c5b6239e98caa2c9df1d1d3d575bb473f93a094c48",
        cache: "out/proxy400-source-review/0x9b6a1d4fa5d90e5f2d34130053978d14cd301d58.json",
        source: "@openzeppelin/contracts/proxy/transparent/TransparentUpgradeableProxy.sol",
        name: "TransparentUpgradeableProxy",
        count: 12,
        compiler: "0.8.28+commit.7893614a",
        runtime: "0xd880cb6fa8a18faabeb57bb1cb2b1900371f7338c8c3c3f7add63574dad08e3c",
        runtime_len: 1166,
        creation_len: 3714,
        block: "73700424",
        index: "83",
        tx: "0xd10b7561d49decc27b79c2b8702aca3ba3b782eda57911fa9a0b1c956831196e",
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
fn address(s: &str) -> [u8; 32] {
    let mut w = [0; 32];
    w[12..].copy_from_slice(&hex::decode(&s[2..]).unwrap());
    w
}
fn plus(mut w: [u8; 32]) -> [u8; 32] {
    for b in w.iter_mut().rev() {
        let (v, c) = b.overflowing_add(1);
        *b = v;
        if !c {
            break;
        }
    }
    w
}
fn namespace(name: &str) -> [u8; 32] {
    let mut w = hash(name.as_bytes());
    for b in w.iter_mut().rev() {
        let (v, c) = b.overflowing_sub(1);
        *b = v;
        if !c {
            break;
        }
    }
    let mut slot = hash(&w);
    slot[31] = 0;
    slot
}
pub fn admin_words() -> Vec<String> {
    let base = namespace("openzeppelin.storage.AccessControl");
    assert_eq!(format!("0x{}", hex::encode(base)), ROLE_ROOT);
    ["ADMIN_ROLE", "GUARDIAN_ROLE", "BANLIST_OPERATOR_ROLE"]
        .iter()
        .map(|s| {
            let preimage = [hash(s.as_bytes()).as_slice(), base.as_slice()].concat();
            format!("0x{}", hex::encode(plus(hash(&preimage))))
        })
        .collect()
}
/// A tested reconstruction hypothesis, not proof of the old initializer's nonce effects.
pub fn proxy_admin_at_nonce_one() -> [u8; 32] {
    let mut rlp = vec![0xd6, 0x94];
    rlp.extend(hex::decode(&DEEP[2..]).unwrap());
    rlp.push(1);
    let digest = hash(&rlp);
    let mut out = [0; 32];
    out[12..].copy_from_slice(&digest[12..]);
    out
}
pub fn constructor_arguments(p: &Capture) -> Vec<u8> {
    match p.label {
        "kgen" => {
            let mut v = word(160).to_vec();
            v.extend(word(224));
            for a in [ENDPOINT, KGEN_DELEGATE, FORWARDER] {
                v.extend(address(a));
            }
            for s in ["KGEN", "KGEN"] {
                v.extend(word(s.len() as u64));
                let mut w = [0; 32];
                w[..s.len()].copy_from_slice(s.as_bytes());
                v.extend(w);
            }
            v
        }
        "deep" => address(ENDPOINT).to_vec(),
        "proxy" => {
            let data = hex::decode(OLD_INITIALIZER).unwrap();
            let mut v = address(ORIGINAL_IMPLEMENTATION).to_vec();
            v.extend(address(PROXY_OWNER));
            v.extend(word(96));
            v.extend(word(data.len() as u64));
            v.extend(data);
            v.resize(288, 0);
            v
        }
        _ => unreachable!(),
    }
}
fn immutable_specs(p: &Capture) -> Vec<(&'static str, Vec<usize>, [u8; 32])> {
    match p.label {
        "kgen" => vec![
            ("1386", vec![2284, 4605, 8747, 9717, 10890, 13712, 14536, 14721], address(ENDPOINT)),
            ("2778", vec![2794, 12300, 12530, 13403], word(10u64.pow(8 - 6))),
            ("4703", vec![2552], address(FORWARDER)),
        ],
        "deep" => vec![
            ("1356", vec![1898, 3398, 6677, 8281, 9320, 11794, 13400, 13585], address(ENDPOINT)),
            ("3194", vec![2347, 10694, 10811, 11404], word(10u64.pow(18 - 6))),
        ],
        "proxy" => vec![("621", vec![16], proxy_admin_at_nonce_one())],
        _ => unreachable!(),
    }
}
fn expected_cbor(p: &Capture, creation: bool) -> Value {
    let (value, offset) = match p.label {
        "kgen" => (
            "0xa26469706673582212200e47c66b4e3bfac783250597527ba97ee3830214fc4e3b8d48f7104b84aaa2a364736f6c63430008160033",
            if creation { 21925 } else { 19399 },
        ),
        "deep" => (
            "0xa26469706673582212200dfb848e4fb9fea5e4510c4d9cfd8efec531abda5c7e0d78c0cd9d144b2b1d0764736f6c634300081c0033",
            if creation { 19626 } else { 18872 },
        ),
        "proxy" => (
            "0xa26469706673582212200a915166deaf12ab2955d1dad78479e5e2669cafd29b29f0d33c172bb9023a6964736f6c634300081c0033",
            if creation { 2305 } else { 1113 },
        ),
        _ => unreachable!(),
    };
    let mut out = json!({"1":{"value":value,"offset":offset}});
    if p.label == "proxy" && creation {
        out["2"] =
            json!({"value":"0xa26469706673582212202fcfc3f26013797f0418cb34bc88460545f4eda45f19f53b702cf1fe34083ab664736f6c634300081c0033","offset":3629});
    }
    out
}
pub fn verify_capture(raw: &[u8], p: &Capture) -> Result<Value> {
    ensure!(sha(raw) == p.digest, "complete original {} capture digest", p.label);
    let v = serde_json::from_slice(raw)?;
    verify_components(&v, p)?;
    Ok(v)
}
fn verify_components(v: &Value, p: &Capture) -> Result<()> {
    ensure!(
        v["chainId"] == "56" && bytes(&v["address"])? == hex::decode(&p.address[2..])?,
        "network/address identity"
    );
    ensure!(
        ["match", "runtimeMatch", "creationMatch"].iter().all(|k| v[k] == "exact_match"),
        "original exact_match labels"
    );
    let settings = json!({"metadata":{"useLiteralContent":true},"optimizer":{"enabled":true,"runs":200},"evmVersion":"paris"});
    ensure!(
        v["compilation"]["compiler"] == "solc"
            && v["compilation"]["language"] == "Solidity"
            && v["compilation"]["compilerVersion"] == p.compiler
            && v["metadata"]["compiler"]["version"] == p.compiler
            && v["compilation"]["name"] == p.name
            && v["compilation"]["fullyQualifiedName"] == format!("{}:{}", p.source, p.name)
            && v["metadata"]["settings"]["compilationTarget"] == json!({p.source:p.name}),
        "compiler target identity"
    );
    ensure!(
        v["compilation"]["compilerSettings"] == settings && v["stdJsonInput"]["settings"] == settings && v["stdJsonInput"]["language"] == "Solidity",
        "exact compiler settings"
    );
    let sources = v["sources"].as_object().context("sources")?;
    ensure!(
        sources.len() == p.count && v["sources"] == v["stdJsonInput"]["sources"],
        "complete compiler input sources"
    );
    ensure!(
        v["sourceIds"] == v["stdJsonOutput"]["sources"]
            && sources.keys().eq(v["sourceIds"].as_object().context("IDs")?.keys())
            && sources.keys().eq(v["metadata"]["sources"].as_object().context("metadata sources")?.keys()),
        "complete selected source/metadata IDs"
    );
    for (name, s) in sources {
        ensure!(
            format!("0x{}", hex::encode(hash(s["content"].as_str().context("content")?.as_bytes()))) == v["metadata"]["sources"][name]["keccak256"],
            "metadata source Keccak {name}"
        );
    }
    let output = &v["stdJsonOutput"]["contracts"][p.source][p.name];
    ensure!(
        serde_json::from_str::<Value>(output["metadata"].as_str().context("compiler metadata")?)? == v["metadata"],
        "compiler metadata"
    );
    for field in ["abi", "userdoc", "devdoc", "storageLayout", "transientStorageLayout"] {
        ensure!(output[field] == v[field], "compiler output {field}");
    }
    ensure!(
        v["deployment"]["blockNumber"] == p.block
            && v["deployment"]["transactionIndex"] == p.index
            && v["deployment"]["transactionHash"] == p.tx
            && bytes(&v["deployment"]["deployer"])? == hex::decode(&if p.label == "kgen" { KGEN_DELEGATE } else { PROXY_OWNER }[2..])?,
        "recorded deployment identity only"
    );
    verify_bytecode(v, output, p)?;
    verify_layout_and_roles(v, p)?;
    Ok(())
}
fn verify_bytecode(v: &Value, output: &Value, p: &Capture) -> Result<()> {
    let r = &v["runtimeBytecode"];
    let c = &v["creationBytecode"];
    for (saved, compiled, len, creation) in [
        (r, &output["evm"]["deployedBytecode"], p.runtime_len, false),
        (c, &output["evm"]["bytecode"], p.creation_len, true),
    ] {
        ensure!(
            saved["linkReferences"] == json!({}) && saved["linkReferences"] == compiled["linkReferences"] && saved["sourceMap"] == compiled["sourceMap"],
            "complete compiler maps/no links"
        );
        let raw = bytes(&saved["recompiledBytecode"])?;
        ensure!(raw.len() == len && raw == bytes(&compiled["object"])?, "complete compiler bytecode");
        let cbor = expected_cbor(p, creation);
        ensure!(saved["cborAuxdata"] == cbor, "exact compiler CBOR locations");
        for aux in cbor.as_object().unwrap().values() {
            let b = bytes(&aux["value"])?;
            let start = aux["offset"].as_u64().unwrap() as usize;
            ensure!(raw.get(start..start + b.len()) == Some(b.as_slice()), "unchanged CBOR bytes");
        }
    }
    let mut runtime = bytes(&r["recompiledBytecode"])?;
    let mut refs = Map::new();
    let mut values = Map::new();
    let mut transforms = Vec::new();
    for (id, offsets, value) in immutable_specs(p) {
        refs.insert(
            id.into(),
            json!(offsets.iter().map(|offset| json!({"start":offset,"length":32})).collect::<Vec<_>>()),
        );
        values.insert(id.into(), json!(format!("0x{}", hex::encode(value))));
        for offset in offsets {
            ensure!(runtime.get(offset..offset + 32) == Some([0u8; 32].as_slice()), "zero immutable placeholder");
            runtime[offset..offset + 32].copy_from_slice(&value);
            transforms.push(json!({"id":id,"type":"replace","offset":offset,"reason":"immutable"}));
        }
    }
    ensure!(
        r["immutableReferences"] == Value::Object(refs)
            && r["immutableReferences"] == output["evm"]["deployedBytecode"]["immutableReferences"]
            && r["transformationValues"] == json!({"immutables":values})
            && r["transformations"] == json!(transforms),
        "exact immutable schema and independently derived values"
    );
    ensure!(
        runtime == bytes(&r["onchainBytecode"])? && format!("0x{}", hex::encode(hash(&runtime))) == p.runtime,
        "full reconstructed runtime binding"
    );
    let args = constructor_arguments(p);
    let mut creation = bytes(&c["recompiledBytecode"])?;
    creation.extend(&args);
    ensure!(
        c["transformations"] == json!([{"type":"insert","offset":p.creation_len,"reason":"constructorArguments"}])
            && c["transformationValues"] == json!({"constructorArguments":format!("0x{}",hex::encode(&args))})
            && creation == bytes(&c["onchainBytecode"])?,
        "complete creation and exact constructor append"
    );
    Ok(())
}
fn body<'a>(v: &'a Value, path: &str) -> Result<&'a str> {
    v["sources"][path]["content"].as_str().context("required source body")
}
fn verify_layout_and_roles(v: &Value, p: &Capture) -> Result<()> {
    if p.label == "proxy" {
        ensure!(v["storageLayout"] == json!({"storage":[],"types":null}), "empty proxy compiler layout");
        let s = body(v, p.source)?;
        ensure!(
            s.contains("payable ERC1967Proxy(_logic, _data)") && s.contains("_admin = address(new ProxyAdmin(initialOwner));"),
            "base initializer before ProxyAdmin CREATE"
        );
        return Ok(());
    }
    let role_source = if p.label == "kgen" {
        "@openzeppelin/contracts/access/AccessControl.sol"
    } else {
        "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol"
    };
    let r = body(v, role_source)?;
    ensure!(
        r.contains("mapping(address account => bool) hasRole;") && r.contains("bytes32 adminRole;"),
        "plain boolean RoleData"
    );
    let ids: Vec<_> = v["sources"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|s| identifiers(s["content"].as_str().unwrap()))
        .collect();
    ensure!(
        ids.iter().filter(|s| s.as_str() == "_setRoleAdmin").count() == if p.label == "kgen" { 1 } else { 4 },
        "only reviewed admin declaration/calls"
    );
    ensure!(
        !ids.iter().any(|s| s == "AccessControlEnumerable" || s == "AccessControlEnumerableUpgradeable"),
        "no coupled role enumeration"
    );
    if p.label == "kgen" {
        let fields = v["storageLayout"]["storage"].as_array().context("Kgen fields")?;
        let expected = [
            ("_owner", 0),
            ("peers", 1),
            ("preCrime", 2),
            ("enforcedOptions", 3),
            ("msgInspector", 4),
            ("_balances", 5),
            ("_allowances", 6),
            ("_totalSupply", 7),
            ("_name", 8),
            ("_symbol", 9),
            ("_roles", 10),
            ("_paused", 11),
            ("_status", 12),
            ("_pendingOwner", 13),
            ("FEE_VAULT", 14),
            ("_trustedForwardersSet", 15),
            ("isBlackListed", 17),
            ("crossChainPaused", 18),
        ];
        ensure!(fields.len() == expected.len(), "complete Kgen layout");
        for (field, (name, slot)) in fields.iter().zip(expected) {
            ensure!(
                field["label"] == name && field["slot"].as_str() == Some(slot.to_string().as_str()) && field["offset"] == 0,
                "Kgen field identity"
            );
        }
        let types = &v["storageLayout"]["types"];
        let outer = &types[fields[10]["type"].as_str().context("role type")?];
        let record = &types[outer["value"].as_str().context("record type")?];
        ensure!(
            outer["key"] == "t_bytes32"
                && record["numberOfBytes"] == "64"
                && record["members"].as_array().context("record members")?.len() == 2
                && record["members"][0]["slot"] == "0"
                && record["members"][0]["type"] == "t_mapping(t_address,t_bool)"
                && record["members"][1]["slot"] == "1"
                && record["members"][1]["type"] == "t_bytes32",
            "Kgen exact role layout"
        );
        ensure!(
            body(v, p.source)?.contains("return 8;")
                && body(v, "@layerzerolabs/oft-evm/contracts/OFT.sol")?.contains("OFTCore(decimals(), _lzEndpoint, _delegate)"),
            "Kgen local decimals binding"
        );
    } else {
        ensure!(v["storageLayout"] == json!({"storage":[],"types":null}), "namespaced Deep layout");
        ensure!(
            r.contains(ROLE_ROOT)
                && format!("0x{}", hex::encode(namespace("openzeppelin.storage.ERC20"))) == BALANCE_ROOT
                && body(v, "@openzeppelin/contracts-upgradeable/token/ERC20/ERC20Upgradeable.sol")?.contains(BALANCE_ROOT),
            "independent ERC7201 roots"
        );
        let token = body(v, p.source)?;
        for role in ["ADMIN_ROLE", "GUARDIAN_ROLE", "BANLIST_OPERATOR_ROLE"] {
            ensure!(token.contains(&format!("_setRoleAdmin({role}, ADMIN_ROLE);")), "three fixed admin setter calls");
        }
        ensure!(
            token.contains("_disableInitializers();")
                && body(v, "@openzeppelin/contracts-upgradeable/token/ERC20/ERC20Upgradeable.sol")?.contains("return 18;"),
            "Deep constructor and local decimals"
        );
    }
    let core = if p.label == "kgen" {
        "@layerzerolabs/oft-evm/contracts/OFTCore.sol"
    } else {
        "@layerzerolabs/oft-evm-upgradeable/contracts/oft/OFTCoreUpgradeable.sol"
    };
    ensure!(
        body(v, core)?.contains("decimalConversionRate = 10 ** (_localDecimals - sharedDecimals());") && body(v, core)?.contains("return 6;"),
        "source-derived decimal conversion"
    );
    Ok(())
}
// Lexical guard for these exact bundles, not a general Solidity call-graph proof.
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
pub fn primary_url(name: &str) -> Result<String> {
    let (repo, pin, path) = if let Some(s) = name.strip_prefix("@openzeppelin/contracts-upgradeable/") {
        (
            "OpenZeppelin/openzeppelin-contracts-upgradeable",
            "e725abddf1e01cf05ace496e950fc8e243cc7cab",
            format!("contracts/{s}"),
        )
    } else if let Some(s) = name.strip_prefix("@openzeppelin/contracts/") {
        (
            "OpenZeppelin/openzeppelin-contracts",
            "c64a1edb67b6e3f4a15cca8909c9482ad33a02b0",
            format!("contracts/{s}"),
        )
    } else if let Some(s) = name.strip_prefix("@layerzerolabs/lz-evm-protocol-v2/contracts/") {
        (
            "LayerZero-Labs/LayerZero-v2",
            "9c741e7f9790639537b1710a203bcdfd73b0b9ac",
            format!("packages/layerzero-v2/evm/protocol/contracts/{s}"),
        )
    } else if let Some(s) = name.strip_prefix("@layerzerolabs/") {
        ("LayerZero-Labs/devtools", "7af2d6e17c9ea84468dda803c380fc52c5548e68", format!("packages/{s}"))
    } else {
        anyhow::bail!("no independent custom-source pin")
    };
    Ok(format!("https://raw.githubusercontent.com/{repo}/{pin}/{path}"))
}
pub fn source_status(p: &Capture, name: &str) -> &'static str {
    if p.label == "kgen" && name == p.source {
        "nonmatching_public_token_whitespace_not_normalized"
    } else if name == INTERFACE {
        if p.label == "kgen" {
            "exact_vendored_snapshot_upstream_differs"
        } else {
            "nonmatching_upstream_interface"
        }
    } else if name.starts_with("contracts/") {
        "independent_custom_primary_gap"
    } else {
        "exact_upstream"
    }
}
pub fn verify_primary(captures: &[Value], primary: &Value, vendored_raw: &[u8]) -> Result<()> {
    ensure!(
        captures.len() == 3 && sha(vendored_raw) == VENDORED_SHA,
        "complete captures and pinned vendored input"
    );
    let vendored: Value = serde_json::from_slice(vendored_raw)?;
    ensure!(
        primary["vendored_input"] == json!({"url":VENDORED_URL,"sha256":VENDORED_SHA}) && primary["qualified"] == false,
        "vendored provenance and qualification"
    );
    let sources = primary["sources"].as_array().context("primary source evidence")?;
    ensure!(sources.len() == 133, "all source statuses retained");
    let mut seen = BTreeSet::new();
    for s in sources {
        let i = CAPTURES.iter().position(|p| s["capture"] == p.label).context("known capture")?;
        let p = &CAPTURES[i];
        let name = s["path"].as_str().context("source path")?;
        ensure!(seen.insert((i, name)), "no duplicate source proof");
        let captured = body(&captures[i], name)?;
        let status = source_status(p, name);
        ensure!(
            s["status"] == status && s["captured_sha256"] == sha(captured.as_bytes()),
            "source status and captured hash"
        );
        if status == "independent_custom_primary_gap" {
            ensure!(
                s["url"].is_null() && s["content"].is_null() && s["sha256"].is_null(),
                "explicit unresolved custom source"
            );
            continue;
        }
        let content = s["content"].as_str().context("full independent source body")?;
        ensure!(s["sha256"] == sha(content.as_bytes()), "primary body digest");
        match status {
            "exact_upstream" => ensure!(s["url"] == primary_url(name)? && content == captured, "exact upstream URL/bytes"),
            "exact_vendored_snapshot_upstream_differs" => ensure!(
                s["url"] == VENDORED_URL && content == captured && vendored["sources"][name]["content"] == content,
                "exact vendored source, no upstream equivalence"
            ),
            "nonmatching_public_token_whitespace_not_normalized" => ensure!(
                s["url"] == NEAR_URL && s["sha256"] == NEAR_SHA && content != captured,
                "pinned nonmatching token, no normalization"
            ),
            "nonmatching_upstream_interface" => ensure!(
                s["url"] == primary_url(name)? && s["sha256"] == NONMATCHING_INTERFACE_SHA && content != captured,
                "explicit protocol interface mismatch"
            ),
            _ => unreachable!(),
        }
    }
    for (i, v) in captures.iter().enumerate() {
        for name in v["sources"].as_object().context("source set")?.keys() {
            ensure!(seen.contains(&(i, name.as_str())), "complete primary status inventory");
        }
    }
    Ok(())
}
pub fn candidate(baseline: &[u8]) -> Result<Value> {
    ensure!(sha(baseline) == BASELINE, "unchanged historical431 baseline");
    let all: Value = serde_json::from_slice(baseline)?;
    let mut out = Vec::new();
    for (contract, slot, balance, runtime) in [
        (KGEN, root(10), root(5), CAPTURES[0].runtime),
        (DEEP, ROLE_ROOT.into(), BALANCE_ROOT.into(), CAPTURES[2].runtime),
    ] {
        let mut v = all
            .as_array()
            .context("baseline array")?
            .iter()
            .find(|v| v["contract"] == contract)
            .context("OFT profile")?
            .clone();
        ensure!(
            v["balance_slot"] == balance && v["code_hash"] == runtime && v.get("deployment").is_none() && v.get("other_mapping_paths").is_none(),
            "existing runtime/balance/deployment guard"
        );
        if contract == DEEP {
            ensure!(
                v["proxy"] == json!({"implementation":IMPLEMENTATION,"implementation_slot":IMPLEMENTATION_SLOT,"code_hash":CAPTURES[1].runtime}),
                "later implementation pointer guard"
            );
        } else {
            ensure!(v.get("proxy").is_none(), "direct Kgen runtime");
        }
        ensure!(
            v["other_mapping_words"].as_object_mut().context("legacy width")?.remove(&slot) == Some(json!(2)),
            "only original role width2"
        );
        v["other_mapping_paths"] = json!([{"root":slot,"key_types":["bytes32","address"],"offset":0,"words":1}]);
        if contract == DEEP {
            v["other_slots"]
                .as_array_mut()
                .context("scalar slots")?
                .extend(admin_words().into_iter().map(Value::String));
        }
        out.push(v);
    }
    Ok(json!(out))
}
pub fn verify_candidate(baseline: &[u8], value: &Value) -> Result<()> {
    ensure!(candidate(baseline)? == *value, "only the two exact role replacements and three fixed admins");
    Ok(())
}
pub fn review(captures: &[Value]) -> Result<Value> {
    ensure!(captures.len() == 3, "all three captures");
    let mut proofs = Vec::new();
    for (v, p) in captures.iter().zip(&CAPTURES) {
        verify_components(v, p)?;
        let sources:Vec<_>=v["sources"].as_object().unwrap().iter().map(|(name,s)|json!({"path":name,"sha256":sha(s["content"].as_str().unwrap().as_bytes()),"metadata_keccak256":v["metadata"]["sources"][name]["keccak256"],"primary_status":source_status(p,name)})).collect();
        proofs.push(json!({"capture":p.label,"address":p.address,"chain_id":56,"capture_sha256":p.digest,"compiler":p.compiler,"compiler_settings":v["compilation"]["compilerSettings"],"runtime_hash":p.runtime,"runtime_bytes":p.runtime_len,"saved_creation_hash":format!("0x{}",hex::encode(hash(&bytes(&v["creationBytecode"]["onchainBytecode"])?))),"constructor_append_bytes":constructor_arguments(p).len(),"immutable_references":v["runtimeBytecode"]["immutableReferences"],"immutable_values":v["runtimeBytecode"]["transformationValues"],"recorded_deployment":v["deployment"],"original_match_labels":{"match":v["match"],"runtimeMatch":v["runtimeMatch"],"creationMatch":v["creationMatch"]},"source_files":sources}));
    }
    Ok(
        json!({"qualified":false,"captures":proofs,"contracts":[KGEN,DEEP],"kgen_membership_root":root(10),"deep_membership_root":ROLE_ROOT,"deep_fixed_admin_words":admin_words(),"source_gaps":SOURCE_GAPS,"deep_proxy_history_limit":HISTORY_LIMIT,"deep_proxy_original_implementation":ORIGINAL_IMPLEMENTATION,"deep_configured_implementation":IMPLEMENTATION,"deep_proxy_admin_nonce_hypothesis":1,"old_initializer_hex":OLD_INITIALIZER,"old_initializer_interpreted":false,"runtime_reconstructed_from_saved_compiler_bytes":true,"fresh_compilation":false,"initial_state_seeded":false,"scope":"Separate plain-role candidates only. Forwarder enumeration and long-byte metadata remain unsupported. No authorization, producer visibility, deployment or replacement-package qualification."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    const RAW: [&[u8]; 3] = [
        include_bytes!("../../tests/fixtures/oft-role-candidates/kgen.json"),
        include_bytes!("../../tests/fixtures/oft-role-candidates/deep.json"),
        include_bytes!("../../tests/fixtures/oft-role-candidates/proxy.json"),
    ];
    fn captures() -> Vec<Value> {
        RAW.iter().zip(&CAPTURES).map(|(raw, p)| verify_capture(raw, p).unwrap()).collect()
    }
    #[test]
    fn all_three_complete_captures_bind_identity_sources_settings_and_original_labels() {
        for (raw, p) in RAW.iter().zip(&CAPTURES) {
            let good = verify_capture(raw, p).unwrap();
            let mut changed = raw.to_vec();
            changed.push(b' ');
            assert!(verify_capture(&changed, p).is_err());
            for (pointer, value) in [
                ("/address", json!(KGEN)),
                ("/chainId", json!("1")),
                ("/match", json!("match")),
                ("/runtimeMatch", json!("match")),
                ("/creationMatch", json!("match")),
                ("/sources", json!({})),
                ("/sourceIds", json!({})),
                ("/metadata/sources", json!({})),
                ("/stdJsonInput/sources", json!({})),
                ("/stdJsonOutput/sources", json!({})),
                ("/compilation/fullyQualifiedName", json!("Other:Other")),
                ("/metadata/compiler/version", json!("changed")),
                ("/stdJsonInput/settings/optimizer/runs", json!(1)),
                ("/compilation/compilerSettings/evmVersion", json!("shanghai")),
                ("/storageLayout/storage", json!([{}])),
                ("/abi", json!([])),
                ("/deployment/blockNumber", json!("0")),
                ("/deployment/transactionIndex", json!("0")),
                ("/deployment/transactionHash", json!("0x00")),
            ] {
                let mut bad = good.clone();
                // Kgen needs a different address too.
                *bad.pointer_mut(pointer).unwrap() = if pointer == "/address" {
                    json!("0x0000000000000000000000000000000000000001")
                } else {
                    value
                };
                assert!(verify_components(&bad, p).is_err(), "{} {pointer}", p.label);
            }
        }
    }
    #[test]
    fn every_immutable_site_value_and_full_runtime_or_creation_mutation_refuses() {
        for (good, p) in captures().iter().zip(&CAPTURES) {
            for (id, offsets, _) in immutable_specs(p) {
                for (n, _) in offsets.iter().enumerate() {
                    for field in ["start", "length"] {
                        let mut bad = good.clone();
                        bad["runtimeBytecode"]["immutableReferences"][id][n][field] = json!(0);
                        assert!(verify_components(&bad, p).is_err());
                    }
                }
                let mut bad = good.clone();
                bad["runtimeBytecode"]["transformationValues"]["immutables"][id] = json!(root(7));
                assert!(verify_components(&bad, p).is_err());
            }
            for pointer in [
                "/runtimeBytecode/onchainBytecode",
                "/runtimeBytecode/recompiledBytecode",
                "/creationBytecode/onchainBytecode",
                "/creationBytecode/recompiledBytecode",
            ] {
                let mut bad = good.clone();
                let mut code = bytes(bad.pointer(pointer).unwrap()).unwrap();
                code[0] ^= 1;
                *bad.pointer_mut(pointer).unwrap() = json!(format!("0x{}", hex::encode(code)));
                assert!(verify_components(&bad, p).is_err(), "{} {pointer}", p.label);
            }
            for field in ["runtimeBytecode", "creationBytecode"] {
                let mut bad = good.clone();
                bad[field]["cborAuxdata"]["1"]["offset"] = json!(0);
                assert!(verify_components(&bad, p).is_err());
                let mut bad = good.clone();
                bad[field]["linkReferences"] = json!({"injected":[]});
                assert!(verify_components(&bad, p).is_err());
            }
        }
    }
    #[test]
    fn independently_encoded_append_binds_every_byte_and_keeps_old_initializer_opaque() {
        for (good, p) in captures().iter().zip(&CAPTURES) {
            let args = constructor_arguments(p);
            assert_eq!(args.len(), if p.label == "deep" { 32 } else { 288 });
            for offset in 0..args.len() {
                let mut changed = args.clone();
                changed[offset] ^= 1;
                let mut bad = good.clone();
                bad["creationBytecode"]["transformationValues"]["constructorArguments"] = json!(format!("0x{}", hex::encode(changed)));
                // Test the binding directly; no repeated metadata hashing per byte.
                let o = &bad["stdJsonOutput"]["contracts"][p.source][p.name];
                assert!(verify_bytecode(&bad, o, p).is_err(), "{} append byte {offset}", p.label);
            }
        }
        let args = constructor_arguments(&CAPTURES[2]);
        assert_eq!(&args[..32], address(ORIGINAL_IMPLEMENTATION));
        assert_ne!(&args[..32], address(IMPLEMENTATION));
        assert_eq!(&args[128..260], hex::decode(OLD_INITIALIZER).unwrap());
        let report = review(&captures()).unwrap();
        assert_eq!(report["old_initializer_interpreted"], false);
        assert_eq!(report["initial_state_seeded"], false);
        assert_eq!(report["deep_proxy_history_limit"], HISTORY_LIMIT);
    }
    #[test]
    fn proxy_admin_nonce_one_is_only_a_matching_hypothesis() {
        let expected = address("0xcc1c3381fde6bc19596f7f6119fc4a44c5a3d636");
        assert_eq!(proxy_admin_at_nonce_one(), expected);
        for nonce in [0x80, 2, 3] {
            let mut rlp = vec![0xd6, 0x94];
            rlp.extend(hex::decode(&DEEP[2..]).unwrap());
            rlp.push(nonce);
            assert_ne!(&hash(&rlp)[12..], &expected[12..]);
        }
        let proxy = &captures()[2];
        let s = body(proxy, CAPTURES[2].source).unwrap();
        assert!(s.find("ERC1967Proxy(_logic, _data)").unwrap() < s.find("new ProxyAdmin(initialOwner)").unwrap());
        assert!(body(proxy, "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol")
            .unwrap()
            .contains("ERC1967Utils.upgradeToAndCall(implementation, _data);"));
    }
    #[test]
    fn source_role_reachability_and_namespace_derivations_are_profile_specific() {
        assert_eq!(
            admin_words(),
            [
                "0xb16e88c42fd4e48df2dd6a2eabd6bc9aec654ec170056b470819f8892cc6431d",
                "0x18476f5b3d6d00091ddd56161ac5e9ba807d29b59f48f8df98938ee352a7cf24",
                "0xcc685bd430dd10cd13db9f52a68d39fc6e8fa7409ed69c8c1ed8aecaff3c8832",
            ]
        );
        let v = captures();
        assert_eq!(
            sha(body(&v[0], CAPTURES[0].source).unwrap().as_bytes()),
            "146cc456e313a2f21776fa010ed08196423a52c0b3a0d372fe263316217d4995"
        );
        assert_eq!(
            sha(body(&v[1], CAPTURES[1].source).unwrap().as_bytes()),
            "4231e0e6246c5474093a383bf33c0deb29a3989bd0e2dfb0f8b624490ac8c0a0"
        );
        for (i, p) in CAPTURES[..2].iter().enumerate() {
            let mut bad = v[i].clone();
            bad["sources"][p.source]["content"] = json!(format!(
                "{}\nfunction unexpected() {{ _setRoleAdmin(bytes32(0), bytes32(0)); }}",
                body(&v[i], p.source).unwrap()
            ));
            assert!(verify_layout_and_roles(&bad, p).is_err());
        }
        assert_eq!(
            identifiers("// _setRoleAdmin\n /*_setRoleAdmin*/ \"_setRoleAdmin\" _setRoleAdmin"),
            ["_setRoleAdmin"]
        );
    }
    #[test]
    fn primary_inventory_keeps_complete_exact_vendored_nonmatching_and_gap_statuses() {
        let v = captures();
        let good: Value = serde_json::from_slice(include_bytes!("../../tests/fixtures/oft-role-candidates/primary-sources.json")).unwrap();
        let vendored = include_bytes!("../../tests/fixtures/oft-role-candidates/vendored-input.json");
        verify_primary(&v, &good, vendored).unwrap();
        assert_eq!(
            good["sources"].as_array().unwrap().iter().filter(|s| s["status"] == "exact_upstream").count(),
            123
        );
        assert_eq!(
            good["sources"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| s["status"] == "independent_custom_primary_gap")
                .count(),
            7
        );
        let mut raw = vendored.to_vec();
        raw.push(b' ');
        assert!(verify_primary(&v, &good, &raw).is_err());
        for field in ["url", "sha256", "status", "content", "captured_sha256"] {
            let mut bad = good.clone();
            bad["sources"][0][field] = json!("tampered");
            assert!(verify_primary(&v, &bad, vendored).is_err());
        }
        let mut bad = good.clone();
        bad["sources"].as_array_mut().unwrap().pop();
        assert!(verify_primary(&v, &bad, vendored).is_err());
        let mut bad = good.clone();
        bad["sources"][1] = bad["sources"][0].clone();
        assert!(verify_primary(&v, &bad, vendored).is_err());
        for status in [
            "nonmatching_public_token_whitespace_not_normalized",
            "nonmatching_upstream_interface",
            "exact_vendored_snapshot_upstream_differs",
            "independent_custom_primary_gap",
        ] {
            let mut bad = good.clone();
            let s = bad["sources"].as_array_mut().unwrap().iter_mut().find(|s| s["status"] == status).unwrap();
            s["status"] = json!("exact_upstream");
            assert!(verify_primary(&v, &bad, vendored).is_err());
        }
    }
    #[test]
    fn candidate_all_other_fields_remain_exact_and_fixed_admins_do_not_become_generic() {
        let baseline = include_bytes!("../../tests/fixtures/bsc-refined450-layouts.json");
        let good = candidate(baseline).unwrap();
        let committed: Value = serde_json::from_slice(include_bytes!("../../tests/fixtures/oft-role-candidates/layouts.json")).unwrap();
        assert_eq!(good, committed);
        for i in 0..2 {
            for field in [
                "code_hash",
                "balance_slot",
                "other_mapping_slots",
                "other_mapping_words",
                "other_slots",
                "other_mapping_paths",
                "proxy",
                "deployment",
            ] {
                let mut bad = good.clone();
                bad[i][field] = json!("changed");
                assert!(verify_candidate(baseline, &bad).is_err());
            }
        }
        let mut changed = baseline.to_vec();
        changed.push(b' ');
        assert!(candidate(&changed).is_err());
        let mut bad = good.clone();
        bad[1]["other_mapping_paths"]
            .as_array_mut()
            .unwrap()
            .push(json!({"root":ROLE_ROOT,"key_types":["bytes32"],"offset":1,"words":1}));
        assert!(verify_candidate(baseline, &bad).is_err());
    }
}
