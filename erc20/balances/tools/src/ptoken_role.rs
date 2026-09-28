//! One NOT-QUALIFIED coupled candidate bound to the frozen Phase A compiler proof.
//! No independent membership allowlist; no changes to the historical baseline.
use crate::ptoken_proof as proof;
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
pub const FIXTURE: &str = "tests/fixtures/ptoken-coupled-role-candidate";
pub const PROOF_FIXTURE: &str = "tests/fixtures/ptoken-operation-proof";
pub const CACHE: &str = "out/next50-source-review/0xc63961bf9a7d6bb5844524851d04fbd76dbe915b.json";
pub const BASELINE: &str = "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468";
pub const TRANSCRIPTS: &str = "0dedf0a50e6cecd63a0a696337d7d79c109e08fe655ed2a41727c18192e1fbe0";
pub const PROOF_REPORT: &str = "e1b50cb54212466811036c5d4a36fe79171d5ba8cac9492dbfbded730f65f8c7";
pub const PRIMARY: &str = "fc9d2c66c9a7a63714b98f9e5d6a15642deb34e1558914525e6493ad44111650";
pub use proof::{CAPTURE, CONTRACT, RUNTIME};
pub fn root(v: u64) -> String {
    format!("0x{v:064x}")
}
pub fn candidate(baseline: &[u8]) -> Result<Value> {
    ensure!(proof::sha(baseline) == BASELINE, "complete unchanged431baseline");
    let value: Value = serde_json::from_slice(baseline)?;
    let profiles = value.as_array().context("baseline array")?;
    ensure!(profiles.len() == 431, "baseline cohort");
    let mut c = profiles.iter().find(|c| c["contract"] == CONTRACT).context("PToken baseline")?.clone();
    ensure!(c["balance_slot"] == root(0) && c["code_hash"] == RUNTIME, "runtime/balance binding");
    ensure!(
        c["other_mapping_words"].as_object_mut().context("legacy words")?.remove(&root(5)) == Some(json!(2)),
        "exact legacy root5 width2"
    );
    ensure!(
        c.get("other_mapping_paths").is_none() && c.get("enumerable_address_sets").is_none(),
        "no independent membership or set rule"
    );
    c["enumerable_address_sets"] =
        json!([{"root":root(6),"membership_root":root(5),"key_types":["bytes32"],"semantics":erc20_balances::layout::PTOKEN_ENUMERABLE_SEMANTICS}]);
    let result = json!([c]);
    erc20_balances::layout::parse(&result.to_string()).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(result)
}
pub fn verify_candidate(baseline: &[u8], value: &Value) -> Result<()> {
    ensure!(candidate(baseline)? == *value, "only coupled root5/root6 replacement may change");
    Ok(())
}
pub fn review(raw: &[u8], compiled: &[u8], transcripts: &[u8], report: &[u8], primary: &[u8]) -> Result<Value> {
    let capture = proof::verify_capture(raw)?;
    proof::verify_compiled(&capture, compiled)?;
    ensure!(proof::sha(primary) == PRIMARY, "frozen complete Phase A primary-source artifact");
    let primary: Value = serde_json::from_slice(primary)?;
    let sources = primary["sources"].as_object().context("primary sources")?;
    ensure!(sources.len() == 20, "exact independent dependency scope");
    ensure!(primary["token_gap"] == proof::PRIMARY_GAP, "primary token gap remains explicit");
    for (name, source) in sources {
        let content = source["content"].as_str().context("primary source content")?;
        ensure!(
            source["content"] == capture["sources"][name]["content"] && source["sha256"] == proof::sha(content.as_bytes()),
            "primary dependency bytes/hash differ: {name}"
        );
        ensure!(
            source["url"]
                == format!(
                    "https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{}/{}",
                    proof::OZ_PIN,
                    name.strip_prefix("lib/openzeppelin-contracts/").context("primary dependency path")?
                ),
            "exact primary dependency pin"
        );
    }
    ensure!(
        proof::sha(transcripts) == TRANSCRIPTS && proof::sha(report) == PROOF_REPORT,
        "frozen complete Phase A operation evidence"
    );
    let report: Value = serde_json::from_slice(report)?;
    ensure!(
        report["calls"] == 382 && report["returned_calls"] == 338 && report["reverted_calls"] == 44 && report["invalid_or_unsupported_calls"] == 0,
        "exact operation proof scope"
    );
    let storage = capture["storageLayout"]["storage"].as_array().context("layout fields")?;
    for (label, slot) in [("_balances", "0"), ("_roles", "5"), ("_roleMembers", "6")] {
        let field = storage.iter().find(|f| f["label"] == label).context("bound layout field")?;
        ensure!(field["slot"] == slot && field["offset"] == 0, "exact layout root");
    }
    Ok(
        json!({"qualified":false,"contract":CONTRACT,"chain_id":56,"capture_sha256":CAPTURE,"compiler_output_sha256":proof::COMPILED_SHA,"compiler":proof::SOLC_VERSION,"source_files":21,"primary_dependencies":20,"primary_sources_sha256":PRIMARY,"primary_token_gap":proof::PRIMARY_GAP,"runtime_keccak256":RUNTIME,"runtime_bytes":6065,"runtime_transformations":0,"source_reconstruction":"fresh official compiler, exact complete runtime and creation, Phase A","proof_transcripts_sha256":TRANSCRIPTS,"proof_report_sha256":PROOF_REPORT,"synthetic_calls":382,"membership_root":root(5),"set_root":root(6),"semantics":erc20_balances::layout::PTOKEN_ENUMERABLE_SEMANTICS,"scope":"exact reviewed ordered template; not universal OZ5 support","producer_structure":{"versions":[4,5],"actual_positive_root_begin_required":true,"v3_fallback":false,"actual_ptoken_role_write_visibility":"unqualified"},"equal_stages":"only source-executed zero-address append/tail clear may be omitted","initial_role_set_coherence_and_unused_tail_zero":"qualification assumption","logical_length_growth":"checked conservative admission restriction; source max-length push wraps","independent_membership_permission":false,"clear_or_admin_permission":false,"baseline_sha256":BASELINE}),
    )
}
