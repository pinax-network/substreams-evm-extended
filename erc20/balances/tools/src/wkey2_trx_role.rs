//! Two NOT-QUALIFIED legacy enumerable candidates with exact final source-proof bindings.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use substreams_ethereum::pb::eth::v2 as eth;
pub const CONTRACTS: [&str; 2] = ["0xe0a281deff5c9d8d67af09d39340e134ac81b82e", "0xce7de646e7208a4ef112cb6ed5038fa6cc6b12e3"];
pub const RUNTIMES: [&str; 2] = [
    "0x9054d0efc5311cd08c2d5c1204bc9f600f43e301ea5fe6446b4d581c44d6e6a3",
    "0x84f4834aef7376b01cc68f967cd357e4bfbe13ba2b775757ed1f8e28b67be82c",
];
pub const BASELINE: &str = "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468";
pub const FIXTURE: &str = "tests/fixtures/wkey2-trx-enumerable-candidate";
pub fn root(n: u64) -> String {
    format!("0x{n:064x}")
}
pub fn role_root(address: &str) -> Option<u64> {
    match address {
        a if a == CONTRACTS[0] => Some(8),
        a if a == CONTRACTS[1] => Some(6),
        _ => None,
    }
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
    for (i, address) in CONTRACTS.iter().enumerate() {
        let matches: Vec<_> = profiles.iter().filter(|p| p["contract"] == *address).collect();
        ensure!(matches.len() == 1, "exact selected profile {address}");
        let mut c = matches[0].clone();
        let r = role_root(address).unwrap();
        let slots: &[u64] = if i == 0 {
            &[2, 3, 4, 5, 7, 9, 10, 11, 12, 13]
        } else {
            &[2, 3, 4, 5, 7, 8, 9]
        };
        let mappings = if i == 0 { json!([root(1), root(6)]) } else { json!([root(1), root(10)]) };
        ensure!(
            c["balance_slot"] == root(0)
                && c["code_hash"] == RUNTIMES[i]
                && c["other_slots"] == json!(slots.iter().map(|v| root(*v)).collect::<Vec<_>>())
                && c["other_mapping_slots"] == mappings,
            "exact target layout/runtime"
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
            c["other_mapping_words"].as_object_mut().context("legacy words")?.remove(&root(r)) == Some(json!(if i == 0 { 2 } else { 3 })),
            "remove only exact target-specific legacy width"
        );
        c["enumerable_address_sets"] = json!([{"root":root(r),"key_types":["bytes32"],"semantics":"oz_3_4_2"}]);
        selected.push(c);
    }
    let out = json!(selected);
    erc20_balances::layout::parse(&out.to_string()).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(out)
}
pub fn verify_candidate(raw: &[u8], selected: &Value) -> Result<()> {
    ensure!(candidate(raw)? == *selected, "only two exact legacy role replacements");
    Ok(())
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
        let Some(root) = role_root(&address).map(|n| word(&[n as u8]).unwrap()) else {
            continue;
        };
        let count = counts.get_mut(&address).unwrap();
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

use std::{fs, path::Path};
pub const PROOF_FIXTURE: &str = "tests/fixtures/wkey2-trx-operation-proof";
pub const ARTIFACTS: &[(&str, &str)] = &[
    (
        "docs/evidence/wkeydao2-trx-operation-proof-20260929-cases.json",
        "5c3258d8f28a8a2cb4cdd23f5676257f300bacb13c5a2dcc3248b891ea7e8dac",
    ),
    (
        "docs/evidence/wkeydao2-trx-operation-proof-20260929-compiler.json",
        "842b71fd927ccbd8332f437d14003e0a27e9a540553e6038292792f40e83af56",
    ),
    (
        "docs/evidence/wkeydao2-trx-operation-proof-20260929-source-inputs.json",
        "829cc8b9d643071169bf2c4e3c5c71a22c314334b4eb46f4b25aa14d7a9bc977",
    ),
    (
        "docs/evidence/wkeydao2-trx-operation-proof-20260929-transcripts.json",
        "871d6d72c4785d518179c3afa8bdd13529766fa42043d44bfda0f77fb2d1e637",
    ),
    (
        "docs/evidence/wkeydao2-trx-operation-proof-20260929.json",
        "e31c76e8a17f75531a2af286a6c04418b663ecd75cd4bd75a628d743169764ef",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/LICENSE-openzeppelin",
        "5d77daf99ea6e8033e57b449761ab4e9b486c45ef578c387d2805fe98e560f46",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/README.md",
        "f00e6a427d6530f157e2920c35d127d72ba273cb00d281372c955b8759d929dc",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/solc-list.json",
        "22e8ba1c7c8d0fc5eb60964083b238d267ea2c1afec9757521fea20c45b206af",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/trx/capture.json",
        "e0260fa176be3c3e945de5d82af3c07ab9fec026705dc868b42881d474560ffc",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/trx/compiler-input-original.json",
        "fbde1ef50ed0fd54f40e29662181a7f1376f62d8299252119b4c6d065feeabff",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/trx/compiler-input.json",
        "62607ac503f9bdc16e29007537f22de4d37d0f6c7a66720ae25e2ee4bca0343f",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/trx/compiler-output.json",
        "6d9938caf954382e95f098db0d4d85c6d0733b455079d1d264271e367e2e7bd8",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/trx/compiler-version.txt",
        "c1c5e1e9465568a7947ea73fdb73fd31a3dd7ba06df1010289b5d847b0a2e28e",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/trx/metadata-fresh.json",
        "7b2ac0822096f31021f28a808890c48a8ecd96d50fad8bcbe153a5f58367689d",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/trx/metadata-original.json",
        "7b2ac0822096f31021f28a808890c48a8ecd96d50fad8bcbe153a5f58367689d",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/trx/primary-sources.json",
        "1da9b80c7625a5e788585f2f91147551c2f30e7678686c5f3891234a866beeeb",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/trx/source-licenses.json",
        "d31a1c90a2d3b54ab0328801f78ce5c154e668df8431938373187248ce24ff68",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/trx-capture.json",
        "e0260fa176be3c3e945de5d82af3c07ab9fec026705dc868b42881d474560ffc",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/wkeydao2/capture.json",
        "1afd2a842f97acb1eb03dca09e802d7dcec1e06eef97533dee2c2b2ebf180df0",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/wkeydao2/compiler-input-original.json",
        "cdb8ba6d0d41bd83b2949ec75bb8aebba53dc6c26f39b360f7c4e21288cdf988",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/wkeydao2/compiler-input.json",
        "b9861f820b449ea188eb0cc5287bc7f5e9545cd019af1a25bf35854925c5bf6d",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/wkeydao2/compiler-output.json",
        "58f88b6f0a6765147a48d19553a6b69172782eb3355845b741a2f4340df51f3c",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/wkeydao2/compiler-version.txt",
        "025f70e8f8a13b28b6a5b8e369f0360c18bd2106938fdd908ad2afa7bdb823fd",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/wkeydao2/metadata-fresh.json",
        "d0ac7cc5f708b8e153afa4096287c2d49c60f4b99af633ef240f17ff1712c773",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/wkeydao2/metadata-original.json",
        "d0ac7cc5f708b8e153afa4096287c2d49c60f4b99af633ef240f17ff1712c773",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/wkeydao2/primary-sources.json",
        "1b3821934bb55862b5b080ed76d4937b372fc6f32eaa2f1e48a5132dd4fc4b51",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/wkeydao2/source-licenses.json",
        "ec0253f5b42b44750c909b6ae9b59542830546a5e1ef17eca3713795435d2093",
    ),
    (
        "tests/fixtures/wkey2-trx-operation-proof/wkeydao2-capture.json",
        "1afd2a842f97acb1eb03dca09e802d7dcec1e06eef97533dee2c2b2ebf180df0",
    ),
];
pub const CACHE_FILES: [(&str, &str); 2] = [
    (
        "wkeydao2-capture.json",
        "out/ranks101-150-source-review/0xe0a281deff5c9d8d67af09d39340e134ac81b82e.json",
    ),
    (
        "trx-capture.json",
        "out/ranks251-300-source-review/0xce7de646e7208a4ef112cb6ed5038fa6cc6b12e3.json",
    ),
];
pub fn verify_cache(root: &Path, cache: &Path) -> Result<()> {
    for (fixture, original) in CACHE_FILES {
        ensure!(
            fs::read(root.join(PROOF_FIXTURE).join(fixture))? == fs::read(cache.join(original))?,
            "original complete capture {fixture}"
        );
    }
    Ok(())
}
pub fn review(root: &Path) -> Result<Value> {
    use crate::wkey2_trx_proof as proof;
    for (path, digest) in ARTIFACTS {
        ensure!(sha(&fs::read(root.join(path))?) == *digest, "frozen Phase A artifact {path}");
    }
    let read = |path: &str| -> Result<Value> { Ok(serde_json::from_slice(&fs::read(root.join(path))?)?) };
    let fixture = root.join(PROOF_FIXTURE);
    let sources = proof::verify_source_directory(&fixture)?;
    for (i, (target, capture, _)) in sources.iter().enumerate() {
        ensure!(
            target.address() == CONTRACTS[i] && target.runtime_hash() == RUNTIMES[i] && target.role_root().as_u64() == role_root(CONTRACTS[i]).unwrap(),
            "target runtime/root identity"
        );
        let duplicate = proof::verify_capture(&fs::read(fixture.join(format!("{}-capture.json", target.label())))?, *target)?;
        ensure!(*capture == duplicate, "duplicate original complete capture");
        let setters: usize = capture["sources"]
            .as_object()
            .unwrap()
            .values()
            .map(|s| s["content"].as_str().unwrap().matches("_setRoleAdmin(").count())
            .sum();
        ensure!(setters == 1, "only inherited setter declaration, no reachable admin write");
    }
    let report = read("docs/evidence/wkeydao2-trx-operation-proof-20260929.json")?;
    let compiler = read("docs/evidence/wkeydao2-trx-operation-proof-20260929-compiler.json")?;
    let inventory_path = "docs/evidence/wkeydao2-trx-operation-proof-20260929-source-inputs.json";
    let inventory_digest = sha(&fs::read(root.join(inventory_path))?);
    ensure!(
        report["status"] == "passed"
            && report["qualified"] == false
            && report["calls"] == 2460
            && report["returned_calls"] == 1946
            && report["reverted_calls"] == 488
            && report["explicit_source_invalid_controls"] == 12
            && report["explicit_unsupported_path_controls"] == 14
            && report["unexpected_invalid_or_harness_failure_calls"] == 0
            && report["chain_calls"] == 0
            && report["compact_transcripts"] == 872
            && report["compiler_captured_case_pairs"] == 1230
            && report["all_case_pairs_exact"] == true
            && report["cbor_excluded_from_executed_pc_immediates_codecopy"] == true
            && report["constructor_initialization_complete"] == false
            && report["mapped_effect_annotations"] == 10216
            && report["pinned_generated_effect_annotations_without_source_text"] == 84
            && report["total_effect_annotations"] == 10300,
        "complete bounded operation proof"
    );
    ensure!(
        report["source_trees_equal"] == true
            && report["source_inventory_sha256"] == inventory_digest
            && report["compiler_source_inventory_sha256"] == inventory_digest
            && compiler["source_inventory_sha256"] == inventory_digest,
        "same final proof source inventory"
    );
    ensure!(
        read(inventory_path)?["files"].as_object().context("proof source files")?.len() == 243,
        "complete historical proof inputs"
    );
    for (field, suffix) in [("cases_inventory_sha256", "-cases.json"), ("transcripts_sha256", "-transcripts.json")] {
        ensure!(
            report[field] == sha(&fs::read(root.join(format!("docs/evidence/wkeydao2-trx-operation-proof-20260929{suffix}")))?),
            "proof artifact link {field}"
        );
    }
    ensure!(compiler["status"] == "passed" && compiler["qualified"] == false, "bounded compiler result");
    let source_artifacts = compiler["artifacts"].as_array().context("compiler artifacts")?;
    let mut expected = source_artifacts.clone();
    expected.push(json!({"file":"report.json","sha256":sha(&fs::read(root.join("docs/evidence/wkeydao2-trx-operation-proof-20260929-compiler.json"))?) }));
    expected.push(json!({"file":"source-inventory.json","sha256":inventory_digest}));
    ensure!(
        report["source_artifacts"] == json!(expected),
        "same compiled artifacts plus exact compiler report and source inventory"
    );
    for artifact in source_artifacts {
        let p = artifact["file"].as_str().context("artifact file")?;
        ensure!(artifact["sha256"] == sha(&fs::read(fixture.join(p))?), "compiled artifact {p}");
    }
    for (i, t) in proof::Target::ALL.iter().enumerate() {
        let v = &report["targets"][i];
        ensure!(
            v["target"] == t.label()
                && v["address"] == t.address()
                && v["runtime_bytes"] == t.runtime_len()
                && v["runtime_keccak256"] == t.runtime_hash()
                && v["whole_compiler_output_sha256"] == t.compiled_sha(),
            "operation target"
        );
        let expected = if i == 0 { [638, 505, 126, 3, 4] } else { [592, 468, 118, 3, 3] };
        for (j, variant) in ["compiled", "captured"].iter().enumerate() {
            let actual = &v["variants"][j];
            ensure!(actual["variant"] == *variant, "paired code variant");
            for (field, count) in ["calls", "return", "revert", "explicit_source_invalid", "explicit_unsupported"]
                .iter()
                .zip(expected)
            {
                ensure!(actual[*field] == count, "exact target/variant outcome {field}");
            }
        }
    }
    Ok(json!({
        "qualified":false,"contracts":CONTRACTS,"chain_id":56,"baseline_sha256":BASELINE,
        "semantics":"oz_3_4_2","role_roots":[8,6],
        "candidate_delta":"wkeyDAO2 removes only root8 width2; TRX removes only root6 width3; no membership_root/admin/creation permission",
        "runtimes":RUNTIMES,"runtime_bytes":[8495,7088],"compilers":["0.7.5+commit.eb77ed08","0.6.6+commit.6c089d02"],
        "complete_source_files":[5,1],"exact_complete_primary_dependencies":[4,0],"comparison_only_normalized_primary_declarations":[0,2],
        "primary_pin":proof::OZ_PIN,"source_gap":proof::PRIMARY_GAP,
        "runtime_transformations":"One exact 53-byte CBOR replacement per target; no links/immutables/normalization",
        "creation_scope":"Exact substitutions at 9764/8761 preserve 32/79 trailing bytes before independent 96/192-byte arguments. Both constructor attempts stop at CHAINID and roll back; neither candidate permits creation or establishes initialized roles.",
        "generated_effect_scope":"TRX 0.6.6 provides no generated source text: only pinned creation SLOAD1372/SSTORE1379 and runtime SLOAD6169/SSTORE6176, exact -1 spans. All other effects have Solidity attribution. No new exception.",
        "phase_a":{"calls":2460,"compiler_captured_pairs":1230,"returned":1946,"reverted":488,"explicit_invalid":12,"explicit_harness_exclusions":14,"compact_transcripts":872,"source_inputs":243,"artifacts":ARTIFACTS.iter().map(|(path,digest)|json!({"path":path,"sha256":digest})).collect::<Vec<_>>()},
        "admin_callsite_count":0,"producer_structure":"Unchanged legacy v3 fallback and v4/v5 actual positive root begin; real role/preimage/equality visibility unqualified",
        "logical_growth":"Conservative checked refusal; malformed MAX source push wraps",
        "initial_coherence":"Caller qualification assumption; no role cache or inference from no-op; TRX synthetic default-admin/self-suffix context is not deployed authorization or meta-transaction execution"
    }))
}
