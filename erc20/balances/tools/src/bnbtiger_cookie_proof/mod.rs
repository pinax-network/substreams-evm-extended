//! Exact captured BNBTiger/COOKIE getter programs. Host evidence, never admission.
pub mod cases;
pub mod writers;
pub use crate::ptoken_proof::{bytes, kh, sha, source_map, vm};
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};
pub const FIXTURE: &str = "erc20/balances/tests/fixtures/bnbtiger-cookie-getter-proof";
pub const SOLC_PIN: &str = "16a99b8c26ed33a91796e209ff6797ee7baf2b0d";
pub const MANIFEST_SHA: &str = "22e8ba1c7c8d0fc5eb60964083b238d267ea2c1afec9757521fea20c45b206af";
pub const LIMITS: &str = "Bounded host balanceOf execution in explicit synthetic storage, with exact saved source/compiler/runtime binding. No deployed-state or producer qualification, production candidate, creation admission, transfer/router execution, external calls, VM changes or chain calls. Metadata independence does not authorize metadata writes.";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Bnbtiger,
    Cookie,
}
impl Target {
    pub const ALL: [Self; 2] = [Self::Bnbtiger, Self::Cookie];
    pub fn label(self) -> &'static str {
        match self {
            Self::Bnbtiger => "bnbtiger",
            Self::Cookie => "cookie",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Bnbtiger => "BNBTiger",
            Self::Cookie => "CookieToken",
        }
    }
    pub fn source(self) -> &'static str {
        match self {
            Self::Bnbtiger => "BNBTiger.sol",
            Self::Cookie => "/C/Users/wesle/Desktop/Deploying Tokens/goose-contracts-referral/contracts/CookieToken.sol",
        }
    }
    pub fn address(self) -> &'static str {
        match self {
            Self::Bnbtiger => "0xac68931b666e086e9de380cfdb0fb5704a35dc2d",
            Self::Cookie => "0x3505bee89d3b4e351dbd4849241a6b0716ea407f",
        }
    }
    pub fn account(self) -> U256 {
        U256::from_str_radix(&self.address()[2..], 16).unwrap()
    }
    pub fn root(self) -> U256 {
        if self == Self::Bnbtiger {
            7.into()
        } else {
            1.into()
        }
    }
    pub fn capture_sha(self) -> &'static str {
        match self {
            Self::Bnbtiger => "9fd9afe36b7c9adf159e344ef6e693b15ae37a0703063e153890566a14503242",
            Self::Cookie => "ec5f5628b41b1eced18fc284246559422db795d71f5bb3c0dbffe9eec06d58b1",
        }
    }
    pub fn captured(self) -> &'static [u8] {
        match self {
            Self::Bnbtiger => include_bytes!("../../../tests/fixtures/bnbtiger-cookie-getter-proof/bnbtiger-capture.json"),
            Self::Cookie => include_bytes!("../../../tests/fixtures/bnbtiger-cookie-getter-proof/cookie-capture.json"),
        }
    }
    pub fn runtime_hash(self) -> &'static str {
        match self {
            Self::Bnbtiger => "0x18f0619e94b94d884d917414ff4248c11e327998aef6a3e7089b9977e6ecef1c",
            Self::Cookie => "0x258d1c6c6dcac1bfb46886ed147f9445408e9e5420a2c7a9434cd1b60bcf200f",
        }
    }
    pub fn length(self, creation: bool) -> usize {
        match (self, creation) {
            (Self::Bnbtiger, false) => 10594,
            (Self::Bnbtiger, true) => 12976,
            (Self::Cookie, false) => 23518,
            (Self::Cookie, true) => 25171,
        }
    }
    pub fn version(self) -> &'static str {
        if self == Self::Bnbtiger {
            "0.8.4"
        } else {
            "0.6.12"
        }
    }
    pub fn solc_version(self) -> &'static str {
        if self == Self::Bnbtiger {
            "0.8.4+commit.c7e474f2"
        } else {
            "0.6.12+commit.27d51765"
        }
    }
    pub fn solc_path(self) -> String {
        format!("solc-macosx-amd64-v{}", self.solc_version())
    }
    pub fn solc_sha(self) -> &'static str {
        if self == Self::Bnbtiger {
            "4f6f2e6942a09051bbbc850d4fa9b0d907749612cb5db58cac0c87745435070f"
        } else {
            "05ad8afa83df3b51d36fe9a84ea4467b3ed17585c903946985d6e2cd5e95685a"
        }
    }
    pub fn solc_keccak(self) -> &'static str {
        if self == Self::Bnbtiger {
            "0xb8027ceccd3892550e0389af02716a658123eb4530bc02fc437d167c38501101"
        } else {
            "0xd3ad11d172cf5b8ca7c7c0fb6b751611a85323becd788180af5b30fc8b4b4987"
        }
    }
    pub fn compiled_sha(self) -> &'static str {
        match self {
            Self::Bnbtiger => "aa63df18318d835da823852d3638ce9b6b327fe8cb41b0e4627fb666347c45e8",
            Self::Cookie => "c46a969fa4cbc87004c7cb106acaa5440751001b252f029001ea873ef55b6397",
        }
    }
    pub fn settings(self) -> Value {
        if self == Self::Bnbtiger {
            json!({"libraries":{},"optimizer":{"runs":200,"enabled":true}})
        } else {
            json!({"libraries":{},"optimizer":{"runs":200,"enabled":false},"evmVersion":"istanbul","remappings":[]})
        }
    }
    pub fn cbor(self, captured: bool) -> String {
        let hash = match (self, captured) {
            (Self::Bnbtiger, false) => "040ee07adad104c1a46d84a18b9e029fc8b86fc243c06cb0b6c6eb442c46df15",
            (Self::Bnbtiger, true) => "49b0d8db59c6f3fa3d2aef93d72140b27ab5f1e48d0c8267d62078440fd15cbc",
            (Self::Cookie, _) => "5c7f0a3256c7268f8c43df4068cc5fea29ee409a963259fc25e4c2d2df0a22ad",
        };
        format!(
            "0xa2646970667358221220{hash}64736f6c6343{}0033",
            if self == Self::Bnbtiger { "000804" } else { "00060c" }
        )
    }
}
pub fn selected(v: &Value, t: Target) -> &Value {
    &v["contracts"][t.source()][t.name()]
}
pub fn runtime(c: &Value) -> Result<Vec<u8>> {
    bytes(&c["runtimeBytecode"]["onchainBytecode"])
}
pub fn immutables(t: Target) -> Value {
    if t == Target::Bnbtiger {
        json!({"1369":[{"start":1347,"length":32},{"start":3534,"length":32}]})
    } else {
        json!({})
    }
}
pub fn reconstruct(code: &[u8], t: Target, creation: bool) -> Result<Vec<u8>> {
    ensure!(code.len() == t.length(creation), "complete bytecode length");
    let off = code.len() - 53;
    ensure!(code[off..] == bytes(&json!(t.cbor(false)))?, "exact compiler CBOR bytes");
    let mut out = code.to_vec();
    if t == Target::Bnbtiger {
        if !creation {
            for off in [1347, 3534] {
                ensure!(out[off..off + 32] == [0; 32], "exact zero immutable placeholder");
                out[off..off + 32].copy_from_slice(&vm::word(0xdead.into()));
            }
        }
        out[off..].copy_from_slice(&bytes(&json!(t.cbor(true)))?);
    }
    Ok(out)
}
pub fn verify_capture(raw: &[u8], t: Target) -> Result<Value> {
    ensure!(sha(raw) == t.capture_sha(), "complete raw capture pin before parse");
    let v = serde_json::from_slice(raw)?;
    verify_components(&v, t)?;
    Ok(v)
}
pub fn verify_components(c: &Value, t: Target) -> Result<()> {
    ensure!(
        sha(t.captured()) == t.capture_sha() && *c == serde_json::from_slice::<Value>(t.captured())?,
        "complete original capture identity"
    );
    ensure!(
        c["chainId"] == "56" && c["address"].as_str().is_some_and(|a| a.eq_ignore_ascii_case(t.address())),
        "chain/address"
    );
    for f in ["match", "runtimeMatch", "creationMatch"] {
        ensure!(c[f] == if t == Target::Bnbtiger { "match" } else { "exact_match" }, "original match label");
    }
    ensure!(
        c["compilation"]["fullyQualifiedName"] == format!("{}:{}", t.source(), t.name())
            && c["compilation"]["compilerVersion"] == t.solc_version()
            && c["metadata"]["compiler"]["version"] == t.solc_version(),
        "compiler/target"
    );
    ensure!(
        c["compilation"]["compilerSettings"] == t.settings() && c["stdJsonInput"]["settings"] == t.settings(),
        "unaltered complete compiler settings"
    );
    ensure!(
        c["sources"] == c["stdJsonInput"]["sources"] && c["sourceIds"] == c["stdJsonOutput"]["sources"],
        "all original sources and IDs"
    );
    let sources = c["sources"].as_object().context("sources")?;
    ensure!(
        sources.len() == if t == Target::Bnbtiger { 1 } else { 11 }
            && sources.keys().eq(c["metadata"]["sources"].as_object().context("metadata sources")?.keys()),
        "complete source set"
    );
    for (path, s) in sources {
        ensure!(
            c["metadata"]["sources"][path]["keccak256"] == kh(s["content"].as_str().context("body")?.as_bytes()),
            "literal original source {path}"
        );
    }
    let a = selected(&c["stdJsonOutput"], t);
    ensure!(
        serde_json::from_str::<Value>(a["metadata"].as_str().context("raw metadata")?)? == c["metadata"],
        "full metadata"
    );
    for f in ["abi", "storageLayout", "devdoc", "userdoc"] {
        ensure!(a[f] == c[f], "full selected {f}");
    }
    for (f, k, creation) in [("runtimeBytecode", "deployedBytecode", false), ("creationBytecode", "bytecode", true)] {
        let b = &c[f];
        let raw = bytes(&b["recompiledBytecode"])?;
        let off = t.length(creation) - 53;
        ensure!(
            bytes(&a["evm"][k]["object"])? == raw && a["evm"][k]["sourceMap"] == b["sourceMap"],
            "full saved compiler object/map"
        );
        ensure!(b["linkReferences"] == json!({}) && a["evm"][k]["linkReferences"] == json!({}), "no links");
        ensure!(
            b["cborAuxdata"] == json!({"1":{"offset":off,"value":t.cbor(false)}}),
            "exact complete CBOR identity"
        );
        let (transforms, values) = if t == Target::Bnbtiger {
            let mut ts = vec![];
            let mut vs = json!({"cborAuxdata":{"1":t.cbor(true)}});
            if !creation {
                for offset in [1347, 3534] {
                    ts.push(json!({"id":"1369","type":"replace","offset":offset,"reason":"immutable"}));
                }
                vs["immutables"] = json!({"1369":format!("0x{}",hex::encode(vm::word(0xdead.into())))});
            }
            ts.push(json!({"id":"1","type":"replace","offset":off,"reason":"cborAuxdata"}));
            (json!(ts), vs)
        } else {
            (json!([]), json!({}))
        };
        ensure!(
            b["transformations"] == transforms && b["transformationValues"] == values,
            "only exact declared transformations; no constructor arguments"
        );
        if !creation {
            ensure!(
                b["immutableReferences"] == immutables(t) && a["evm"][k]["immutableReferences"] == immutables(t),
                "exact immutable set"
            );
        }
        let rebuilt = reconstruct(&raw, t, creation)?;
        ensure!(rebuilt == bytes(&b["onchainBytecode"])?, "entire saved runtime/creation equality");
        if !creation {
            ensure!(kh(&rebuilt) == t.runtime_hash(), "captured runtime hash");
        }
    }
    writers::review(c, t)?;
    Ok(())
}
pub fn input(c: &Value) -> Result<Value> {
    let mut v = c["stdJsonInput"].clone();
    ensure!(v["settings"].get("outputSelection").is_none(), "outputSelection absent originally");
    v["settings"]["outputSelection"] =
        json!({"*":{"*":["abi","metadata","devdoc","userdoc","storageLayout","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}});
    Ok(v)
}
pub fn check_compiled(c: &Value, raw: &[u8], t: Target) -> Result<Value> {
    let o: Value = serde_json::from_slice(raw)?;
    ensure!(
        !o["errors"].as_array().is_some_and(|es| es.iter().any(|e| e["severity"] == "error")),
        "compiler errors"
    );
    ensure!(o["sources"] == c["sourceIds"], "exact fresh source IDs");
    let a = selected(&o, t);
    let b = selected(&c["stdJsonOutput"], t);
    for f in ["abi", "metadata", "devdoc", "userdoc", "storageLayout"] {
        ensure!(a[f] == b[f], "fresh complete {f}");
    }
    for (f, k, creation) in [("runtimeBytecode", "deployedBytecode", false), ("creationBytecode", "bytecode", true)] {
        ensure!(
            bytes(&a["evm"][k]["object"])? == bytes(&c[f]["recompiledBytecode"])?
                && a["evm"][k]["sourceMap"] == c[f]["sourceMap"]
                && a["evm"][k]["linkReferences"] == json!({}),
            "fresh whole bytecode/map/links"
        );
        ensure!(
            reconstruct(&bytes(&a["evm"][k]["object"])?, t, creation)? == bytes(&c[f]["onchainBytecode"])?,
            "fresh whole reconstruction"
        );
        mapped_sources(c, &o, t, k)?;
    }
    ensure!(a["evm"]["deployedBytecode"]["immutableReferences"] == immutables(t), "fresh immutable set");
    Ok(o)
}
pub fn verify_compiled(c: &Value, raw: &[u8], t: Target) -> Result<Value> {
    ensure!(sha(raw) == t.compiled_sha(), "complete raw compiler output pin");
    check_compiled(c, raw, t)
}
pub fn mapped_sources(c: &Value, o: &Value, t: Target, kind: &str) -> Result<BTreeMap<i64, (String, String)>> {
    let mut out = BTreeMap::new();
    for (path, v) in o["sources"].as_object().context("sources")? {
        ensure!(
            out.insert(
                v["id"].as_i64().context("ID")?,
                (path.clone(), c["sources"][path]["content"].as_str().context("body")?.into())
            )
            .is_none(),
            "unique source ID"
        );
    }
    if let Some(gs) = selected(o, t)["evm"][kind]["generatedSources"].as_array() {
        for g in gs {
            ensure!(g["language"] == "Yul", "generated Yul");
            ensure!(
                out.insert(
                    g["id"].as_i64().context("generated ID")?,
                    (
                        g["name"].as_str().context("generated name")?.into(),
                        g["contents"].as_str().context("generated body")?.into()
                    )
                )
                .is_none(),
                "unique generated ID"
            );
        }
    }
    Ok(out)
}
pub fn verify_manifest(raw: &[u8], t: Target) -> Result<()> {
    ensure!(sha(raw) == MANIFEST_SHA, "official manifest raw pin");
    let v: Value = serde_json::from_slice(raw)?;
    let bs = v["builds"]
        .as_array()
        .context("builds")?
        .iter()
        .filter(|b| b["longVersion"] == t.solc_version())
        .collect::<Vec<_>>();
    ensure!(
        bs.len() == 1
            && bs[0]["path"] == t.solc_path()
            && bs[0]["sha256"] == format!("0x{}", t.solc_sha())
            && bs[0]["keccak256"] == t.solc_keccak()
            && v["releases"][t.version()] == t.solc_path(),
        "official release binding"
    );
    Ok(())
}
/// Complete source notices are retained without inventing an upstream license.
pub fn source_licenses(c: &Value) -> Result<Value> {
    let mut records = vec![];
    for (path, v) in c["sources"].as_object().context("sources")? {
        let s = v["content"].as_str().context("body")?;
        let notices = s
            .lines()
            .enumerate()
            .filter(|(_, s)| {
                let s = s.to_ascii_lowercase();
                s.contains("license") || s.contains("copyright")
            })
            .map(|(n, s)| json!({"line":n+1,"text":s}))
            .collect::<Vec<_>>();
        records.push(json!({"path":path,"sha256":sha(s.as_bytes()),"metadata_license":c["metadata"]["sources"][path]["license"],"notices":notices}));
    }
    Ok(json!(records))
}
pub fn snapshot_sources(out: &Path) -> Result<String> {
    fn walk(p: &Path, files: &mut BTreeMap<String, Value>) -> Result<()> {
        if p.is_dir() {
            for e in fs::read_dir(p)? {
                walk(&e?.path(), files)?;
            }
        } else if p.extension().is_some_and(|e| e == "rs" || e == "toml" || e == "lock") {
            let raw = fs::read(p)?;
            files.insert(p.to_string_lossy().into(), json!({"bytes":raw.len(),"sha256":sha(&raw)}));
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
        let b = fs::read(p)?;
        files.insert(p.into(), json!({"bytes":b.len(),"sha256":sha(&b)}));
    }
    for e in fs::read_dir(FIXTURE)? {
        let p = e?.path();
        if p.is_file() && p.file_name().is_none_or(|n| n != "README.md") {
            let b = fs::read(&p)?;
            files.insert(p.to_string_lossy().into(), json!({"bytes":b.len(),"sha256":sha(&b)}));
        }
    }
    fs::create_dir(out.join("as-run"))?;
    for (name, body) in [
        ("bnbtiger_cookie_proof/mod.rs", include_str!("mod.rs")),
        ("bnbtiger_cookie_proof/writers.rs", include_str!("writers.rs")),
        ("bnbtiger_cookie_proof/cases.rs", include_str!("cases.rs")),
        ("bin/build_bnbtiger_cookie_proof.rs", include_str!("../bin/build_bnbtiger_cookie_proof.rs")),
        ("bin/execute_bnbtiger_cookie_proof.rs", include_str!("../bin/execute_bnbtiger_cookie_proof.rs")),
        ("ptoken_proof/vm.rs", include_str!("../ptoken_proof/vm.rs")),
        ("ptoken_proof/source_map.rs", include_str!("../ptoken_proof/source_map.rs")),
        ("ptoken_proof/cases.rs", include_str!("../ptoken_proof/cases.rs")),
        (
            "../tests/bnbtiger_cookie_getter_binding.rs",
            include_str!("../../tests/bnbtiger_cookie_getter_binding.rs"),
        ),
        (
            "../tests/bnbtiger_cookie_getter_cases.rs",
            include_str!("../../tests/bnbtiger_cookie_getter_cases.rs"),
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

pub const PRIMARY_GAP: &str = "Eight COOKIE dependency bodies exactly match the immutable OpenZeppelin/Uniswap files listed here; this does not establish a unique historical import commit. The original CookieToken, BEP20 and IBEP20 custom origins, and the complete flattened BNBTiger origin, remain unestablished. No source text, line ending, import path or license is normalized.";
pub const LICENSES: [(&str, &str, &str, &str); 3] = [
    (
        "openzeppelin",
        "OpenZeppelin/openzeppelin-contracts",
        "8e0296096449d9b1cd7c5631e917330635244c37",
        "5d77daf99ea6e8033e57b449761ab4e9b486c45ef578c387d2805fe98e560f46",
    ),
    (
        "uniswap-core",
        "Uniswap/v2-core",
        "4dd59067c76dea4a0e8e4bfdda41877a6b16dedc",
        "3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986",
    ),
    (
        "uniswap-periphery",
        "Uniswap/v2-periphery",
        "ed24991304291297c3b4a52818d02f46a17aa9a2",
        "3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986",
    ),
];
pub fn primary_url(path: &str) -> Option<String> {
    for (prefix, repo, pin) in [
        ("@openzeppelin/", "OpenZeppelin/openzeppelin-contracts", LICENSES[0].2),
        ("@uniswap/v2-core/", "Uniswap/v2-core", LICENSES[1].2),
        ("@uniswap/v2-periphery/", "Uniswap/v2-periphery", LICENSES[2].2),
    ] {
        if let Some(tail) = path.strip_prefix(prefix) {
            return Some(format!("https://raw.githubusercontent.com/{repo}/{pin}/{tail}"));
        }
    }
    None
}
pub fn verify_primary(c: &Value, p: &Value, t: Target) -> Result<()> {
    ensure!(p["target"] == t.label() && p["source_gap"] == PRIMARY_GAP, "primary classification scope");
    let sources = c["sources"].as_object().context("sources")?;
    let records = p["sources"].as_array().context("primary records")?;
    ensure!(records.len() == sources.len(), "complete primary source set");
    let mut exact = 0;
    for ((path, s), r) in sources.iter().zip(records) {
        let body = s["content"].as_str().context("body")?;
        let url = primary_url(path);
        ensure!(
            r["path"] == *path && r["capture_sha256"] == sha(body.as_bytes()) && r["url"] == json!(url),
            "literal source/immutable URL identity"
        );
        if url.is_some() {
            ensure!(
                r["classification"] == "exact_full_file" && r["content"] == body && r["primary_sha256"] == sha(body.as_bytes()),
                "exact whole dependency body"
            );
            exact += 1;
        } else {
            ensure!(
                r["classification"] == "unestablished_full_file" && r["content"].is_null() && r["primary_sha256"].is_null(),
                "explicit custom/flattened gap"
            );
        }
    }
    ensure!(exact == if t == Target::Cookie { 8 } else { 0 }, "exact eight dependency files");
    Ok(())
}
pub fn verify_source_directory(dir: &Path) -> Result<Vec<(Target, Value, Value)>> {
    let manifest = fs::read(dir.join("solc-list.json"))?;
    for (name, _, _, hash) in LICENSES {
        ensure!(sha(&fs::read(dir.join(format!("LICENSE-{name}")))?) == hash, "exact upstream LICENSE");
    }
    let mut out = vec![];
    for t in Target::ALL {
        verify_manifest(&manifest, t)?;
        let d = dir.join(t.label());
        let binary = fs::read(d.join("solc"))?;
        ensure!(
            sha(&binary) == t.solc_sha() && kh(&binary) == t.solc_keccak(),
            "exact saved official compiler binary"
        );
        let c = verify_capture(&fs::read(d.join("capture.json"))?, t)?;
        let o = verify_compiled(&c, &fs::read(d.join("compiler-output.json"))?, t)?;
        let read = |name: &str| -> Result<Value> { Ok(serde_json::from_slice(&fs::read(d.join(name))?)?) };
        ensure!(
            read("compiler-input-original.json")? == c["stdJsonInput"] && read("compiler-input.json")? == input(&c)?,
            "original and augmented input"
        );
        let raw = selected(&c["stdJsonOutput"], t)["metadata"].as_str().context("metadata")?;
        for name in ["metadata-original.json", "metadata-fresh.json"] {
            ensure!(fs::read(d.join(name))? == raw.as_bytes(), "exact raw metadata {name}");
        }
        ensure!(
            fs::read_to_string(d.join("compiler-version.txt"))?
                == format!(
                    "solc, the solidity compiler commandline interface\nVersion: {}.Darwin.appleclang\n",
                    t.solc_version()
                ),
            "exact compiler full version"
        );
        verify_primary(&c, &read("primary-sources.json")?, t)?;
        ensure!(read("source-licenses.json")? == source_licenses(&c)?, "original full source notices");
        ensure!(read("writer-review.json")? == writers::review(&c, t)?, "complete source writer review");
        out.push((t, c, o));
    }
    Ok(out)
}

pub fn source_artifacts(dir: &Path) -> Result<Value> {
    let names: Vec<_> = ["solc-list.json", "LICENSE-openzeppelin", "LICENSE-uniswap-core", "LICENSE-uniswap-periphery"]
        .into_iter()
        .map(str::to_owned)
        .chain(Target::ALL.into_iter().flat_map(|t| {
            [
                "capture.json",
                "compiler-input-original.json",
                "compiler-input.json",
                "compiler-output.json",
                "compiler-version.txt",
                "metadata-original.json",
                "metadata-fresh.json",
                "primary-sources.json",
                "source-licenses.json",
                "writer-review.json",
                "solc",
            ]
            .into_iter()
            .map(move |n| format!("{}/{n}", t.label()))
        }))
        .collect();
    let mut records = vec![];
    for file in names {
        records.push(json!({"file":file,"sha256":sha(&fs::read(dir.join(&file))?)}));
    }
    Ok(json!(records))
}
pub fn source_targets() -> Value {
    json!(Target::ALL.into_iter().map(|t|json!({"target":t.label(),"capture_sha256":t.capture_sha(),"compiler_output_sha256":t.compiled_sha(),"compiler_url":format!("https://raw.githubusercontent.com/ethereum/solc-bin/{SOLC_PIN}/macosx-amd64/{}",t.solc_path()),"runtime_keccak256":t.runtime_hash(),"source_files":if t==Target::Bnbtiger{1}else{11}})).collect::<Vec<_>>())
}
pub fn validate_source_report(report: &Value, artifacts: &Value, saved_inventory: &[u8], current_inventory: &str) -> Result<()> {
    ensure!(
        report["status"] == "passed"
            && report["qualified"] == false
            && report["scope"] == LIMITS
            && report["source_gap"] == PRIMARY_GAP
            && report["chain_calls"] == 0,
        "passed source report with exact scope"
    );
    ensure!(
        sha(saved_inventory) == current_inventory && report["source_inventory_sha256"] == current_inventory,
        "same current/compiler inventory"
    );
    ensure!(
        report["artifacts"] == *artifacts && report["targets"] == source_targets(),
        "complete exact source artifact/target manifest"
    );
    let licenses=json!(LICENSES.into_iter().map(|(name,repo,pin,hash)|json!({"file":format!("LICENSE-{name}"),"url":format!("https://raw.githubusercontent.com/{repo}/{pin}/LICENSE"),"sha256":hash})).collect::<Vec<_>>());
    ensure!(report["licenses"] == licenses, "exact source license manifest");
    Ok(())
}
pub fn verify_source_report(dir: &Path, current_inventory: &str) -> Result<Value> {
    let v: Value = serde_json::from_slice(&fs::read(dir.join("report.json"))?)?;
    validate_source_report(&v, &source_artifacts(dir)?, &fs::read(dir.join("source-inventory.json"))?, current_inventory)?;
    Ok(v)
}
