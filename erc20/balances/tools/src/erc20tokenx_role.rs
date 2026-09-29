//! Three separate NOT-QUALIFIED legacy enumerable candidates. No boolean or
//! admin permission is introduced; complete source proof is bound separately.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::{fs, path::Path};
use substreams_ethereum::pb::eth::v2 as eth;
pub const CONTRACTS: [&str; 3] = [
    "0xda033999bb6165e64db01bd9be14b40f5653092e",
    "0x08332a515cb2a57884176e887b682e7da2eb114e",
    "0xa71add46ea4fbf0058b36e6baa39530f8e48b103",
];
pub const FIXTURE: &str = "tests/fixtures/erc20tokenx-enumerable-candidate";
pub const PROOF_FIXTURE: &str = "tests/fixtures/erc20tokenx-operation-proof";
pub const BASELINE: &str = "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468";
pub const RUNTIME_HASH: &str = "0xbaaa46c4d0133a30bd1020c46f86c0ea4ac0609d18317a95fc923efb5b22afdf";
pub const ARTIFACTS: &[(&str, &str)] = &[
    (
        "tests/fixtures/erc20tokenx-operation-proof/LICENSE-openzeppelin",
        "5d77daf99ea6e8033e57b449761ab4e9b486c45ef578c387d2805fe98e560f46",
    ),
    (
        "tests/fixtures/erc20tokenx-operation-proof/README.md",
        "7d54fcebba6e73b4f6f82d7bdb819af34451758716ce89106ac2a7dbc0397ea6",
    ),
    (
        "tests/fixtures/erc20tokenx-operation-proof/compiler-input-original.json",
        "97df1fc77a9035e864b7966632a5da7a276a31771a97b7408c563ef1f5b866e1",
    ),
    (
        "tests/fixtures/erc20tokenx-operation-proof/compiler-input.json",
        "5c4260d4a67986a97f684042fa5f41ed86e775d931801250d093c29701d14add",
    ),
    (
        "tests/fixtures/erc20tokenx-operation-proof/compiler-output.json",
        "ddc187357403c4cd7dd9d70cdfc63603a36178fe33e58cf986c4af585318aa61",
    ),
    (
        "tests/fixtures/erc20tokenx-operation-proof/compiler-version.txt",
        "025f70e8f8a13b28b6a5b8e369f0360c18bd2106938fdd908ad2afa7bdb823fd",
    ),
    (
        "tests/fixtures/erc20tokenx-operation-proof/ori-capture.json",
        "dc3274a3ed22c01d0e47a84efd72f419a5123bdb56e6be2aca9df3dd51039073",
    ),
    (
        "tests/fixtures/erc20tokenx-operation-proof/fna-capture.json",
        "24f4fa1fc512e62d50384358f58099318d2975a95025ee812abd821639bfee43",
    ),
    (
        "tests/fixtures/erc20tokenx-operation-proof/phi-source-request.json",
        "22e72c35e6291c2c849d2c55740d8f978495be80ac62c37e8d5098e91f2bbed1",
    ),
    (
        "tests/fixtures/erc20tokenx-operation-proof/phi-runtime.hex",
        "7b959b3716c57ba670cbdd1f3ab90970c1e52309d311a16bdbb868daa6b62600",
    ),
    (
        "tests/fixtures/erc20tokenx-operation-proof/phi-runtime-report.json",
        "0d6726eac4c0923fa6b8c40e44fe4d639f6f79fb10fb5b5942482e4f64ba7f45",
    ),
    (
        "tests/fixtures/erc20tokenx-operation-proof/primary-sources.json",
        "359223338b5c961764fb6ac524a764a07f7f783008684534dfa115382dc82153",
    ),
    (
        "tests/fixtures/erc20tokenx-operation-proof/solc-list.json",
        "22e8ba1c7c8d0fc5eb60964083b238d267ea2c1afec9757521fea20c45b206af",
    ),
    (
        "docs/evidence/erc20tokenx-operation-proof-20260929-cases.json",
        "57794c74e64ac143bb2a651e44c44990c1eae1d162eded4ea211f891d1832a7b",
    ),
    (
        "docs/evidence/erc20tokenx-operation-proof-20260929-compiler.json",
        "55b1632bae101628b69af32f7ca490f88386d26bc920cbb3ed097857ae6c0fe6",
    ),
    (
        "docs/evidence/erc20tokenx-operation-proof-20260929-source-inputs.json",
        "b83de18f701ecf8641a638f68a0821598b261088870de9bdc37dd5fb1f20b0f0",
    ),
    (
        "docs/evidence/erc20tokenx-operation-proof-20260929-transcripts.json",
        "acb9ec23a5b0a5ffbfc6e9ed67237cd3f8252d6baf46fd4848fa803fdc6b1d8d",
    ),
    (
        "docs/evidence/erc20tokenx-operation-proof-20260929.json",
        "340d3d1b0ed1542a8e2328b05560bddb2d61d4e712411e399c9bcaf5628033a1",
    ),
];
pub const CACHE_FILES: [(&str, &str); 5] = [
    (
        "ori-capture.json",
        "out/ranks201-250-source-review/0xda033999bb6165e64db01bd9be14b40f5653092e.json",
    ),
    (
        "fna-capture.json",
        "out/ranks251-300-source-review/0x08332a515cb2a57884176e887b682e7da2eb114e.json",
    ),
    (
        "phi-source-request.json",
        "out/ranks251-300-source-review/0xa71add46ea4fbf0058b36e6baa39530f8e48b103.json",
    ),
    (
        "phi-runtime.hex",
        "out/ranks251-300-zero-paths/a71add46ea4fbf0058b36e6baa39530f8e48b103/runtime.hex",
    ),
    (
        "phi-runtime-report.json",
        "out/ranks251-300-zero-paths/a71add46ea4fbf0058b36e6baa39530f8e48b103/report.json",
    ),
];
pub fn root(n: u64) -> String {
    format!("0x{n:064x}")
}
pub fn sha(raw: &[u8]) -> String {
    hex::encode(Sha256::digest(raw))
}
pub fn candidate(raw: &[u8]) -> Result<Value> {
    ensure!(sha(raw) == BASELINE, "complete unchanged 431-profile baseline");
    let baseline: Value = serde_json::from_slice(raw)?;
    let profiles = baseline.as_array().context("baseline profiles")?;
    ensure!(profiles.len() == 431, "baseline cohort");
    let mut selected = vec![];
    for address in CONTRACTS {
        let matches: Vec<_> = profiles.iter().filter(|p| p["contract"] == address).collect();
        ensure!(matches.len() == 1, "exact selected profile {address}");
        let mut c = matches[0].clone();
        ensure!(
            c["balance_slot"] == root(0)
                && c["code_hash"] == RUNTIME_HASH
                && c["other_mapping_slots"] == json!([root(1), root(6)])
                && c["other_slots"] == json!([2, 3, 4, 5, 7, 9, 10, 11, 12].map(root)),
            "exact balances/allowances/nonces/scalars/runtime"
        );
        for field in [
            "proxy",
            "beacon_proxy",
            "minimal_proxy",
            "deployment",
            "enumerable_address_sets",
            "other_mapping_paths",
        ] {
            ensure!(c.get(field).is_none(), "no added/inferred {field}");
        }
        ensure!(
            c["other_mapping_words"].as_object_mut().context("legacy words")?.remove(&root(8)) == Some(json!(3)),
            "remove only legacy root8 width3"
        );
        c["enumerable_address_sets"] = json!([{"root":root(8),"key_types":["bytes32"],"semantics":"oz_3_4_2"}]);
        selected.push(c);
    }
    let candidate = json!(selected);
    erc20_balances::layout::parse(&candidate.to_string()).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(candidate)
}
pub fn verify_candidate(raw: &[u8], selected: &Value) -> Result<()> {
    ensure!(candidate(raw)? == *selected, "only three exact legacy role replacements");
    Ok(())
}
pub fn verify_cache(root: &Path, cache: &Path) -> Result<()> {
    for (fixture, original) in CACHE_FILES {
        ensure!(
            fs::read(root.join(PROOF_FIXTURE).join(fixture))? == fs::read(cache.join(original))?,
            "original complete {fixture}"
        );
    }
    Ok(())
}
pub fn review(root: &Path) -> Result<Value> {
    use crate::erc20tokenx_proof as proof;
    for (path, digest) in ARTIFACTS {
        ensure!(sha(&fs::read(root.join(path))?) == *digest, "frozen Phase A artifact {path}");
    }
    let read = |name: &str| -> Result<Value> { Ok(serde_json::from_slice(&fs::read(root.join(name))?)?) };
    let fixture = root.join(PROOF_FIXTURE);
    let compiled = fs::read(fixture.join("compiler-output.json"))?;
    let primary = read(&format!("{PROOF_FIXTURE}/primary-sources.json"))?;
    let mut captures = vec![];
    for (i, (label, address, _)) in proof::CAPTURES.iter().enumerate() {
        ensure!(*address == CONTRACTS[i], "exact source-bound addresses");
        let c = proof::verify_capture(&fs::read(fixture.join(format!("{label}-capture.json")))?, i)?;
        proof::verify_compiled(&c, &compiled)?;
        proof::verify_primary(&c, &primary)?;
        proof::verify_auxiliary(&fixture, &c)?;
        captures.push(c);
    }
    ensure!(
        RUNTIME_HASH == proof::RUNTIME_HASH && CONTRACTS[2] == proof::PHI,
        "runtime attribution identity"
    );
    ensure!(proof::runtime(&captures[0])? == proof::runtime(&captures[1])?, "ORI/FNA whole runtime equality");
    proof::verify_phi(&fixture, &proof::runtime(&captures[0])?)?;
    let references: usize = captures[0]["sources"]
        .as_object()
        .unwrap()
        .values()
        .map(|s| s["content"].as_str().unwrap().matches("_setRoleAdmin(").count())
        .sum();
    ensure!(references == 1, "only inherited admin setter declaration; no reachable callsite");
    let report = read("docs/evidence/erc20tokenx-operation-proof-20260929.json")?;
    ensure!(
        report["status"] == "passed"
            && report["calls"] == 520
            && report["returned_calls"] == 431
            && report["reverted_calls"] == 82
            && report["explicit_source_invalid_controls"] == 3
            && report["explicit_unsupported_path_controls"] == 4,
        "complete bounded operation proof"
    );
    let inventory = read("docs/evidence/erc20tokenx-operation-proof-20260929-source-inputs.json")?;
    ensure!(
        inventory["files"].as_object().context("frozen proof inventory")?.len() == 203,
        "complete historical proof inventory"
    );
    Ok(
        json!({"qualified":false,"contracts":CONTRACTS,"chain_id":56,"baseline_sha256":BASELINE,"semantics":"oz_3_4_2","root":root_word(),"candidate_delta":"Remove only broad root8 width3; no membership_root or admin permission","runtime_keccak256":RUNTIME_HASH,"runtime_bytes":7896,"runtime_substitutions":0,"compiler":proof::SOLC_VERSION,"complete_source_files":5,"exact_primary_dependencies":4,"primary_pin":proof::OZ_PIN,"source_gap":proof::PRIMARY_GAP,"captures":proof::CAPTURES.iter().map(|(label,address,digest)|json!({"label":label,"address":address,"sha256":digest})).collect::<Vec<_>>(),"phi":{"scope":"exact whole-runtime attribution only; no individual source/creation/deployment","historical_runtime_block":122288067},"phase_a":{"calls":520,"returned":431,"reverted":82,"explicit_invalid":3,"explicit_harness_exclusions":4,"source_inputs":203,"artifacts":ARTIFACTS.iter().map(|(path,digest)|json!({"path":path,"sha256":digest})).collect::<Vec<_>>()},"creation_scope":"Complete ORI/FNA creation append bound; constructor execution stops at unsupported CHAINID and proves no initialized role set; all candidate creation remains refused","admin_callsite_count":0,"producer_structure":"Existing legacy v3 root fallback and v4/v5 actual positive boundaries; no new producer qualification","logical_growth":"Conservative logical refusal at MAX; source malformed push wraps","initial_coherence":"Caller qualification assumption, never inferred from no-op or missing evidence"}),
    )
}
fn root_word() -> String {
    root(8)
}

/// Changed persisted legacy set-length witnesses, not boolean membership or
/// authorization. Only after successful projection can these count complete
/// legacy operations. No permission is conferred by this diagnostic.
pub fn role_length_writes(block: &eth::Block) -> Result<BTreeMap<String, u64>> {
    fn word(raw: &[u8]) -> Option<[u8; 32]> {
        let mut out = [0; 32];
        if raw.len() > 32 {
            return None;
        }
        out[32 - raw.len()..].copy_from_slice(raw);
        Some(out)
    }
    let root = word(&[8]).unwrap();
    let mut images = BTreeMap::new();
    for call in block.system_calls.iter().chain(block.transaction_traces.iter().flat_map(|t| &t.calls)) {
        for (key, image) in &call.keccak_preimages {
            let key = hex::decode(key.trim_start_matches("0x"))?;
            let raw = hex::decode(image.trim_start_matches("0x"))?;
            ensure!(key == erc20_balances::hash(&raw), "invalid legacy length counter preimage");
            if let Some(prior) = images.insert(key, raw.clone()) {
                ensure!(prior == raw, "conflicting legacy counter preimage");
            }
        }
    }
    let mut counts = CONTRACTS.map(|a| (a.to_owned(), 0)).into_iter().collect::<BTreeMap<_, _>>();
    for w in block
        .system_calls
        .iter()
        .chain(
            block
                .transaction_traces
                .iter()
                .filter(|t| t.status() == eth::TransactionTraceStatus::Succeeded)
                .flat_map(|t| &t.calls),
        )
        .filter(|c| !c.state_reverted)
        .flat_map(|c| &c.storage_changes)
    {
        let address = format!("0x{}", hex::encode(&w.address));
        let Some(count) = counts.get_mut(&address) else { continue };
        let (Some(key), Some(old), Some(new)) = (word(&w.key), word(&w.old_value), word(&w.new_value)) else {
            continue;
        };
        if old == new {
            continue;
        }
        if images.get(key.as_slice()).is_some_and(|raw| raw.len() == 64 && raw[32..] == root) {
            *count += 1;
        }
    }
    Ok(counts)
}
