//! Exact historical evidence, with explicit unavailable helper source. This does
//! not alter the complete-source contract of the older calculated adapters.
use super::{address, word, Address, Slot, Word};
use crate::calculated_retention::binding::sha;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::OnceLock;
pub const TOKEN: &str = "0xebc2d768147f2d058f4266bb57e34ca1b6ef1319";
pub const HELPER: &str = "0xe8058da568eb94194588bc9bae53a5e584fe7471";
pub const POOL: &str = "0x5473f664eaa1c6fea8306133241000b758cb326a";
pub const WBNB: &str = "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c";
pub fn token() -> Address {
    address(TOKEN).unwrap()
}
pub fn helper() -> Address {
    address(HELPER).unwrap()
}
pub fn pool() -> Address {
    address(POOL).unwrap()
}
pub fn helper_word() -> Word {
    let mut w = [0; 32];
    w[12..].copy_from_slice(&helper());
    w
}
pub fn pointer() -> Slot {
    Slot {
        contract: token(),
        key: word(7),
    }
}
pub fn protected(a: Address) -> bool {
    [token(), helper(), pool()].contains(&a)
}
pub fn codes() -> Vec<(Address, Word)> {
    [
        (TOKEN, "0b81e9dd6c0529eb50a2f0437da20126cbaf40f6c79c4973fe1c44ee95ba7128"),
        (HELPER, "b3d6938e8c9477247d16e146b97c511d1c0c232b3b76394f204a31d239b38c1f"),
        (POOL, "60e4bcb14447615ab7c14fda2c2d70ca4191570e8841c75618e627c8f72662f8"),
    ]
    .map(|(a, h)| (address(a).unwrap(), hex::decode(h).unwrap().try_into().unwrap()))
    .to_vec()
}
pub struct Original {
    pub file: &'static str,
    pub sha256: &'static str,
    pub raw: &'static [u8],
}
macro_rules! original {
    ($file:literal,$hash:literal) => {
        Original {
            file: $file,
            sha256: $hash,
            raw: include_bytes!(concat!("../../tests/fixtures/ybc-retention/", $file)),
        }
    };
}
pub const ORIGINALS: [Original; 7] = [
    original!("token-source.json", "5edd2d1462a99b23fc8208f38d42a400bdb36e98caeadca85785a82b46f976df"),
    original!("prestate.json", "ba654457882e7aa30f92e550c33330a6367ca468cc843ff58e09ecfb1a60c443"),
    original!("calls.json", "d506a55bcf5980fb4cb253d551f7cb3d7b9f3679dc3ccc01910039179c6a5c9b"),
    original!("parent-storage.json", "832529828c0618dfe18bd2bba6ef9705b27a01a231c8bd2d0e28c21a3f35d37b"),
    original!("final-storage.json", "8497b145bb57f5721a0ee3c0a8c0011b4d0f28616b96e5890c522bdd6f362fef"),
    original!("historical.json", "6e4f06c35e9bfd8d9f72f98911d7b58b3702b6fb17f6def607613b349f418ab4"),
    original!("overrides.json", "5782fbe68e9ae5e5d01b3dc8c3599aa49a03ac630cbb4ff2edc565909b29a383"),
];
pub fn verify_original(file: &str, raw: &[u8]) -> Result<Value> {
    let o = ORIGINALS.iter().find(|o| o.file == file).context("unknown YBC original")?;
    ensure!(sha(raw) == o.sha256, "YBC original raw digest differs: {file}");
    Ok(serde_json::from_slice(raw)?)
}
pub fn original(file: &str) -> Result<Value> {
    let o = ORIGINALS.iter().find(|o| o.file == file).context("unknown YBC original")?;
    verify_original(file, o.raw)
}
pub fn bytes(v: &Value) -> Result<Vec<u8>> {
    Ok(hex::decode(v.as_str().context("hex text")?.trim_start_matches("0x"))?)
}
pub fn hexword(v: &Value) -> Result<Word> {
    bytes(v)?.try_into().map_err(|_| anyhow::anyhow!("word width"))
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub schema: u32,
    pub chain_id: u64,
    pub model: String,
    pub model_revision: u32,
    pub persistence_revision: u32,
    pub epoch: u32,
    pub activation_block: u64,
    pub evidence: Vec<String>,
}
impl Binding {
    pub fn historical(epoch: u32, activation_block: u64) -> Result<Self> {
        verify()?;
        let b = Self {
            schema: 1,
            chain_id: 56,
            model: "YBC captured reward helper".into(),
            model_revision: 1,
            persistence_revision: 1,
            epoch,
            activation_block,
            evidence: ORIGINALS.iter().map(|o| o.sha256.into()).collect(),
        };
        b.validate()?;
        Ok(b)
    }
    pub fn validate(&self) -> Result<()> {
        verify()?;
        ensure!(
            self.schema == 1
                && self.chain_id == 56
                && self.model == "YBC captured reward helper"
                && self.model_revision == 1
                && self.persistence_revision == 1
                && self.epoch > 0
                && self.activation_block >= 122288005
                && self.evidence == ORIGINALS.iter().map(|o| o.sha256.to_string()).collect::<Vec<_>>(),
            "closed YBC binding differs"
        );
        Ok(())
    }
}
pub fn verify() -> Result<()> {
    static VERIFIED: OnceLock<std::result::Result<(), String>> = OnceLock::new();
    VERIFIED
        .get_or_init(|| verify_inner().map_err(|e| format!("{e:#}")))
        .clone()
        .map_err(anyhow::Error::msg)
}
fn verify_inner() -> Result<()> {
    for o in ORIGINALS {
        verify_original(o.file, o.raw)?;
    }
    let s = original("token-source.json")?;
    let p = original("prestate.json")?;
    let calls = original("calls.json")?;
    ensure!(
        address(s["address"].as_str().context("capture address")?)? == token() && s["chainId"] == "56" && s["runtimeMatch"] == "match",
        "capture identity/match"
    );
    let out = &s["stdJsonOutput"]["contracts"]["YBC.sol"]["YBC"];
    ensure!(
        s["sources"] == s["stdJsonInput"]["sources"] && s["sources"].as_object().context("sources")?.len() == 1 && s["storageLayout"] == out["storageLayout"],
        "source/layout closure"
    );
    let metadata: Value = serde_json::from_str(out["metadata"].as_str().context("metadata")?)?;
    ensure!(
        metadata == s["metadata"]
            && s["compilation"]["compilerVersion"] == "0.8.26+commit.8a97fa7a"
            && s["stdJsonInput"]["settings"]["optimizer"] == json!({"enabled":true,"runs":5})
            && s["stdJsonInput"]["settings"]["evmVersion"] == "shanghai",
        "compiler metadata/settings"
    );
    let source = s["sources"]["YBC.sol"]["content"].as_str().context("complete source")?;
    ensure!(
        metadata["sources"]["YBC.sol"]["keccak256"] == format!("0x{}", hex::encode(erc20_balances::hash(source.as_bytes()))),
        "source hash"
    );
    for (name, slot) in [
        ("_balances", 0),
        ("swapper", 7),
        ("users", 22),
        ("userLasts", 23),
        ("usersCycleStaticRate", 26),
        ("hourlyCycleRate", 28),
        ("hourlyCycleReward", 29),
        ("launchTime", 31),
        ("params", 35),
        ("noBurnCycle", 49),
    ] {
        let fields = s["storageLayout"]["storage"].as_array().context("layout")?;
        let rows: Vec<_> = fields.iter().filter(|f| f["label"] == name).collect();
        let expected_slot = slot.to_string();
        ensure!(
            rows.len() == 1 && rows[0]["slot"] == expected_slot.as_str() && rows[0]["offset"] == 0,
            "layout {name}"
        );
    }
    let r = &s["runtimeBytecode"];
    ensure!(
        r["immutableReferences"] == json!({})
            && r["linkReferences"] == json!({})
            && r["transformations"] == json!([{"id":"1","type":"replace","offset":23573,"reason":"cborAuxdata"}]),
        "runtime patch closure"
    );
    let mut compiled = bytes(&out["evm"]["deployedBytecode"]["object"])?;
    ensure!(
        compiled == bytes(&r["recompiledBytecode"])? && r["sourceMap"] == out["evm"]["deployedBytecode"]["sourceMap"],
        "compiler runtime/map"
    );
    let old = bytes(&r["cborAuxdata"]["1"]["value"])?;
    let new = bytes(&r["transformationValues"]["cborAuxdata"]["1"])?;
    ensure!(
        r["cborAuxdata"]["1"]["offset"] == 23573 && old.len() == 53 && new.len() == 53 && compiled.len() == 23626 && compiled[23573..] == old,
        "exact CBOR suffix"
    );
    compiled[23573..].copy_from_slice(&new);
    ensure!(compiled == bytes(&r["onchainBytecode"])?, "full runtime reconstruction");
    let mut runtimes = Vec::new();
    for (a, h) in codes() {
        let raw = bytes(&p["result"][format!("0x{}", hex::encode(a))]["code"])?;
        ensure!(erc20_balances::hash(&raw) == h, "captured runtime identity");
        runtimes.push(raw);
    }
    ensure!(runtimes[0] == compiled, "token trace runtime differs");
    // PUSH operand identity, never an arbitrary byte substring through metadata.
    let mut pushes = Vec::new();
    let mut pc = 0;
    let code = &runtimes[1];
    while pc < code.len() {
        let op = code[pc];
        pc += 1;
        if (0x60..=0x7f).contains(&op) {
            let n = (op - 0x5f) as usize;
            if pc + n > code.len() {
                break;
            }
            pushes.push(&code[pc..pc + n]);
            pc += n;
        }
    }
    for a in [token(), pool(), address(WBNB)?] {
        ensure!(
            pushes
                .iter()
                .any(|p| p.len() >= 20 && p[..p.len() - 20].iter().all(|b| *b == 0) && p[p.len() - 20..] == a),
            "helper literal address absent"
        );
    }
    let rows = calls.as_array().context("captured calls")?;
    ensure!(
        rows.len() == 968 && rows.iter().all(|r| r["to"] == TOKEN || r["to"] == POOL),
        "captured call scope"
    );
    let h = original("historical.json")?;
    let c = original("overrides.json")?;
    ensure!(
        h["status"] == "inspected"
            && h["snapshot_count"] == 24
            && h["rpc_getter_calls"] == 48
            && c["status"] == "inspected"
            && c["case_count"] == 30
            && c["mismatches"] == json!([]),
        "historical result scope"
    );
    for ((a, hash), v) in codes().iter().zip(h["runtimes"].as_array().context("runtimes")?) {
        ensure!(
            address(v["address"].as_str().context("runtime address")?)? == *a && hexword(&v["hash"])? == *hash,
            "historical code binding"
        );
    }
    ensure!(h["runtimes"] == c["runtimes"], "control code binding");
    Ok(())
}
