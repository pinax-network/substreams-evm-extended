//! Separate NOT-QUALIFIED role-only candidate. Historical whitelist permissions
//! are preserved verbatim; the direct whitelist set is not newly admitted.
use crate::btr_proof as proof;
use anyhow::{ensure, Context, Result};
pub use erc20_balances::layout::{BTR_ENUMERABLE_SEMANTICS as SEMANTICS, BTR_PAUSER_ADMIN_SLOT as ADMIN_SLOT, BTR_PAUSER_ROLE as ADMIN_VALUE};
use primitive_types::U256;
use serde_json::{json, Value};
pub const CONTRACT: &str = proof::PROXY;
pub const FIXTURE: &str = "tests/fixtures/btr-coupled-role-candidate";
pub const PROOF_FIXTURE: &str = "tests/fixtures/btr-operation-proof";
pub const CACHES: [&str; 2] = [
    "out/next50-extra-dependencies/0xc8b5a0c5453c15157328b6cc1f1452be032a41f1.json",
    "out/next50-source-review/0xfed13d0c40790220fbde712987079eda1ed75c51.json",
];
pub const BASELINE: &str = "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468";
pub const PRIMARY: &str = "198705036dc5a4b78cf6fe460651d89f8c356cbc5527feb8148e5b47ab47bdcc";
pub const TRANSCRIPTS: &str = "93d600e5a11585d70320a8c40fd3f812be783038117c25353cc2996f7b082c5d";
pub const PROOF_REPORT: &str = "4789b115a4b8af1d7be497ac192a0f62ba74f8815d417c2e8cd3b8584e3159c0";
pub const COMPILER_REPORT: &str = "6611a2d5959a206a632b1063813eddedb1a2dcbf083a41891454205622fa9866";
pub fn root(n: u64) -> String {
    format!("0x{}", hex::encode(proof::vm::word(n.into())))
}
pub fn pauser_role() -> [u8; 32] {
    erc20_balances::hash(b"PAUSER_ROLE")
}
pub fn admin_slot() -> [u8; 32] {
    let hash = erc20_balances::hash(&[pauser_role().as_slice(), &proof::vm::word(101.into())].concat());
    let (slot, overflow) = U256::from_big_endian(&hash).overflowing_add(U256::one());
    assert!(!overflow);
    proof::vm::word(slot)
}
pub fn candidate(raw: &[u8]) -> Result<Value> {
    ensure!(proof::sha(raw) == BASELINE, "complete unchanged 431 baseline");
    let baseline: Value = serde_json::from_slice(raw)?;
    let profiles = baseline.as_array().context("baseline profiles")?;
    ensure!(profiles.len() == 431, "baseline cohort");
    let selected: Vec<_> = profiles.iter().filter(|p| p["contract"] == CONTRACT).collect();
    ensure!(selected.len() == 1, "exact BTR profile");
    let mut c = selected[0].clone();
    ensure!(
        c["balance_slot"] == root(201)
            && c["code_hash"] == proof::CAPTURES[1].runtime_hash
            && c["proxy"]["implementation"] == proof::IMPLEMENTATION
            && c["proxy"]["code_hash"] == proof::CAPTURES[0].runtime_hash,
        "BTR proxy/implementation/balance binding"
    );
    ensure!(
        c["other_mapping_slots"] == json!([root(202), root(303), root(556)]),
        "allowance/nonce/whitelist index unchanged"
    );
    ensure!(
        c.get("enumerable_address_sets").is_none() && c.get("other_mapping_paths").is_none(),
        "no independent coupled or membership bypass"
    );
    ensure!(
        c["other_mapping_words"].as_object_mut().context("broad role words")?.remove(&root(101)) == Some(json!(2)),
        "remove exact broad membership width2"
    );
    let slots = c["other_slots"].as_array_mut().context("BTR scalar slots")?;
    ensure!(
        slots.contains(&json!(root(555))) && !slots.contains(&json!(ADMIN_SLOT)),
        "whitelist length unchanged and admin previously implicit"
    );
    slots.push(json!(ADMIN_SLOT));
    c["enumerable_address_sets"] = json!([{"root":root(151),"membership_root":root(101),"key_types":["bytes32"],"semantics":SEMANTICS}]);
    let candidate = json!([c]);
    erc20_balances::layout::parse(&candidate.to_string()).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(candidate)
}
pub fn verify_candidate(baseline: &[u8], candidate_value: &Value) -> Result<()> {
    ensure!(
        candidate(baseline)? == *candidate_value,
        "only exact role rule and PAUSER scalar may replace legacy broad root101"
    );
    Ok(())
}
pub fn review(raw: [&[u8]; 2], compiled: [&[u8]; 2], transcripts: &[u8], report: &[u8], primary: &[u8], compiler_report: &[u8]) -> Result<Value> {
    let mut captures = vec![];
    for (i, p) in proof::CAPTURES.iter().enumerate() {
        let c = proof::verify_capture(raw[i], p)?;
        proof::verify_compiled(&c, compiled[i], p)?;
        captures.push(c);
    }
    ensure!(
        proof::sha(primary) == PRIMARY
            && proof::sha(transcripts) == TRANSCRIPTS
            && proof::sha(report) == PROOF_REPORT
            && proof::sha(compiler_report) == COMPILER_REPORT,
        "complete frozen Phase A artifacts"
    );
    proof::verify_primary(&captures, &serde_json::from_slice(primary)?)?;
    let r: Value = serde_json::from_slice(report)?;
    ensure!(
        r["status"] == "passed" && r["calls"] == 910 && r["returned_calls"] == 836 && r["reverted_calls"] == 74,
        "exact bounded operation proof"
    );
    let source = captures[0]["sources"][proof::SOURCE]["content"].as_str().context("token source")?;
    ensure!(
        source.matches("_setRoleAdmin(").count() == 1 && source.contains("_setRoleAdmin(PAUSER_ROLE, PAUSER_ROLE);"),
        "only fixed initializer admin callsite"
    );
    ensure!(
        format!("0x{}", hex::encode(pauser_role())) == ADMIN_VALUE && format!("0x{}", hex::encode(admin_slot())) == ADMIN_SLOT,
        "independently derived PAUSER role/admin word"
    );
    let traces: Value = serde_json::from_slice(transcripts)?;
    let initialize = traces
        .as_array()
        .context("compact traces")?
        .iter()
        .find(|c| c["name"] == "initialize_captured_arguments" && c["signature"] == "initialize(string,string,address,address,uint256)")
        .context("exact initializer transcript")?;
    let admin: Vec<_> = initialize["execution"]["writes"]
        .as_array()
        .context("initializer stores")?
        .iter()
        .filter(|w| proof::bytes(&w["key"]).is_ok_and(|key| key == admin_slot()))
        .collect();
    ensure!(
        admin.len() == 1 && proof::bytes(&admin[0]["old"])? == [0; 32] && proof::bytes(&admin[0]["new"])? == pauser_role(),
        "source-executed initializer sets PAUSER self-admin exactly"
    );
    Ok(
        json!({"qualified":false,"scope":"role-only candidate; inherited whitelist permissions unchanged and partial","contract":CONTRACT,"chain_id":56,"implementation":proof::IMPLEMENTATION,"captures":proof::CAPTURES.iter().map(|p|json!({"label":p.label,"sha256":p.sha,"compiler_output_sha256":p.compiled_sha,"runtime_keccak256":p.runtime_hash,"source_files":p.files})).collect::<Vec<_>>(),"compiler":proof::SOLC_VERSION,"compiler_report_sha256":COMPILER_REPORT,"primary_sources_sha256":PRIMARY,"exact_primary_files":38,"source_gap":proof::PRIMARY_GAP,"proof_report_sha256":PROOF_REPORT,"proof_transcripts_sha256":TRANSCRIPTS,"synthetic_calls":910,"semantics":SEMANTICS,"membership_root":root(101),"set_root":root(151),"fixed_admin":{"role":ADMIN_VALUE,"slot":ADMIN_SLOT,"new_value":ADMIN_VALUE,"source_initialization_scope":"exact compiled synthetic initializer, not deployed proxy/authorization qualification"},"whitelist":{"length":root(555),"index_root":root(556),"scope":"unchanged inherited partial permissions; nonzero array mutations refused, zero-address add/sole-remove can pass when equal array stores are omitted or ignored"},"initial_coherence":"caller qualification assumption; all four void-super one-sided successes refused","logical_growth":"conservative checked admission; source maximum-length push wraps","producer_structure":{"versions":[4,5],"actual_positive_root_begin_required":true,"v3_fallback":false,"actual_role_visibility":"unqualified"},"baseline_sha256":BASELINE}),
    )
}
