use crate::cli::Cli;
use crate::replay::{run, Replay};
use clap::Parser;
use serde_json::Value;
use std::{fs, path::Path};

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn captured_fixtures_replay_with_index_oracle_checks() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("blocks");
    fs::create_dir(&dir).unwrap();
    let fixtures = root().join("aave/balance-state/tests/fixtures");
    for (file, height) in [
        ("122288220-tx71-ausdt-withdraw.pb", 122288220u64),
        ("122288734-tx81-ausdt-supply.pb", 122288734),
        ("122288932-tx1-ausdt-ausdc-two-reserves.pb", 122288932),
    ] {
        fs::copy(fixtures.join(file), dir.join(format!("{height}.pb"))).unwrap();
    }
    let ok = run(Replay {
        blocks: vec![dir],
        params: fixtures.join("bsc-aave-v3-epochs.json"),
        output: tmp.path().join("out"),
    })
    .unwrap();
    let report: Value = serde_json::from_str(&fs::read_to_string(tmp.path().join("out/report.json")).unwrap()).unwrap();
    assert!(ok, "{report}");
    assert_eq!(report["status"], "replayed");
    assert_eq!(report["replayed_blocks"], 3);
    assert_eq!(report["holder_basis_rows"], 4);
    assert_eq!(report["global_state_rows"], 16);
    assert_eq!(report["index_oracle"]["checks"], 4);
    assert!(report["index_oracle"]["mismatches"].as_array().unwrap().is_empty());
    // Non-adjacent blocks: continuity is skipped, not asserted.
    assert_eq!(report["continuity"]["global_checks"], 0);
    assert!(report["continuity"]["global_skipped_across_gaps"].as_u64().unwrap() > 0);
}

#[test]
fn cli_requires_blocks_params_and_output() {
    assert!(Cli::try_parse_from(["tools", "replay", "--blocks", "x", "--params", "p.json", "--output", "o"]).is_ok());
    assert!(Cli::try_parse_from(["tools", "replay", "--blocks", "x", "--output", "o"]).is_err());
}
