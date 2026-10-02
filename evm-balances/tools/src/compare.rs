//! Compares a candidate legacy ClickHouse package with the deployed
//! substreams-evm `evm-clickhouse-balances-v0.3.4.spkg`.
//!
//! The candidate must keep `db_out`'s wiring, the SQL sink config, the schema
//! and every ERC-20 module of the reference, replace only the native input by
//! the qualified Extended map, and produce byte-identical `db_out` outcomes for
//! the same inputs.

use std::collections::BTreeMap;

use anyhow::Result;
use prost::Message;
use serde::Serialize;
use sha2::{Digest, Sha256};
use substreams_database_change::pb::database::DatabaseChanges;

use crate::{
    corpus::{Case, NATIVE_PARAMS},
    spkg::{sha256_hex, Module, Package},
    wasm::{MapModule, Outcome},
};

/// WASM of the live-qualified native package (`spkg` sha256 `fbb46fc7…`).
pub const NATIVE_QUALIFIED_WASM_SHA256: &str = "48d89d28d21b972307385c5e973fc4fe55d9160a4606f900a480318becbe6230";
pub const DB_OUT: &str = "db_out";
pub const NATIVE_MAP: &str = "db:native_balances:map_events";
pub const ERC20_PREFIX: &str = "db:erc20_balances:";

#[derive(Debug, Serialize)]
pub struct Check {
    pub name: String,
    pub passed: bool,
    pub reference: String,
    pub candidate: String,
}

fn check(name: impl Into<String>, reference: impl Into<String>, candidate: impl Into<String>, passed: bool) -> Check {
    Check {
        name: name.into(),
        passed,
        reference: reference.into(),
        candidate: candidate.into(),
    }
}

fn same(name: impl Into<String>, reference: impl Into<String>, candidate: impl Into<String>) -> Check {
    let (reference, candidate) = (reference.into(), candidate.into());
    let passed = reference == candidate;
    check(name, reference, candidate, passed)
}

fn module_summary(package: &Package, module: &Module) -> String {
    let binary = package.binary(module).map(sha256_hex).unwrap_or_else(|e| e.to_string());
    format!(
        "wasm={binary} entry={} initial={} inputs={:?} output={}",
        module.binary_entrypoint,
        module.initial_block,
        module.input_descriptions(),
        module.output_type()
    )
}

pub fn artifact_checks(reference: &Package, candidate: &Package) -> Result<Vec<Check>> {
    let (ref_db, cand_db) = (reference.module(DB_OUT)?, candidate.module(DB_OUT)?);
    let (ref_sql, cand_sql) = (reference.sql_service()?, candidate.sql_service()?);
    let mut checks = vec![
        same(
            "db_out inputs",
            format!("{:?}", ref_db.input_descriptions()),
            format!("{:?}", cand_db.input_descriptions()),
        ),
        same("db_out output type", ref_db.output_type(), cand_db.output_type()),
        same(
            "db_out entrypoint and initial block",
            format!("{} {}", ref_db.binary_entrypoint, ref_db.initial_block),
            format!("{} {}", cand_db.binary_entrypoint, cand_db.initial_block),
        ),
        same("sink module", &reference.sink_module, &candidate.sink_module),
        same("sink engine", ref_sql.engine.to_string(), cand_sql.engine.to_string()),
        same(
            "sink schema sha256",
            sha256_hex(ref_sql.schema.as_bytes()),
            sha256_hex(cand_sql.schema.as_bytes()),
        ),
        check("network", &reference.network, &candidate.network, candidate.network == "bsc"),
    ];

    for module in reference.modules().iter().filter(|m| m.name.starts_with(ERC20_PREFIX)) {
        let candidate_summary = candidate
            .module(&module.name)
            .map(|m| module_summary(candidate, m))
            .unwrap_or_else(|e| e.to_string());
        checks.push(same(format!("{} unchanged", module.name), module_summary(reference, module), candidate_summary));
    }

    let native = candidate.module(NATIVE_MAP)?;
    let native_wasm = candidate.binary(native).map(sha256_hex)?;
    checks.push(check(
        "native map wasm is the qualified build",
        NATIVE_QUALIFIED_WASM_SHA256,
        &native_wasm,
        native_wasm == NATIVE_QUALIFIED_WASM_SHA256,
    ));
    let expected_inputs = format!("{:?}", [format!("params:{NATIVE_PARAMS}"), "source:sf.ethereum.type.v2.Block".to_string()]);
    let native_inputs = format!("{:?}", native.input_descriptions());
    checks.push(check("native map inputs", &expected_inputs, &native_inputs, native_inputs == expected_inputs));
    checks.push(same(
        "native map output type",
        reference.module(NATIVE_MAP)?.output_type(),
        native.output_type(),
    ));

    let extra: Vec<&str> = candidate
        .modules()
        .iter()
        .map(|m| m.name.as_str())
        .filter(|n| *n != DB_OUT && *n != NATIVE_MAP && !n.starts_with(ERC20_PREFIX))
        .collect();
    checks.push(check("no other modules", "[]", format!("{extra:?}"), extra.is_empty()));
    Ok(checks)
}

/// Loads `db_out` of a package for execution.
pub fn db_out(package: &Package) -> Result<MapModule> {
    let module = package.module(DB_OUT)?;
    MapModule::new(package.binary(module)?, &module.binary_entrypoint)
}

#[derive(Debug, Default, Serialize)]
pub struct Comparison {
    pub cases: usize,
    pub identical: usize,
    pub differing: Vec<String>,
    /// Cases where both packages panicked with the same message and location.
    pub identical_panics: usize,
    pub empty_outputs: usize,
    /// Row changes per table across identical outputs.
    pub table_changes: BTreeMap<String, usize>,
    /// SHA-256 over the length-prefixed outputs, in case order.
    pub outputs_sha256: String,
}

pub fn compare(reference: &MapModule, candidate: &MapModule, cases: &[Case]) -> Result<Comparison> {
    let mut summary = Comparison::default();
    let mut digest = Sha256::new();
    for case in cases {
        let inputs = case.inputs();
        let inputs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
        let (left, right) = (reference.call(&inputs)?, candidate.call(&inputs)?);
        summary.cases += 1;
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

fn describe(outcome: &Outcome) -> String {
    match (&outcome.panic, &outcome.output) {
        (Some(p), _) => format!("panic {:?} at {}:{}", p.message, p.line, p.column),
        (None, Some(o)) => format!("output {} bytes sha256 {}", o.len(), sha256_hex(o)),
        (None, None) => format!("no output (trapped: {})", outcome.trapped),
    }
}
