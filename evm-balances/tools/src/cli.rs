use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use serde::Serialize;

use crate::{
    compare::{self, Check, Comparison},
    corpus,
    spkg::{sha256_hex, Package},
};

/// Compare a legacy evm-balances ClickHouse package with the deployed
/// substreams-evm v0.3.4 package: wiring, sink config, schema and `db_out`
/// outcomes on the same inputs.
#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "spkg/reference/evm-clickhouse-balances-v0.3.4.spkg")]
    reference: PathBuf,
    #[arg(long, default_value = "spkg/evm-clickhouse-balances-v0.3.4-extended.spkg")]
    candidate: PathBuf,
    /// Repository root holding the committed fixtures.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Extra native `map_events` recordings (`substreams run -o jsonl --bytes-encoding hex`).
    #[arg(long)]
    native_events: Vec<PathBuf>,
    /// Extra directories of Extended `<height>.pb` blocks, reduced by the native map.
    #[arg(long)]
    blocks: Vec<PathBuf>,
    /// Locally built `db_evm_balances.wasm` expected to equal the candidate's `db_out`.
    #[arg(long)]
    built_wasm: Option<PathBuf>,
    /// Fresh directory for `report.json`.
    #[arg(long)]
    output: PathBuf,
}

#[derive(Serialize)]
struct Report {
    status: &'static str,
    tool: &'static str,
    reference: Artifact,
    candidate: Artifact,
    built_wasm: Option<Check>,
    artifact_checks: Vec<Check>,
    corpora: Vec<Corpus>,
    refused_blocks: Vec<(String, String)>,
    limits: &'static str,
}

#[derive(Serialize)]
struct Artifact {
    path: String,
    spkg_sha256: String,
    db_out_wasm_sha256: String,
}

#[derive(Serialize)]
struct Corpus {
    name: String,
    /// SHA-256 of a recorded input file, binding the run to that recording.
    #[serde(skip_serializing_if = "Option::is_none")]
    input_sha256: Option<String>,
    comparison: Comparison,
}

fn artifact(path: &Path, package: &Package) -> Result<Artifact> {
    let db_out = package.module(compare::DB_OUT)?;
    Ok(Artifact {
        path: path.display().to_string(),
        spkg_sha256: sha256_hex(&fs::read(path)?),
        db_out_wasm_sha256: sha256_hex(package.binary(db_out)?),
    })
}

pub fn run() -> Result<bool> {
    let args = Args::parse();
    if args.output.exists() {
        bail!("output directory {} already exists; use a fresh one", args.output.display());
    }
    let (reference, candidate) = (Package::read(&args.reference)?, Package::read(&args.candidate)?);
    let (reference_db, candidate_db) = (compare::db_out(&reference)?, compare::db_out(&candidate)?);

    let artifact_checks = compare::artifact_checks(&reference, &candidate)?;
    let built_wasm = match &args.built_wasm {
        Some(path) => {
            let built = sha256_hex(&fs::read(path).with_context(|| format!("reading {}", path.display()))?);
            let packaged = sha256_hex(candidate.binary(candidate.module(compare::DB_OUT)?)?);
            Some(Check {
                name: format!("candidate db_out equals {}", path.display()),
                passed: built == packaged,
                reference: built,
                candidate: packaged,
            })
        }
        None => None,
    };

    let mut corpora = vec![
        Corpus {
            name: "committed BSC block 122260950 with RPC ERC-20 rows".into(),
            input_sha256: None,
            comparison: compare::compare(&reference_db, &candidate_db, &[corpus::fixture_case(&args.root)?])?,
        },
        Corpus {
            name: "synthetic edge cases".into(),
            input_sha256: None,
            comparison: compare::compare(&reference_db, &candidate_db, &corpus::synthetic_cases())?,
        },
    ];
    for path in &args.native_events {
        let cases = corpus::jsonl_cases(path)?;
        corpora.push(Corpus {
            name: format!("{} (synthetic clocks)", path.display()),
            input_sha256: Some(sha256_hex(&fs::read(path)?)),
            comparison: compare::compare(&reference_db, &candidate_db, &cases)?,
        });
    }
    let mut refused_blocks = Vec::new();
    for dir in &args.blocks {
        let (cases, refused) = corpus::block_dir_cases(dir)?;
        refused_blocks.extend(refused.into_iter().map(|(p, e)| (p.display().to_string(), e)));
        corpora.push(Corpus {
            name: dir.display().to_string(),
            input_sha256: None,
            comparison: compare::compare(&reference_db, &candidate_db, &cases)?,
        });
    }

    let passed =
        artifact_checks.iter().all(|c| c.passed) && built_wasm.as_ref().is_none_or(|c| c.passed) && corpora.iter().all(|c| c.comparison.differing.is_empty());
    let report = Report {
        status: if passed { "passed" } else { "failed" },
        tool: "evm-balances-tools",
        reference: artifact(&args.reference, &reference)?,
        candidate: artifact(&args.candidate, &candidate)?,
        built_wasm,
        artifact_checks,
        corpora,
        refused_blocks,
        limits: "Executes both packaged db_out WASM binaries on identical inputs in an interpreter implementing the four Substreams host imports; \
                 it does not run the Substreams engine, the native or ERC-20 maps' RPC behaviour, or the SQL sink.",
    };
    fs::create_dir_all(&args.output)?;
    fs::write(args.output.join("report.json"), serde_json::to_string_pretty(&report)? + "\n")?;
    println!("{} → {}", report.status, args.output.join("report.json").display());
    Ok(passed)
}
