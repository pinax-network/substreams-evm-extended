//! Two NOT-QUALIFIED legacy enumerable candidates with exact final source-proof bindings.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use substreams_ethereum::pb::eth::v2 as eth;
pub const CONTRACTS: [&str; 2] = ["0x194b302a4b0a79795fb68e2adf1b8c9ec5ff8d1f", "0x701add4311e85c1f9c1549319fe2c476bc8a1b8b"];
pub const RUNTIMES: [&str; 2] = [
    "0x84d1cbfc7b7c569181930ce930f0dbe6edb8e8df5631b0a066bd0197d109b9f3",
    "0x8f10d493bbd10ba2062c25efaa1cfe1035b392f339a485fce3faed0c0768dc9c",
];
pub const BASELINE: &str = "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468";
pub const FIXTURE: &str = "tests/fixtures/wkey-got-enumerable-candidate";
pub fn root(n: u64) -> String {
    format!("0x{n:064x}")
}
pub fn role_root(address: &str) -> Option<u64> {
    match address {
        a if a == CONTRACTS[0] => Some(9),
        a if a == CONTRACTS[1] => Some(8),
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
            &[2, 3, 4, 5, 6, 8, 10, 11, 12, 13, 14]
        } else {
            &[2, 3, 4, 5, 6, 9, 10, 11, 12, 13]
        };
        let mappings = if i == 0 {
            json!([root(1), root(7)])
        } else {
            json!([root(1), root(7), root(8)])
        };
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
            c["other_mapping_words"].as_object_mut().context("legacy words")?.remove(&root(r)) == Some(json!(3)),
            "remove exact width3"
        );
        if i == 1 {
            ensure!(
                c["other_mapping_slots"].as_array_mut().unwrap().remove(2) == root(8),
                "GOT also removes broad root8 at original position"
            );
        }
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
pub const PROOF_FIXTURE: &str = "tests/fixtures/wkey-got-operation-proof";
pub const ARTIFACTS: &[(&str, &str)] = &[
    (
        "docs/evidence/wkey-got-operation-proof-20260929-cases.json",
        "b49b4d16017cfb38b70d13e7a69244fcf1049994f0973a64222836cf2ce1712e",
    ),
    (
        "docs/evidence/wkey-got-operation-proof-20260929-compiler.json",
        "bd61d288b2d02fc6820c89c7f88e2907c11551fae3271c2bf429634bdcb555c0",
    ),
    (
        "docs/evidence/wkey-got-operation-proof-20260929-source-inputs.json",
        "4782430d017a36991bbefe3577cf72380b311025a701a632cb503d8aff7a2ca9",
    ),
    (
        "docs/evidence/wkey-got-operation-proof-20260929-transcripts.json",
        "d4e9a8ec2aac91d9ee9af1a273a959d30cda089c1598df1f409ac91f15502426",
    ),
    (
        "docs/evidence/wkey-got-operation-proof-20260929.json",
        "0dfc95a2b5c949ec5eab362fb7a638256a86de527b1ce94cb06966426aa86e08",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/LICENSE-openzeppelin",
        "5d77daf99ea6e8033e57b449761ab4e9b486c45ef578c387d2805fe98e560f46",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/README.md",
        "201b4e22b022a8d19008367a2fac5dc52d4f6f8340811122b6c36c9a69982730",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/compiler-version.txt",
        "025f70e8f8a13b28b6a5b8e369f0360c18bd2106938fdd908ad2afa7bdb823fd",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/got-capture.json",
        "cfbda1cef9b3fb83fb92c388f9a3b0d92b7975378050cc90376028b81f0e6ceb",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/got/capture.json",
        "cfbda1cef9b3fb83fb92c388f9a3b0d92b7975378050cc90376028b81f0e6ceb",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/got/compiler-input-original.json",
        "ea1dc101f3c3223109ba4723e0be2c3dcfee167caae1edcb86d7e6860a3e8527",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/got/compiler-input.json",
        "00560d31fd206fddf4f20fccca726e6802a2c19b8be02c8f26b3b702dbd4c87e",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/got/compiler-output.json",
        "d6a4e5151228a4e89545f1193e349c60ad9e1f1e7c628abda7b6cbf2fae76552",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/got/metadata-fresh.json",
        "d33921901ded8d4b225c778b9b7dd2593cacfc5c0adbc1d16ff417c74fb7433e",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/got/metadata-original.json",
        "d33921901ded8d4b225c778b9b7dd2593cacfc5c0adbc1d16ff417c74fb7433e",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/got/primary-sources.json",
        "545e46cba10eaaeba67e6e21bdd4da890b6e5f534203c90748f7ab40a8bd57c3",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/solc-list.json",
        "22e8ba1c7c8d0fc5eb60964083b238d267ea2c1afec9757521fea20c45b206af",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/wkeydao-capture.json",
        "e74c8d858f361712037871c6e5a94c0fb4a81064861f423e2df929b62e02108d",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/wkeydao/capture.json",
        "e74c8d858f361712037871c6e5a94c0fb4a81064861f423e2df929b62e02108d",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/wkeydao/compiler-input-original.json",
        "0a336acd324692cfb548ed1a1288b814f168e48f88ce5309fc004b27a13d389b",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/wkeydao/compiler-input.json",
        "e979c4cc39ea47c71abc2a1e9b9f5104bfbc46cc3ffb09ee858cc82d33f2ed61",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/wkeydao/compiler-output.json",
        "73a1801e4de6ec8a6c6d22c649de3782996da063b104edbccde843718b12633c",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/wkeydao/metadata-fresh.json",
        "333bebf71481952d8b631ba6be0074a02072c90c228c4c27ae35e28e46a5e3fe",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/wkeydao/metadata-original.json",
        "333bebf71481952d8b631ba6be0074a02072c90c228c4c27ae35e28e46a5e3fe",
    ),
    (
        "tests/fixtures/wkey-got-operation-proof/wkeydao/primary-sources.json",
        "68122c5a5b0f03ca2156816acbfa4d9958cbc9736adac2f47dceba90a0840667",
    ),
];

pub const CACHE_FILES: [(&str, &str); 2] = [
    (
        "wkeydao-capture.json",
        "out/ranks251-300-source-review/0x194b302a4b0a79795fb68e2adf1b8c9ec5ff8d1f.json",
    ),
    (
        "got-capture.json",
        "out/ranks351-400-source-review/0x701add4311e85c1f9c1549319fe2c476bc8a1b8b.json",
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
    use crate::wkey_got_proof as proof;
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
    let report = read("docs/evidence/wkey-got-operation-proof-20260929.json")?;
    let compiler = read("docs/evidence/wkey-got-operation-proof-20260929-compiler.json")?;
    let inventory_path = "docs/evidence/wkey-got-operation-proof-20260929-source-inputs.json";
    let inventory_digest = sha(&fs::read(root.join(inventory_path))?);
    ensure!(
        report["status"] == "passed"
            && report["qualified"] == false
            && report["calls"] == 1263
            && report["returned_calls"] == 1000
            && report["reverted_calls"] == 252
            && report["explicit_source_invalid_controls"] == 6
            && report["explicit_unsupported_path_controls"] == 5
            && report["unexpected_invalid_or_harness_failure_calls"] == 0
            && report["chain_calls"] == 0
            && report["compact_transcripts"] == 445,
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
        read(inventory_path)?["files"].as_object().context("proof source files")?.len() == 231,
        "complete historical proof inputs"
    );
    for (field, suffix) in [("cases_inventory_sha256", "-cases.json"), ("transcripts_sha256", "-transcripts.json")] {
        ensure!(
            report[field] == sha(&fs::read(root.join(format!("docs/evidence/wkey-got-operation-proof-20260929{suffix}")))?),
            "proof artifact link {field}"
        );
    }
    ensure!(compiler["status"] == "passed" && compiler["qualified"] == false, "bounded compiler result");
    let source_artifacts = compiler["artifacts"].as_array().context("compiler artifacts")?;
    let mut expected = source_artifacts.clone();
    expected.push(json!({"file":"report.json","sha256":sha(&fs::read(root.join("docs/evidence/wkey-got-operation-proof-20260929-compiler.json"))?) }));
    ensure!(
        report["source_artifacts"] == json!(expected),
        "same compiled artifacts plus exact compiler report"
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
    }
    Ok(
        json!({"qualified":false,"contracts":CONTRACTS,"chain_id":56,"baseline_sha256":BASELINE,"semantics":"oz_3_4_2","role_roots":[9,8],"candidate_delta":"WKEYDAO removes root9 width3; GOT removes root8 width3 and original broad mapping entry; no membership_root/admin permission","runtimes":RUNTIMES,"runtime_bytes":[7991,8440],"runtime_substitutions":0,"compiler":proof::SOLC_VERSION,"complete_source_files":[5,7],"exact_primary_dependencies":[4,6],"primary_pin":proof::OZ_PIN,"source_gap":proof::PRIMARY_GAP,"phase_a":{"calls":1263,"returned":1000,"reverted":252,"explicit_invalid":6,"explicit_harness_exclusions":5,"compact_transcripts":445,"source_inputs":231,"artifacts":ARTIFACTS.iter().map(|(path,digest)|json!({"path":path,"sha256":digest})).collect::<Vec<_>>()},"creation_scope":"Both complete creation appends bound. WKEYDAO CHAINID failure rolls back five stores; GOT synthetic constructor returns exact runtime after 17 stores/3 logs. Neither establishes live initialization; both candidate creations refuse.","admin_callsite_count":0,"producer_structure":"Unchanged legacy v3 fallback and v4/v5 actual positive root begin; real role/preimage/equality visibility unqualified","logical_growth":"Conservative checked refusal; malformed MAX source push wraps","initial_coherence":"Caller qualification assumption; no role cache or inference from no-op"}),
    )
}
