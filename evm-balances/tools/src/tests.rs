use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use prost::Message;
use substreams_database_change::pb::database::{table_change::PrimaryKey, DatabaseChanges};

use crate::{compare, corpus, spkg::Package};

const REFERENCE: &str = "spkg/reference/evm-clickhouse-balances-v0.3.4.spkg";
const CANDIDATE: &str = "spkg/evm-clickhouse-balances-v0.3.4-extended.spkg";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn packages() -> (Package, Package) {
    (Package::read(&root().join(REFERENCE)).unwrap(), Package::read(&root().join(CANDIDATE)).unwrap())
}

#[test]
fn committed_package_keeps_the_reference_wiring_sink_and_erc20_modules() {
    let (reference, candidate) = packages();
    let checks = compare::artifact_checks(&reference, &candidate).unwrap();
    let failed: Vec<_> = checks.iter().filter(|c| !c.passed).collect();
    assert!(failed.is_empty(), "{failed:#?}");
    // Three ERC-20 modules of the RPC package plus db_out, sink, schema and native checks.
    assert_eq!(checks.iter().filter(|c| c.name.starts_with(compare::ERC20_PREFIX)).count(), 4);
}

/// `make -C evm-balances/clickhouse schema` concatenates `schema.*.sql` in
/// sorted order under a generated header; both packages embed exactly that.
#[test]
fn packaged_schema_is_the_concatenation_of_the_committed_schema_files() {
    let dir = root().join("evm-balances/clickhouse");
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.strip_prefix("schema.").is_some_and(|rest| rest.ends_with(".sql")))
        .collect();
    names.sort();
    assert_eq!(names.len(), 6, "{names:?}");
    let mut expected = String::from("-- This file is generated. Do not edit.\n\n");
    for name in &names {
        expected += &fs::read_to_string(dir.join(name)).unwrap();
        expected.push('\n');
    }
    let (reference, candidate) = packages();
    assert_eq!(reference.sql_service().unwrap().schema, expected);
    assert_eq!(candidate.sql_service().unwrap().schema, expected);
}

#[test]
fn db_out_outcomes_are_identical_on_the_committed_bsc_block() {
    let (reference, candidate) = packages();
    let case = corpus::fixture_case(&root()).unwrap();
    let summary = compare::compare(&compare::db_out(&reference).unwrap(), &compare::db_out(&candidate).unwrap(), &[case]).unwrap();
    assert!(summary.differing.is_empty(), "{:#?}", summary.differing);
    // Not vacuous: 82 native rows (all equal to eth_getBalance) and 18 RPC ERC-20 rows.
    assert_eq!(
        summary.table_changes,
        BTreeMap::from([
            ("blocks".to_string(), 1),
            ("erc20_balances".to_string(), 18),
            ("native_balances".to_string(), 82)
        ])
    );
}

#[test]
fn db_out_outcomes_are_identical_on_synthetic_edge_cases() {
    let (reference, candidate) = packages();
    let cases = corpus::synthetic_cases();
    let summary = compare::compare(&compare::db_out(&reference).unwrap(), &compare::db_out(&candidate).unwrap(), &cases).unwrap();
    assert!(summary.differing.is_empty(), "{:#?}", summary.differing);
    assert_eq!(summary.identical, cases.len());
    // Invalid params, a row without a timestamp and an unknown genesis block panic in both.
    assert_eq!(summary.identical_panics, 3);
}

/// The legacy row shape the existing ClickHouse tables consume, read from the
/// candidate's output for the full captured block.
#[test]
fn native_rows_carry_the_legacy_columns() {
    let (_, candidate) = packages();
    let case = corpus::fixture_case(&root()).unwrap();
    let inputs = case.inputs();
    let inputs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
    let outcome = compare::db_out(&candidate).unwrap().call(&inputs).unwrap();
    let changes = DatabaseChanges::decode(outcome.output.unwrap().as_slice()).unwrap();

    let system = "0x0000000000000000000000000000000000001000";
    let row = changes
        .table_changes
        .iter()
        .find(|c| c.table == "native_balances" && c.primary_key == Some(PrimaryKey::Pk(system.into())))
        .unwrap();
    let fields: BTreeMap<&str, &str> = row.fields.iter().map(|f| (f.name.as_str(), f.new_value.as_str())).collect();
    assert_eq!(
        fields,
        BTreeMap::from([
            ("address", system),
            // Same-block eth_getBalance in bsc-122260950.json.
            ("balance", "954951803286322533679"),
            ("block_hash", "0x00de5f67359c37d39e58dca906906eb8b72b3bc2fce3a087746a3acb95fcbab4"),
            ("block_num", "122260950"),
            ("timestamp", case.clock.timestamp.unwrap().seconds.to_string().as_str()),
        ])
    );
    assert_eq!(changes.table_changes.iter().filter(|c| c.table == "blocks").count(), 1);
}
