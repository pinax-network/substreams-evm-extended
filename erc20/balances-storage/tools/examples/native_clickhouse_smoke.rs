//! Bounded native-CLI integration check. The CLI is the only database writer.
#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    native::main()
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use anyhow::{ensure, Context, Result};
    use clap::Parser;
    use erc20_balances_storage_tools::data::sha256;
    use serde_json::{json, Value};
    use std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    const START: u64 = 122288006;
    const SPLIT: u64 = 122288020;
    const STOP: u64 = 122288150;

    #[derive(Parser)]
    struct Args {
        #[arg(long, default_value = "spkg/erc20-balances-storage-v0.1.0.spkg")]
        package: PathBuf,
        #[arg(long, default_value = "erc20/balances-storage/tests/fixtures/bridge450/layouts.json")]
        layouts: PathBuf,
        #[arg(long, default_value = "erc20/balances-storage/tests/fixtures/bridge450/cases.json")]
        expected: PathBuf,
        /// A fresh directory; failed attempts remain available here.
        #[arg(long)]
        output: PathBuf,
        /// Must be an unused erc20_storage_smoke_* database on localhost.
        #[arg(long)]
        database: Option<String>,
        #[arg(long, default_value = "substreams")]
        substreams: PathBuf,
        #[arg(long, default_value = "bsc.substreams.pinax.network:443")]
        endpoint: String,
    }

    fn read(path: &Path) -> Result<Value> {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }
    fn save(path: &Path, value: &Value) -> Result<()> {
        Ok(fs::write(path, serde_json::to_vec_pretty(value)?)?)
    }
    fn string(v: &Value) -> Result<&str> {
        v.as_str().context("expected string")
    }
    fn number(v: &Value) -> Result<u64> {
        v.as_u64().map(Ok).unwrap_or_else(|| Ok(string(v)?.parse()?))
    }

    struct Local {
        database: String,
        user: String,
        password: String,
    }
    impl Local {
        fn query(&self, sql: &str) -> Result<Vec<Value>> {
            ensure!(sql.starts_with("SELECT "), "validation is read-only");
            let body = ureq::post("http://127.0.0.1:8123/")
                .query("readonly", "1")
                .set("X-ClickHouse-User", &self.user)
                .set("X-ClickHouse-Key", &self.password)
                .timeout(Duration::from_secs(20))
                .send_string(&format!("{sql} FORMAT JSONEachRow"))
                .map_err(|_| anyhow::anyhow!("local ClickHouse read failed"))?
                .into_string()?;
            body.lines().map(|line| Ok(serde_json::from_str(line)?)).collect()
        }
        fn rows(&self) -> Result<Vec<Value>> {
            self.query(&format!("SELECT _block_number_ AS block, _row_id_ AS row, contract, address, amount, toString(_version_) AS version FROM {}.Balance FINAL WHERE NOT _deleted_ ORDER BY block, row", self.database))
        }
        fn count(&self) -> Result<u64> {
            number(&self.query(&format!("SELECT count() AS n FROM {}.Balance", self.database))?[0]["n"])
        }
    }

    fn sanitize(mut content: String, dsn: &str) -> String {
        content = content.replace(dsn, "<local-dsn-redacted>");
        for key in ["SUBSTREAMS_API_KEY", "SUBSTREAMS_API_TOKEN", "CH_PASSWORD"] {
            if let Ok(secret) = std::env::var(key) {
                if !secret.is_empty() {
                    content = content.replace(&secret, "<redacted>");
                }
            }
        }
        content
    }

    fn command(args: &Args, dsn: &str, phase: &str, flags: &[String]) -> Result<()> {
        let log_path = args.output.join(format!("{phase}.log"));
        let log = fs::File::create(&log_path)?;
        let mut child = Command::new(&args.substreams)
            .args(flags)
            .env("SUBSTREAMS_SINK_DSN", dsn)
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()?;
        let start = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if start.elapsed() > Duration::from_secs(240) {
                child.kill()?;
                let _ = child.wait();
                fs::write(&log_path, sanitize(fs::read_to_string(&log_path)?, dsn))?;
                anyhow::bail!("{phase} timed out; inspect preserved log");
            }
            std::thread::sleep(Duration::from_millis(200));
        };
        fs::write(&log_path, sanitize(fs::read_to_string(&log_path)?, dsn))?;
        ensure!(status.success(), "{phase} failed; inspect preserved log");
        Ok(())
    }

    fn setup(args: &Args, dsn: &str, state: &Path, phase: &str) -> Result<()> {
        fs::create_dir_all(state.join("meta"))?;
        command(
            args,
            dsn,
            phase,
            &[
                "sink".into(),
                "clickhouse".into(),
                "setup".into(),
                args.package.to_string_lossy().into_owned(),
                "map_events".into(),
                "--bytes-encoding".into(),
                "0xhex".into(),
                "--sink-info-folder".into(),
                state.join("meta").to_string_lossy().into_owned(),
            ],
        )
    }

    fn ingest(args: &Args, dsn: &str, state: &Path, phase: &str, stop: u64) -> Result<()> {
        let params = serde_json::to_string(&read(&args.layouts)?)?;
        command(
            args,
            dsn,
            phase,
            &[
                "sink".into(),
                "clickhouse".into(),
                args.package.to_string_lossy().into_owned(),
                "map_events".into(),
                "-e".into(),
                args.endpoint.clone(),
                "-p".into(),
                format!("map_events={params}"),
                "-s".into(),
                START.to_string(),
                "-t".into(),
                stop.to_string(),
                "--final-blocks-only".into(),
                "--bytes-encoding".into(),
                "0xhex".into(),
                "--sink-info-folder".into(),
                state.join("meta").to_string_lossy().into_owned(),
                "--cursor-file-path".into(),
                state.join("cursor.txt").to_string_lossy().into_owned(),
                "--spool-dir".into(),
                state.join("spool").to_string_lossy().into_owned(),
                "--spool-max-size".into(),
                "64MiB".into(),
                "--spool-max-idle".into(),
                "100ms".into(),
                "--decode-batch-size".into(),
                "1".into(),
                "--max-retries".into(),
                "2".into(),
                "--prometheus-addr".into(),
                "127.0.0.1:0".into(),
            ],
        )
    }

    type Expected = BTreeMap<(u64, String, String), String>;
    fn expected(cases: &Value, stop: u64) -> Result<Expected> {
        let mut result = BTreeMap::new();
        for case in cases.as_array().context("expected fixture array")? {
            let block = number(&case["block"])?;
            if block < START || block >= stop {
                continue;
            }
            for row in case["balances"].as_array().context("expected balances")? {
                ensure!(
                    result
                        .insert(
                            (block, string(&case["contract"])?.into(), string(&row["address"])?.into()),
                            string(&row["rpc"])?.into()
                        )
                        .is_none(),
                    "duplicate fixture row"
                );
            }
        }
        Ok(result)
    }
    fn compare(rows: &[Value], expected: &Expected) -> Result<()> {
        let mut actual = BTreeMap::new();
        for row in rows {
            ensure!(
                actual
                    .insert(
                        (number(&row["block"])?, string(&row["contract"])?.into(), string(&row["address"])?.into()),
                        string(&row["amount"])?.into()
                    )
                    .is_none(),
                "duplicate logical balance"
            );
        }
        ensure!(&actual == expected, "native ClickHouse rows differ from independent captured RPC expectations");
        Ok(())
    }

    fn run(args: &Args, report: &mut Value) -> Result<()> {
        ensure!(args.package.is_file(), "build the new package first");
        ensure!(
            sha256(&args.layouts)? == "385c9c1aab972d2bc9f153b09018e03a09d525352e37b67e56aa4ab27b428b38"
                && sha256(&args.expected)? == "752e113c3c7f4ad16022f75343950aa3a2d8576c42282ce6eee7d7ca2735b37c",
            "this smoke check requires the exact reviewed bridge450 layouts and RPC fixtures"
        );
        let version = Command::new(&args.substreams).arg("--version").output()?;
        ensure!(version.status.success(), "cannot identify native CLI");
        let cli_version = String::from_utf8(version.stdout)?.trim().to_owned();
        ensure!(
            cli_version.contains("1.22.0") || cli_version.contains("Commit be35ad3"),
            "this integration check is qualified for CLI1.22.0"
        );
        let db = args
            .database
            .clone()
            .unwrap_or_else(|| format!("erc20_storage_smoke_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        ensure!(
            db.starts_with("erc20_storage_smoke_") && db.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
            "isolated smoke database name required"
        );
        let local = Local {
            database: db.clone(),
            user: std::env::var("CH_USER").unwrap_or_else(|_| "default".into()),
            password: std::env::var("CH_PASSWORD").unwrap_or_default(),
        };
        ensure!(
            !local.user.contains([':', '@', '/', '?', '#']) && !local.password.contains(['@', '/', '?', '#']),
            "local DSN credentials require URL-safe characters"
        );
        let server = local.query("SELECT version() AS version")?;
        ensure!(
            number(&local.query(&format!("SELECT count() AS n FROM system.databases WHERE name = '{db}'"))?[0]["n"])? == 0,
            "database already exists; refusing to touch it"
        );
        let dsn = format!("clickhouse://{}:{}@127.0.0.1:9000/{db}", local.user, local.password);
        let cases = read(&args.expected)?;
        let wanted = expected(&cases, STOP)?;
        ensure!(
            wanted.len() == 7 && expected(&cases, SPLIT)?.len() == 5,
            "smoke expectations require reviewed bridge450 fixtures"
        );
        report["database"] = json!(db);
        report["substreams_version"] = json!(cli_version);
        report["clickhouse_version"] = server[0]["version"].clone();
        report["package_sha256"] = json!(sha256(&args.package)?);
        report["layouts_sha256"] = json!(sha256(&args.layouts)?);
        report["expected_sha256"] = json!(sha256(&args.expected)?);
        save(&args.output.join("report.json"), report)?;
        let state = args.output.join("resume-state");
        setup(args, &dsn, &state, "01-setup")?;
        let schema = local.query(&format!(
            "SELECT name, type FROM system.columns WHERE database = '{db}' AND table = 'Balance' ORDER BY position"
        ))?;
        for (name, kind) in [
            ("contract", "String"),
            ("address", "String"),
            ("amount", "String"),
            ("_block_number_", "UInt64"),
            ("_row_id_", "UInt32"),
        ] {
            ensure!(schema.iter().any(|row| row["name"] == name && row["type"] == kind), "unexpected column {name}");
        }
        let tables = local.query(&format!(
            "SELECT name, engine, sorting_key, partition_key, create_table_query FROM system.tables WHERE database = '{db}' ORDER BY name"
        ))?;
        ensure!(
            tables.len() == 2
                && tables
                    .iter()
                    .any(|r| r["name"] == "Balance" && r["engine"] == "ReplacingMergeTree" && r["sorting_key"] == "_block_number_, _row_id_"),
            "unexpected native table/key mapping"
        );
        save(&args.output.join("schema.json"), &json!({"columns":schema,"tables":tables}))?;
        ingest(args, &dsn, &state, "02-first-window", SPLIT)?;
        let first = local.rows()?;
        compare(&first, &expected(&cases, SPLIT)?)?;
        let cursor_before = fs::read(state.join("cursor.txt"))?;
        ensure!(!cursor_before.is_empty(), "first cursor missing");
        save(&args.output.join("first-rows.json"), &json!(first))?;
        ingest(args, &dsn, &state, "03-resume-window", STOP)?;
        let resumed = local.rows()?;
        compare(&resumed, &wanted)?;
        for old in &first {
            ensure!(resumed.iter().any(|row| row == old), "resume rewrote an already applied row");
        }
        let cursor_after = fs::read(state.join("cursor.txt"))?;
        ensure!(!cursor_after.is_empty() && cursor_after != cursor_before, "cursor did not advance on resume");
        save(&args.output.join("resumed-rows.json"), &json!(resumed))?;
        let before_replay_count = local.count()?;
        let replay_state = args.output.join("replay-state");
        setup(args, &dsn, &replay_state, "04-replay-setup")?;
        ingest(args, &dsn, &replay_state, "05-identical-replay", STOP)?;
        let replayed = local.rows()?;
        compare(&replayed, &wanted)?;
        for (new, old) in replayed.iter().zip(&resumed) {
            ensure!(
                string(&new["version"])?.parse::<i64>()? > string(&old["version"])?.parse::<i64>()?,
                "fresh replay did not replace every row"
            );
        }
        let blocks = local.query(&format!("SELECT number, hash FROM {db}._blocks_ FINAL WHERE NOT deleted ORDER BY number"))?;
        ensure!(blocks.len() == cases.as_array().unwrap().len(), "unexpected nonempty-block markers");
        for case in cases.as_array().unwrap() {
            ensure!(
                blocks.iter().any(|b| number(&b["number"]).ok() == number(&case["block"]).ok()
                    && string(&b["hash"]).unwrap().trim_start_matches("0x") == string(&case["hash"]).unwrap().trim_start_matches("0x")),
                "block hash differs from captured RPC fixture"
            );
        }
        save(&args.output.join("replayed-rows.json"), &json!(replayed))?;
        save(&args.output.join("blocks.json"), &json!(blocks))?;
        ensure!(report["package_sha256"] == sha256(&args.package)?, "package changed during smoke run");
        report["status"] = json!("native_clickhouse_parity");
        report["start"] = json!(START);
        report["resume_split"] = json!(SPLIT);
        report["stop_exclusive"] = json!(STOP);
        report["logical_balance_rows"] = json!(wanted.len());
        report["zero_balances"] = json!(wanted.values().filter(|v| *v == "0").count());
        report["maximum_decimal_digits"] = json!(wanted.values().map(String::len).max().unwrap());
        report["multiple_holders_same_block"] = json!(true);
        report["resume_preserved_prior_versions"] = json!(true);
        report["cursor_advanced"] = json!(true);
        report["resume_cursor_sha256"] = json!(sha256(&state.join("cursor.txt"))?);
        report["replay_cursor_sha256"] = json!(sha256(&replay_state.join("cursor.txt"))?);
        report["replay_final_rows_equal"] = json!(true);
        report["physical_rows_before_replay"] = json!(before_replay_count);
        report["physical_rows_after_replay"] = json!(local.count()?);
        report["optional_contract_sql_type"] = json!("String; protobuf presence is not represented as SQL NULL by this native CLI");
        report["native_balance_null_contract_claimed"] = json!(false);
        report["schema_sha256"] = json!(sha256(&args.output.join("schema.json"))?);
        report["scope"] = json!("Two ERC20 contracts; seven independently RPC-audited emitted balances in a144-block finalized window. Native Substreams CLI is the sole writer. Empty-output blocks do not get _blocks_ rows. No holder bootstrap, atomic block-publication, native-coin NULL-presence, or all-token completeness claim. Database and cursor/spool files are retained for review; no existing database was changed.");
        Ok(())
    }

    pub fn main() -> Result<()> {
        let args = Args::parse();
        ensure!(!args.output.exists(), "fresh output directory required");
        fs::create_dir_all(&args.output)?;
        let mut report = json!({"status":"incomplete","tool_language":"Rust","database_writer":"substreams sink clickhouse"});
        let result = run(&args, &mut report);
        if let Err(error) = &result {
            report["error"] = json!(error.to_string());
        }
        save(&args.output.join("report.json"), &report)?;
        result?;
        println!("{}", serde_json::to_string(&report)?);
        Ok(())
    }
}
