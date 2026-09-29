//! Whole wkeyDAO2/TRX compiler and captured programs. Host proof, never admission.
pub mod cases;
pub use crate::ptoken_proof::{bytes, kh, sha, source_map, vm};
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};

pub const FIXTURE: &str = "erc20/balances/tests/fixtures/wkey2-trx-operation-proof";
pub const SOLC_PIN: &str = "16a99b8c26ed33a91796e209ff6797ee7baf2b0d";
pub const MANIFEST_SHA: &str = "22e8ba1c7c8d0fc5eb60964083b238d267ea2c1afec9757521fea20c45b206af";
pub const OZ_PIN: &str = "8e0296096449d9b1cd7c5631e917330635244c37";
pub const LICENSE_SHA: &str = "5d77daf99ea6e8033e57b449761ab4e9b486c45ef578c387d2805fe98e560f46";
pub const PRIMARY_GAP: &str = "wkeyDAO2 has four exact full OpenZeppelin 3.4.2 dependency files and an unestablished custom AGPL source origin. TRX is one original CRLF flattened file: only its complete AccessControl and EnumerableSet declarations match the pinned upstream after CRLF-to-LF normalization. The whole flattened token, other custom components and overall license are not independently attributed. The reason for both captured CBOR digest differences remains unestablished.";
pub const LIMITS: &str = "Bounded host execution of compiled and captured wkeyDAO2/TRX programs in synthetic local state. Exact 53-byte CBOR substitutions preserve all other bytes, including creation trailing constants. No production candidate, deployed-state/initial-set qualification, external-call or signature emulator, schema, dependency or VM change. Unsupported opcodes/context are HarnessFailure, distinct from source REVERT/INVALID. No chain calls.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Wkeydao2,
    Trx,
}
impl Target {
    pub const ALL: [Self; 2] = [Self::Wkeydao2, Self::Trx];
    pub fn label(self) -> &'static str {
        match self {
            Self::Wkeydao2 => "wkeydao2",
            Self::Trx => "trx",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Wkeydao2 => "wkeyDAO2",
            Self::Trx => "TRX",
        }
    }
    pub fn source(self) -> &'static str {
        match self {
            Self::Wkeydao2 => "contracts/2.0/wkeyDAO2.sol",
            Self::Trx => "TRX.sol",
        }
    }
    pub fn address(self) -> &'static str {
        match self {
            Self::Wkeydao2 => "0xe0a281deff5c9d8d67af09d39340e134ac81b82e",
            Self::Trx => "0xce7de646e7208a4ef112cb6ed5038fa6cc6b12e3",
        }
    }
    pub fn account(self) -> U256 {
        address(self.address())
    }
    pub fn role_root(self) -> U256 {
        match self {
            Self::Wkeydao2 => 8.into(),
            Self::Trx => 6.into(),
        }
    }
    pub fn runtime_len(self) -> usize {
        match self {
            Self::Wkeydao2 => 8495,
            Self::Trx => 7088,
        }
    }
    pub fn creation_len(self) -> usize {
        match self {
            Self::Wkeydao2 => 9849,
            Self::Trx => 8893,
        }
    }
    pub fn argument_len(self) -> usize {
        match self {
            Self::Wkeydao2 => 96,
            Self::Trx => 192,
        }
    }
    pub fn cbor_offset(self, creation: bool) -> usize {
        if creation {
            match self {
                Self::Wkeydao2 => 9764,
                Self::Trx => 8761,
            }
        } else {
            self.runtime_len() - 53
        }
    }
    pub fn capture_sha(self) -> &'static str {
        match self {
            Self::Wkeydao2 => "1afd2a842f97acb1eb03dca09e802d7dcec1e06eef97533dee2c2b2ebf180df0",
            Self::Trx => "e0260fa176be3c3e945de5d82af3c07ab9fec026705dc868b42881d474560ffc",
        }
    }
    pub fn runtime_hash(self) -> &'static str {
        match self {
            Self::Wkeydao2 => "0x9054d0efc5311cd08c2d5c1204bc9f600f43e301ea5fe6446b4d581c44d6e6a3",
            Self::Trx => "0x84f4834aef7376b01cc68f967cd357e4bfbe13ba2b775757ed1f8e28b67be82c",
        }
    }
    pub fn compiled_sha(self) -> &'static str {
        match self {
            Self::Wkeydao2 => "58f88b6f0a6765147a48d19553a6b69172782eb3355845b741a2f4340df51f3c",
            Self::Trx => "6d9938caf954382e95f098db0d4d85c6d0733b455079d1d264271e367e2e7bd8",
        }
    }
    pub fn version(self) -> &'static str {
        match self {
            Self::Wkeydao2 => "0.7.5",
            Self::Trx => "0.6.6",
        }
    }
    pub fn solc_version(self) -> &'static str {
        match self {
            Self::Wkeydao2 => "0.7.5+commit.eb77ed08",
            Self::Trx => "0.6.6+commit.6c089d02",
        }
    }
    pub fn solc_path(self) -> String {
        format!("solc-macosx-amd64-v{}", self.solc_version())
    }
    pub fn solc_sha(self) -> &'static str {
        match self {
            Self::Wkeydao2 => "1c100ce86a3167fd4c194290aafec0d3d94fe86c7a1aa0837c1346cc93d8b6ce",
            Self::Trx => "45c7f956197ce08b69f793ea610cf1ee65e12b6a518d6160cc28c8eeff41517c",
        }
    }
    pub fn solc_keccak(self) -> &'static str {
        match self {
            Self::Wkeydao2 => "0xff52724ea7d3e0913219c765b40f311f72fe1e5c95389020a165a1959e57e24f",
            Self::Trx => "0xe7810e0e9d8bdb79087d4a8ae84ea88d33422a37906fad7114531675790fd0f8",
        }
    }
    pub fn captured(self) -> &'static [u8] {
        match self {
            Self::Wkeydao2 => include_bytes!("../../../tests/fixtures/wkey2-trx-operation-proof/wkeydao2-capture.json"),
            Self::Trx => include_bytes!("../../../tests/fixtures/wkey2-trx-operation-proof/trx-capture.json"),
        }
    }
    pub fn settings(self) -> Value {
        match self {
            Self::Wkeydao2 => {
                json!({"viaIR":false,"metadata":{"bytecodeHash":"ipfs","useLiteralContent":false},"optimizer":{"runs":200,"enabled":true},"evmVersion":"istanbul","remappings":["@ensdomains/=node_modules/@ensdomains/","@openzeppelin/=node_modules/@openzeppelin/","hardhat/=node_modules/hardhat/"]})
            }
            Self::Trx => json!({"libraries":{},"optimizer":{"runs":200,"enabled":true}}),
        }
    }
    pub fn cbor(self, captured: bool) -> String {
        let digest = match (self, captured) {
            (Self::Wkeydao2, false) => "49612744558ae66a570abe76a38301db5fa1768b4b77dd4cab6d9d484e199307",
            (Self::Wkeydao2, true) => "e45ef6c0177a324ac6c9f3625bfff38021a357370a1c6d68be0e97e47d34119e",
            (Self::Trx, false) => "b34311cc03e8db20ef7f48f64aa38b801390ef87549f794d9f731c48b424e297",
            (Self::Trx, true) => "998943ee624b50801f7940b0c241ecc938347d36617745f8aa0427a614055cb6",
        };
        let version = if self == Self::Wkeydao2 { "000705" } else { "000606" };
        format!("0xa2646970667358221220{digest}64736f6c6343{version}0033")
    }
}
pub fn address(s: &str) -> U256 {
    U256::from_str_radix(s.trim_start_matches("0x"), 16).unwrap()
}
pub fn runtime(v: &Value) -> Result<Vec<u8>> {
    bytes(&v["runtimeBytecode"]["onchainBytecode"])
}
pub fn arguments(t: Target) -> Vec<u8> {
    let words = |v: &[U256]| v.iter().flat_map(|v| vm::word(*v)).collect::<Vec<_>>();
    match t {
        Target::Wkeydao2 => words(&[
            address("0xea52fe6730078b5a55c26971ec3351eba873aa91"),
            address("0x14734534efc59d3dcdbecccfe79c74fda0e124a8"),
            100000.into(),
        ]),
        Target::Trx => {
            let mut out = words(&[64.into(), 128.into()]);
            for s in ["TRON", "TRX"] {
                out.extend(vm::word(s.len().into()));
                let mut b = [0; 32];
                b[..s.len()].copy_from_slice(s.as_bytes());
                out.extend(b);
            }
            out
        }
    }
}
pub fn patched(code: &[u8], t: Target, creation: bool) -> Result<Vec<u8>> {
    let len = if creation { t.creation_len() } else { t.runtime_len() };
    let off = t.cbor_offset(creation);
    ensure!(
        code.len() == len && code.get(off..off + 53) == Some(bytes(&json!(t.cbor(false)))?.as_slice()),
        "exact compiler CBOR span/length"
    );
    let mut out = code.to_vec();
    out[off..off + 53].copy_from_slice(&bytes(&json!(t.cbor(true)))?);
    Ok(out)
}
pub fn verify_capture(raw: &[u8], t: Target) -> Result<Value> {
    ensure!(sha(raw) == t.capture_sha(), "complete raw capture pin");
    let v = serde_json::from_slice(raw)?;
    verify_components(&v, t)?;
    Ok(v)
}
pub fn verify_components(v: &Value, t: Target) -> Result<()> {
    ensure!(
        sha(t.captured()) == t.capture_sha() && *v == serde_json::from_slice::<Value>(t.captured())?,
        "complete original capture content"
    );
    ensure!(
        v["chainId"] == "56" && v["address"].as_str().is_some_and(|a| a.eq_ignore_ascii_case(t.address())),
        "network/address"
    );
    for f in ["match", "runtimeMatch", "creationMatch"] {
        ensure!(v[f] == "match", "preserve original non-exact label");
    }
    ensure!(
        v["compilation"]["compilerVersion"] == t.solc_version() && v["metadata"]["compiler"]["version"] == t.solc_version(),
        "compiler version"
    );
    ensure!(
        v["compilation"]["compilerSettings"] == t.settings() && v["stdJsonInput"]["settings"] == t.settings(),
        "complete original settings"
    );
    ensure!(
        v["compilation"]["fullyQualifiedName"] == format!("{}:{}", t.source(), t.name())
            && v["sources"] == v["stdJsonInput"]["sources"]
            && v["sourceIds"] == v["stdJsonOutput"]["sources"],
        "target/sources/source IDs"
    );
    let count = if t == Target::Wkeydao2 { 5 } else { 1 };
    let sources = v["sources"].as_object().context("sources")?;
    ensure!(
        sources.len() == count && sources.keys().eq(v["metadata"]["sources"].as_object().context("metadata sources")?.keys()),
        "complete source sets"
    );
    for (path, s) in sources {
        ensure!(
            v["metadata"]["sources"][path]["keccak256"] == kh(s["content"].as_str().context("body")?.as_bytes()),
            "source Keccak {path}"
        );
    }
    let selected = &v["stdJsonOutput"]["contracts"][t.source()][t.name()];
    ensure!(
        serde_json::from_str::<Value>(selected["metadata"].as_str().context("metadata")?)? == v["metadata"],
        "full saved metadata"
    );
    for f in ["abi", "storageLayout", "devdoc", "userdoc"] {
        ensure!(selected[f] == v[f], "selected output {f}");
    }
    for (field, kind, creation) in [("runtimeBytecode", "deployedBytecode", false), ("creationBytecode", "bytecode", true)] {
        let b = &v[field];
        let code = bytes(&b["recompiledBytecode"])?;
        let off = t.cbor_offset(creation);
        ensure!(
            b["linkReferences"] == json!({}) && selected["evm"][kind]["linkReferences"] == json!({}),
            "empty link references"
        );
        ensure!(
            bytes(&selected["evm"][kind]["object"])? == code && selected["evm"][kind]["sourceMap"] == b["sourceMap"],
            "saved full compiler code/map"
        );
        ensure!(b["cborAuxdata"] == json!({"1":{"value":t.cbor(false),"offset":off}}), "exact CBOR description");
        let replacement = json!({"id":"1","type":"replace","offset":off,"reason":"cborAuxdata"});
        let mut transformed = patched(&code, t, creation)?;
        if creation {
            ensure!(
                b["transformations"] == json!([replacement,{"type":"insert","offset":t.creation_len(),"reason":"constructorArguments"}]),
                "only exact creation transforms"
            );
            ensure!(
                b["transformationValues"] == json!({"cborAuxdata":{"1":t.cbor(true)},"constructorArguments":format!("0x{}",hex::encode(arguments(t)))}),
                "independent complete constructor ABI and CBOR values"
            );
            transformed.extend(arguments(t));
        } else {
            ensure!(
                b["transformations"] == json!([replacement])
                    && b["transformationValues"] == json!({"cborAuxdata":{"1":t.cbor(true)}})
                    && b["immutableReferences"] == json!({}),
                "runtime single CBOR transform/no immutables"
            );
            ensure!(kh(&transformed) == t.runtime_hash(), "captured runtime Keccak");
        }
        ensure!(
            transformed == bytes(&b["onchainBytecode"])?,
            "entire saved code reconstruction including trailing constants"
        );
    }
    verify_layout(&v["storageLayout"], t)
}
pub fn verify_layout(layout: &Value, t: Target) -> Result<()> {
    let mut names = vec!["_balances", "_allowances", "_totalSupply", "_name", "_symbol", "_decimals"];
    names.extend(if t == Target::Wkeydao2 {
        vec![
            "_nonces",
            "DOMAIN_SEPARATOR",
            "_roles",
            "mainPair",
            "feeReceiver",
            "buyFeeReceiver",
            "feeRatio",
            "buyFeeRatio",
        ]
    } else {
        vec!["_roles", "_revertMsg", "inited", "domainSeperator", "nonces"]
    });
    let fields = layout["storage"].as_array().context("layout")?;
    ensure!(fields.len() == names.len(), "complete declaration count");
    for (i, (f, name)) in fields.iter().zip(names).enumerate() {
        let slot = i.to_string();
        ensure!(
            f["label"] == name && f["slot"].as_str() == Some(slot.as_str()) && f["offset"] == 0,
            "exact {name} slot"
        );
    }
    let types = &layout["types"];
    let rt = fields[t.role_root().as_usize()]["type"].as_str().context("role type")?;
    ensure!(types[rt]["key"] == "t_bytes32", "outer key");
    let role = &types[types[rt]["value"].as_str().context("role value")?];
    ensure!(
        role["numberOfBytes"] == "96"
            && role["members"].as_array().is_some_and(|m| m.len() == 2)
            && role["members"][0]["label"] == "members"
            && role["members"][0]["slot"] == "0"
            && role["members"][1]["label"] == "adminRole"
            && role["members"][1]["slot"] == "2",
        "combined three-word role layout"
    );
    Ok(())
}
pub fn input(c: &Value) -> Result<Value> {
    let mut v = c["stdJsonInput"].clone();
    ensure!(v["settings"].get("outputSelection").is_none(), "no original outputSelection");
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
    ensure!(out["sources"] == c["sourceIds"], "exact fresh source IDs");
    let a = &out["contracts"][t.source()][t.name()];
    let b = &c["stdJsonOutput"]["contracts"][t.source()][t.name()];
    for f in ["abi", "metadata", "devdoc", "userdoc", "storageLayout"] {
        ensure!(a[f] == b[f], "fresh exact {f}");
    }
    verify_layout(&a["storageLayout"], t)?;
    for (field, kind) in [("runtimeBytecode", "deployedBytecode"), ("creationBytecode", "bytecode")] {
        ensure!(
            bytes(&a["evm"][kind]["object"])? == bytes(&c[field]["recompiledBytecode"])?
                && a["evm"][kind]["sourceMap"] == c[field]["sourceMap"]
                && a["evm"][kind]["linkReferences"] == json!({}),
            "full freshly compiled code/map/links"
        );
        mapped_sources(c, &out, t, kind)?;
    }
    ensure!(
        a["evm"]["deployedBytecode"]["immutableReferences"] == b["evm"]["deployedBytecode"]["immutableReferences"],
        "fresh immutable schema unchanged"
    );
    Ok(out)
}
pub fn verify_compiled(c: &Value, raw: &[u8], t: Target) -> Result<Value> {
    ensure!(sha(raw) == t.compiled_sha(), "full raw compiler output pin");
    check_compiled(c, raw, t)
}
pub fn mapped_sources(c: &Value, out: &Value, t: Target, kind: &str) -> Result<BTreeMap<i64, (String, String)>> {
    let mut map = BTreeMap::new();
    for (path, id) in out["sources"].as_object().context("source IDs")? {
        ensure!(
            map.insert(
                id["id"].as_i64().context("source ID")?,
                (path.clone(), c["sources"][path]["content"].as_str().context("raw source")?.into())
            )
            .is_none(),
            "unique source ID"
        );
    }
    let generated = &out["contracts"][t.source()][t.name()]["evm"][kind]["generatedSources"];
    ensure!(
        if t == Target::Trx { generated.is_null() } else { generated == &json!([]) },
        "exact older compiler generated-source schema"
    );
    Ok(map)
}
/// The exact 0.6.6 output omits generated source text for this two-instruction
/// helper. Keep the original -1 spans visible; never invent Solidity attribution.
#[allow(clippy::too_many_arguments)]
pub fn annotate_effect(
    pc: usize,
    field: &str,
    t: Target,
    creation: bool,
    code: &[u8],
    map: &BTreeMap<usize, source_map::Span>,
    sources: &BTreeMap<i64, (String, String)>,
) -> Result<Value> {
    let mut description = source_map::describe(pc, map, sources)?;
    if description["path"].is_string() && description["source_sha256"].is_string() {
        description["attribution"] = json!("exact_solidity_source");
        return Ok(description);
    }
    let (expected_pc, expected_opcode, start, length) = match (creation, field) {
        (true, "reads") => (1372, 0x54, 27, 10),
        (true, "writes") => (1379, 0x55, 45, 23),
        (false, "reads") => (6169, 0x54, 27, 10),
        (false, "writes") => (6176, 0x55, 45, 23),
        _ => anyhow::bail!("unmapped effect has no exact compiler-generated allowance"),
    };
    ensure!(
        t == Target::Trx && pc == expected_pc && code.get(pc) == Some(&expected_opcode),
        "exact TRX compiler-generated effect PC/opcode"
    );
    ensure!(
        description
            == json!({"pc":pc,"file_id":-1,"source":"compiler-generated span without source ID","start":start,"length":length,"jump":"-","modifier_depth":0}),
        "exact original compiler-generated source-map span"
    );
    description["attribution"] = json!("pinned_compiler_generated_instruction_without_source_text");
    description["compiler_output_sha256"] = json!(t.compiled_sha());
    description["solidity_source_attributed"] = json!(false);
    Ok(description)
}
pub fn primary_url(path: &str, t: Target) -> Result<Option<String>> {
    if path == t.source() {
        return Ok(None);
    }
    ensure!(t == Target::Wkeydao2, "TRX remains one full flattened file");
    let tail = path.strip_prefix("node_modules/@openzeppelin/contracts/").context("dependency prefix")?;
    ensure!(
        ["access/AccessControl.sol", "utils/EnumerableSet.sol", "utils/Address.sol", "utils/Context.sol"].contains(&tail),
        "selected dependency"
    );
    Ok(Some(format!(
        "https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{OZ_PIN}/contracts/{tail}"
    )))
}
pub const DECLARATIONS: [(&str, &str); 2] = [
    ("abstract contract AccessControl", "access/AccessControl.sol"),
    ("library EnumerableSet", "utils/EnumerableSet.sol"),
];
/// Used only on exactly pinned source bodies. Compilation retains original CRLF.
pub fn declaration(s: &str, name: &str) -> Result<String> {
    let s = s.replace("\r\n", "\n");
    let start = s.find(name).context("declaration start")?;
    ensure!(!s[start + name.len()..].contains(name), "unique declaration");
    let open = start + s[start..].find('{').context("declaration body")?;
    let mut depth = 0;
    for (offset, ch) in s[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(s[start..=open + offset].into());
                }
            }
            _ => {}
        }
    }
    anyhow::bail!("unclosed declaration")
}
pub fn verify_primary(c: &Value, p: &Value, t: Target) -> Result<()> {
    ensure!(p["target"] == t.label() && p["source_gap"] == PRIMARY_GAP, "primary scope");
    let records = p["sources"].as_array().context("primary sources")?;
    let originals = c["sources"].as_object().context("sources")?;
    ensure!(records.len() == originals.len(), "complete primary classifications");
    for ((path, s), r) in originals.iter().zip(records) {
        let body = s["content"].as_str().context("body")?;
        let url = primary_url(path, t)?;
        ensure!(
            r["path"] == *path && r["capture_sha256"] == sha(body.as_bytes()) && r["url"] == json!(url),
            "primary source identity"
        );
        if url.is_some() {
            ensure!(
                r["classification"] == "exact_full_file" && r["content"] == body && r["sha256"] == sha(body.as_bytes()),
                "exact dependency"
            );
        } else {
            ensure!(
                r["classification"] == "unestablished_full_file" && r["content"].is_null() && r["sha256"].is_null(),
                "custom/flattened source gap"
            );
        }
    }
    let comparisons = p["declarations"].as_array().context("declaration comparisons")?;
    ensure!(comparisons.len() == if t == Target::Trx { 2 } else { 0 }, "exact declaration comparison count");
    if t == Target::Trx {
        let body = c["sources"][t.source()]["content"].as_str().context("flattened source")?;
        for ((name, path), r) in DECLARATIONS.iter().zip(comparisons) {
            let source = r["primary_content"].as_str().context("complete primary body")?;
            let expected = match *path {
                "access/AccessControl.sol" => "dcebb99daefb7b6c2b5ddb1052f670cf9986240e5549da4ad47b5072857c620e",
                _ => "c8b73a000476872a00f6153d66be31a4a99b7565068f05336129748bfad704ea",
            };
            let a = declaration(body, name)?;
            let b = declaration(source, name)?;
            ensure!(
                r["name"] == *name
                    && r["url"] == format!("https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{OZ_PIN}/contracts/{path}")
                    && sha(source.as_bytes()) == expected
                    && r["primary_sha256"] == expected,
                "complete primary declaration file pin"
            );
            ensure!(
                a == b
                    && r["normalized_declaration"] == a
                    && r["normalized_sha256"] == sha(a.as_bytes())
                    && r["normalization"] == "CRLF to LF only; complete declaration, not flattened file",
                "precise declaration match"
            );
        }
    }
    Ok(())
}
pub fn verify_auxiliary(dir: &Path, c: &Value, t: Target) -> Result<()> {
    let read = |name: &str| -> Result<Value> { Ok(serde_json::from_slice(&fs::read(dir.join(name))?)?) };
    ensure!(
        read("compiler-input-original.json")? == c["stdJsonInput"] && read("compiler-input.json")? == input(c)?,
        "original and augmented inputs"
    );
    let original = c["stdJsonOutput"]["contracts"][t.source()][t.name()]["metadata"].as_str().context("metadata")?;
    ensure!(
        fs::read(dir.join("metadata-original.json"))? == original.as_bytes() && fs::read(dir.join("metadata-fresh.json"))? == original.as_bytes(),
        "both raw metadata strings"
    );
    ensure!(
        fs::read_to_string(dir.join("compiler-version.txt"))?
            == format!(
                "solc, the solidity compiler commandline interface\nVersion: {}.Darwin.appleclang\n",
                t.solc_version()
            ),
        "official compiler full version"
    );
    verify_primary(c, &read("primary-sources.json")?, t)?;
    ensure!(
        read("source-licenses.json")? == source_licenses(c, t)?,
        "complete original source/license notices"
    );
    Ok(())
}
/// Preserve every original notice without assigning a license to unattributed code.
pub fn source_licenses(c: &Value, t: Target) -> Result<Value> {
    let mut records = vec![];
    for (path, source) in c["sources"].as_object().context("sources")? {
        let body = source["content"].as_str().context("source body")?;
        let notices: Vec<_> = body
            .lines()
            .enumerate()
            .filter_map(|(n, line)| {
                let lower = line.to_ascii_lowercase();
                (lower.contains("license") || lower.contains("copyright")).then(|| json!({"line":n+1,"text":line}))
            })
            .collect();
        records.push(json!({"path":path,"sha256":sha(body.as_bytes()),"metadata_license":c["metadata"]["sources"][path]["license"],"notices":notices,"attribution":if primary_url(path,t)?.is_some(){"Exact full dependency at the pinned OpenZeppelin commit; upstream MIT license retained."}else if t==Target::Wkeydao2{"Original custom AGPL notice retained; no independent custom-source origin established."}else{"Original flattened notices retained; whole-file license and custom-source origin unestablished. Two declaration matches do not attribute the whole file."}}));
    }
    Ok(json!({"target":t.label(),"sources":records,"upstream_license_sha256":LICENSE_SHA,"source_gap":PRIMARY_GAP}))
}
pub fn verify_manifest(raw: &[u8], t: Target) -> Result<()> {
    ensure!(sha(raw) == MANIFEST_SHA, "exact compiler manifest");
    let v: Value = serde_json::from_slice(raw)?;
    let list = v["builds"]
        .as_array()
        .context("builds")?
        .iter()
        .filter(|b| b["longVersion"] == t.solc_version())
        .collect::<Vec<_>>();
    ensure!(
        list.len() == 1
            && list[0]["path"] == t.solc_path()
            && list[0]["sha256"] == format!("0x{}", t.solc_sha())
            && list[0]["keccak256"] == t.solc_keccak()
            && v["releases"][t.version()] == t.solc_path(),
        "exact official release binding"
    );
    Ok(())
}
pub fn verify_source_directory(dir: &Path) -> Result<Vec<(Target, Value, Value)>> {
    ensure!(sha(&fs::read(dir.join("LICENSE-openzeppelin"))?) == LICENSE_SHA, "original MIT license");
    let mut result = vec![];
    for t in Target::ALL {
        verify_manifest(&fs::read(dir.join("solc-list.json"))?, t)?;
        let d = dir.join(t.label());
        let c = verify_capture(&fs::read(d.join("capture.json"))?, t)?;
        verify_auxiliary(&d, &c, t)?;
        let out = verify_compiled(&c, &fs::read(d.join("compiler-output.json"))?, t)?;
        result.push((t, c, out));
    }
    Ok(result)
}
/// Executed path only. A byte outside the changed span may be copied normally.
pub fn verify_trace(execution: &Value, t: Target, creation: bool) -> Result<()> {
    let start = t.cbor_offset(creation);
    let end = start + 53;
    for step in execution["trace"].as_array().context("trace")? {
        let pc = step["pc"].as_u64().context("PC")? as usize;
        let op = step["opcode"].as_u64().context("opcode")?;
        ensure!(op <= 255, "opcode byte");
        let width = 1 + if (0x60..=0x7f).contains(&op) { (op - 0x5f) as usize } else { 0 };
        let stop = pc.checked_add(width).context("instruction end")?;
        ensure!(stop <= start || pc >= end, "executed opcode/immediate consumes changed CBOR");
        if op == 0x39 {
            let stack = step["stack_top"].as_array().context("CODECOPY stack")?;
            ensure!(stack.len() >= 3, "copy operands");
            let from = bytes(&stack[1])?;
            let length = bytes(&stack[2])?;
            ensure!(from.len() == 32 && length.len() == 32, "exact copy words");
            let from = U256::from_big_endian(&from);
            let length = U256::from_big_endian(&length);
            if !length.is_zero() {
                let (stop, overflow) = from.overflowing_add(length);
                ensure!(!overflow && (stop <= start.into() || from >= end.into()), "CODECOPY consumes changed CBOR");
            }
        }
    }
    Ok(())
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
        ("wkey2_trx_proof/mod.rs", include_str!("mod.rs")),
        ("wkey2_trx_proof/cases.rs", include_str!("cases.rs")),
        ("bin/execute_wkey2_trx_proof.rs", include_str!("../bin/execute_wkey2_trx_proof.rs")),
        ("bin/build_wkey2_trx_proof.rs", include_str!("../bin/build_wkey2_trx_proof.rs")),
        ("ptoken_proof/vm.rs", include_str!("../ptoken_proof/vm.rs")),
        ("ptoken_proof/source_map.rs", include_str!("../ptoken_proof/source_map.rs")),
        ("ptoken_proof/cases.rs", include_str!("../ptoken_proof/cases.rs")),
        ("ptoken_proof/vm_tests.rs", include_str!("../ptoken_proof/vm_tests.rs")),
        (
            "../tests/wkey2_trx_operation_binding.rs",
            include_str!("../../tests/wkey2_trx_operation_binding.rs"),
        ),
        (
            "../tests/wkey2_trx_operation_cases.rs",
            include_str!("../../tests/wkey2_trx_operation_cases.rs"),
        ),
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
