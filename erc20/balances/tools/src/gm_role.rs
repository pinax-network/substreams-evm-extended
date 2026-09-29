//! Two NOT-QUALIFIED GM candidates bound to the immutable host operation proof.
use crate::gm_proof as proof;
use anyhow::{ensure, Context, Result};
pub use proof::{IMPLEMENTATION, PROXY, PROXY_B};
use serde_json::{json, Value};
use std::{fs, path::Path};
pub const FIXTURE: &str = "tests/fixtures/gm-coupled-role-candidate";
pub const PROOF_FIXTURE: &str = "tests/fixtures/gm-operation-proof";
pub const BASELINE: &str = "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468";
pub const CACHES: [&str; 3] = [
    "out/next-proxy-dependencies/implementation.json",
    "out/next50-source-review/0x9b8e987e6fec8cf1380c4dca7071e2c7853aeea1.json",
    "out/ranks151-200-source-review/0xa9ee28c80f960b889dfbd1902055218cba016f75.json",
];
pub const ARTIFACTS: [(&str, &str); 6] = [
    (
        "docs/evidence/gm-operation-proof-20260928.json",
        "13ba3d4874ccc21310a2c90df3aa7b5ccfe5c9de215ff4a1969c2bc8c5eb38d8",
    ),
    (
        "docs/evidence/gm-operation-proof-20260928-transcripts.json",
        "2f65ebb07d904bb3351d299c65802b9bd0c17c1be79e048e4b6a3bde4c571b19",
    ),
    (
        "docs/evidence/gm-operation-proof-20260928-cases.json",
        "aed377335cb7f09ca58e95c90b314e4c57d76786cad04f18d39292dee04bef6c",
    ),
    (
        "docs/evidence/gm-operation-proof-20260928-compiler.json",
        "b25593f51a3bd887827999a90775bbc973d868964f6cfa110407802eff37a77a",
    ),
    (
        "docs/evidence/gm-operation-proof-20260928-source-inputs.json",
        "cb3c63c450dcaab117839717bf712836d7e326966f2e42162e86d9f8c4d05248",
    ),
    (
        "tests/fixtures/gm-operation-proof/primary-sources.json",
        "e462e19b745f5263df94a18e36337e7e70ab56ee9bee4b17b11f2895782405b8",
    ),
];
pub fn root(n: u64) -> String {
    format!("0x{n:064x}")
}
pub fn long_name_words() -> Vec<String> {
    [54u64, 401]
        .into_iter()
        .flat_map(|n| {
            let hash = proof::vm::hash(&proof::vm::word(n.into()));
            [hash, hash.overflowing_add(1.into()).0].map(|v| format!("0x{}", hex::encode(proof::vm::word(v))))
        })
        .collect()
}
pub fn candidate(baseline: &[u8]) -> Result<Value> {
    ensure!(proof::sha(baseline) == BASELINE, "complete unchanged431 baseline");
    let value: Value = serde_json::from_slice(baseline)?;
    let rows = value.as_array().context("baseline array")?;
    ensure!(rows.len() == 431, "baseline cohort");
    let mut result = vec![];
    for contract in [PROXY, PROXY_B] {
        let original = rows.iter().find(|v| v["contract"] == contract).context("GM baseline profile")?;
        let mut c = original.clone();
        ensure!(
            c["balance_slot"] == root(51) && c["code_hash"] == proof::CAPTURES[1].runtime_hash,
            "proxy runtime and balance root"
        );
        ensure!(
            c["beacon_proxy"]
                == json!({"beacon":"0xc046b05a920e4b412815934dd8e58904dda73315","beacon_code_hash":"0x90d14c8f8d7d3468b5215829b587058c5e7f78acaef6b946c20caa3de92bf410","beacon_slot":"0xa3f0ad74e5423aebfd80d3ef4346578335a9a72aeaee59ff6cb3582b35133d50","implementation":IMPLEMENTATION,"implementation_code_hash":proof::CAPTURES[0].runtime_hash,"implementation_slot":root(1)}),
            "exact frozen beacon dependency binding"
        );
        ensure!(
            c["other_mapping_words"].as_object_mut().context("legacy mapping")?.remove(&root(201)) == Some(json!(2)),
            "exact width-two legacy root201"
        );
        ensure!(
            c.get("other_mapping_paths").is_none() && c.get("enumerable_address_sets").is_none(),
            "no independent membership permission"
        );
        ensure!(c["other_mapping_slots"] == json!([root(52)]), "allowance root remains unchanged");
        let mut scalars: Vec<_> = [0, 53, 54, 55, 301, 351, 401, 402].into_iter().map(root).collect();
        scalars.extend(long_name_words());
        ensure!(c["other_slots"] == json!(scalars), "all scalar and four finite long-name words preserved");
        c["enumerable_address_sets"] =
            json!([{"root":root(251),"membership_root":root(201),"key_types":["bytes32"],"semantics":erc20_balances::layout::GM_ENUMERABLE_SEMANTICS}]);
        let mut restored = c.clone();
        restored.as_object_mut().unwrap().remove("enumerable_address_sets");
        restored["other_mapping_words"][root(201)] = json!(2);
        ensure!(restored == *original, "only role-rule edit round-trips every baseline field");
        result.push(c);
    }
    let result = json!(result);
    erc20_balances::layout::parse(&result.to_string()).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(result)
}
pub fn verify_candidate(baseline: &[u8], value: &Value) -> Result<()> {
    ensure!(candidate(baseline)? == *value, "only exact two-profile role-rule replacement allowed");
    Ok(())
}
pub fn verify_cache(root: &Path, cache: &Path) -> Result<()> {
    for (p, path) in proof::CAPTURES.iter().zip(CACHES) {
        ensure!(
            fs::read(root.join(PROOF_FIXTURE).join(format!("{}-capture.json", p.label)))? == fs::read(cache.join(path))?,
            "original {} cache differs from frozen capture",
            p.label
        );
    }
    Ok(())
}
pub fn review(package_root: &Path) -> Result<Value> {
    let mut evidence = vec![];
    for (path, sha) in ARTIFACTS {
        let raw = fs::read(package_root.join(path))?;
        ensure!(proof::sha(&raw) == sha, "frozen complete Phase A artifact {path}");
        evidence.push(json!({"path":path,"sha256":sha}));
    }
    let dir = package_root.join(PROOF_FIXTURE);
    let mut captures = vec![];
    for p in &proof::CAPTURES {
        let c = proof::verify_capture(&fs::read(dir.join(format!("{}-capture.json", p.label)))?, p)?;
        proof::verify_compiled(&c, &fs::read(dir.join(format!("{}-compiler-output.json", p.label)))?, p)?;
        captures.push(c);
    }
    let primary: Value = serde_json::from_slice(&fs::read(dir.join("primary-sources.json"))?)?;
    proof::verify_primary(&captures, &primary)?;
    proof::verify_auxiliary(&dir, &captures)?;
    // Complete immutable sources were manually reviewed. The only literal call
    // spelling is the inherited internal definition: no override/callsite exists.
    let mut admin_occurrences = vec![];
    for (path, source) in captures[0]["sources"].as_object().context("sources")? {
        for (line, text) in source["content"].as_str().context("content")?.lines().enumerate() {
            if text.contains("_setRoleAdmin(") {
                admin_occurrences.push((path.clone(), line + 1, text.trim().to_string()));
            }
        }
    }
    ensure!(
        admin_occurrences
            == vec![(
                "contracts/external/openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol".into(),
                257,
                "function _setRoleAdmin(bytes32 role, bytes32 adminRole) internal virtual {".into()
            )],
        "no reachable role-admin writer beyond frozen inherited definition"
    );
    let report: Value = serde_json::from_slice(&fs::read(package_root.join(ARTIFACTS[0].0))?)?;
    ensure!(
        report["calls"] == 1567
            && report["compiler_captured_matrix_pairs"] == 783
            && report["matrix_pairs_exact"] == true
            && report["runtime_metadata_excluded_from_executed_pc_immediates_codecopy"] == true,
        "complete paired operation scope"
    );
    Ok(
        json!({"qualified":false,"contracts":[PROXY,PROXY_B],"baseline_sha256":BASELINE,"semantics":erc20_balances::layout::GM_ENUMERABLE_SEMANTICS,"membership_root":root(201),"set_root":root(251),"balance_root":root(51),"allowance_root":root(52),"finite_long_name_payload_words":long_name_words(),"independent_membership_permission":false,"admin_write_permission":false,"admin_definition_only":admin_occurrences,"compiler":proof::SOLC_VERSION,"bindings":report["bindings"],"frozen_phase_a_artifacts":evidence,"producer_structure":{"versions":[4,5],"actual_positive_root_begin_required":true,"v3_fallback":false,"real_role_write_visibility":"unqualified"},"operation_contract":"Complete coherent boolean-plus-set only; all four legacy one-sided operations rejected; source-executed zero equality stages optional; checked logical growth refuses malformed maximum-length wrap","qualification_assumptions":["initial role-set coherence","zero unused tail","source and dependency/runtime attribution","actual producer completeness"],"custom_primary_gap":proof::PRIMARY_GAP,"history_limit":proof::HISTORY_LIMIT,"scope":"offline candidate only; no source request, RPC, stream, deployment seed, proxy dispatch or external-client qualification"}),
    )
}
