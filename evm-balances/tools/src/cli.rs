use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use serde::Serialize;

use crate::{
    compare::{self, Check, Comparison, Erc20Input},
    corpus,
    coverage::{self, Coverage},
    pipeline::{self, Replay},
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
    /// Extra directories of Extended `<height>.pb` blocks, reduced by the native
    /// map (and the ERC-20 map with `--erc20-layouts`).
    #[arg(long)]
    blocks: Vec<PathBuf>,
    /// Layouts of a candidate whose ERC-20 input is the Extended map (the
    /// no-RPC package). Without it the candidate must keep the RPC modules.
    #[arg(long)]
    erc20_layouts: Option<PathBuf>,
    /// Directory of Extended blocks to replay through the candidate's packaged
    /// native and ERC-20 maps and both `db_out`s (requires `--erc20-layouts`).
    #[arg(long, requires = "erc20_layouts")]
    pipeline_blocks: Option<PathBuf>,
    /// The engine's recorded ERC-20 `map_events` output for those blocks.
    #[arg(long, requires = "pipeline_blocks")]
    engine_erc20_events: Option<PathBuf>,
    /// RPC reference ERC-20 recordings (`substreams run -o jsonl`) to measure
    /// how much of their activity the `--erc20-layouts` tokens cover.
    #[arg(long, requires = "erc20_layouts")]
    rpc_reference: Vec<PathBuf>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pipeline: Option<PipelineReport>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    coverage: Vec<CoverageReport>,
    limits: &'static str,
}

#[derive(Serialize)]
struct CoverageReport {
    rpc_reference: String,
    rpc_reference_sha256: String,
    coverage: Coverage,
}

#[derive(Serialize)]
struct PipelineReport {
    blocks_dir: String,
    engine_erc20_events: Option<String>,
    engine_erc20_events_sha256: Option<String>,
    replay: Replay,
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

    let layouts = args.erc20_layouts.as_deref().map(corpus::read_layouts).transpose()?;
    let erc20 = match &layouts {
        Some(l) => Erc20Input::Extended { layouts: l.json.clone() },
        None => Erc20Input::Reference,
    };
    let artifact_checks = compare::artifact_checks(&reference, &candidate, &erc20)?;
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
    if let Some(layouts) = &layouts {
        let case = corpus::extended_fixture_case(&args.root, &layouts.parsed)?;
        corpora.push(Corpus {
            name: "committed BSC block 122260950 with Extended ERC-20 rows".into(),
            input_sha256: None,
            comparison: compare::compare(&reference_db, &candidate_db, &[case])?,
        });
    }
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
        let (cases, refused) = corpus::block_dir_cases(dir, layouts.as_ref().map(|l| l.parsed.as_slice()))?;
        refused_blocks.extend(refused.into_iter().map(|(p, e)| (p.display().to_string(), e)));
        corpora.push(Corpus {
            name: dir.display().to_string(),
            input_sha256: None,
            comparison: compare::compare(&reference_db, &candidate_db, &cases)?,
        });
    }

    let pipeline = match (&args.pipeline_blocks, &layouts) {
        (Some(dir), Some(layouts)) => Some(PipelineReport {
            blocks_dir: dir.display().to_string(),
            engine_erc20_events: args.engine_erc20_events.as_ref().map(|p| p.display().to_string()),
            engine_erc20_events_sha256: args.engine_erc20_events.as_ref().map(fs::read).transpose()?.map(|b| sha256_hex(&b)),
            replay: pipeline::replay(
                &reference,
                &candidate,
                &corpus::sorted_pb(dir)?,
                &layouts.parsed,
                args.engine_erc20_events.as_deref(),
            )?,
        }),
        _ => None,
    };

    let mut coverage = Vec::new();
    if let Some(layouts) = &layouts {
        let configured = coverage::configured_contracts(&layouts.json)?;
        for path in &args.rpc_reference {
            coverage.push(CoverageReport {
                rpc_reference: path.display().to_string(),
                rpc_reference_sha256: sha256_hex(&fs::read(path)?),
                coverage: coverage::measure(&corpus::read_jsonl_events(path)?, &configured),
            });
        }
    }

    let passed = artifact_checks.iter().all(|c| c.passed)
        && built_wasm.as_ref().is_none_or(|c| c.passed)
        && corpora.iter().all(|c| c.comparison.differing.is_empty())
        && pipeline.as_ref().is_none_or(|p| p.replay.passed());
    let report = Report {
        status: if passed { "passed" } else { "failed" },
        tool: "evm-balances-tools",
        reference: artifact(&args.reference, &reference)?,
        candidate: artifact(&args.candidate, &candidate)?,
        built_wasm,
        artifact_checks,
        corpora,
        refused_blocks,
        pipeline,
        coverage,
        limits: "Executes both packaged db_out WASM binaries on identical inputs in an interpreter implementing the four Substreams host imports; \
                 it does not run the Substreams engine, the native or ERC-20 maps' RPC behaviour, or the SQL sink.",
    };
    fs::create_dir_all(&args.output)?;
    fs::write(args.output.join("report.json"), serde_json::to_string_pretty(&report)? + "\n")?;
    println!("{} → {}", report.status, args.output.join("report.json").display());
    Ok(passed)
}
