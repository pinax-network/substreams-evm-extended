//! Offline additions to an immutable baseline, never a qualified package update.
use crate::bnbtiger_cookie_proof::{self as proof, Target};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};
pub const BASELINE: &str = "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468";
pub const FIXTURE: &str = "erc20/balances/tests/fixtures/bnbtiger-cookie-candidates";
pub const LIMITS: &str = "Two NOT-QUALIFIED noncohort direct-balance candidates with exact selected-source metadata field guards. Historical431 and qualified425 remain unchanged. Saved replay and finite initialized-holder controls do not qualify a new package, deployment, current runtime, transfer/router/signature authorization or global holder coverage.";
pub fn sha(raw: &[u8]) -> String {
    hex::encode(Sha256::digest(raw))
}
pub fn save(path: &Path, value: &Value) -> Result<()> {
    fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))?;
    Ok(())
}
pub fn read(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
pub fn candidates(raw: &[u8]) -> Result<Value> {
    ensure!(sha(raw) == BASELINE, "immutable431 baseline digest");
    let baseline: Value = serde_json::from_slice(raw)?;
    let rows = baseline.as_array().context("profiles")?;
    ensure!(rows.len() == 431, "431 scope");
    let mut selected = Vec::new();
    for t in Target::ALL {
        ensure!(rows.iter().all(|p| p["contract"] != t.address()), "candidate already in baseline");
        selected.push(json!({"contract":t.address(),"code_hash":t.runtime_hash(),"balance_slot":format!("0x{:064x}",t.root()),"metadata_semantics":if t==Target::Bnbtiger{"bnbtiger_solc_0_8_4"}else{"cookie_solc_0_6_12"}}));
    }
    let out = json!(selected);
    erc20_balances::layout::parse(&out.to_string()).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(out)
}
pub fn combined(raw: &[u8], selected: &Value) -> Result<Value> {
    ensure!(candidates(raw)? == *selected, "exact two candidate profiles required");
    let mut all: Value = serde_json::from_slice(raw)?;
    let original = all.clone();
    all.as_array_mut().unwrap().extend(selected.as_array().unwrap().iter().cloned());
    ensure!(all.as_array().unwrap().len() == 433, "433 scope");
    let mut restored = all.clone();
    restored.as_array_mut().unwrap().truncate(431);
    ensure!(restored == original, "exact historical array restoration");
    Ok(all)
}
pub fn artifact_pins() -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    for (name, pin) in [
        ("LICENSE-openzeppelin", "5d77daf99ea6e8033e57b449761ab4e9b486c45ef578c387d2805fe98e560f46"),
        ("LICENSE-uniswap-core", "3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986"),
        ("LICENSE-uniswap-periphery", "3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986"),
        ("solc-list.json", "22e8ba1c7c8d0fc5eb60964083b238d267ea2c1afec9757521fea20c45b206af"),
        ("bnbtiger-capture.json", "9fd9afe36b7c9adf159e344ef6e693b15ae37a0703063e153890566a14503242"),
        (
            "bnbtiger-compiler-input-original.json",
            "14d0b1b79f254000d4d5ebcdd30707e8d6467ac17cf98add2fab27173e0735ee",
        ),
        (
            "bnbtiger-compiler-input.json",
            "32328b00cece1301ef29c56bb42fdb7882f5f0ebe56380f94a443ddc95e90171",
        ),
        (
            "bnbtiger-compiler-output.json",
            "aa63df18318d835da823852d3638ce9b6b327fe8cb41b0e4627fb666347c45e8",
        ),
        (
            "bnbtiger-compiler-version.txt",
            "16925bd068671dea710ac4ddce1298229fca9aa0df94e2c6fc5980bd58456e5b",
        ),
        (
            "bnbtiger-metadata-fresh.json",
            "39712709b3ab4dca16f7d05f111473460841263990e49c27a243fe6a832a3e46",
        ),
        (
            "bnbtiger-metadata-original.json",
            "39712709b3ab4dca16f7d05f111473460841263990e49c27a243fe6a832a3e46",
        ),
        (
            "bnbtiger-primary-sources.json",
            "cea2e13c83027501c91745ad3935b293fecb54c3bc55035670e2cf676f4be01c",
        ),
        (
            "bnbtiger-source-licenses.json",
            "43a2e312f3a9005305eb30de44a43857c47deff59b2a366d1a29c3c995981b98",
        ),
        (
            "bnbtiger-writer-review.json",
            "65473b99cd1da18a429cf3c9cf8ad113ee88da68309919c8bca94ac7bdb0b8b8",
        ),
        ("cookie-capture.json", "ec5f5628b41b1eced18fc284246559422db795d71f5bb3c0dbffe9eec06d58b1"),
        (
            "cookie-compiler-input-original.json",
            "cc735a62fc6dc5521ade82a01c7ff275113a4cb7e639d44b29bab03be5e35f82",
        ),
        ("cookie-compiler-input.json", "15d537e90363e8f902e27a6ca73138d4dcfad00f897256f9453a0e5c5bd30e9f"),
        (
            "cookie-compiler-output.json",
            "c46a969fa4cbc87004c7cb106acaa5440751001b252f029001ea873ef55b6397",
        ),
        (
            "cookie-compiler-version.txt",
            "4c4be572097efc783e35c16382b862402ff6c374dd7a4767039bf002c9b47754",
        ),
        ("cookie-metadata-fresh.json", "93606dcbeaf588e4f576a796eab142586abccc13edcad52145791c7613989637"),
        (
            "cookie-metadata-original.json",
            "93606dcbeaf588e4f576a796eab142586abccc13edcad52145791c7613989637",
        ),
        (
            "cookie-primary-sources.json",
            "61ef9dfaa7fa5e10ed4e18690b1e3fe2460201db9d3cb12cd7ffe18e8ee20cca",
        ),
        (
            "cookie-source-licenses.json",
            "6256031aa42a29bf81f307f54dc6e157f9016e08beff73bc966cfe9d927e7bbf",
        ),
        ("cookie-writer-review.json", "c49e79d60ee9407f7cebb6bcbb9403fc77837573f1e47740885608efe5d77db5"),
    ] {
        out.push((format!("{}/{name}", proof::FIXTURE), pin));
    }
    for (suffix, pin) in [
        ("-compiler", "6f7ab61359902685cfb1465cb18feff12a24c2ec3d27eb654fdafeb33702cef4"),
        ("", "c7645326d19b08fd615df49a6fd66a4c1afe2f8ac44965263975b90e9274f50c"),
        ("-source-inputs", "1eeffd537ace32dcc780eb23c6fc805b6c4edaa28bcdd4935fcd8b5b5da6c978"),
        ("-cases", "98a67fc54e020def9049c9d2b02d2b8bc09d6c69b8bd84ef394a04e562b9606c"),
        ("-transcripts", "3735614a3931f0fd35cc8d04bdc20d8827650fb1cd14bbbc6bc315a795b82fc3"),
    ] {
        out.push((format!("erc20/balances/docs/evidence/bnbtiger-cookie-getter-proof-20260929{suffix}.json"), pin));
    }
    out
}
pub fn review(root: &Path) -> Result<Value> {
    let mut artifacts = Vec::new();
    for (path, pin) in artifact_pins() {
        let raw = fs::read(root.join(&path))?;
        ensure!(sha(&raw) == pin, "frozen Phase A artifact changed: {path}");
        artifacts.push(json!({"path":path,"sha256":pin,"bytes":raw.len()}));
    }
    let mut targets = Vec::new();
    for t in Target::ALL {
        let dir = root.join(proof::FIXTURE);
        let c = proof::verify_capture(&fs::read(dir.join(format!("{}-capture.json", t.label())))?, t)?;
        let compiled = fs::read(dir.join(format!("{}-compiler-output.json", t.label())))?;
        proof::verify_compiled(&c, &compiled, t)?;
        let primary = read(&dir.join(format!("{}-primary-sources.json", t.label())))?;
        proof::verify_primary(&c, &primary, t)?;
        let writer = read(&dir.join(format!("{}-writer-review.json", t.label())))?;
        ensure!(proof::writers::review(&c, t)? == writer, "writer anchors/layout changed");
        targets.push(json!({"contract":t.address(),"runtime_hash":t.runtime_hash(),"source_capture_sha256":t.capture_sha(),"writer_review":writer,"types":c["storageLayout"]["types"],"primary_gap":proof::PRIMARY_GAP}));
    }
    Ok(
        json!({"status":"passed","qualified":false,"mode":"offline_source_bound_candidate_preparation","limits":LIMITS,"baseline_sha256":BASELINE,"historical_profiles":431,"appended_profiles":2,"combined_profiles":433,"phase_a_artifacts":artifacts,"targets":targets,"source_authentication":"Frozen historical proof input inventory remains historical; new candidate source is independently inventoried. No historical artifact is rewritten."}),
    )
}

/// Diagnostic records only; callers must first successfully project the block.
/// Equal packed stores cannot identify which unchanged source field was written.
pub fn metadata_observations(block: &substreams_ethereum::pb::eth::v2::Block, review: &Value) -> Result<Vec<Value>> {
    use erc20_balances::{
        hash,
        persist::{self, Ctx, Sink},
    };
    use substreams_ethereum::pb::eth::v2::{BalanceChange, CodeChange, NonceChange, StorageChange};
    #[derive(Default)]
    struct Records(Vec<StorageChange>);
    impl Sink for Records {
        fn storage(&mut self, c: &StorageChange, _: Ctx) {
            self.0.push(c.clone())
        }
        fn storage_noop(&mut self, c: &StorageChange, _: Ctx) {
            self.0.push(c.clone())
        }
        fn balance(&mut self, _: &BalanceChange, _: Ctx) {}
        fn nonce(&mut self, _: &NonceChange, _: Ctx) {}
        fn code(&mut self, _: &CodeChange, _: Ctx) {}
    }
    fn word(raw: &[u8]) -> Result<[u8; 32]> {
        ensure!(raw.len() <= 32, "counter word width");
        let mut out = [0; 32];
        out[32 - raw.len()..].copy_from_slice(raw);
        Ok(out)
    }
    fn n(v: u64) -> [u8; 32] {
        let mut out = [0; 32];
        out[24..].copy_from_slice(&v.to_be_bytes());
        out
    }
    fn offset(mut v: [u8; 32], off: u8) -> [u8; 32] {
        for _ in 0..off {
            for b in v.iter_mut().rev() {
                let (next, borrow) = b.overflowing_sub(1);
                *b = next;
                if !borrow {
                    break;
                }
            }
        }
        v
    }
    let mut images = BTreeMap::new();
    for call in block.system_calls.iter().chain(block.transaction_traces.iter().flat_map(|t| &t.calls)) {
        if !Target::ALL.iter().any(|t| {
            call.address == hex::decode(&t.address()[2..]).unwrap() || call.storage_changes.iter().any(|w| w.address == hex::decode(&t.address()[2..]).unwrap())
        }) {
            continue;
        }
        for (k, p) in &call.keccak_preimages {
            let key = hex::decode(k.trim_start_matches("0x"))?;
            let raw = hex::decode(p.trim_start_matches("0x"))?;
            ensure!(key == hash(&raw), "counter preimage hash");
            images.insert(word(&key)?, raw);
        }
    }
    let matches = |key: [u8; 32], root: u64, widths: &[usize], off: u8| {
        let mut key = offset(key, off);
        for width in widths.iter().rev() {
            let Some(raw) = images.get(&key) else { return false };
            if raw.len() != 64 || raw[..32 - width].iter().any(|b| *b != 0) {
                return false;
            }
            key = raw[32..].try_into().unwrap();
        }
        key == n(root)
    };
    let mut records = Records::default();
    persist::collect_block(block, &mut records).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let mut out = Vec::new();
    for c in records.0 {
        let contract = format!("0x{}", hex::encode(&c.address));
        let Some(t) = Target::ALL.into_iter().find(|t| t.address() == contract) else {
            continue;
        };
        let key = word(&c.key)?;
        let old = word(&c.old_value)?;
        let new = word(&c.new_value)?;
        let target = review["targets"]
            .as_array()
            .context("targets")?
            .iter()
            .find(|v| v["contract"] == contract)
            .context("target review")?;
        let mut labels = Vec::new();
        for f in target["writer_review"]["fields"].as_array().context("fields")? {
            let d = &f["declaration"];
            if d["slot"].as_str().context("slot")?.parse::<u64>()? != small_slot(key) {
                continue;
            }
            let ty = &target["types"][d["type"].as_str().context("field type")?];
            if ty["encoding"] != "inplace" {
                continue;
            }
            let size = ty["numberOfBytes"].as_str().context("field bytes")?.parse::<usize>()?;
            let off = d["offset"].as_u64().context("field offset")? as usize;
            ensure!(size + off <= 32, "diagnostic scalar field");
            if old == new || old[32 - off - size..32 - off] != new[32 - off - size..32 - off] {
                labels.push(d["label"].as_str().context("label")?.to_owned());
            }
        }
        let paths: Vec<(u64, Vec<usize>, u8, &str)> = if t == Target::Bnbtiger {
            vec![
                (8, vec![20, 20], 0, "_allowances"),
                (9, vec![20], 0, "isExcludedFromFee"),
                (10, vec![20], 0, "isWalletLimitExempt"),
                (11, vec![20], 0, "isTxLimitExempt"),
                (12, vec![20], 0, "isMarketPair"),
            ]
        } else {
            vec![
                (2, vec![20, 20], 0, "_allowances"),
                (7, vec![20], 0, "_excludedFromAntiWhale"),
                (12, vec![20], 0, "_includeToBlackList"),
                (14, vec![20], 0, "_delegates"),
                (15, vec![20, 4], 0, "checkpoints.fromBlock"),
                (15, vec![20, 4], 1, "checkpoints.votes"),
                (16, vec![20], 0, "numCheckpoints"),
                (17, vec![20], 0, "nonces"),
            ]
        };
        for (root, widths, off, label) in paths {
            if matches(key, root, &widths, off) {
                labels.push(label.into());
            }
        }
        if labels.is_empty() {
            continue;
        }
        out.push(json!({"contract":contract,"ordinal":c.ordinal,"key":format!("0x{}",hex::encode(key)),"old":format!("0x{}",hex::encode(old)),"new":format!("0x{}",hex::encode(new)),"source_fields":labels,"equal_record":old==new,"attribution":if old==new{"declared fields in an equal-value word; writer not determined"}else{"changed field bits after successful guarded projection; authorization not executed"}}));
    }
    Ok(out)
}
fn small_slot(key: [u8; 32]) -> u64 {
    if key[..24].iter().any(|b| *b != 0) {
        u64::MAX
    } else {
        u64::from_be_bytes(key[24..].try_into().unwrap())
    }
}
pub fn inventory() -> Result<Value> {
    fn walk(path: &Path, out: &mut BTreeMap<String, Value>) -> Result<()> {
        if path.is_dir() {
            for e in fs::read_dir(path)? {
                walk(&e?.path(), out)?;
            }
        } else if path.extension().is_some_and(|e| e == "rs" || e == "toml" || e == "lock" || e == "proto") {
            let raw = fs::read(path)?;
            out.insert(path.to_string_lossy().into(), json!({"sha256":sha(&raw),"bytes":raw.len()}));
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    for path in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "erc20/balances/Cargo.toml",
        "erc20/balances/src",
        "erc20/balances/tests",
        "erc20/balances/tools/Cargo.toml",
        "erc20/balances/tools/src",
        "erc20/balances/tools/tests",
        "common/persist",
        "common/retention",
        "proto",
    ] {
        walk(Path::new(path), &mut files)?;
    }
    for path in [
        "docs/follow-up.md",
        "docs/handoff.md",
        "docs/research/README.md",
        "erc20/balances/docs/role-shape-audit.md",
    ] {
        let raw = fs::read(path)?;
        files.insert(path.into(), json!({"sha256":sha(&raw),"bytes":raw.len()}));
    }
    Ok(json!({"files":files}))
}
pub fn snapshot(out: &Path) -> Result<Value> {
    fs::create_dir(out.join("as-run"))?;
    for (path, body) in [
        ("Cargo.toml", include_str!("../../../../Cargo.toml")),
        ("Cargo.lock", include_str!("../../../../Cargo.lock")),
        ("rust-toolchain.toml", include_str!("../../../../rust-toolchain.toml")),
        ("erc20/balances/Cargo.toml", include_str!("../../Cargo.toml")),
        ("erc20/balances/tools/Cargo.toml", include_str!("../Cargo.toml")),
        ("erc20/balances/tools/src/lib.rs", include_str!("lib.rs")),
        (
            "erc20/balances/tools/src/bnbtiger_cookie_proof/mod.rs",
            include_str!("bnbtiger_cookie_proof/mod.rs"),
        ),
        (
            "erc20/balances/tools/src/bnbtiger_cookie_proof/writers.rs",
            include_str!("bnbtiger_cookie_proof/writers.rs"),
        ),
        ("common/retention/src/lib.rs", include_str!("../../../../common/retention/src/lib.rs")),
        ("proto/src/lib.rs", include_str!("../../../../proto/src/lib.rs")),
        ("common/retention/Cargo.toml", include_str!("../../../../common/retention/Cargo.toml")),
        (
            "common/retention/src/positions.rs",
            include_str!("../../../../common/retention/src/positions.rs"),
        ),
        ("common/retention/src/protocol.rs", include_str!("../../../../common/retention/src/protocol.rs")),
        ("proto/Cargo.toml", include_str!("../../../../proto/Cargo.toml")),
        ("proto/src/pb/mod.rs", include_str!("../../../../proto/src/pb/mod.rs")),
        ("proto/src/pb/evm.balances.v1.rs", include_str!("../../../../proto/src/pb/evm.balances.v1.rs")),
        (
            "proto/src/pb/evm.balance_state.v1.rs",
            include_str!("../../../../proto/src/pb/evm.balance_state.v1.rs"),
        ),
        (
            "proto/src/pb/evm.executions.v1.rs",
            include_str!("../../../../proto/src/pb/evm.executions.v1.rs"),
        ),
        ("proto/src/pb/aave.actions.v1.rs", include_str!("../../../../proto/src/pb/aave.actions.v1.rs")),
        (
            "proto/src/pb/dex.pool_state.v1.rs",
            include_str!("../../../../proto/src/pb/dex.pool_state.v1.rs"),
        ),
        ("proto/src/pb/erc20.events.v1.rs", include_str!("../../../../proto/src/pb/erc20.events.v1.rs")),
        ("proto/src/pb/uniswap.v2.rs", include_str!("../../../../proto/src/pb/uniswap.v2.rs")),
        ("proto/src/pb/uniswap.v3.rs", include_str!("../../../../proto/src/pb/uniswap.v3.rs")),
        ("erc20/balances/src/deployment.rs", include_str!("../../src/deployment.rs")),
        ("erc20/balances/src/checkpoints.rs", include_str!("../../src/checkpoints.rs")),
        ("erc20/balances/src/address_lists.rs", include_str!("../../src/address_lists.rs")),
        ("erc20/balances/src/enumerable_sets.rs", include_str!("../../src/enumerable_sets.rs")),
        ("erc20/balances/src/computed.rs", include_str!("../../src/computed.rs")),
        ("erc20/balances/src/discovery.rs", include_str!("../../src/discovery.rs")),
        ("erc20/balances/tools/src/ptoken_proof/mod.rs", include_str!("ptoken_proof/mod.rs")),
        (
            "erc20/balances/tools/src/ptoken_proof/source_map.rs",
            include_str!("ptoken_proof/source_map.rs"),
        ),
        ("erc20/balances/tools/src/ptoken_proof/vm.rs", include_str!("ptoken_proof/vm.rs")),
        ("erc20/balances/tools/src/ptoken_proof/cases.rs", include_str!("ptoken_proof/cases.rs")),
        (
            "erc20/balances/tools/src/bnbtiger_cookie_proof/cases.rs",
            include_str!("bnbtiger_cookie_proof/cases.rs"),
        ),
        ("erc20/balances/src/mapping_paths.rs", include_str!("../../src/mapping_paths.rs")),
        ("erc20/balances/src/persist.rs", include_str!("../../src/persist.rs")),
        (
            "erc20/balances/tests/fixtures/bsc-refined450-layouts.json",
            include_str!("../../tests/fixtures/bsc-refined450-layouts.json"),
        ),
        ("erc20/balances/src/metadata_words.rs", include_str!("../../src/metadata_words.rs")),
        ("erc20/balances/src/layout.rs", include_str!("../../src/layout.rs")),
        ("erc20/balances/src/lib.rs", include_str!("../../src/lib.rs")),
        (
            "erc20/balances/tools/src/bnbtiger_cookie_candidates.rs",
            include_str!("bnbtiger_cookie_candidates.rs"),
        ),
        (
            "erc20/balances/tools/src/bin/prepare_bnbtiger_cookie_candidates.rs",
            include_str!("bin/prepare_bnbtiger_cookie_candidates.rs"),
        ),
        (
            "erc20/balances/tools/src/bin/replay_bnbtiger_cookie_candidates.rs",
            include_str!("bin/replay_bnbtiger_cookie_candidates.rs"),
        ),
    ] {
        ensure!(fs::read(path)? == body.as_bytes(), "stale compiled candidate tool {path}");
        fs::write(out.join("as-run").join(path.replace('/', "_")), body)?;
    }
    let inv = inventory()?;
    save(&out.join("source-inputs.json"), &inv)?;
    Ok(inv)
}
