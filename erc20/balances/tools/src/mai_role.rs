//! One NOT-QUALIFIED Mai candidate bound to its exact unoptimized source proof.
use crate::mai_proof as proof;
use anyhow::{ensure, Context, Result};
pub use proof::ADDRESS;
use serde_json::{json, Value};
use std::{fs, path::Path};

pub const FIXTURE: &str = "tests/fixtures/mai-coupled-role-candidate";
pub const PROOF_FIXTURE: &str = "tests/fixtures/mai-operation-proof";
pub const CACHE: &str = "out/ranks151-200-source-review/0x35803e77c3163fed8a942536c1c8e0d5bf90f906.json";
pub const BASELINE: &str = "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468";
pub const ARTIFACTS: [(&str, &str); 16] = [
    (
        "docs/evidence/mai-operation-proof-20260929.json",
        "dc63c4701919dc758e1484d5d24d9b6912d870c0836077613f1b1d4c3714a926",
    ),
    (
        "docs/evidence/mai-operation-proof-20260929-transcripts.json",
        "c0722b4f25f79d0f26ffed1e1259987afb51e0efdb6267f0154c9d32da000685",
    ),
    (
        "docs/evidence/mai-operation-proof-20260929-cases.json",
        "c72512bff281b4f9430d4e6206359956db16078311d3ba3d77000896bea48a6f",
    ),
    (
        "docs/evidence/mai-operation-proof-20260929-compiler.json",
        "cab29d18fdf543edcc779dcc8ce773b945ffa044a97da1bc541cc04e3d8104a6",
    ),
    (
        "docs/evidence/mai-operation-proof-20260929-source-inputs.json",
        "23ff1571cf7808ac328c6ce494ab64dbc35351fda6f84921ff7435004c4234e5",
    ),
    (
        "tests/fixtures/mai-operation-proof/primary-sources.json",
        "a835cfc598f289cedc7d5eb47d4faa1fa8808ffe34f168dae9e4ae1d726ad608",
    ),
    (
        "tests/fixtures/mai-operation-proof/LICENSE-openzeppelin",
        "8c21a3d62814a86c6e6db50768e5a885a1f586873628e9be79a93f4eb6c18820",
    ),
    (
        "tests/fixtures/mai-operation-proof/README.md",
        "e828d0e147748e92529b652de5eb8bb491aea1c9f1d60e97432db0d0344c6677",
    ),
    (
        "tests/fixtures/mai-operation-proof/capture.json",
        "e5fb99260765d58a5fecdd4d2eccd61c84e6f0032821fc75402f00e93535386b",
    ),
    (
        "tests/fixtures/mai-operation-proof/compiler-input-original.json",
        "649570084d364b09c1c858fec26a1a92dac864b9365a21816857d15a8f7dd88c",
    ),
    (
        "tests/fixtures/mai-operation-proof/compiler-input.json",
        "30fc984c545558237be971b646fd4d4f6c236eeb3d815d0b2be7ad2b4bc1a1ed",
    ),
    (
        "tests/fixtures/mai-operation-proof/compiler-output.json",
        "18a24ae3680b1b25d1b47c0d0c9a9b9d53dbd5a90c6bd6c003e6a0f5a5a20764",
    ),
    (
        "tests/fixtures/mai-operation-proof/compiler-version.txt",
        "42b3b944e56b8dec08f839a426951c22f960b4599c531e72dc3820c6e85f15cc",
    ),
    (
        "tests/fixtures/mai-operation-proof/metadata-fresh.json",
        "30382f478302f7d52b3c1223f0d73dc34c22c15f648f18f42853339c8d853b78",
    ),
    (
        "tests/fixtures/mai-operation-proof/metadata-original.json",
        "30382f478302f7d52b3c1223f0d73dc34c22c15f648f18f42853339c8d853b78",
    ),
    (
        "tests/fixtures/mai-operation-proof/solc-list.json",
        "22e8ba1c7c8d0fc5eb60964083b238d267ea2c1afec9757521fea20c45b206af",
    ),
];
pub fn root(n: u64) -> String {
    format!("0x{n:064x}")
}
pub fn candidate(baseline: &[u8]) -> Result<Value> {
    ensure!(proof::sha(baseline) == BASELINE, "complete unchanged431 baseline");
    let value: Value = serde_json::from_slice(baseline)?;
    let rows = value.as_array().context("baseline array")?;
    ensure!(rows.len() == 431, "baseline cohort");
    let profiles: Vec<_> = rows.iter().filter(|v| v["contract"] == ADDRESS).collect();
    ensure!(profiles.len() == 1, "exact single Mai profile");
    let original = profiles[0];
    ensure!(
        original["balance_slot"] == root(2) && original["code_hash"] == proof::RUNTIME_HASH,
        "Mai balance/runtime"
    );
    ensure!(
        original["other_mapping_slots"] == json!([root(0), root(3)]),
        "exact legacy membership and allowance permissions"
    );
    ensure!(original["other_mapping_words"] == json!({root(1):2}), "exact legacy set width");
    ensure!(original["other_slots"] == json!([root(4), root(5), root(6)]), "supply/name/symbol scalars");
    ensure!(
        original.get("enumerable_address_sets").is_none() && original.get("other_mapping_paths").is_none(),
        "no preexisting independent role path"
    );
    ensure!(
        original.get("deployment").is_none() && original.get("proxy").is_none() && original.get("beacon_proxy").is_none(),
        "no creation/proxy admission"
    );
    let mut c = original.clone();
    ensure!(
        c["other_mapping_slots"].as_array_mut().unwrap().remove(0) == json!(root(0)),
        "remove only membership root0"
    );
    ensure!(
        c["other_mapping_words"].as_object_mut().unwrap().remove(&root(1)) == Some(json!(2)),
        "remove only set root1 width2"
    );
    c["enumerable_address_sets"] =
        json!([{"root":root(1),"membership_root":root(0),"key_types":["bytes32"],"semantics":erc20_balances::layout::MAI_ENUMERABLE_SEMANTICS}]);
    let mut restored = c.clone();
    restored.as_object_mut().unwrap().remove("enumerable_address_sets");
    restored["other_mapping_slots"].as_array_mut().unwrap().insert(0, json!(root(0)));
    restored["other_mapping_words"][root(1)] = json!(2);
    ensure!(restored == *original, "only exact two permission removals round-trip every baseline field");
    let result = json!([c]);
    erc20_balances::layout::parse(&result.to_string()).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(result)
}
pub fn verify_candidate(baseline: &[u8], value: &Value) -> Result<()> {
    ensure!(candidate(baseline)? == *value, "only exact Mai role-rule replacement allowed");
    Ok(())
}
pub fn verify_cache(root: &Path, cache: &Path) -> Result<()> {
    ensure!(
        fs::read(root.join(PROOF_FIXTURE).join("capture.json"))? == fs::read(cache.join(CACHE))?,
        "original Mai capture differs from frozen evidence"
    );
    Ok(())
}
/// Reconcile independently pinned raw artifacts with their compiler/operation links.
pub fn verify_report_links(package_root: &Path, compiler: &Value, report: &Value) -> Result<()> {
    ensure!(
        compiler["status"] == "passed" && compiler["qualified"] == false && report["status"] == "passed",
        "passed bounded proof reports"
    );
    let raw_inventory = fs::read(package_root.join(ARTIFACTS[4].0))?;
    let inventory_digest = proof::sha(&raw_inventory);
    let inventory: Value = serde_json::from_slice(&raw_inventory)?;
    ensure!(
        inventory["files"].as_object().context("proof source inventory")?.len() == 237,
        "complete 237-input historical proof"
    );
    ensure!(
        report["source_trees_equal"] == true
            && compiler["source_inventory_sha256"] == inventory_digest
            && report["source_inventory_sha256"] == inventory_digest
            && report["compiler_source_inventory_sha256"] == inventory_digest,
        "identical compiler/operation source inventory"
    );
    for (field, index) in [("transcripts_sha256", 1), ("cases_inventory_sha256", 2)] {
        ensure!(
            report[field] == proof::sha(&fs::read(package_root.join(ARTIFACTS[index].0))?),
            "exact report link {field}"
        );
    }
    let files = [
        "solc-list.json",
        "compiler-version.txt",
        "LICENSE-openzeppelin",
        "capture.json",
        "compiler-input-original.json",
        "compiler-input.json",
        "compiler-output.json",
        "metadata-original.json",
        "metadata-fresh.json",
        "primary-sources.json",
    ];
    let mut expected = Vec::new();
    for file in files {
        expected.push(json!({"file":file,"sha256":proof::sha(&fs::read(package_root.join(PROOF_FIXTURE).join(file))?)}));
    }
    ensure!(
        compiler["artifacts"] == json!(expected),
        "exact complete compiler artifact names/order/current raw bytes"
    );
    expected.push(json!({"file":"report.json","sha256":proof::sha(&fs::read(package_root.join(ARTIFACTS[3].0))?)}));
    ensure!(
        report["source_artifacts"] == json!(expected),
        "operation source artifacts equal compiler prefix plus exact report"
    );
    Ok(())
}
pub fn review(package_root: &Path) -> Result<Value> {
    let mut evidence = vec![];
    for (path, expected) in ARTIFACTS {
        let raw = fs::read(package_root.join(path))?;
        ensure!(proof::sha(&raw) == expected, "frozen complete Phase A artifact {path}");
        evidence.push(json!({"path":path,"sha256":expected}));
    }
    let (capture, _) = proof::verify_source_directory(&package_root.join(PROOF_FIXTURE))?;
    let mut admin_occurrences = vec![];
    for (path, source) in capture["sources"].as_object().context("sources")? {
        for (line, text) in source["content"].as_str().context("source content")?.lines().enumerate() {
            if text.contains("_setRoleAdmin(") {
                admin_occurrences.push((path.clone(), line + 1, text.trim().to_string()));
            }
        }
    }
    ensure!(
        admin_occurrences
            == vec![(
                "@openzeppelin/contracts/access/AccessControl.sol".to_owned(),
                214,
                "function _setRoleAdmin(bytes32 role, bytes32 adminRole) internal virtual {".to_owned()
            )],
        "no role-admin writer beyond inherited definition"
    );
    let report: Value = serde_json::from_slice(&fs::read(package_root.join(ARTIFACTS[0].0))?)?;
    let compiler: Value = serde_json::from_slice(&fs::read(package_root.join(ARTIFACTS[3].0))?)?;
    verify_report_links(package_root, &compiler, &report)?;
    ensure!(
        report["calls"] == 1632
            && report["returned_calls"] == 1427
            && report["reverted_calls"] == 205
            && report["full_opcode_records"] == 445655
            && report["mapped_effect_annotations"] == 4334,
        "exact complete Phase A scope"
    );
    ensure!(
        report["explicit_source_invalid_controls"] == 0 && report["explicit_unsupported_path_controls"] == 0 && report["qualified"] == false,
        "bounded successful host proof"
    );
    Ok(json!({
        "qualified":false,"contracts":[ADDRESS],"baseline_sha256":BASELINE,
        "semantics":erc20_balances::layout::MAI_ENUMERABLE_SEMANTICS,
        "membership_root":root(0),"set_root":root(1),"balance_root":root(2),"allowance_root":root(3),
        "independent_membership_permission":false,"admin_write_permission":false,"creation_admission":false,
        "admin_definition_only":admin_occurrences,"compiler":proof::SOLC_VERSION,"optimizer_enabled":false,
        "capture_sha256":proof::CAPTURE_SHA,"runtime_keccak256":proof::RUNTIME_HASH,"frozen_phase_a_artifacts":evidence,
        "producer_structure":{"versions":[4,5],"actual_positive_root_begin_required":true,"v3_fallback":false,"real_role_write_visibility":"unqualified"},
        "operation_contract":"Complete coherent boolean-plus-set only; all four void-super one-sided successes rejected; source-executed zero equality stages optional; checked logical growth refuses source maximum-length wrap",
        "qualification_assumptions":["initial role-set coherence","zero unused tail","source/runtime attribution","actual producer completeness"],
        "custom_primary_gap":proof::PRIMARY_GAP,
        "scope":"Offline candidate only; no chain calls, deployment/holder seed, authorization proof, creation admission or live package/getter qualification"
    }))
}
