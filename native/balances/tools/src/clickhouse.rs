//! Offline check of both ClickHouse paths of the native package in `clickhouse local`.
//!
//! Native sink path: the sink's own tables come from the DDL captured after
//! `make setup` on a real server, and the committed `schema.*.sql` files add
//! the legacy native tables, the bridge views and the base-data TTL. The
//! native map's output for captured Extended blocks is written the way
//! `substreams sink clickhouse` writes it: `_blocks_` before `Balance` in every
//! flush, markers only for blocks with output, unprefixed block hashes and `0x`
//! addresses.
//!
//! Legacy SQL sink path: the package's `db_out` (host build) turns the same
//! blocks into `DatabaseChanges`, whose rows go into a second database created
//! from the package's legacy schema.
//!
//! Both databases' legacy tables are compared with values computed here from
//! the same blocks and with each other, and the native sink's tables again
//! after the TTL expires its base rows.
//!
//! This does not stream, run the packaged WASM, or run either sink.
use anyhow::{bail, ensure, Context, Result};
use clap::Args;
use native_balances::{project, Params};
use prost::Message;
use proto::pb::evm::balances::v1 as pb;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use substreams::pb::substreams::Clock;
use substreams_ethereum::pb::eth::v2 as eth;

/// Bar intervals of `mv_historical_native_balances`, in minutes.
pub const INTERVALS: [u32; 8] = [1, 5, 10, 30, 60, 240, 1440, 10080];
const DATABASE: &str = "native";
/// The database the legacy SQL sink path writes, from the package's schema.
const LEGACY_DATABASE: &str = "legacy";

#[derive(Args, Debug)]
pub struct ClickhouseBridge {
    /// Directories of captured `<height>.pb` Extended blocks; may be repeated.
    #[arg(long, required = true)]
    pub blocks: Vec<PathBuf>,
    #[arg(long, default_value = "5", value_delimiter = ',')]
    pub producer_versions: Vec<i32>,
    /// `system.tables` rows captured after `make setup` (JSONEachRow); supplies
    /// the sink's `Balance` and `_blocks_` DDL.
    #[arg(long, default_value = "native/balances/clickhouse/evidence/setup-native-sink-tables.jsonl")]
    pub setup_tables: PathBuf,
    /// Directory of the committed `schema.*.sql` files.
    #[arg(long, default_value = "native/balances/clickhouse")]
    pub schema_dir: PathBuf,
    #[arg(long, default_value_t = 7)]
    pub ttl_days: u32,
    /// Blocks per sink flush.
    #[arg(long, default_value_t = 100)]
    pub flush_blocks: usize,
    #[arg(long, default_value = "clickhouse")]
    pub clickhouse: String,
    #[arg(long)]
    pub output: PathBuf,
}

/// One block's native map output and clock.
#[derive(Clone, Debug)]
pub struct Reduced {
    pub number: u64,
    /// Lowercase hex without `0x`, as the sink stores `_blocks_.hash`.
    pub hash: String,
    pub timestamp: u32,
    /// `(0x address, amount)` in output order.
    pub rows: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct Latest {
    pub block_num: u32,
    pub block_hash: String,
    pub balance: String,
}

/// One `historical_native_balances` row, keyed by `(interval_min, window start, address)`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Bar {
    pub min_block_num: u32,
    pub max_block_num: u32,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub transactions: u64,
}

pub type BarKey = (u32, u32, String);

/// Exact `UInt256` order for canonical decimal strings.
fn numeric_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

/// What the legacy tables must contain after these blocks, in block order.
pub fn expected(blocks: &[Reduced]) -> (BTreeMap<String, Latest>, BTreeMap<BarKey, Bar>) {
    let (mut latest, mut bars) = (BTreeMap::new(), BTreeMap::<BarKey, Bar>::new());
    for block in blocks {
        let number = block.number as u32;
        for (address, amount) in &block.rows {
            latest.insert(
                address.clone(),
                Latest {
                    block_num: number,
                    block_hash: format!("0x{}", block.hash),
                    balance: amount.clone(),
                },
            );
            for interval in INTERVALS {
                let window = block.timestamp / (interval * 60) * interval * 60;
                bars.entry((interval, window, address.clone()))
                    .and_modify(|bar| {
                        bar.max_block_num = number;
                        bar.close = amount.clone();
                        if numeric_cmp(amount, &bar.high).is_gt() {
                            bar.high = amount.clone();
                        }
                        if numeric_cmp(amount, &bar.low).is_lt() {
                            bar.low = amount.clone();
                        }
                        bar.transactions += 1;
                    })
                    .or_insert_with(|| Bar {
                        min_block_num: number,
                        max_block_num: number,
                        open: amount.clone(),
                        high: amount.clone(),
                        low: amount.clone(),
                        close: amount.clone(),
                        transactions: 1,
                    });
            }
        }
    }
    (latest, bars)
}

/// The sink's rows for one flush, as JSONEachRow: `(_blocks_, Balance)`.
pub fn sink_rows(flush: &[Reduced], version: &mut i64) -> (String, String) {
    let (mut markers, mut rows) = (String::new(), String::new());
    for block in flush.iter().filter(|b| !b.rows.is_empty()) {
        *version += 1;
        let marker = json!({"number": block.number, "hash": block.hash, "timestamp": block.timestamp, "version": *version, "deleted": false});
        writeln!(markers, "{marker}").unwrap();
        for (row_id, (address, amount)) in block.rows.iter().enumerate() {
            let row = json!({
                "_block_number_": block.number, "_block_timestamp_": block.timestamp, "_version_": *version, "_deleted_": false,
                "_row_id_": row_id, "contract": "", "address": address, "amount": amount,
            });
            writeln!(rows, "{row}").unwrap();
        }
    }
    (markers, rows)
}

fn reduce(path: &Path, params: &Params) -> Result<Reduced> {
    let block = eth::Block::decode(fs::read(path)?.as_slice()).with_context(|| format!("decoding {}", path.display()))?;
    let timestamp = block
        .header
        .as_ref()
        .and_then(|h| h.timestamp.as_ref())
        .context("block without timestamp")?
        .seconds;
    let events = project(&block, params).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(Reduced {
        number: block.number,
        hash: hex::encode(&block.hash),
        timestamp: u32::try_from(timestamp)?,
        rows: events
            .balances
            .iter()
            .map(|b| (format!("0x{}", hex::encode(&b.address)), b.amount.clone()))
            .collect(),
    })
}

/// The legacy SQL sink's rows for one flush: the native `db_out` run on each
/// block, every table change as one JSONEachRow line of its fields, by table.
pub fn legacy_rows(flush: &[Reduced]) -> Result<BTreeMap<String, String>> {
    let mut tables: BTreeMap<String, String> = BTreeMap::new();
    for block in flush {
        let events = pb::Events {
            balances: block
                .rows
                .iter()
                .map(|(address, amount)| {
                    Ok(pb::Balance {
                        contract: None,
                        address: hex::decode(address.trim_start_matches("0x"))?,
                        amount: amount.clone(),
                    })
                })
                .collect::<Result<_>>()?,
        };
        let clock = Clock {
            id: block.hash.clone(),
            number: block.number,
            timestamp: Some(prost_types::Timestamp {
                seconds: i64::from(block.timestamp),
                nanos: 0,
            }),
        };
        let changes = native_balances_db_out::db_out("hex".into(), clock, events).map_err(|e| anyhow::anyhow!("db_out: {e}"))?;
        for change in changes.table_changes {
            let row: serde_json::Map<String, Value> = change.fields.into_iter().map(|f| (f.name, Value::String(f.new_value))).collect();
            writeln!(tables.entry(change.table).or_default(), "{}", Value::Object(row))?;
        }
    }
    Ok(tables)
}

/// `CREATE TABLE` of a sink table as captured, moved to `database`, without
/// the TTL that `schema.5.ttl.sql` adds.
pub(crate) fn sink_ddl(setup_tables: &Path, table: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Row {
        name: String,
        create_table_query: String,
    }
    for line in fs::read_to_string(setup_tables)?.lines() {
        let row: Row = serde_json::from_str(line)?;
        if row.name == table {
            let (head, rest) = row.create_table_query.split_once(&format!(".{table} ")).context("unexpected DDL")?;
            ensure!(head.starts_with("CREATE TABLE "), "unexpected DDL for {table}");
            let rest = match (rest.find(" TTL "), rest.find(" SETTINGS ")) {
                (Some(ttl), Some(settings)) if ttl < settings => format!("{}{}", &rest[..ttl], &rest[settings..]),
                _ => rest.to_string(),
            };
            return Ok(format!("CREATE TABLE {DATABASE}.{table} {rest};"));
        }
    }
    bail!("{table} not in {}", setup_tables.display())
}

struct Local<'a> {
    binary: &'a str,
    path: PathBuf,
    ttl_days: u32,
}

impl Local<'_> {
    fn run(&self, args: &[&str]) -> Result<String> {
        let output = Command::new(self.binary)
            .arg("local")
            .arg("--path")
            .arg(&self.path)
            .arg(format!("--param_ttl_days={}", self.ttl_days))
            // UInt64 `transactions` as a JSON number, not a quoted string.
            .arg("--output_format_json_quote_64bit_integers=0")
            .arg("--multiquery")
            .args(args)
            .stdin(Stdio::null())
            .output()
            .with_context(|| format!("running {} local", self.binary))?;
        ensure!(output.status.success(), "clickhouse local failed: {}", String::from_utf8_lossy(&output.stderr));
        Ok(String::from_utf8(output.stdout)?)
    }

    fn file(&self, path: &Path) -> Result<String> {
        self.run(&["--queries-file", path.to_str().context("non-UTF-8 path")?])
    }

    fn query(&self, sql: &str) -> Result<String> {
        self.run(&["--query", sql])
    }

    fn rows<T: for<'de> Deserialize<'de>>(&self, sql: &str) -> Result<Vec<T>> {
        self.query(&format!("{sql} FORMAT JSONEachRow"))?
            .lines()
            .map(|l| Ok(serde_json::from_str(l)?))
            .collect()
    }
}

#[derive(Deserialize)]
struct LatestRow {
    address: String,
    #[serde(flatten)]
    latest: Latest,
}

#[derive(Deserialize)]
struct BarRow {
    interval_min: u32,
    ts: u32,
    address: String,
    #[serde(flatten)]
    bar: Bar,
}

#[derive(Deserialize, PartialEq, Debug)]
struct BlockRow {
    block_num: u32,
    block_hash: String,
    ts: u32,
}

/// The legacy tables as read back from ClickHouse.
#[derive(PartialEq)]
struct Tables {
    latest: BTreeMap<String, Latest>,
    bars: BTreeMap<BarKey, Bar>,
    blocks: BTreeMap<u32, BlockRow>,
}

impl Tables {
    fn read(local: &Local, database: &str) -> Result<Self> {
        let latest = local
            .rows::<LatestRow>(&format!(
                "SELECT address, block_num, block_hash, toString(balance) AS balance FROM {database}.native_balances FINAL"
            ))?
            .into_iter()
            .map(|r| (r.address, r.latest))
            .collect();
        let bars = local
            .rows::<BarRow>(&format!(
                "SELECT interval_min, toUInt32(timestamp) AS ts, address, min_block_num, max_block_num, toString(open) AS open, \
                 toString(high) AS high, toString(low) AS low, toString(close) AS close, transactions FROM {database}.historical_native_balances"
            ))?
            .into_iter()
            .map(|r| ((r.interval_min, r.ts, r.address), r.bar))
            .collect();
        let blocks = local
            .rows::<BlockRow>(&format!("SELECT block_num, block_hash, toUInt32(timestamp) AS ts FROM {database}.blocks"))?
            .into_iter()
            .map(|r| (r.block_num, r))
            .collect();
        Ok(Tables { latest, bars, blocks })
    }
}

/// Differences between expected and actual maps: missing, extra, unequal keys.
fn diff<K: Ord + Clone + std::fmt::Debug, V: PartialEq>(expected: &BTreeMap<K, V>, actual: &BTreeMap<K, V>) -> Value {
    let missing: Vec<_> = expected.keys().filter(|k| !actual.contains_key(*k)).cloned().collect();
    let extra: Vec<_> = actual.keys().filter(|k| !expected.contains_key(*k)).cloned().collect();
    let unequal: Vec<_> = expected
        .iter()
        .filter(|(k, v)| actual.get(*k).is_some_and(|a| a != *v))
        .map(|(k, _)| k.clone())
        .collect();
    json!({
        "expected": expected.len(), "actual": actual.len(),
        "missing": missing.len(), "extra": extra.len(), "unequal": unequal.len(),
        "first_differences": missing.iter().chain(&extra).chain(&unequal).take(10).map(|k| format!("{k:?}")).collect::<Vec<_>>(),
    })
}

fn digest(value: &impl serde::Serialize) -> Result<String> {
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(value)?)))
}

pub fn run(args: ClickhouseBridge) -> Result<bool> {
    ensure!(
        !args.output.exists(),
        "output directory exists; use a fresh directory: {}",
        args.output.display()
    );
    ensure!(args.flush_blocks > 0, "--flush-blocks must be positive");
    fs::create_dir_all(args.output.join("flushes"))?;
    let params = Params {
        producer_versions: args.producer_versions.clone(),
    };

    let (mut blocks, mut refused) = (Vec::new(), Vec::new());
    for path in crate::replay::enumerate(&args.blocks)?.values() {
        match reduce(path, &params) {
            Ok(block) => blocks.push(block),
            Err(e) => refused.push(json!({"file": path.display().to_string(), "reason": format!("{e:#}")})),
        }
    }
    ensure!(!blocks.is_empty(), "no block reduced");
    let (latest, bars) = expected(&blocks);
    let markers: BTreeMap<u32, BlockRow> = blocks
        .iter()
        .filter(|b| !b.rows.is_empty())
        .map(|b| {
            (
                b.number as u32,
                BlockRow {
                    block_num: b.number as u32,
                    block_hash: format!("0x{}", b.hash),
                    ts: b.timestamp,
                },
            )
        })
        .collect();

    // Schema: the sink's tables, then every committed schema file in order.
    let mut setup = format!(
        "CREATE DATABASE {DATABASE};\n{}\n{}\nUSE {DATABASE};\n",
        sink_ddl(&args.setup_tables, "_blocks_")?,
        sink_ddl(&args.setup_tables, "Balance")?
    );
    let mut schema_files: Vec<PathBuf> = fs::read_dir(&args.schema_dir)?.map(|e| e.map(|e| e.path())).collect::<Result<_, _>>()?;
    schema_files.retain(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.strip_prefix("schema.").is_some_and(|rest| rest.ends_with(".sql")))
    });
    schema_files.sort();
    let mut schema_hashes = BTreeMap::new();
    for file in &schema_files {
        let sql = fs::read_to_string(file)?;
        schema_hashes.insert(
            file.file_name().unwrap().to_string_lossy().into_owned(),
            hex::encode(Sha256::digest(sql.as_bytes())),
        );
        setup.push_str(&sql);
        setup.push('\n');
    }
    fs::write(args.output.join("setup.sql"), &setup)?;

    // Loads: one file pair per flush, `_blocks_` first, as the sink orders them.
    let (mut load, mut version, mut base_rows) = (String::new(), 1_790_000_000_000_000_000i64, 0usize);
    for (index, flush) in blocks.chunks(args.flush_blocks).enumerate() {
        let (marker_rows, balance_rows) = sink_rows(flush, &mut version);
        base_rows += balance_rows.lines().count();
        for (table, rows) in [("_blocks_", marker_rows), ("Balance", balance_rows)] {
            let file = args.output.join("flushes").join(format!("{index:05}-{table}.jsonl"));
            fs::write(&file, rows)?;
            writeln!(load, "INSERT INTO {DATABASE}.{table} FROM INFILE '{}' FORMAT JSONEachRow;", file.display())?;
        }
    }
    fs::write(args.output.join("load.sql"), &load)?;

    // The legacy SQL sink path: db_out's rows into the package's legacy schema.
    let mut legacy = format!(
        "CREATE DATABASE {LEGACY_DATABASE};\nUSE {LEGACY_DATABASE};\n{}\n",
        crate::db_out_parity::legacy_schema(&args.schema_dir)?
    );
    for (index, flush) in blocks.chunks(args.flush_blocks).enumerate() {
        for (table, rows) in legacy_rows(flush)? {
            let file = args.output.join("flushes").join(format!("{index:05}-legacy-{table}.jsonl"));
            fs::write(&file, rows)?;
            writeln!(
                legacy,
                "INSERT INTO {LEGACY_DATABASE}.{table} FROM INFILE '{}' FORMAT JSONEachRow;",
                file.display()
            )?;
        }
    }
    fs::write(args.output.join("legacy.sql"), &legacy)?;

    // ClickHouse discards rows already past the TTL at insert; the views still
    // receive them. Only rows within the retention stay in `Balance`.
    let cutoff = unix_now()? - i64::from(args.ttl_days) * 86_400;
    let within_ttl: usize = blocks.iter().filter(|b| i64::from(b.timestamp) >= cutoff).map(|b| b.rows.len()).sum();

    let local = Local {
        binary: &args.clickhouse,
        path: args.output.join("data"),
        ttl_days: args.ttl_days,
    };
    let version_string = local.query("SELECT version()")?.trim().to_string();
    local.file(&args.output.join("setup.sql"))?;
    local.file(&args.output.join("load.sql"))?;
    local.file(&args.output.join("legacy.sql"))?;

    let count = |table: &str| -> Result<u64> { Ok(local.query(&format!("SELECT count() FROM {DATABASE}.{table}"))?.trim().parse()?) };

    let actual = Tables::read(&local, DATABASE)?;
    let base_before = count("Balance")?;
    let ddl = local.query(&format!(
        "SELECT create_table_query FROM system.tables WHERE database = '{DATABASE}' AND name = 'Balance' FORMAT TSVRaw"
    ))?;
    let ttl_clause = format!("TTL _block_timestamp_ + toIntervalDay(_CAST({}, 'UInt32'))", args.ttl_days);
    // Force the TTL merge; the legacy tables must not change.
    local.query(&format!("OPTIMIZE TABLE {DATABASE}.Balance FINAL"))?;
    let base_after = count("Balance")?;
    let unexpired: u64 = local
        .query(&format!(
            "SELECT count() FROM {DATABASE}.Balance WHERE _block_timestamp_ < now() - INTERVAL {} DAY",
            args.ttl_days
        ))?
        .trim()
        .parse()?;
    let after = Tables::read(&local, DATABASE)?;
    let legacy_tables = Tables::read(&local, LEGACY_DATABASE)?;
    let legacy_diffs = json!({
        "native_balances": diff(&latest, &legacy_tables.latest),
        "historical_native_balances": diff(&bars, &legacy_tables.bars),
        "blocks_table": diff(&markers, &legacy_tables.blocks),
    });
    let paths_equal = legacy_tables == actual;

    let latest_diff = diff(&latest, &actual.latest);
    let bars_diff = diff(&bars, &actual.bars);
    let markers_diff = diff(&markers, &actual.blocks);
    let clean = |d: &Value| d["missing"] == 0 && d["extra"] == 0 && d["unequal"] == 0;
    let unchanged_by_ttl = after == actual;
    let legacy_clean = ["native_balances", "historical_native_balances", "blocks_table"]
        .iter()
        .all(|k| clean(&legacy_diffs[*k]));
    let passed = legacy_clean
        && paths_equal
        && clean(&latest_diff)
        && clean(&bars_diff)
        && clean(&markers_diff)
        && ddl.contains(&ttl_clause)
        && base_before as usize == within_ttl
        && base_after as usize == within_ttl
        && unexpired == 0
        && unchanged_by_ttl;

    let report = json!({
        "status": if passed { "passed" } else { "failed" },
        "tool": "native-balances-tools clickhouse-bridge",
        "clickhouse_version": version_string,
        "blocks": {
            "reduced": blocks.len(), "first": blocks.first().map(|b| b.number), "last": blocks.last().map(|b| b.number),
            "with_output": markers.len(), "refused": refused,
            "files_sha256": digest(&blocks.iter().map(|b| (b.number, &b.hash)).collect::<Vec<_>>())?,
        },
        "producer_versions": args.producer_versions,
        "setup_tables_sha256": hex::encode(Sha256::digest(fs::read(&args.setup_tables)?)),
        "schema_files_sha256": schema_hashes,
        "flush_blocks": args.flush_blocks,
        "native_rows": base_rows,
        "accounts": latest.len(),
        "native_balances": latest_diff,
        "historical_native_balances": bars_diff,
        "blocks_table": markers_diff,
        "legacy_db_out_path": {
            "tables": legacy_diffs,
            "equal_to_native_sink_path": paths_equal,
        },
        "ttl": {
            "days": args.ttl_days,
            "clause_present": ddl.contains(&ttl_clause),
            "base_rows_inserted": base_rows, "base_rows_before_merge": base_before, "base_rows_after_merge": base_after,
            "base_rows_within_ttl": within_ttl,
            "rows_past_ttl_after_merge": unexpired,
            "legacy_tables_unchanged_by_ttl": unchanged_by_ttl,
        },
        "digests": {
            "native_balances": digest(&actual.latest.iter().collect::<Vec<_>>())?,
            "historical_native_balances": digest(&actual.bars.iter().map(|((i, t, a), b)| (i, t, a, b.min_block_num, b.max_block_num, &b.open, &b.high, &b.low, &b.close, b.transactions)).collect::<Vec<_>>())?,
        },
        "limits": "Writes the native map's and db_out's host output the way each sink does and runs the committed SQL in clickhouse local. It does not stream, run the packaged WASM or either sink, or check balances against RPC.",
    });
    fs::write(args.output.join("report.json"), serde_json::to_string_pretty(&report)? + "\n")?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(passed)
}

fn unix_now() -> Result<i64> {
    Ok(i64::try_from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs())?)
}
