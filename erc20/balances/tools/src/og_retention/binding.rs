//! Closed saved-runtime identity. A hash binding is not source qualification.
use super::{address, word, Address, Slot, Word};
use crate::calculated_retention::{binding::sha, Write};
pub use crate::og_model::fixture::{HELPER, POOL_A, POOL_B, ROUTER, TOKEN};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn token() -> Address {
    address(TOKEN).unwrap()
}
pub fn helper() -> Address {
    address(HELPER).unwrap()
}
pub fn router() -> Address {
    address(ROUTER).unwrap()
}
pub fn pool_a() -> Address {
    address(POOL_A).unwrap()
}
pub fn pool_b() -> Address {
    address(POOL_B).unwrap()
}
pub fn codes() -> Vec<(Address, Word)> {
    [
        (TOKEN, "e904e21712652b72bb09176cbfa0ef41be3cbbf56121e292cb86bacde74b9373"),
        (HELPER, "1c8507bc61f3c46a5d90674b6172d2384b657059d49c3f39621a9cb10c5c06e5"),
        (ROUTER, "69aef35de7236f9ae83edadff736f01ea40edd8919b2e73d87dea5d3255b8c3e"),
        (POOL_A, "60e4bcb14447615ab7c14fda2c2d70ca4191570e8841c75618e627c8f72662f8"),
        (POOL_B, "60e4bcb14447615ab7c14fda2c2d70ca4191570e8841c75618e627c8f72662f8"),
    ]
    .map(|(a, h)| (address(a).unwrap(), hex::decode(h).unwrap().try_into().unwrap()))
    .to_vec()
}
pub fn protected(a: Address) -> bool {
    codes().iter().any(|c| c.0 == a)
}
pub fn pointers() -> Vec<(Slot, Address)> {
    [(token(), 6, helper()), (helper(), 1, token()), (helper(), 2, pool_a())]
        .map(|(contract, key, target)| (Slot { contract, key: word(key) }, target))
        .to_vec()
}
pub fn low_address(w: &Word) -> Address {
    w[12..].try_into().unwrap()
}
pub fn retainable(s: &Slot) -> bool {
    s.contract == token() || pointers().iter().any(|(p, _)| p == s) || ([pool_a(), pool_b()].contains(&s.contract) && s.key == word(8))
}
pub fn invalidating_write(w: &Write) -> bool {
    if let Some((_, target)) = pointers().iter().find(|(p, _)| p == &w.slot) {
        // Address masking binds the called account, not independence of opaque
        // helper upper bits. With unavailable source, every changed helper word
        // remains unreviewed even when both addresses are still bound.
        return low_address(&w.old) != *target || low_address(&w.new) != *target || (w.slot.contract == helper() && w.old != w.new);
    }
    // Only the two reserve words are reviewed changing dependency inputs. Other
    // pool/helper/router state remains opaque and suspends on any excursion.
    w.old != w.new && (w.slot.contract == helper() || w.slot.contract == router() || ([pool_a(), pool_b()].contains(&w.slot.contract) && w.slot.key != word(8)))
}
pub struct Original {
    pub file: &'static str,
    pub sha256: &'static str,
    pub raw: &'static [u8],
}
macro_rules! original {
    ($f:literal,$h:literal) => {
        Original {
            file: $f,
            sha256: $h,
            raw: include_bytes!(concat!("../../../tests/fixtures/og-model/", $f)),
        }
    };
}
pub const ORIGINALS: &[Original] = &[
    original!("historical.json", "b1816517fc9299975e93f43d00bec0fa29e2e5faa05fd8073b388aea4422900e"),
    original!("controls.json", "ac5597f5160409f808a8fb5463a8b58e514689cfeaeb55aa9e20cc4088175d41"),
    original!("preview.json", "b4601b52db02e3a8f7c069f489837e0986be4f6791ad3ec55e43d426936faf6e"),
    original!("recursive.json", "123aeb0380f47b972d36ba5926f24cac1f2d47843faa426b3eead0992821f767"),
    original!("pool-range.json", "24d9901dccab1d35435f30999b75da71dd190d02bdff35ef01e6a1e32db24f02"),
];
pub fn verify_original(file: &str, raw: &[u8]) -> Result<Value> {
    let original = ORIGINALS.iter().find(|v| v.file == file).context("unknown OG original")?;
    ensure!(sha(raw) == original.sha256, "OG original raw digest differs: {file}");
    Ok(serde_json::from_slice(raw)?)
}
pub fn original(file: &str) -> Result<Value> {
    let o = ORIGINALS.iter().find(|o| o.file == file).context("unknown OG original")?;
    verify_original(file, o.raw)
}
pub fn hexword(v: &Value) -> Result<Word> {
    hex::decode(v.as_str().context("word text")?.strip_prefix("0x").context("word prefix")?)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("word width"))
}
macro_rules! artifact {
    ($f:literal,$h:literal) => {
        Original {
            file: $f,
            sha256: $h,
            raw: include_bytes!(concat!("../../tests/fixtures/og-retention/", $f)),
        }
    };
}
/// Complete original captures, not regenerated summaries. Historical Rust sources
/// are inert evidence and are never executed by this offline adapter.
pub const ARTIFACTS: &[Original] = &[
    artifact!("control-capture.rs.txt", "25c789d6212eee4191d043208b42bfc45fce24aec674279136190c4a06fdcb03"),
    artifact!("controls-report.json", "8c7df845ebd4589d77bde0b1ae3e4323c1af2f71d06bcda35f7a8de7eabd5cc5"),
    artifact!("helper-runtime.hex", "2eb5863235f1fc100434a6c3e18c4157f2ed75fc141b663ea1fc3cdf6f65825b"),
    artifact!(
        "helper-source-unavailable.json",
        "f558f47848f80ab7db5638b1a2703a42231c9a3b788347f336f1aceb5a79efe7"
    ),
    artifact!(
        "intermediate-historical.json",
        "7fe911273f232e68c13b81a180640b5c22fbd082ab382fd1f1d5e3f7e9f14b51"
    ),
    artifact!("mismatch-capture.rs.txt", "942a7641f19eecee4bbb2dcbcf4160f7141b3836cd36957f5da26bc20374a14f"),
    artifact!("mismatch-report.json", "26f3aade88cbf56a20958ced00b6d4cd67922f7f420cb57448c273e79dba0afd"),
    artifact!(
        "original-block-manifest.json",
        "56397f18ea7371231b261baefb10122c1439e604d130be2cbc97fca598e38ba6"
    ),
    artifact!("pool-scan-report.json", "1b07212ec751e3cc74a0131ee92dcec374e2aff7de6bc3d6d417f37c52517933"),
    artifact!("pool-snapshots.jsonl", "0da397d9a73b62770089364c0c962b87381355c59e0824bb2972b0afae33c50e"),
    artifact!("pool-ybc-prestate.json", "ba654457882e7aa30f92e550c33330a6367ca468cc843ff58e09ecfb1a60c443"),
    artifact!("pool-ybc-report.json", "629b1cf6ee7b79bb0fd9b1047fd1504b52ffc8ec21cc0027c7254b36e3f6b0e1"),
    artifact!("publisher.rs.txt", "ef200173da93000a6ffd238b765d26451fe4bbde28b355cdbe0b7dc2519518ca"),
    artifact!("router-report.json", "913736d3974c5d6bb5d41c573739c707adb941d6c649d2abc42bfba834690d7a"),
    artifact!("router-runtime.hex", "2f446ab1d35b522b898f270c3d6d444d3695c0f255a2939a686a090c160a4e88"),
    artifact!("source-inspector.rs.txt", "81d1addd02300ba942f08574092f8fd7b955413112381eb983ec420b8a220d0f"),
    artifact!("source-report.json", "4ff8fe4b2aa4d3c87f33737653464f929cd1c6bfb286d1622e3d0ecf09a18d27"),
    artifact!("token-runtime.hex", "2c6a8dec498791052f6c44f1c9e93f6be69c69ae9d9ffb0b3c8f3b8b7f253893"),
    artifact!(
        "token-source-unavailable.json",
        "df6703fc0d4c538161cdd685773d6658c4daa885f0717b330b15168bd221b728"
    ),
];
pub const REPORTS: &[Original] = &[
    Original {
        file: "og450-host-model.json",
        sha256: "c5074ffa75f99d3cc762984a64f56b89894c618b13a7d3639cbbade94c70c9ee",
        raw: include_bytes!("../../../docs/evidence/og450-host-model.json"),
    },
    Original {
        file: "og450-preview-model.json",
        sha256: "477ef4977226212bd3e9a8898ca1189ed4378e07f13e397fd51337f566a26232",
        raw: include_bytes!("../../../docs/evidence/og450-preview-model.json"),
    },
    Original {
        file: "og450-recursive-model.json",
        sha256: "1cc9f1b4b8ea96a4865823f6d9076a1491fbd1857dd6f10ef35d629fe92e3115",
        raw: include_bytes!("../../../docs/evidence/og450-recursive-model.json"),
    },
];
pub fn artifact_bytes(file: &str) -> Result<&'static [u8]> {
    let a = ARTIFACTS.iter().chain(REPORTS).find(|a| a.file == file).context("unknown OG artifact")?;
    verify_artifact(file, a.raw)?;
    Ok(a.raw)
}
pub fn verify_artifact(file: &str, bytes: &[u8]) -> Result<()> {
    let a = ARTIFACTS.iter().chain(REPORTS).find(|a| a.file == file).context("unknown OG artifact")?;
    ensure!(sha(bytes) == a.sha256, "OG artifact raw digest differs: {file}");
    Ok(())
}
pub fn artifact(file: &str) -> Result<Value> {
    Ok(serde_json::from_slice(artifact_bytes(file)?)?)
}
fn decoded_hex(text: &str) -> Result<Vec<u8>> {
    Ok(hex::decode(text.trim().strip_prefix("0x").context("runtime prefix")?)?)
}
pub fn runtime(account: Address) -> Result<Vec<u8>> {
    let file = if account == token() {
        Some("token-runtime.hex")
    } else if account == helper() {
        Some("helper-runtime.hex")
    } else if account == router() {
        Some("router-runtime.hex")
    } else {
        None
    };
    if let Some(file) = file {
        return decoded_hex(std::str::from_utf8(artifact_bytes(file)?)?);
    }
    ensure!([pool_a(), pool_b()].contains(&account), "unknown runtime account");
    // Byte provenance only. No state of the differently addressed YBC pool is imported.
    let v = artifact("pool-ybc-prestate.json")?;
    decoded_hex(
        v["result"]["0x5473f664eaa1c6fea8306133241000b758cb326a"]["code"]
            .as_str()
            .context("cross-capture pool code")?,
    )
}
pub fn verify_literal(code: &[u8], pc: usize, expected: Address) -> Result<()> {
    let mut i = 0;
    while i < pc && i < code.len() {
        let op = code[i];
        i += 1 + if (0x60..=0x7f).contains(&op) { usize::from(op - 0x5f) } else { 0 };
    }
    ensure!(i == pc && code.get(pc) == Some(&0x7f), "literal is not an instruction-boundary PUSH32");
    let mut expected_word = [0u8; 32];
    expected_word[12..].copy_from_slice(&expected);
    ensure!(code.get(pc + 1..pc + 33) == Some(expected_word.as_slice()), "literal address operand differs");
    Ok(())
}
fn hashes() -> Vec<String> {
    ORIGINALS.iter().chain(ARTIFACTS).chain(REPORTS).map(|o| o.sha256.into()).collect()
}
pub fn verify_uncached() -> Result<()> {
    for o in ORIGINALS {
        verify_original(o.file, o.raw)?;
    }
    for a in ARTIFACTS.iter().chain(REPORTS) {
        verify_artifact(a.file, a.raw)?;
    }
    for ((a, h), len) in codes().iter().zip([24422, 12655, 21936, 14981, 14981]) {
        let code = runtime(*a)?;
        ensure!(
            code.len() == len && erc20_balances::hash(&code).as_slice() == h,
            "full OG runtime binding differs"
        );
    }
    ensure!(
        sha(&runtime(pool_a())?) == "ab89a0244c7e5a86aa8e1673b4c5b825db93bc406c7934e923ebb0a7c4548ac7",
        "cross-capture decoded bytes"
    );
    let ybc = artifact("pool-ybc-report.json")?;
    ensure!(
        ybc["prestateTracer"]["sha256"] == ARTIFACTS.iter().find(|a| a.file == "pool-ybc-prestate.json").unwrap().sha256,
        "cross-capture report link"
    );
    let witnessed = ybc["execution_code"]
        .as_array()
        .context("YBC code witnesses")?
        .iter()
        .find(|v| v["address"] == "0x5473f664eaa1c6fea8306133241000b758cb326a")
        .context("YBC pool witness")?;
    ensure!(
        witnessed["runtime_hash"] == format!("0x{}", hex::encode(codes()[3].1)),
        "cross-capture witness hash"
    );
    let historical = original("historical.json")?;
    let mismatch = artifact("mismatch-report.json")?;
    let h = historical.as_array().context("historical cases")?;
    ensure!(h.len() == 5, "historical cohort");
    for (i, c) in h.iter().enumerate() {
        let m = &mismatch["cases"][i];
        ensure!(
            c["runtime_bindings"] == m["execution_code"] && c["hash"] == m["hash"] && c["block"] == m["height"] && c["original_failure"] == m["original"],
            "published mismatch linkage"
        );
        let rows = c["runtime_bindings"].as_array().context("runtime witnesses")?;
        ensure!(rows.len() == 5, "runtime witness closure");
        for (a, hash) in codes() {
            ensure!(
                rows.iter()
                    .filter(|r| r["address"] == format!("0x{}", hex::encode(a)) && r["runtime_hash"] == format!("0x{}", hex::encode(hash)))
                    .count()
                    == 1,
                "runtime witness identity"
            );
        }
        for r in m["contexts"]["storage_reads"].as_array().context("original reads")? {
            ensure!(
                c["raw_state"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v["contract"] == r["storage_address"] && v["key"] == r["key"] && v["value"] == r["value"]),
                "published observed SLOAD not raw-bound"
            );
        }
    }
    let source = artifact("source-report.json")?;
    ensure!(
        source["block"] == h[0]["block"] && source["hash"] == h[0]["hash"] && source["contracts"].as_array().is_some_and(|a| a.len() == 2),
        "source lookup identity"
    );
    for (i, (a, name)) in [(token(), "token"), (helper(), "helper")].iter().enumerate() {
        let c = &source["contracts"][i];
        let e = artifact(&format!("{name}-source-unavailable.json"))?;
        ensure!(
            address(c["address"].as_str().context("source address")?)? == *a
                && c["bound"] == false
                && c["source_status"] == 404
                && c["runtime_hash"] == format!("0x{}", hex::encode(codes()[i].1))
                && c["runtime_bytes"] == runtime(*a)?.len(),
            "saved unavailable-source result"
        );
        ensure!(
            address(e["address"].as_str().context("unavailable address")?)? == *a && e["chainId"] == "56" && e["match"].is_null(),
            "unavailable response identity"
        );
    }
    let router_report = artifact("router-report.json")?;
    ensure!(
        router_report["router"] == ROUTER
            && router_report["block_hash"] == h[0]["hash"]
            && router_report["qualified"] == false
            && router_report["runtime_hash"] == format!("0x{}", hex::encode(codes()[2].1))
            && router_report["router_code_sha256"] == ARTIFACTS.iter().find(|a| a.file == "router-runtime.hex").unwrap().sha256,
        "router evidence link"
    );
    for (pc, a) in [
        (421, ROUTER),
        (700, "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c"),
        (829, "0x55d398326f99059ff775485246999027b3197955"),
    ] {
        verify_literal(&runtime(helper())?, pc, address(a)?)?;
    }
    for (pc, a) in [
        (427, "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c"),
        (4294, "0xca143ce32fe78f1f7019d7d551a6402fc5350c73"),
    ] {
        verify_literal(&runtime(router())?, pc, address(a)?)?;
    }
    let host = artifact("og450-host-model.json")?;
    ensure!(
        host["tracked_fixtures"]["historical"]["sha256"] == ORIGINALS[0].sha256
            && host["tracked_fixtures"]["controls"]["sha256"] == ORIGINALS[1].sha256
            && host["runtime_bindings"] == h[0]["runtime_bindings"]
            && host["promoted_layouts"] == 0,
        "published host evidence link"
    );
    let preview = artifact("og450-preview-model.json")?;
    let recursive = artifact("og450-recursive-model.json")?;
    ensure!(
        preview["new_fixture"]["sha256"] == ORIGINALS[2].sha256
            && recursive["fixtures"]["recursive_sha256"] == ORIGINALS[3].sha256
            && recursive["fixtures"]["pool_range_sha256"] == ORIGINALS[4].sha256
            && recursive["unchanged_prior_fixtures"]["historical_sha256"] == ORIGINALS[0].sha256
            && recursive["unchanged_prior_fixtures"]["initial_controls_sha256"] == ORIGINALS[1].sha256
            && recursive["unchanged_prior_fixtures"]["preview_sha256"] == ORIGINALS[2].sha256,
        "published preview/recursive links"
    );
    let controls = artifact("controls-report.json")?;
    ensure!(
        controls["historical_sha256"] == ARTIFACTS.iter().find(|a| a.file == "intermediate-historical.json").unwrap().sha256,
        "intermediate publisher evidence"
    );
    Ok(())
}
pub fn verify() -> Result<()> {
    static CHECK: std::sync::OnceLock<std::result::Result<(), String>> = std::sync::OnceLock::new();
    CHECK
        .get_or_init(|| verify_uncached().map_err(|e| format!("{e:#}")))
        .clone()
        .map_err(anyhow::Error::msg)
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
        let b = Self {
            schema: 1,
            chain_id: 56,
            model: "OG captured initialized hourly/daily paths".into(),
            model_revision: 1,
            persistence_revision: 1,
            epoch,
            activation_block,
            evidence: hashes(),
        };
        b.validate()?;
        Ok(b)
    }
    pub fn validate(&self) -> Result<()> {
        verify()?;
        ensure!(
            self.schema == 1
                && self.chain_id == 56
                && self.model == "OG captured initialized hourly/daily paths"
                && self.model_revision == 1
                && self.persistence_revision == 1
                && self.epoch > 0
                && self.activation_block >= 122288107
                && self.evidence == hashes(),
            "closed OG binding differs"
        );
        Ok(())
    }
}
