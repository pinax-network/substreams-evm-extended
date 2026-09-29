//! Seventeen separate NOT-QUALIFIED candidates, derived from immutable baseline
//! bytes and the frozen Securities Phase A proof. No independent bool permission.
use crate::securities_proof as proof;
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde_json::{json, Value};
pub const FIXTURE: &str = "tests/fixtures/securities-coupled-role-candidate";
pub const PROOF_FIXTURE: &str = "tests/fixtures/securities-operation-proof";
pub const CACHE: &str = "out/proxy-source-review/0xcfed6c4679297ea4889f8183bc057b4a86c64e46.json";
pub const BASELINE: &str = "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468";
pub const TRANSCRIPTS: &str = "77e77db3ae6b2113343ad3869c701a2fb80f783a3ebdeb87b2ae2f2bffc8399d";
pub const PROOF_REPORT: &str = "16c390c746cd886a2a3f0880bc36b0ec7f6d1b0349f81462ad4c13ef329fc0f2";
pub const COMPILER_REPORT: &str = "d142396867d2fd13b51185046e5232a75685f8cd8a0daf8b1ad0cf3e1708020c";
pub const PRIMARY: &str = "8e6e998a596f5323cd9166ff2247c2997edfe561986c1723bc05992d99ceff99";
pub const PROXY_HASH: &str = "0xdf946913977a2ed76735b4b2e66f2272d1a76af911c4e520655888c9e32269f9";
pub const BEACON_HASH: &str = "0x80fbad22136c0abdce6e0f3cc46cd0572318e01b92dbd5b4d5795ef6e8808711";
pub use erc20_balances::layout::{
    SECURITIES_ENUMERABLE_SEMANTICS as SEMANTICS, SECURITIES_ISSUER_ADMIN_SLOT as ADMIN_SLOT, SECURITIES_MEMBERSHIP_ROOT as MEMBERSHIP_ROOT,
    SECURITIES_SET_ROOT as SET_ROOT,
};
pub const BALANCE_ROOT: &str = "0x52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace00";
fn hex(v: U256) -> String {
    format!("0x{}", hex::encode(proof::vm::word(v)))
}
pub fn issuer_role() -> [u8; 32] {
    erc20_balances::hash(b"ISSUER_ROLE")
}
pub fn issuer_admin_slot() -> [u8; 32] {
    let outer = erc20_balances::hash(&[issuer_role().as_slice(), &hex::decode(&MEMBERSHIP_ROOT[2..]).unwrap()].concat());
    let (slot, overflow) = U256::from_big_endian(&outer).overflowing_add(U256::one());
    assert!(!overflow);
    proof::vm::word(slot)
}
pub fn candidate(baseline: &[u8]) -> Result<Value> {
    ensure!(proof::sha(baseline) == BASELINE, "complete unchanged431baseline");
    let value: Value = serde_json::from_slice(baseline)?;
    let profiles = value.as_array().context("baseline array")?;
    ensure!(profiles.len() == 431, "baseline cohort");
    let selected: Vec<_> = profiles.iter().filter(|p| p["beacon_proxy"]["implementation"] == proof::CONTRACT).collect();
    ensure!(selected.len() == 17, "exact selected17profile cohort");
    let mut template = selected[0].clone();
    template.as_object_mut().unwrap().remove("contract");
    ensure!(
        format!("0x{}", hex::encode(issuer_admin_slot())) == ADMIN_SLOT,
        "independently derived fixed ISSUER admin"
    );
    let mut candidates = vec![];
    for original in selected {
        let mut common = original.clone();
        common.as_object_mut().unwrap().remove("contract");
        ensure!(common == template, "all selected guards/metadata identical apart from contract");
        ensure!(
            original["balance_slot"] == BALANCE_ROOT
                && original["code_hash"] == PROXY_HASH
                && original["beacon_proxy"]
                    == json!({"beacon":"0x156d6dce9a4f6139a3406f1f021f1a4880de93a3","beacon_code_hash":BEACON_HASH,"beacon_slot":"0xa3f0ad74e5423aebfd80d3ef4346578335a9a72aeaee59ff6cb3582b35133d50","implementation":proof::CONTRACT,"implementation_code_hash":proof::RUNTIME_HASH,"implementation_slot":hex(U256::one())}),
            "exact balance/proxy/beacon/runtime/pointer guards"
        );
        let mut c = original.clone();
        ensure!(
            c["other_mapping_words"].as_object_mut().context("legacy words")?.remove(MEMBERSHIP_ROOT) == Some(json!(2)),
            "exact legacy membership width2"
        );
        ensure!(
            c.get("other_mapping_paths").is_none() && c.get("enumerable_address_sets").is_none(),
            "no independent membership or set rule"
        );
        let slots = c["other_slots"].as_array_mut().context("scalar fields")?;
        ensure!(
            !slots.contains(&json!(ADMIN_SLOT)),
            "fixed admin was implicit in broad rule, not already scalar"
        );
        slots.push(json!(ADMIN_SLOT));
        c["enumerable_address_sets"] = json!([{"root":SET_ROOT,"membership_root":MEMBERSHIP_ROOT,"key_types":["bytes32"],"semantics":SEMANTICS}]);
        candidates.push(c);
    }
    let result = json!(candidates);
    erc20_balances::layout::parse(&result.to_string()).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(result)
}
pub fn verify_candidate(baseline: &[u8], value: &Value) -> Result<()> {
    ensure!(
        candidate(baseline)? == *value,
        "only exact coupled rule and fixed admin scalar may replace broad membership width"
    );
    Ok(())
}
pub fn review(raw: &[u8], compiled: &[u8], transcripts: &[u8], report: &[u8], primary: &[u8], compiler_report: &[u8]) -> Result<Value> {
    let capture = proof::verify_capture(raw)?;
    proof::verify_compiled(&capture, compiled)?;
    ensure!(
        proof::sha(primary) == PRIMARY
            && proof::sha(transcripts) == TRANSCRIPTS
            && proof::sha(report) == PROOF_REPORT
            && proof::sha(compiler_report) == COMPILER_REPORT,
        "frozen complete Phase A source and operation proof"
    );
    let primary: Value = serde_json::from_slice(primary)?;
    proof::verify_primary(&capture, &primary)?;
    let report: Value = serde_json::from_slice(report)?;
    ensure!(
        report["status"] == "passed"
            && report["calls"] == 831
            && report["returned_calls"] == 711
            && report["reverted_calls"] == 112
            && report["explicit_unsupported_path_controls"] == 8
            && report["unexpected_invalid_or_harness_failure_calls"] == 0,
        "exact bounded Phase A scope"
    );
    let source = capture["sources"][proof::SOURCE]["content"].as_str().context("exact token body")?;
    ensure!(
        source.matches("_setRoleAdmin(").count() == 1
            && source.contains("_setRoleAdmin(ISSUER_ROLE, DEFAULT_ADMIN_ROLE);")
            && source.contains("bytes32 public constant ISSUER_ROLE = keccak256(\"ISSUER_ROLE\");"),
        "only fixed initializer admin callsite"
    );
    for (name, expected) in [
        ("openzeppelin.storage.AccessControl", MEMBERSHIP_ROOT),
        ("openzeppelin.storage.AccessControlEnumerable", SET_ROOT),
    ] {
        let inner = U256::from_big_endian(&erc20_balances::hash(name.as_bytes())) - U256::one();
        let namespace = U256::from_big_endian(&erc20_balances::hash(&proof::vm::word(inner))) & !U256::from(255);
        ensure!(hex(namespace) == expected, "source ERC7201 namespace");
    }
    ensure!(
        format!("0x{}", hex::encode(issuer_admin_slot())) == ADMIN_SLOT,
        "fixed source-derived admin slot"
    );
    Ok(
        json!({"qualified":false,"profiles":17,"chain_id":56,"implementation":proof::CONTRACT,"capture_sha256":proof::CAPTURE_SHA,"compiler_output_sha256":proof::COMPILED_SHA,"compiler_report_sha256":COMPILER_REPORT,"compiler":proof::SOLC_VERSION,"source_files":31,"exact_primary_files":23,"differing_primary_files":3,"unrecovered_primary_files":5,"primary_sources_sha256":PRIMARY,"source_gap":proof::PRIMARY_GAP,"runtime_keccak256":proof::RUNTIME_HASH,"runtime_bytes":10836,"runtime_transformations":0,"proof_transcripts_sha256":TRANSCRIPTS,"proof_report_sha256":PROOF_REPORT,"synthetic_calls":831,"excluded_path_controls":8,"onchain_creation_and_deployment":"absent; successful initializer/client/proxy execution remains unproven","raw_metadata":{"saved_sha256":proof::SAVED_METADATA_SHA,"fresh_sha256":proof::FRESH_METADATA_SHA,"parsed_equal":true,"raw_equal":false},"membership_root":MEMBERSHIP_ROOT,"set_root":SET_ROOT,"semantics":SEMANTICS,"fixed_admin":{"role":format!("0x{}",hex::encode(issuer_role())),"slot":ADMIN_SLOT,"new_value":hex(U256::zero()),"scope":"initializer-only exact scalar; no arbitrary admin path, independent membership or initializer qualification"},"producer_structure":{"versions":[4,5],"actual_positive_root_begin_required":true,"v3_fallback":false,"actual_role_write_visibility":"unqualified"},"equal_stages":"only source-executed zero-address append/tail clear may be omitted","initial_role_set_coherence_and_unused_tail_zero":"qualification assumption","logical_length_growth":"checked conservative admission restriction; malformed source max-length push wraps","baseline_sha256":BASELINE}),
    )
}
