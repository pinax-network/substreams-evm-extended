//! `db_out` parity with the deployed substreams-evm package.
//!
//! Runs the deployed `evm-clickhouse-balances-v0.3.4.spkg` `db_out` WASM with
//! an empty ERC-20 input, and the native package's own `db_out` WASM, on the
//! same params, clock and native `Events`. Their outputs (fields compared by
//! name), logs and panic messages must be identical. The package checks bind the native map to the
//! live-qualified build and the SQL sink schema to the verbatim legacy files.
use anyhow::{anyhow, ensure, Context, Result};
use clap::Args;
use native_balances::{parse_params, project};
use prost::Message;
use proto::pb::evm::balances::v1 as pb;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};
use substreams::pb::substreams::Clock;
use substreams_database_change::pb::database::DatabaseChanges;
use substreams_ethereum::pb::eth::v2 as eth;

use crate::{
    spkg::{sha256_hex, Package},
    wasm::{import_modules, MapModule, Outcome},
};

/// WASM of the live-qualified native map (`spkg` sha256 `fbb46fc7…`).
pub const NATIVE_QUALIFIED_WASM_SHA256: &str = "48d89d28d21b972307385c5e973fc4fe55d9160a4606f900a480318becbe6230";
pub const NATIVE_PARAMS: &str = r#"{"producer_versions":[5]}"#;
/// The four verbatim legacy files the SQL sink schema concatenates, in order.
pub const LEGACY_SCHEMA_FILES: [&str; 4] = [
    "schema.0.blocks.sql",
    "schema.1.table.native-balances.sql",
    "schema.2.mv.historical-native-balances.sql",
    "schema.3.view.historical-native-balances.sql",
];
const FIXTURE_BLOCK: &str = "erc20/balances/tests/fixtures/bsc-122260950.pb";

#[derive(Args, Debug)]
pub struct DbOutParity {
    #[arg(long, default_value = "spkg/reference/evm-clickhouse-balances-v0.3.4.spkg")]
    pub reference: PathBuf,
    #[arg(long, default_value = "spkg/native-balances-v0.2.0.spkg")]
    pub candidate: PathBuf,
    /// The live-qualified map-only package whose `map_events` the candidate must carry.
    #[arg(long, default_value = "spkg/native-balances-v0.1.0.spkg")]
    pub qualified: PathBuf,
    /// Repository root holding the committed fixtures and schema files.
    #[arg(long, default_value = ".")]
    pub root: PathBuf,
    /// Native `map_events` recordings (`substreams run -o jsonl --bytes-encoding hex`).
    #[arg(long)]
    pub native_events: Vec<PathBuf>,
    /// Directories of Extended `<height>.pb` blocks, reduced by the native map.
    #[arg(long)]
    pub blocks: Vec<PathBuf>,
    /// Locally built `native_balances_db_out.wasm` expected to equal the packaged one.
    #[arg(long)]
    pub built_wasm: Option<PathBuf>,
    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct Check {
    pub name: String,
    pub passed: bool,
    pub expected: String,
    pub actual: String,
}

fn check(name: &str, expected: impl Into<String>, actual: impl Into<String>) -> Check {
    let (expected, actual) = (expected.into(), actual.into());
    Check {
        name: name.into(),
        passed: expected == actual,
        expected,
        actual,
    }
}

/// The SQL sink schema as `make schema` writes it: a header, then each legacy
/// file followed by a blank line.
pub fn legacy_schema(schema_dir: &Path) -> Result<String> {
    let mut schema = String::from("-- This file is generated. Do not edit.\n\n");
    for file in LEGACY_SCHEMA_FILES {
        schema += &fs::read_to_string(schema_dir.join(file))?;
        schema.push('\n');
    }
    Ok(schema)
}

pub fn package_checks(reference: &Package, candidate: &Package, qualified: &Package, schema_dir: &Path) -> Result<Vec<Check>> {
    let map = candidate.module("map_events")?;
    let db_out = candidate.module("db_out")?;
    let reference_db = reference.module("db_out")?;
    let (sql, reference_sql) = (candidate.sql_service()?, reference.sql_service()?);
    let mut rpc_imports = Vec::new();
    for module in candidate.modules() {
        if import_modules(candidate.binary(module)?)?.contains("rpc") {
            rpc_imports.push(module.name.clone());
        }
    }
    Ok(vec![
        check(
            "map_events wasm is the qualified build",
            NATIVE_QUALIFIED_WASM_SHA256,
            sha256_hex(candidate.binary(map)?),
        ),
        check(
            "map_events equals the qualified package's",
            format!(
                "{:?}",
                qualified
                    .module("map_events")
                    .map(|m| (m.input_descriptions(), m.output_type().to_string(), m.binary_entrypoint.clone()))?
            ),
            format!("{:?}", (map.input_descriptions(), map.output_type().to_string(), map.binary_entrypoint.clone())),
        ),
        check(
            "map_events params",
            format!("params:{NATIVE_PARAMS}"),
            map.input_descriptions().first().cloned().unwrap_or_default(),
        ),
        check(
            "db_out inputs: the reference's without ERC-20",
            format!("{:?}", ["params:hex", "source:sf.substreams.v1.Clock", "map:map_events"]),
            format!("{:?}", db_out.input_descriptions()),
        ),
        check("db_out output type", reference_db.output_type(), db_out.output_type()),
        check(
            "db_out entrypoint and initial block",
            "db_out 0",
            format!("{} {}", db_out.binary_entrypoint, db_out.initial_block),
        ),
        check("db_out has its own binary", "true", (db_out.binary_index != map.binary_index).to_string()),
        check("sink module", &reference.sink_module, &candidate.sink_module),
        check("sink engine", reference_sql.engine.to_string(), sql.engine.to_string()),
        check(
            "sink schema is the verbatim legacy files",
            sha256_hex(legacy_schema(schema_dir)?.as_bytes()),
            sha256_hex(sql.schema.as_bytes()),
        ),
        check("no module imports an RPC host function", "[]", format!("{rpc_imports:?}")),
        check("network", "bsc", &candidate.network),
    ])
}

/// One `db_out` input set; the reference also receives an empty ERC-20 input.
#[derive(Debug, Clone)]
pub struct Case {
    pub name: String,
    pub params: String,
    pub clock: Clock,
    pub native: pb::Events,
}

#[derive(Debug, Default, Serialize)]
pub struct Comparison {
    pub cases: usize,
    pub identical: usize,
    pub differing: Vec<String>,
    pub identical_panics: usize,
    pub empty_outputs: usize,
    pub table_changes: BTreeMap<String, usize>,
    /// SHA-256 over the length-prefixed outputs, in case order.
    pub outputs_sha256: String,
}

pub fn compare(reference: &MapModule, candidate: &MapModule, cases: &[Case]) -> Result<Comparison> {
    let mut summary = Comparison::default();
    let mut digest = Sha256::new();
    let empty_erc20 = pb::Events::default().encode_to_vec();
    for case in cases {
        let (params, clock, native) = (case.params.as_bytes(), case.clock.encode_to_vec(), case.native.encode_to_vec());
        let left = reference.call(&[params, &clock, &native, &empty_erc20])?;
        let right = candidate.call(&[params, &clock, &native])?;
        summary.cases += 1;
        let (left, right) = (normalized(left)?, normalized(right)?);
        if !left.same_as(&right) {
            summary
                .differing
                .push(format!("{}: reference {} / candidate {}", case.name, describe(&left), describe(&right)));
            continue;
        }
        summary.identical += 1;
        summary.identical_panics += usize::from(right.panic.is_some());
        let output = right.output.unwrap_or_default();
        summary.empty_outputs += usize::from(output.is_empty());
        digest.update((output.len() as u64).to_be_bytes());
        digest.update(&output);
        for change in DatabaseChanges::decode(output.as_slice())?.table_changes {
            *summary.table_changes.entry(change.table).or_default() += 1;
        }
    }
    summary.outputs_sha256 = hex::encode(digest.finalize());
    Ok(summary)
}

/// The outcome with each row's fields sorted by name. Changes are already in
/// creation (ordinal) order, but `substreams-database-change` keeps a row's
/// columns in a `HashMap`, whose order follows the hasher state of the
/// instance; the SQL sink maps columns by name.
fn normalized(mut outcome: Outcome) -> Result<Outcome> {
    if let Some(output) = &outcome.output {
        let mut changes = DatabaseChanges::decode(output.as_slice())?;
        for change in &mut changes.table_changes {
            change.fields.sort_by(|a, b| a.name.cmp(&b.name));
        }
        outcome.output = Some(changes.encode_to_vec());
    }
    Ok(outcome)
}

fn describe(outcome: &Outcome) -> String {
    match (&outcome.panic, &outcome.output) {
        (Some(p), _) => format!("panic {:?} at {}:{}", p.message, p.line, p.column),
        (None, Some(o)) => format!("output {} bytes sha256 {}", o.len(), sha256_hex(o)),
        (None, None) => format!("no output (trapped: {})", outcome.trapped),
    }
}

/// Loads a package's `db_out` for execution.
pub fn db_out(package: &Package) -> Result<MapModule> {
    let module = package.module("db_out")?;
    MapModule::new(package.binary(module)?, &module.binary_entrypoint)
}

pub fn clock_of(block: &eth::Block) -> Result<Clock> {
    let header = block.header.as_ref().ok_or_else(|| anyhow!("block {} has no header", block.number))?;
    Ok(Clock {
        id: hex::encode(&block.hash),
        number: block.number,
        timestamp: header.timestamp,
    })
}

pub fn block_case(path: &Path) -> Result<Case> {
    let block = eth::Block::decode(fs::read(path)?.as_slice()).with_context(|| format!("decoding {}", path.display()))?;
    let native =
        project(&block, &parse_params(NATIVE_PARAMS).map_err(|e| anyhow!("{e}"))?).map_err(|e| anyhow!("native map refused block {}: {e}", block.number))?;
    Ok(Case {
        name: path.display().to_string(),
        params: "hex".into(),
        clock: clock_of(&block)?,
        native,
    })
}

pub fn fixture_case(root: &Path) -> Result<Case> {
    block_case(&root.join(FIXTURE_BLOCK))
}

/// A synthetic clock for recordings that carry none: hash = sha256 of the
/// number, 450 ms cadence. Both packages receive the same clock.
pub fn synthetic_clock(number: u64) -> Clock {
    let seconds = 1_790_121_600 + (number % 1_000_000) as i64 * 45 / 100;
    Clock {
        id: hex::encode(Sha256::digest(number.to_be_bytes())),
        number,
        timestamp: Some(prost_types::Timestamp { seconds, nanos: 0 }),
    }
}

#[derive(Deserialize)]
struct JsonlLine {
    #[serde(rename = "@block")]
    block: u64,
    #[serde(rename = "@data", default)]
    data: JsonlEvents,
}

#[derive(Deserialize, Default)]
struct JsonlEvents {
    #[serde(default)]
    balances: Vec<JsonlBalance>,
}

#[derive(Deserialize)]
struct JsonlBalance {
    contract: Option<String>,
    address: String,
    amount: String,
}

fn hex_bytes(value: &str) -> Result<Vec<u8>> {
    let digits = value.strip_prefix("0x").unwrap_or(value);
    ensure!(digits.len() % 2 == 0, "odd-length hex {value:?}");
    Ok(hex::decode(digits)?)
}

/// Native `map_events` recordings, with synthetic clocks.
pub fn jsonl_cases(path: &Path) -> Result<Vec<Case>> {
    let file = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut cases = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line: JsonlLine = serde_json::from_str(&line?).with_context(|| format!("{}:{}", path.display(), index + 1))?;
        let balances = line
            .data
            .balances
            .iter()
            .map(|b| {
                Ok(pb::Balance {
                    contract: b.contract.as_deref().map(hex_bytes).transpose()?,
                    address: hex_bytes(&b.address)?,
                    amount: b.amount.clone(),
                })
            })
            .collect::<Result<_>>()?;
        cases.push(Case {
            name: format!("{}@{}", path.display(), line.block),
            params: "hex".into(),
            clock: synthetic_clock(line.block),
            native: pb::Events { balances },
        });
    }
    Ok(cases)
}

/// Branches real BSC v5 blocks do not reach.
pub fn synthetic_cases() -> Vec<Case> {
    let address = |byte: u8| vec![byte; 20];
    let native = |rows: &[(Vec<u8>, &str)]| pb::Events {
        balances: rows
            .iter()
            .map(|(a, amount)| pb::Balance {
                contract: None,
                address: a.clone(),
                amount: (*amount).into(),
            })
            .collect(),
    };
    let case = |name: &str, params: &str, clock: Clock, events: pb::Events| Case {
        name: name.into(),
        params: params.into(),
        clock,
        native: events,
    };
    let no_timestamp = |number: u64, id: &str| Clock {
        id: id.into(),
        number,
        timestamp: None,
    };
    let max = "115792089237316195423570985008687907853269984665640564039457584007913129639935";

    let mut cases = vec![
        case("empty input", "hex", synthetic_clock(1), pb::Events::default()),
        case("empty params default to hex", "", synthetic_clock(2), native(&[(address(1), "1")])),
        case(
            "zero, uint256 max and unparsed amounts",
            "hex",
            synthetic_clock(3),
            native(&[(address(1), "0"), (address(2), max), (address(3), "not-a-number")]),
        ),
        case(
            "a contract on a native row is ignored",
            "hex",
            synthetic_clock(4),
            pb::Events {
                balances: vec![pb::Balance {
                    contract: Some(address(7)),
                    address: address(1),
                    amount: "5".into(),
                }],
            },
        ),
        case(
            "repeated address within one block",
            "hex",
            synthetic_clock(5),
            native(&[(address(1), "1"), (address(1), "2")]),
        ),
        case(
            "tron base58 with 20, 21 and malformed address lengths",
            "tron_base58",
            synthetic_clock(6),
            native(&[
                (address(1), "1"),
                ([vec![0x41], address(2)].concat(), "2"),
                ([vec![0x42], address(3)].concat(), "3"),
                (vec![4; 19], "4"),
            ]),
        ),
        case("invalid params", "base64", synthetic_clock(7), native(&[(address(1), "1")])),
        case("missing timestamp", "hex", no_timestamp(8, "ab"), native(&[(address(1), "1")])),
        case("missing timestamp without rows", "hex", no_timestamp(9, "ab"), pb::Events::default()),
        case("unknown genesis without timestamp", "hex", no_timestamp(0, "00"), native(&[(address(1), "1")])),
    ];
    for (network, id) in [
        ("Ethereum", "d4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3"),
        ("Arbitrum One", "7ee576b35482195fc49205cec9af72ce14f003b9ae69f6ba0faef4514be8b442"),
        ("Arbitrum Nova", "2ad24e03026118f9b3a48626f0636e38c93660e90a6812e853a99aa8c5371561"),
        ("Boba", "dcd9e6a8f9973eaa62da2874959cb152faeb4fd6929177bd6335a1a16074ef9c"),
    ] {
        cases.push(case(
            &format!("{network} genesis clock"),
            "hex",
            no_timestamp(0, id),
            native(&[(address(1), "1")]),
        ));
    }
    cases
}

pub fn run(args: DbOutParity) -> Result<bool> {
    ensure!(
        !args.output.exists(),
        "output directory exists; use a fresh directory: {}",
        args.output.display()
    );
    let (reference, candidate, qualified) = (
        Package::read(&args.reference)?,
        Package::read(&args.candidate)?,
        Package::read(&args.qualified)?,
    );
    let checks = package_checks(&reference, &candidate, &qualified, &args.root.join("native/balances/clickhouse"))?;
    let built = match &args.built_wasm {
        Some(path) => Some(check(
            "packaged db_out equals the local build",
            sha256_hex(&fs::read(path).with_context(|| format!("reading {}", path.display()))?),
            sha256_hex(candidate.binary(candidate.module("db_out")?)?),
        )),
        None => None,
    };
    let (reference_db, candidate_db) = (db_out(&reference)?, db_out(&candidate)?);

    let mut corpora = vec![
        json!({"name": "committed BSC block 122260950", "comparison": compare(&reference_db, &candidate_db, &[fixture_case(&args.root)?])?}),
        json!({"name": "synthetic edge cases", "comparison": compare(&reference_db, &candidate_db, &synthetic_cases())?}),
    ];
    for path in &args.native_events {
        corpora.push(json!({
            "name": format!("{} (synthetic clocks)", path.display()),
            "input_sha256": sha256_hex(&fs::read(path)?),
            "comparison": compare(&reference_db, &candidate_db, &jsonl_cases(path)?)?,
        }));
    }
    let mut refused = Vec::new();
    for dir in &args.blocks {
        let mut cases = Vec::new();
        for path in crate::replay::enumerate(std::slice::from_ref(dir))?.values() {
            match block_case(path) {
                Ok(case) => cases.push(case),
                Err(e) => refused.push(json!({"file": path.display().to_string(), "reason": format!("{e:#}")})),
            }
        }
        corpora.push(json!({"name": dir.display().to_string(), "comparison": compare(&reference_db, &candidate_db, &cases)?}));
    }

    let passed = checks.iter().all(|c| c.passed)
        && built.as_ref().is_none_or(|c| c.passed)
        && corpora.iter().all(|c| c["comparison"]["differing"].as_array().is_some_and(Vec::is_empty));
    let artifact = |path: &Path, package: &Package| -> Result<serde_json::Value> {
        Ok(json!({
            "path": path.display().to_string(),
            "spkg_sha256": sha256_hex(&fs::read(path)?),
            "db_out_wasm_sha256": sha256_hex(package.binary(package.module("db_out")?)?),
        }))
    };
    let report = json!({
        "status": if passed { "passed" } else { "failed" },
        "tool": "native-balances-tools db-out-parity",
        "reference": artifact(&args.reference, &reference)?,
        "candidate": artifact(&args.candidate, &candidate)?,
        "package_checks": checks,
        "built_wasm": built,
        "corpora": corpora,
        "refused_blocks": refused,
        "limits": "Executes both packaged db_out WASM binaries on identical inputs (the reference with an empty ERC-20 input) in an interpreter implementing the four Substreams host imports. Panic locations are not compared. It does not run the Substreams engine or the SQL sink.",
    });
    fs::create_dir_all(&args.output)?;
    fs::write(args.output.join("report.json"), serde_json::to_string_pretty(&report)? + "\n")?;
    println!("{} → {}", report["status"], args.output.join("report.json").display());
    Ok(passed)
}
