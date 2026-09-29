//! Separate NOT-QUALIFIED TOPS LPInfo candidate. The historical cohorts stay immutable.
use crate::tops_proof as proof;
use anyhow::{ensure, Context, Result};
pub use proof::CONTRACT;
use serde_json::{json, Value};
use std::{fs, path::Path};
pub const FIXTURE: &str = "tests/fixtures/tops-lpinfo-candidate";
pub const CACHE: &str = "out/ranks201-250-source-review/0xcdf52c0b13c24f32f1d8d4ec6356203a1ef0826a.json";
pub const BASELINE: &str = "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468";
pub const LIMIT: usize = 6;

pub fn verify_compiled_settings(raw: &[u8]) -> Result<()> {
    const COMPILED: &[u8] = include_bytes!("../../tests/fixtures/tops-operation-proof/settings.json");
    ensure!(
        raw == COMPILED && proof::sha(COMPILED) == "9b4f4951f0782e9a503ccab777f429b90602ce0a4db247cf11a0556a27c02801",
        "exact compiled TOPS settings bytes"
    );
    Ok(())
}

pub fn snapshot(out: &Path) -> Result<String> {
    verify_compiled_settings(&fs::read("erc20/balances/tests/fixtures/tops-operation-proof/settings.json")?)?;
    proof::snapshot_sources(out)?;
    let mut inventory: Value = serde_json::from_slice(&fs::read(out.join("source-inventory.json"))?)?;
    inventory["scope"] = json!("TOPS NOT-QUALIFIED LPInfo candidate saved-block replay");
    for (path, compiled) in [
        ("Cargo.toml", include_str!("../../../../Cargo.toml")),
        ("Cargo.lock", include_str!("../../../../Cargo.lock")),
        ("rust-toolchain.toml", include_str!("../../../../rust-toolchain.toml")),
        ("erc20/balances/Cargo.toml", include_str!("../../Cargo.toml")),
        ("erc20/balances/tools/Cargo.toml", include_str!("../Cargo.toml")),
        ("erc20/balances/tools/src/lib.rs", include_str!("lib.rs")),
        ("common/retention/Cargo.toml", include_str!("../../../../common/retention/Cargo.toml")),
        ("common/retention/src/lib.rs", include_str!("../../../../common/retention/src/lib.rs")),
        (
            "common/retention/src/positions.rs",
            include_str!("../../../../common/retention/src/positions.rs"),
        ),
        ("common/retention/src/protocol.rs", include_str!("../../../../common/retention/src/protocol.rs")),
        ("proto/Cargo.toml", include_str!("../../../../proto/Cargo.toml")),
        ("proto/src/lib.rs", include_str!("../../../../proto/src/lib.rs")),
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
        ("erc20/balances/src/mapping_paths.rs", include_str!("../../src/mapping_paths.rs")),
        ("erc20/balances/src/persist.rs", include_str!("../../src/persist.rs")),
        ("erc20/balances/src/metadata_words.rs", include_str!("../../src/metadata_words.rs")),
        ("erc20/balances/src/layout.rs", include_str!("../../src/layout.rs")),
        ("erc20/balances/src/lib.rs", include_str!("../../src/lib.rs")),
        ("erc20/balances/src/lpinfo_arrays.rs", include_str!("../../src/lpinfo_arrays.rs")),
        ("erc20/balances/tools/src/ptoken_proof/mod.rs", include_str!("ptoken_proof/mod.rs")),
        (
            "erc20/balances/tools/src/ptoken_proof/source_map.rs",
            include_str!("ptoken_proof/source_map.rs"),
        ),
        ("erc20/balances/tools/src/tops_runtime_proof/mod.rs", include_str!("tops_runtime_proof/mod.rs")),
        ("erc20/balances/tools/src/tops_lpinfo.rs", include_str!("tops_lpinfo.rs")),
        (
            "erc20/balances/tools/src/bin/prepare_tops_lpinfo.rs",
            include_str!("bin/prepare_tops_lpinfo.rs"),
        ),
        ("erc20/balances/tools/src/bin/replay_tops_lpinfo.rs", include_str!("bin/replay_tops_lpinfo.rs")),
        (
            "erc20/balances/tests/fixtures/bsc-refined450-layouts.json",
            include_str!("../../tests/fixtures/bsc-refined450-layouts.json"),
        ),
    ] {
        ensure!(fs::read(path)? == compiled.as_bytes(), "stale candidate executable {path}");
    }
    for path in ARTIFACTS.iter().map(|(p, _)| format!("erc20/balances/{p}")).chain([
        "erc20/balances/tests/tops_lpinfo_operations.rs".to_owned(),
        format!("erc20/balances/{FIXTURE}/layouts.json"),
        format!("erc20/balances/{FIXTURE}/source-review.json"),
        "erc20/balances/docs/evidence/bsc-exclusions-20260928/blocks.json".to_owned(),
    ]) {
        let raw = fs::read(&path)?;
        inventory["files"][&path] = json!({"sha256":proof::sha(&raw),"bytes":raw.len()});
    }
    // Retain every inventoried source byte, including generated protobufs and
    // both historical proof generations, alongside the compiled source guards.
    for (path, entry) in inventory["files"].as_object().context("source files")? {
        let raw = fs::read(path)?;
        ensure!(entry["sha256"] == proof::sha(&raw), "source moved during snapshot");
        let target = out.join("as-run-tree").join(path);
        fs::create_dir_all(target.parent().unwrap())?;
        fs::write(target, raw)?;
    }
    let raw = serde_json::to_vec_pretty(&inventory)?;
    fs::write(out.join("source-inventory.json"), &raw)?;
    Ok(proof::sha(&raw))
}

/// Closed raw-byte inventory of both committed proof generations, including
/// their distinct historical/full-runtime transcripts and source declarations.
pub const ARTIFACTS: &[(&str, &str)] = &[
    (
        "tests/fixtures/tops-operation-proof/LICENSE-OpenZeppelin",
        "20aebc68b11c063133aa2af0ef4bb29875477c6d16d715718f0daec563938b84",
    ),
    (
        "tests/fixtures/tops-operation-proof/LICENSE-execution-specs",
        "f81577cb97c59ed748f9117fa839b80e3be0ec605a4b66c234e07468bc011808",
    ),
    (
        "tests/fixtures/tops-operation-proof/README.md",
        "50a4a0590b99b9f6133f44b9974076915fb252cb919bdca0928e4d532a27a369",
    ),
    (
        "tests/fixtures/tops-operation-proof/TOPSCleanup.sol",
        "1053ef43a3dd61504efe5e7aa22aa8438f68ac98fb0cff16b445a07ab50f3cc3",
    ),
    (
        "tests/fixtures/tops-operation-proof/capture.json",
        "bed5d7155189f849140fce44c4971a526a139b40b1ac4cc7d83db48b057a4c1a",
    ),
    (
        "tests/fixtures/tops-operation-proof/compiler-version.txt",
        "df9e0e873736454fea76f295ae7f597b21a06ef9db8e44f20465ec7e2522521a",
    ),
    (
        "tests/fixtures/tops-operation-proof/execution-spec-block.py",
        "299ca6ed3ad79e3a3b2c8992744e84bb70c781beb1af0804a7dc4bfdab1e9801",
    ),
    (
        "tests/fixtures/tops-operation-proof/full-input.json",
        "666be4712e615d75784570368d854c72d5d226fe0a5e6961a12b419094eb2aa2",
    ),
    (
        "tests/fixtures/tops-operation-proof/full-output.json",
        "653ee0d4f9ac33899d6bcc828f44e18eba517e02882bf09b39114fd3b0d73f7d",
    ),
    (
        "tests/fixtures/tops-operation-proof/harness-extraction.json",
        "27028cd594be2312f4cde32cc6e595e9010cef1e575c1ed9c34867ae9ee5d037",
    ),
    (
        "tests/fixtures/tops-operation-proof/harness-input.json",
        "5fe1baa7be5a236d3e20436c2c1e4ec4224bee312358e9e7c886b6feab08eacf",
    ),
    (
        "tests/fixtures/tops-operation-proof/harness-output.json",
        "d250f4f254f7bb50f8c1fb25995e6ba9d154278f510b1ffbfb7858d339b58956",
    ),
    (
        "tests/fixtures/tops-operation-proof/metadata-fresh.json",
        "86dc8f7a5c56839386bcf34de679f0c6a0cf938ad2c611f6436724a7bd200cda",
    ),
    (
        "tests/fixtures/tops-operation-proof/metadata-original.json",
        "86dc8f7a5c56839386bcf34de679f0c6a0cf938ad2c611f6436724a7bd200cda",
    ),
    (
        "tests/fixtures/tops-operation-proof/original-input.json",
        "1f6f04eb402b6186484be9e748f08f1f523dee13baa18dfc5635ac0607472df8",
    ),
    (
        "tests/fixtures/tops-operation-proof/primary-sources.json",
        "0e5c56672de6090bf1bb2d404a7d59151b2e2690c236f88aad1dd6c48176340e",
    ),
    (
        "tests/fixtures/tops-operation-proof/settings.json",
        "9b4f4951f0782e9a503ccab777f429b90602ce0a4db247cf11a0556a27c02801",
    ),
    (
        "tests/fixtures/tops-operation-proof/solc-list.json",
        "22e8ba1c7c8d0fc5eb60964083b238d267ea2c1afec9757521fea20c45b206af",
    ),
    (
        "tests/fixtures/tops-runtime-cleanup-proof/LICENSE-execution-specs",
        "f81577cb97c59ed748f9117fa839b80e3be0ec605a4b66c234e07468bc011808",
    ),
    (
        "tests/fixtures/tops-runtime-cleanup-proof/README.md",
        "bbeec4cad7b15aea21dfa802a41dd0c4b8b5274e0b30e6307e345b92444f0931",
    ),
    (
        "tests/fixtures/tops-runtime-cleanup-proof/control_flow.py",
        "cfd4781581c0f65d3a25534f8a4eb8cd2e749aa7d54c337ef4a89752fa894448",
    ),
    (
        "tests/fixtures/tops-runtime-cleanup-proof/environment.py",
        "a01c01b4266eed06bff799ef5886f86d7103ee8599f4e7fcba5b11a9b631669d",
    ),
    (
        "tests/fixtures/tops-runtime-cleanup-proof/historical-compiler-inventory.json",
        "a3d755532cdc9c868b2941943e6e42dcc0c469062843d73781e3246c0c456c92",
    ),
    (
        "tests/fixtures/tops-runtime-cleanup-proof/historical-compiler-report.json",
        "e473e5c6284bcc36ee0f325f356912c940eaa91dca29dd3c1006df3f6fde265a",
    ),
    (
        "tests/fixtures/tops-runtime-cleanup-proof/system.py",
        "32bc20f15831feb8327cc6770f6b963d56abfc996a36bc1b49655a8917ab85df",
    ),
    (
        "docs/evidence/tops-operation-proof-20260929-cases.json",
        "5924c7998907b8a60c5a1ca69ab641aa0332fb36ab77138eb91d76c893248008",
    ),
    (
        "docs/evidence/tops-operation-proof-20260929-compiler.json",
        "e473e5c6284bcc36ee0f325f356912c940eaa91dca29dd3c1006df3f6fde265a",
    ),
    (
        "docs/evidence/tops-operation-proof-20260929-source-inputs.json",
        "a3d755532cdc9c868b2941943e6e42dcc0c469062843d73781e3246c0c456c92",
    ),
    (
        "docs/evidence/tops-operation-proof-20260929-transcripts.json",
        "f4f792d1ee0ad68e8d510e11ab2166d13d1f2d495ae32d5e2a817d1086e2011f",
    ),
    (
        "docs/evidence/tops-operation-proof-20260929.json",
        "e93877adf19cafa0cf36353364160f82a5ce13975c458ee9347fb2fdf278fab3",
    ),
    (
        "docs/evidence/tops-runtime-cleanup-proof-20260929-cases.json",
        "a2d0d1b5898cc501cd4ea90c330e7c5a615ab05e7db7c271934c34be5e07932a",
    ),
    (
        "docs/evidence/tops-runtime-cleanup-proof-20260929-preparation.json",
        "b9230e2c3f3b80e394fa3a2e43c400a9284a729523ecb7a7376fd5cd87950de3",
    ),
    (
        "docs/evidence/tops-runtime-cleanup-proof-20260929-source-inputs.json",
        "3496b348d03085d28aeeaa445b3aca9be81b3aa774dbae159e4380eb122bdcb1",
    ),
    (
        "docs/evidence/tops-runtime-cleanup-proof-20260929-transcripts.json",
        "6ea61762e277c6f0b345bdf8b5fe6c86ec13d15d91b3ba73d3de04882686be02",
    ),
    (
        "docs/evidence/tops-runtime-cleanup-proof-20260929.json",
        "e30bf492887a71cd69343939b7ba6467a75a3778b8019da2d2335f32e5eedbff",
    ),
];
pub const LIMITS: &str = "NOT-QUALIFIED metadata admission for the exact selected TOPS runtime only. At most six LPInfo records before and after each single-frame append or expired-prefix cleanup. Equality omissions require independently observed or earlier accepted block-local facts. Whole omitted operations and deletions indistinguishable from another fully grounded valid operation cannot be detected from storage effects alone. The producer must faithfully supply changed persisted effects. Independent or extra lpAmount root32 writes refuse. Other historical field permissions retain their existing limitations; this is not global router-path refusal. No current runtime, deployment, creation, caller authorization, external dependency, package or global-holder qualification.";
pub fn root(n: u64) -> String {
    format!("0x{n:064x}")
}
pub fn candidate(baseline: &[u8]) -> Result<Value> {
    ensure!(proof::sha(baseline) == BASELINE, "complete unchanged431 baseline");
    let rows: Vec<Value> = serde_json::from_slice(baseline)?;
    ensure!(rows.len() == 431, "historical cohort size");
    let selected: Vec<_> = rows.iter().filter(|v| v["contract"] == CONTRACT).collect();
    ensure!(selected.len() == 1, "exact TOPS profile");
    let original = selected[0];
    ensure!(
        original["balance_slot"] == root(5) && original["code_hash"] == proof::RUNTIME,
        "exact balance/runtime"
    );
    ensure!(
        original["other_mapping_slots"] == json!([6, 8, 17, 18, 19, 26, 27, 29, 30, 32].map(root)),
        "exact historical mapping permissions"
    );
    ensure!(
        original["other_slots"] == json!([0, 1, 2, 3, 4, 7, 9, 10, 11, 12, 13, 14, 15, 16, 20, 21, 22, 23, 24, 25, 33, 34].map(root)),
        "exact historical scalar permissions"
    );
    ensure!(original["address_lists"] == json!([root(28)]), "preserved address-list root28");
    ensure!(original.get("lpinfo_array").is_none(), "no preexisting LPInfo permission");
    let mut selected = original.clone();
    let removed = selected["other_mapping_slots"].as_array_mut().context("mapping roots")?.pop();
    ensure!(removed == Some(json!(root(32))), "remove only last broad root32");
    selected["lpinfo_array"] = json!(erc20_balances::layout::TOPS_LPINFO_SEMANTICS);
    let mut restored = selected.clone();
    restored.as_object_mut().unwrap().remove("lpinfo_array");
    restored["other_mapping_slots"].as_array_mut().unwrap().push(json!(root(32)));
    ensure!(restored == *original, "every unrelated field round trips exactly");
    let result = json!([selected]);
    erc20_balances::layout::parse(&result.to_string()).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(result)
}
pub fn verify_candidate(baseline: &[u8], candidate_value: &Value) -> Result<()> {
    ensure!(
        candidate(baseline)? == *candidate_value,
        "only exact TOPS bounded operation replacement allowed"
    );
    Ok(())
}
pub fn verify_cache(package: &Path, original_package: &Path) -> Result<()> {
    let raw = fs::read(original_package.join(CACHE))?;
    proof::verify_capture(&raw)?;
    ensure!(
        raw == fs::read(package.join("tests/fixtures/tops-operation-proof/capture.json"))?,
        "original complete source capture identity"
    );
    Ok(())
}

pub fn verify_artifacts(package: &Path) -> Result<Value> {
    let mut out = Vec::new();
    for (path, digest) in ARTIFACTS {
        let raw = fs::read(package.join(path))?;
        ensure!(proof::sha(&raw) == *digest, "exact frozen TOPS artifact {path}");
        out.push(json!({"path":path,"sha256":digest,"bytes":raw.len()}));
    }
    Ok(json!(out))
}
fn read(package: &Path, path: &str) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(package.join(path))?)?)
}
const OLD: &str = "docs/evidence/tops-operation-proof-20260929";
const NEW: &str = "docs/evidence/tops-runtime-cleanup-proof-20260929";
/// Reconcile all historical preparation/compiler/operation links to the raw
/// committed bytes. These remain historical proofs, not new compiler runs.
pub fn verify_report_links(package: &Path, compiler: &Value, old: &Value, preparation: &Value, report: &Value) -> Result<()> {
    let old_inventory = fs::read(package.join(format!("{OLD}-source-inputs.json")))?;
    let new_inventory = fs::read(package.join(format!("{NEW}-source-inputs.json")))?;
    ensure!(
        serde_json::from_slice::<Value>(&old_inventory)?["files"]
            .as_object()
            .context("old inventory")?
            .len()
            == 214,
        "complete old214 inventory"
    );
    ensure!(
        serde_json::from_slice::<Value>(&new_inventory)?["files"]
            .as_object()
            .context("new inventory")?
            .len()
            == 291,
        "complete runtime291 inventory"
    );
    for value in [compiler, old, preparation, report] {
        ensure!(
            value["status"] == "passed" && value["qualified"] == false && value["chain_calls"] == 0,
            "successful offline proof only"
        );
    }
    ensure!(
        compiler["source_inventory_sha256"] == proof::sha(&old_inventory)
            && old["source_inventory_sha256"] == proof::sha(&old_inventory)
            && old["compiler_source_inventory_sha256"] == proof::sha(&old_inventory)
            && old["source_trees_equal"] == true,
        "old compiler/executor inventory identity"
    );
    ensure!(
        preparation["source_inventory_sha256"] == proof::sha(&new_inventory)
            && report["source_inventory_sha256"] == proof::sha(&new_inventory)
            && report["preparation_source_inventory_sha256"] == proof::sha(&new_inventory)
            && report["source_trees_equal"] == true,
        "runtime preparation/executor inventory identity"
    );
    let compiler_raw = fs::read(package.join(format!("{OLD}-compiler.json")))?;
    for value in [preparation, report] {
        ensure!(
            value["historical_compiler_inventory_sha256"] == proof::sha(&old_inventory)
                && value["historical_compiler_report_sha256"] == proof::sha(&compiler_raw),
            "historical provenance remains separate"
        );
    }
    ensure!(
        fs::read(package.join("tests/fixtures/tops-runtime-cleanup-proof/historical-compiler-report.json"))? == compiler_raw
            && fs::read(package.join("tests/fixtures/tops-runtime-cleanup-proof/historical-compiler-inventory.json"))? == old_inventory,
        "copied historical raw identities"
    );
    ensure!(
        report["source_report_sha256"] == proof::sha(&fs::read(package.join(format!("{NEW}-preparation.json")))?),
        "runtime report links exact preparation"
    );
    let mut combined = std::collections::BTreeMap::new();
    for (path, _) in ARTIFACTS.iter().filter(|(p, _)| p.starts_with("tests/fixtures/") && !p.ends_with("/README.md")) {
        let raw = fs::read(package.join(path))?;
        let name = Path::new(path).file_name().context("basename")?.to_str().context("UTF8")?;
        let entry = json!({"sha256":proof::sha(&raw),"bytes":raw.len()});
        if let Some(previous) = combined.insert(name, entry.clone()) {
            ensure!(previous == entry, "duplicate artifact basename bytes");
        }
    }
    ensure!(
        preparation["artifacts"] == json!(combined) && report["source_artifacts"] == preparation["artifacts"],
        "complete runtime combined artifact closure"
    );
    ensure!(
        compiler["artifacts"].as_array().context("compiler artifacts")?.len() == 16 && old["source_artifacts"] == compiler["artifacts"],
        "old exact compiler/executor artifacts"
    );
    for artifact in compiler["artifacts"].as_array().unwrap() {
        let name = artifact["file"].as_str().context("artifact name")?;
        ensure!(
            artifact["sha256"] == combined.get(name).context("artifact exists")?["sha256"],
            "raw compiler artifact link {name}"
        );
    }
    for (prefix, value, call_count, compact_count) in [(OLD, old, 605, 49), (NEW, report, 400, 138)] {
        let cases_raw = fs::read(package.join(format!("{prefix}-cases.json")))?;
        let compact_raw = fs::read(package.join(format!("{prefix}-transcripts.json")))?;
        ensure!(
            value["cases_inventory_sha256"] == proof::sha(&cases_raw) && value["transcripts_sha256"] == proof::sha(&compact_raw),
            "exact case/transcript links"
        );
        let cases: Vec<Value> = serde_json::from_slice(&cases_raw)?;
        let compact: Vec<Value> = serde_json::from_slice(&compact_raw)?;
        ensure!(
            value["calls"] == call_count && cases.len() == call_count && value["compact_transcripts"] == compact_count && compact.len() == compact_count,
            "exact proof counts"
        );
        let mut files = std::collections::BTreeMap::new();
        for entry in &cases {
            ensure!(files.insert(entry["file"].as_str().context("casefile")?, entry).is_none(), "unique case file");
        }
        let mut seen = std::collections::BTreeSet::new();
        for record in &compact {
            let entry = &record["full_trace_artifact"];
            let file = entry["file"].as_str().context("compact casefile")?;
            ensure!(
                seen.insert(file) && files.get(file).copied() == Some(entry),
                "compact matches complete original case inventory"
            );
            ensure!(record["execution"]["exit"] == entry["exit"], "compact source outcome identity");
        }
    }
    Ok(())
}
pub fn review(package: &Path) -> Result<Value> {
    verify_compiled_settings(&fs::read(package.join("tests/fixtures/tops-operation-proof/settings.json"))?)?;
    let artifacts = verify_artifacts(package)?;
    let (capture, _, _, _) = proof::verify_source_directory(&package.join("tests/fixtures/tops-operation-proof"))?;
    let specs = crate::tops_runtime_proof::specs(&package.join("tests/fixtures/tops-runtime-cleanup-proof"))?;
    let compiler = read(package, &format!("{OLD}-compiler.json"))?;
    let old = read(package, &format!("{OLD}.json"))?;
    let preparation = read(package, &format!("{NEW}-preparation.json"))?;
    let report = read(package, &format!("{NEW}.json"))?;
    verify_report_links(package, &compiler, &old, &preparation, &report)?;
    let profile = candidate(&fs::read(package.join("tests/fixtures/bsc-refined450-layouts.json"))?)?;
    Ok(
        json!({"qualified":false,"network_requests":0,"contract":CONTRACT,"code_hash":proof::RUNTIME,"capture_sha256":proof::CAPTURE_SHA,"source_sha256":proof::SOURCE_SHA,
        "template":erc20_balances::layout::TOPS_LPINFO_SEMANTICS,"record_bound_before_and_after":LIMIT,
        "historical_baseline_sha256":BASELINE,"candidate":profile,"frozen_artifacts":artifacts,
        "source_gap":proof::PRIMARY_GAP,"exact_primary_dependencies":8,"oz_pin":proof::OZ_PIN,
        "saved_source_count":capture["sources"].as_object().unwrap().len(),"compiler_version":proof::SOLC_VERSION,
        "compiler_run":"reuses raw pinned historical full compiler output and declared immutable reconstruction; no new compilation",
        "creation_admission":false,"authorization_qualification":false,"external_dependency_qualification":false,
        "root32_independent_admission":false,"whole_router_path_qualification":false,
        "original_runtime_cleanup_proof_report_sha256":proof::sha(&fs::read(package.join(format!("{NEW}.json")))?),
        "historical_append_proof_report_sha256":proof::sha(&fs::read(package.join(format!("{OLD}.json")))?),
        "specifications":specs,"limits":LIMITS}),
    )
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Counts {
    pub validated_appends: u64,
    pub validated_cleanups: u64,
    pub observed_length_decrements: u64,
    pub observed_metadata_stores: u64,
    pub observed_equal_metadata_stores: u64,
}
/// Diagnostics are produced only after the complete mapper accepts the block.
/// Count changed head/credit witnesses, never infer an operation from a partial
/// prefix. The mapper independently requires all logical stages to be grounded.
pub fn project_counted(
    block: &substreams_ethereum::pb::eth::v2::Block,
    layouts: &[erc20_balances::layout::VerifiedLayout],
) -> Result<(proto::pb::evm::balances::v1::Events, Counts)> {
    use primitive_types::U256;
    use std::collections::BTreeSet;
    use substreams_ethereum::pb::eth::v2::TransactionTraceStatus;
    ensure!(layouts.len() == 1 && layouts[0].lpinfo_array, "exact selected diagnostic layout");
    let events = erc20_balances::project(block, layouts).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let account = hex::decode(&CONTRACT[2..])?;
    let calls: Vec<_> = block
        .transaction_traces
        .iter()
        .filter(|tx| tx.status() == TransactionTraceStatus::Succeeded)
        .flat_map(|tx| &tx.calls)
        .chain(&block.system_calls)
        .filter(|call| !call.state_reverted)
        .collect();
    let mut heads = BTreeSet::new();
    let mut credits = BTreeSet::new();
    let mut array_words = BTreeSet::new();
    // Match mapper discovery: reverted hints may identify a key, while only
    // successful/non-reverted storage below contributes operation counters.
    for call in block
        .system_calls
        .iter()
        .chain(block.transaction_traces.iter().flat_map(|tx| &tx.calls))
        .filter(|call| call.address == account || call.storage_changes.iter().any(|r| r.address == account))
    {
        for value in call.keccak_preimages.values() {
            let p = hex::decode(value.trim_start_matches("0x"))?;
            if p.len() != 64 || p[..12] != [0; 12] || p[32..] != hex::decode(&root(31)[2..])? {
                continue;
            }
            let head = erc20_balances::hash(&p);
            heads.insert(head.to_vec());
            let mut credit = p;
            credit[63] = 32;
            credits.insert(erc20_balances::hash(&credit).to_vec());
            let base = U256::from_big_endian(&erc20_balances::hash(&head));
            for offset in 0..(LIMIT * 3) {
                let mut key = [0; 32];
                base.overflowing_add(U256::from(offset)).0.to_big_endian(&mut key);
                array_words.insert(key.to_vec());
            }
        }
    }
    let mut counts = Counts::default();
    for row in calls.into_iter().flat_map(|c| &c.storage_changes).filter(|r| r.address == account) {
        if heads.contains(&row.key) || credits.contains(&row.key) || array_words.contains(&row.key) {
            ensure!(row.old_value.len() <= 32 && row.new_value.len() <= 32, "counter word width");
            let old = U256::from_big_endian(&row.old_value);
            let new = U256::from_big_endian(&row.new_value);
            counts.observed_metadata_stores += 1;
            counts.observed_equal_metadata_stores += u64::from(old == new);
            if heads.contains(&row.key) {
                if new > old {
                    counts.validated_appends += 1;
                } else {
                    counts.observed_length_decrements += 1;
                }
            } else if credits.contains(&row.key) {
                counts.validated_cleanups += 1;
            }
        }
    }
    Ok((events, counts))
}
