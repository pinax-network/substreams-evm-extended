//! Compares a candidate legacy ClickHouse package with the deployed
//! substreams-evm `evm-clickhouse-balances-v0.3.4.spkg`.
//!
//! The candidate must keep `db_out`'s wiring, the SQL sink config and the
//! schema, replace the native input by the qualified Extended map, and produce
//! byte-identical `db_out` outcomes for the same inputs. Its ERC-20 input is
//! either the reference's RPC modules, unchanged, or the live-run Extended
//! storage-layout map with exactly the expected layouts and no RPC import
//! anywhere in the package.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use prost::Message;
use serde::Serialize;
use sha2::{Digest, Sha256};
use substreams_database_change::pb::database::DatabaseChanges;

use crate::{
    corpus::{Case, NATIVE_PARAMS},
    spkg::{sha256_hex, Module, Package},
    wasm::{import_modules, MapModule, Outcome},
};

/// WASM of the live-qualified native package (`spkg` sha256 `fbb46fc7…`).
pub const NATIVE_QUALIFIED_WASM_SHA256: &str = "48d89d28d21b972307385c5e973fc4fe55d9160a4606f900a480318becbe6230";
/// WASM of the ERC-20 storage-layout package run on live BSC on 2026-09-23
/// (`spkg/erc20-balances-v0.1.0.spkg`, sha256 `532b571f…`).
pub const ERC20_LIVE_WASM_SHA256: &str = "005a2d3d22d10c5c61fa36b00adea1c0b5e9c55536a553c14b9f423d09546099";
pub const DB_OUT: &str = "db_out";
pub const NATIVE_MAP: &str = "db:native_balances:map_events";
pub const ERC20_MAP: &str = "db:erc20_balances:map_events";
pub const ERC20_PREFIX: &str = "db:erc20_balances:";
/// Host module of the Substreams RPC functions (`rpc.eth_call`).
pub const RPC_IMPORT: &str = "rpc";

/// The ERC-20 input a candidate is expected to carry.
pub enum Erc20Input {
    /// The reference's RPC modules, unchanged.
    Reference,
    /// The live-run Extended storage-layout map with exactly these layouts.
    Extended { layouts: serde_json::Value },
}

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

/// Modules whose binary imports an RPC host function.
fn rpc_modules(package: &Package) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for module in package.modules() {
        if import_modules(package.binary(module)?)?.contains(RPC_IMPORT) {
            out.insert(module.name.clone());
        }
    }
    Ok(out)
}

pub fn artifact_checks(reference: &Package, candidate: &Package, erc20: &Erc20Input) -> Result<Vec<Check>> {
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

    let (ref_rpc, cand_rpc) = (rpc_modules(reference)?, rpc_modules(candidate)?);
    match erc20 {
        Erc20Input::Reference => {
            for module in reference.modules().iter().filter(|m| m.name.starts_with(ERC20_PREFIX)) {
                let candidate_summary = candidate
                    .module(&module.name)
                    .map(|m| module_summary(candidate, m))
                    .unwrap_or_else(|e| e.to_string());
                checks.push(same(format!("{} unchanged", module.name), module_summary(reference, module), candidate_summary));
            }
            let ref_erc20_rpc: BTreeSet<String> = ref_rpc.iter().filter(|m| m.starts_with(ERC20_PREFIX)).cloned().collect();
            checks.push(same(
                "RPC imports only in the ERC-20 modules",
                format!("{ref_erc20_rpc:?}"),
                format!("{cand_rpc:?}"),
            ));
        }
        Erc20Input::Extended { layouts } => {
            let map = candidate.module(ERC20_MAP)?;
            let wasm = candidate.binary(map).map(sha256_hex)?;
            checks.push(check(
                "ERC-20 map wasm is the live-run build",
                ERC20_LIVE_WASM_SHA256,
                &wasm,
                wasm == ERC20_LIVE_WASM_SHA256,
            ));
            let kinds: Vec<String> = map
                .input_descriptions()
                .iter()
                .map(|d| d.split(':').next().unwrap_or_default().to_string())
                .collect();
            let params = map
                .input_descriptions()
                .first()
                .and_then(|d| d.strip_prefix("params:"))
                .map(str::to_string)
                .unwrap_or_default();
            let params_match = serde_json::from_str::<serde_json::Value>(&params).ok().as_ref() == Some(layouts);
            checks.push(check(
                "ERC-20 map params equal the expected layouts",
                format!(
                    "{} profiles, sha256 {}",
                    layouts.as_array().map_or(0, Vec::len),
                    sha256_hex(layouts.to_string().as_bytes())
                ),
                format!("{kinds:?}, {} bytes of params, equal: {params_match}", params.len()),
                params_match && kinds == ["params", "source"],
            ));
            checks.push(same("ERC-20 map output type", reference.module(ERC20_MAP)?.output_type(), map.output_type()));
            let others: Vec<&str> = candidate
                .modules()
                .iter()
                .map(|m| m.name.as_str())
                .filter(|n| n.starts_with(ERC20_PREFIX) && *n != ERC20_MAP)
                .collect();
            checks.push(check("no ERC-20 RPC modules", "[]", format!("{others:?}"), others.is_empty()));
            checks.push(check(
                "no module imports an RPC host function",
                format!("reference: {ref_rpc:?}"),
                format!("{cand_rpc:?}"),
                cand_rpc.is_empty(),
            ));
        }
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
