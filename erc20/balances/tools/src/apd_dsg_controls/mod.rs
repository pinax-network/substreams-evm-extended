//! Saved APD/DSG bytecode getters and metadata controls. Host evidence only.
pub mod cases;
pub mod projector;
pub use crate::ptoken_proof::{bytes, kh, sha, source_map, vm};
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
pub const FIXTURE: &str = "erc20/balances/tests/fixtures/apd-dsg-getter-controls";
pub const LIMITS: &str = "Exact saved APD/DSG runtime execution in synthetic local storage plus actual native projector controls; NOT-QUALIFIED. No compiler rerun, VM extension, new production layout/rule, deployment/constructor/permit/tax-transfer proof, RPC or stream call. Raw metadata perturbations are storage-domain tests, not claims of setter reachability. PR93 retained-ledger controls remain separate. No universal holder or replacement-package qualification.";
pub const SOURCE_GAP: &str = "Complete flattened Sourcify captures and saved compiler outputs are pinned. Neither token has an independently established exact public maintainer source revision here; flattened dependency origins are not independently pinned. Saved outputs omit AST and generated source bodies: unresolved map IDs are explicit, never invented source attribution.";
pub const DSG_CASES_SHA: &str = "2fdf38dd23910cdb3b8e096d60ad6c7b004ed127adf65e137757087873b8c592";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Apd,
    Dsg,
}
impl Target {
    pub fn label(self) -> &'static str {
        match self {
            Self::Apd => "apd",
            Self::Dsg => "dsg",
        }
    }
    pub fn address(self) -> &'static str {
        match self {
            Self::Apd => "0x001208f7f53f78db2b32e1c68198d3e8f320aa23",
            Self::Dsg => "0x3a090ac70c4f453838c34490e3b1cf925c03fc71",
        }
    }
    pub fn account(self) -> U256 {
        U256::from_str_radix(&self.address()[2..], 16).unwrap()
    }
    pub fn capture_sha(self) -> &'static str {
        match self {
            Self::Apd => "4b751efc3542d9392a25f633b634e185a1fd73909c8809cbd16b1d6baa249a40",
            Self::Dsg => "cd276e84539bc0158313ebafc5c54a3558d9306b1675fe8e3308026a4e71f336",
        }
    }
    pub fn source_sha(self) -> &'static str {
        match self {
            Self::Apd => "3a8564a5eb1cdefc6b72b2e82bfb639842b8b4653fa2dc0be2fba90056281074",
            Self::Dsg => "8eb6fe9ff8ab4d163c95db324a27e5a2ab8604a96cc49e67db7101bc8ce71dcc",
        }
    }
    pub fn runtime_hash(self) -> &'static str {
        match self {
            Self::Apd => "0x7d0c87901df63e0ee81cd5aaac3cf2b0d1a29ca31ada80792620e09598765232",
            Self::Dsg => "0x60cdb82077b195e34bf239223f78f452a414001e509cac48230a865895d86884",
        }
    }
    pub fn runtime_file_sha(self) -> &'static str {
        match self {
            Self::Apd => "1efedfc224ab09c1066d2d265334e42f3f7e4e0ac36d1aae7e10c372e8f28e27",
            Self::Dsg => "b440351c90ad73d6c255718adc29ea356fcae6b6c16efa44b6306938d4637225",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Apd => "APWithPremium",
            Self::Dsg => "ERC20TokenX",
        }
    }
    pub fn compiler(self) -> &'static str {
        match self {
            Self::Apd => "0.8.24+commit.e11b9ed9",
            Self::Dsg => "0.7.5+commit.eb77ed08",
        }
    }
    pub fn captured(self) -> &'static [u8] {
        match self {
            Self::Apd => include_bytes!("../../../tests/fixtures/apd-dsg-getter-controls/apd.json"),
            Self::Dsg => include_bytes!("../../../tests/fixtures/apd-dsg-getter-controls/dsg.json"),
        }
    }
    pub fn nonce_root(self) -> U256 {
        if self == Self::Apd {
            5.into()
        } else {
            7.into()
        }
    }
    pub fn role_root(self) -> U256 {
        if self == Self::Apd {
            6.into()
        } else {
            8.into()
        }
    }
}
pub fn verify_capture(t: Target, raw: &[u8]) -> Result<Value> {
    ensure!(sha(raw) == t.capture_sha(), "complete original capture digest");
    let c: Value = serde_json::from_slice(raw)?;
    verify_components(t, &c)?;
    Ok(c)
}
pub fn verify_components(t: Target, c: &Value) -> Result<()> {
    ensure!(
        sha(t.captured()) == t.capture_sha() && *c == serde_json::from_slice::<Value>(t.captured())?,
        "exact complete original record"
    );
    ensure!(
        c["chainId"] == "56" && bytes(&c["address"])? == hex::decode(&t.address()[2..])?,
        "chain/address"
    );
    for field in ["match", "runtimeMatch", "creationMatch"] {
        ensure!(c[field] == "match", "original match label");
    }
    let settings = json!({"libraries":{},"optimizer":{"enabled":t==Target::Dsg,"runs":200}});
    ensure!(
        c["compilation"]["compilerVersion"] == t.compiler() && c["metadata"]["compiler"]["version"] == t.compiler(),
        "compiler identity"
    );
    ensure!(
        c["compilation"]["compilerSettings"] == settings && c["stdJsonInput"]["settings"] == settings,
        "exact compiler settings"
    );
    let path = format!("{}.sol", t.name());
    ensure!(c["compilation"]["fullyQualifiedName"] == format!("{path}:{}", t.name()), "target");
    ensure!(
        c["sources"].as_object().context("sources")?.len() == 1 && c["sources"] == c["stdJsonInput"]["sources"],
        "complete single source"
    );
    let body = c["sources"][&path]["content"].as_str().context("source body")?;
    ensure!(
        sha(body.as_bytes()) == t.source_sha() && c["metadata"]["sources"][&path]["keccak256"] == kh(body.as_bytes()),
        "source digests"
    );
    let compiled = &c["stdJsonOutput"]["contracts"][&path][t.name()];
    for k in ["abi", "storageLayout", "devdoc", "userdoc"] {
        ensure!(c[k] == compiled[k], "saved compiler {k}");
    }
    ensure!(
        serde_json::from_str::<Value>(compiled["metadata"].as_str().context("raw metadata")?)? == c["metadata"],
        "raw parsed metadata"
    );
    let mut declarations = vec!["_balances", "_allowances", "_totalSupply", "_name", "_symbol"];
    if t == Target::Dsg {
        declarations.extend(["_decimals", "totalBurnt"]);
    }
    declarations.extend(["_nonces", "_roles", "mainPair"]);
    if t == Target::Apd {
        declarations.extend(["buyTaxReceiver", "sellTaxReceiver", "swapBuyTaxRatio", "swapSellTaxRatio"]);
    } else {
        declarations.extend(["feeReceiver", "sellFeeRatio", "buyFeeRatio"]);
    }
    let fields = c["storageLayout"]["storage"].as_array().context("layout")?;
    ensure!(fields.len() == declarations.len(), "complete declarations");
    for (i, (f, n)) in fields.iter().zip(declarations).enumerate() {
        let slot = i.to_string();
        ensure!(f["label"] == n && f["slot"] == slot && f["offset"] == 0, "layout {n}");
    }
    for (field, kind) in [("runtimeBytecode", "deployedBytecode"), ("creationBytecode", "bytecode")] {
        let b = &c[field];
        let o = &compiled["evm"][kind];
        ensure!(
            bytes(&o["object"])? == bytes(&b["recompiledBytecode"])?
                && o["sourceMap"] == b["sourceMap"]
                && b["linkReferences"] == json!({})
                && o["linkReferences"] == json!({}),
            "complete saved code/maps/links"
        );
        let mut code = bytes(&b["recompiledBytecode"])?;
        let offset = match (t, kind) {
            (Target::Apd, "deployedBytecode") => 14677,
            (Target::Apd, _) => 17455,
            (Target::Dsg, "deployedBytecode") => 8028,
            (Target::Dsg, _) => 9120,
        };
        ensure!(b["cborAuxdata"]["1"]["offset"] == offset, "exact CBOR offset");
        let before = bytes(&b["cborAuxdata"]["1"]["value"])?;
        let after = bytes(&b["transformationValues"]["cborAuxdata"]["1"])?;
        ensure!(
            before.len() == 53 && after.len() == 53 && code.get(offset..offset + 53) == Some(before.as_slice()),
            "exact CBOR source region"
        );
        let mut expected = vec![];
        if t == Target::Apd && kind == "deployedBytecode" {
            let imm = [
                ("831", 6425),
                ("833", 6386),
                ("835", 6502),
                ("837", 6535),
                ("839", 6469),
                ("1071", 2357),
                ("1826", 3941),
            ];
            ensure!(
                b["immutableReferences"].as_object().context("immutables")?.len() == 7
                    && b["transformationValues"]["immutables"].as_object().context("values")?.len() == 7,
                "exact immutable sets"
            );
            for (id, at) in imm {
                ensure!(b["immutableReferences"][id] == json!([{"start":at,"length":32}]), "immutable site");
                let value = bytes(&b["transformationValues"]["immutables"][id])?;
                ensure!(value.len() == 32 && code.get(at..at + 32) == Some(&[0; 32]), "immutable width/placeholder");
                code[at..at + 32].copy_from_slice(&value);
                expected.push(json!({"id":id,"type":"replace","offset":at,"reason":"immutable"}));
            }
            ensure!(o["immutableReferences"] == b["immutableReferences"], "compiler immutable schema");
        } else if kind == "deployedBytecode" {
            ensure!(
                b["immutableReferences"] == json!({}) && o["immutableReferences"] == json!({}),
                "no DSG immutables"
            );
        }
        code[offset..offset + 53].copy_from_slice(&after);
        expected.push(json!({"id":"1","type":"replace","offset":offset,"reason":"cborAuxdata"}));
        if kind == "bytecode" {
            let end = if t == Target::Apd { 17508 } else { 9205 };
            ensure!(code.len() == end, "complete creation prefix length");
            let args = bytes(&b["transformationValues"]["constructorArguments"])?;
            ensure!(args.len() == if t == Target::Apd { 64 } else { 288 }, "captured constructor argument width");
            expected.push(json!({"type":"insert","offset":end,"reason":"constructorArguments"}));
            code.extend(args);
        } else {
            ensure!(code.len() == offset + 53, "runtime CBOR suffix");
        }
        ensure!(
            b["transformations"] == json!(expected) && code == bytes(&b["onchainBytecode"])?,
            "only declared exact whole-byte reconstruction"
        );
    }
    ensure!(kh(&runtime(c)?) == t.runtime_hash(), "captured runtime hash");
    Ok(())
}
pub fn runtime(c: &Value) -> Result<Vec<u8>> {
    bytes(&c["runtimeBytecode"]["onchainBytecode"])
}
pub fn bound(t: Target) -> Result<(Value, Vec<u8>)> {
    let c = verify_capture(t, t.captured())?;
    let r = runtime(&c)?;
    for h in [122288005, 122289029] {
        let raw = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../tests/fixtures/apd-dsg-getter-controls/{}-runtime-{h}.hex", t.label())))?;
        ensure!(
            sha(&raw) == t.runtime_file_sha() && hex::decode(std::str::from_utf8(&raw)?.trim().trim_start_matches("0x"))? == r,
            "saved boundary runtime"
        );
    }
    Ok((c, r))
}
pub fn saved_dsg() -> Result<Value> {
    let raw = include_bytes!("../../../docs/evidence/dsg-enumerable/runtime-cases.json");
    ensure!(sha(raw) == DSG_CASES_SHA, "historical DSG operations pin");
    Ok(serde_json::from_slice(raw)?)
}
pub fn annotate(c: &Value, pc: usize, map: &BTreeMap<usize, source_map::Span>) -> Result<Value> {
    let mut sources = BTreeMap::new();
    for (name, id) in c["sourceIds"].as_object().context("saved IDs")? {
        sources.insert(
            id["id"].as_i64().context("ID")?,
            (name.clone(), c["sources"][name]["content"].as_str().context("body")?.into()),
        );
    }
    if let Some(span) = map.get(&pc) {
        if span.file >= 0 && !sources.contains_key(&span.file) {
            return Ok(
                json!({"pc":pc,"file_id":span.file,"start":span.start,"length":span.length,"source":"saved compiler source ID without captured body; unresolved"}),
            );
        }
    }
    source_map::describe(pc, map, &sources)
}
pub fn snapshot(out: &Path) -> Result<String> {
    fn walk(p: &Path, all: &mut BTreeSet<std::path::PathBuf>, every: bool) -> Result<()> {
        if p.is_dir() {
            for e in fs::read_dir(p)? {
                walk(&e?.path(), all, every)?;
            }
        } else if (every && p.file_name().is_none_or(|n| n != "README.md")) || p.extension().is_some_and(|e| e == "rs" || e == "toml" || e == "lock") {
            all.insert(p.into());
        }
        Ok(())
    }
    let mut paths = BTreeSet::new();
    for p in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "erc20/balances/src",
        "erc20/balances/Cargo.toml",
        "erc20/balances/tools",
        "common/persist",
        "common/retention",
        "proto",
    ] {
        walk(Path::new(p), &mut paths, false)?;
    }
    for p in [
        FIXTURE,
        "erc20/balances/tests/fixtures/typed450-offline",
        "erc20/balances/docs/evidence/dsg-enumerable",
    ] {
        walk(Path::new(p), &mut paths, true)?;
    }
    for p in [
        "docs/follow-up.md",
        "docs/handoff.md",
        "docs/research/README.md",
        "erc20/balances/docs/role-shape-audit.md",
    ] {
        paths.insert(p.into());
    }
    fs::create_dir(out.join("as-run"))?;
    for (n, body) in [
        ("apd_dsg_controls/mod.rs", include_str!("mod.rs")),
        ("apd_dsg_controls/cases.rs", include_str!("cases.rs")),
        ("apd_dsg_controls/projector.rs", include_str!("projector.rs")),
        ("bin/execute_apd_dsg_controls.rs", include_str!("../bin/execute_apd_dsg_controls.rs")),
        ("ptoken_proof/vm.rs", include_str!("../ptoken_proof/vm.rs")),
        ("ptoken_proof/cases.rs", include_str!("../ptoken_proof/cases.rs")),
        ("ptoken_proof/source_map.rs", include_str!("../ptoken_proof/source_map.rs")),
        ("../tests/apd_dsg_getter_controls.rs", include_str!("../../tests/apd_dsg_getter_controls.rs")),
    ] {
        ensure!(
            fs::read(Path::new("erc20/balances/tools/src").join(n))? == body.as_bytes(),
            "stale executable {n}"
        );
        fs::write(out.join("as-run").join(n.replace('/', "_")), body)?;
    }
    let mut files = BTreeMap::new();
    for p in paths {
        let b = fs::read(&p)?;
        files.insert(p.to_string_lossy().into_owned(), json!({"sha256":sha(&b),"bytes":b.len()}));
    }
    let raw = serde_json::to_vec_pretty(&json!({"scope":LIMITS,"files":files}))?;
    fs::write(out.join("source-inventory.json"), &raw)?;
    Ok(sha(&raw))
}
