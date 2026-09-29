#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    offline::main()
}
#[cfg(not(target_arch = "wasm32"))]
mod offline {
    use anyhow::{ensure, Context, Result};
    use erc20_balances_tools::calculated_retention::{
        binding::{self, Model},
        collect,
        historical::{self, Observation},
        *,
    };
    use prost::Message;
    use serde_json::{json, Value};
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs::{self, File},
        io::{BufWriter, Write as IoWrite},
        path::{Path, PathBuf},
        time::Instant,
    };
    use substreams_ethereum::pb::eth::v2 as eth;
    const BLOCK_MANIFEST: &str = "56397f18ea7371231b261baefb10122c1439e604d130be2cbc97fca598e38ba6";
    fn save(path: &Path, v: &impl serde::Serialize) -> Result<()> {
        fs::write(path, serde_json::to_vec_pretty(v)?)?;
        Ok(())
    }
    fn hexword(v: &Value) -> Result<Word> {
        hex::decode(v.as_str().context("hex word")?.strip_prefix("0x").context("hex prefix")?)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("word width"))
    }
    fn files(path: &Path, out: &mut BTreeSet<PathBuf>) -> Result<()> {
        if path.is_dir() {
            for e in fs::read_dir(path)? {
                files(&e?.path(), out)?;
            }
        } else {
            ensure!(path.is_file(), "missing source {}", path.display());
            out.insert(path.to_owned());
        }
        Ok(())
    }
    fn inventory(repo: &Path) -> Result<Value> {
        let mut paths = BTreeSet::new();
        for path in [
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "erc20/balances/Cargo.toml",
            "erc20/balances/src",
            "erc20/balances/tools/Cargo.toml",
            "erc20/balances/tools/src",
            "common/persist/src",
            "common/retention/src",
            "common/retention/Cargo.toml",
            "proto",
            "erc20/balances/tools/tests/calculated_retention.rs",
            "erc20/balances/tools/tests/calculated_retention_historical.rs",
            "erc20/balances/tools/tests/calculated_retention_replay.rs",
            "erc20/balances/tools/tests/fixtures/calculated-retention",
            "docs/follow-up.md",
            "docs/handoff.md",
            "docs/research/README.md",
            "erc20/balances/docs/role-shape-audit.md",
        ] {
            files(&repo.join(path), &mut paths)?;
        }
        for original in historical::ORIGINALS {
            paths.insert(repo.join(original.path));
        }
        let entries = paths
            .into_iter()
            .map(|p| {
                let raw = fs::read(&p)?;
                Ok(json!({"path":p.strip_prefix(repo)?.to_str().context("source path")?,"bytes":raw.len(),"sha256":binding::sha(&raw)}))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(json!({"schema":1,"files":entries}))
    }
    fn compare(ledgers: &[Ledger], expected: &[Observation], n: u64, writer: &mut BufWriter<File>, counts: &mut BTreeMap<String, u64>) -> Result<()> {
        for o in expected.iter().filter(|o| o.number == n) {
            let l = ledgers.iter().find(|l| l.binding().model == o.model).context("expected model")?;
            let actual = l.evaluate(o.holder)?;
            let amount = value(&o.amount).to_string();
            let okay = actual.at.number == o.number
                && actual.at.hash == o.hash
                && match &actual.outcome {
                    Outcome::Known { amount: a, pending } => {
                        a == &amount && o.pending.is_none_or(|p| pending.as_ref() == Some(&Pending::Known(value(&p).to_string())))
                    }
                    _ => false,
                };
            serde_json::to_writer(
                &mut *writer,
                &json!({"model":o.model,"expected_number":o.number,"expected_hash":o.hash,"holder":o.holder,"expected_amount":amount,"expected_pending":o.pending.map(|p|value(&p).to_string()),"actual":actual,"matches":okay}),
            )?;
            writeln!(writer)?;
            writer.flush()?;
            ensure!(okay, "saved independent getter mismatch at {} for {}", n, o.model.label());
            *counts.entry(o.model.label().into()).or_default() += 1;
        }
        Ok(())
    }
    fn storage_checks(ledgers: &[Ledger], reflection: &[(Model, Value)], n: u64, counts: &mut BTreeMap<String, u64>) -> Result<()> {
        for (model, v) in reflection {
            let l = ledgers.iter().find(|l| l.binding().model == *model).unwrap();
            for s in v["snapshots"].as_array().context("snapshots")?.iter().filter(|s| s["block"] == n) {
                ensure!(hexword(&s["hash"])? == l.at().hash, "storage snapshot hash differs");
                for (key, expected) in s["storage"].as_object().context("snapshot storage")? {
                    let key = hexword(&json!(key))?;
                    let wanted = hexword(expected)?;
                    ensure!(
                        l.facts()
                            .iter()
                            .any(|f| f.slot.contract == l.binding().token() && f.slot.key == key && f.word == wanted),
                        "saved reflection storage differs at {n}"
                    );
                    *counts.entry(model.label().into()).or_default() += 1;
                }
            }
        }
        Ok(())
    }
    fn run(repo: &Path, cache: &Path, out: &Path, report: &mut Value) -> Result<()> {
        snapshots::verify(repo)?;
        let before = inventory(repo)?;
        save(&out.join("source-inputs.json"), &before)?;
        let mut captures = Vec::new();
        for (index, path) in [
            "out/ranks101-150-source-review/0x88886f0fd371dff856291badced45922bc888888.json",
            "out/ranks151-200-source-review/0x5e3cbc82d020be91a989eb747934104e9ab585fe.json",
            "out/ranks351-400-source-review/0xc748673057861a797275cd8a068abb95a902e8de.json",
            "out/ranks351-400-source-review/0x1f64fdad335ed784898effb5ce22d54d8f432523.json",
        ]
        .iter()
        .enumerate()
        {
            let raw = fs::read(cache.join(path))?;
            ensure!(raw == binding::CAPTURES[index].3, "original cache capture differs");
            let identity = binding::verify_capture(&raw, index)?;
            captures.push(json!({"path":path,"bytes":raw.len(),"identity":identity}));
        }
        report["source_capture_cache_bindings"] = json!(captures);
        for file in before["files"].as_array().unwrap() {
            let path = file["path"].as_str().unwrap();
            let dest = out.join("source").join(path);
            fs::create_dir_all(dest.parent().unwrap())?;
            fs::copy(repo.join(path), dest)?;
        }
        let raw = fs::read(cache.join("out/reflection-replay/blocks.json"))?;
        ensure!(binding::sha(&raw) == BLOCK_MANIFEST, "original saved PB manifest changed");
        fs::write(out.join("original-block-manifest.json"), &raw)?;
        let manifest: Vec<Value> = serde_json::from_slice(&raw)?;
        ensure!(manifest.len() == 1024, "saved PB window size");
        let compact = historical::original("lbp-replay")?;
        let expected = historical::observations()?;
        let checkpoints = historical::checkpoints()?;
        save(&out.join("checkpoints.json"), &checkpoints)?;
        let mut ledgers = checkpoints
            .into_iter()
            .map(|c| Ledger::from_checkpoint(c, Limits::default()))
            .collect::<Result<Vec<_>>>()?;
        let reflection = [
            (Model::BabyDoge, historical::original("babydoge")?),
            (Model::TenSet, historical::original("tenset")?),
        ];
        let mut oracle = BufWriter::new(File::create(out.join("getter-comparisons.jsonl"))?);
        let mut journal = BufWriter::new(File::create(out.join("journal.jsonl"))?);
        let mut evaluations = BufWriter::new(File::create(out.join("evaluations.jsonl"))?);
        let mut clocks = BufWriter::new(File::create(out.join("blocks.jsonl"))?);
        let mut comparisons = BTreeMap::new();
        let mut storage = BTreeMap::new();
        let mut changed = BTreeMap::<String, u64>::new();
        compare(&ledgers, &expected, historical::PARENT_NUMBER, &mut oracle, &mut comparisons)?;
        storage_checks(&ledgers, &reflection, historical::PARENT_NUMBER, &mut storage)?;
        let mut resumed: Option<Vec<Ledger>> = None;
        let mut tail = std::collections::VecDeque::new();
        let mut physical = 0usize;
        let mut code_effects = 0usize;
        let mut idle = 0usize;
        let mut retained = BTreeMap::<String, u64>::new();
        for (index, entry) in manifest.iter().enumerate() {
            let n = historical::PARENT_NUMBER + 1 + index as u64;
            ensure!(
                entry["block"] == n
                    && compact["blocks"][index]["number"] == n
                    && entry["sha256"] == compact["blocks"][index]["source_sha256"]
                    && entry["hash"] == compact["blocks"][index]["hash"],
                "canonical saved block binding differs"
            );
            let raw = fs::read(cache.join(format!("out/top50-full-holder-blocks/{n}.pb")))?;
            let digest = binding::sha(&raw);
            ensure!(entry["sha256"] == digest, "PB digest mismatch {n}");
            let block = eth::Block::decode(raw.as_slice())?;
            let input = collect::decode(&block, &digest)?;
            ensure!(
                input.at.number == n
                    && input.at.hash == hexword(&entry["hash"])?
                    && input.at.timestamp == compact["blocks"][index]["timestamp"].as_u64().context("historical timestamp")?,
                "PB/header historical binding differs"
            );
            // Capture every accepted/rejected attempted block before mutating the ledger.
            serde_json::to_writer(&mut journal, &input)?;
            writeln!(journal)?;
            journal.flush()?;
            let results = apply_all(&mut ledgers, &input)?;
            if let Some(shadow) = &mut resumed {
                let shadow_results = apply_all(shadow, &input)?;
                ensure!(shadow_results == results, "snapshot-resume evaluation differs {n}");
            }
            for (l, r) in ledgers.iter().zip(&results) {
                ensure!(!r.suspended, "historical runtime/dependency invalidation at {n}");
                ensure!(
                    r.evaluations.iter().all(|e| matches!(e.outcome, Outcome::Known { .. })),
                    "historical missing/refused input at {n}"
                );
                *changed.entry(l.binding().model.label().into()).or_default() += r.changed.len() as u64;
                *retained.entry(l.binding().model.label().into()).or_default() += r.retained_writes as u64;
                serde_json::to_writer(&mut evaluations, &json!({"model":l.binding().model,"result":r}))?;
                writeln!(evaluations)?;
            }
            physical += input.writes.len();
            code_effects += input.codes.len();
            if input.writes.is_empty() {
                idle += 1;
            }
            compare(&ledgers, &expected, n, &mut oracle, &mut comparisons)?;
            storage_checks(&ledgers, &reflection, n, &mut storage)?;
            serde_json::to_writer(
                &mut clocks,
                &json!({"at":input.at,"source_sha256":digest,"source_bytes":raw.len(),"physical_storage_effects":input.writes.len(),"protected_code_effects":input.codes.len(),"retained_state_sha256":ledgers.iter().map(|l|l.snapshot().map(|s|binding::sha(&s))).collect::<Result<Vec<_>>>()?}),
            )?;
            writeln!(clocks)?;
            tail.push_back(input);
            while tail.len() > 17 {
                tail.pop_front();
            }
            report["completed_blocks"] = json!(index + 1);
            if index == 511 {
                let mut shadows = Vec::new();
                for l in &ledgers {
                    let bytes = l.snapshot()?;
                    fs::write(out.join(format!("resume-{}.json", l.binding().model.label())), &bytes)?;
                    shadows.push(Ledger::restore(&bytes, &binding::sha(&bytes), l.binding())?);
                }
                resumed = Some(shadows);
            }
        }
        ensure!(
            comparisons == BTreeMap::from([("LBP".into(), 594), ("BabyDoge".into(), 209), ("10SET".into(), 120)]),
            "independent comparison scope"
        );
        ensure!(
            storage == BTreeMap::from([("BabyDoge".into(), 1122), ("10SET".into(), 430)]),
            "independent storage scope"
        );
        let shadow = resumed.context("resume midpoint")?;
        for (l, s) in ledgers.iter().zip(shadow) {
            ensure!(l.snapshot()? == s.snapshot()?, "final resumed snapshot differs");
        }
        let last = ledgers.clone();
        let boundary = tail.front().context("undo boundary")?;
        for l in &mut ledgers {
            ensure!(l.undo(boundary.at.number, boundary.at.hash)? == 16, "bounded undo count");
        }
        for b in tail.iter().skip(1) {
            apply_all(&mut ledgers, b)?;
        }
        ensure!(ledgers == last, "undo/replay full ledger differs");
        for l in &ledgers {
            fs::write(out.join(format!("final-{}.json", l.binding().model.label())), l.snapshot()?)?;
        }
        for w in [&mut oracle, &mut journal, &mut evaluations, &mut clocks] {
            w.flush()?;
        }
        ensure!(
            fs::read(out.join("journal.jsonl"))? == erc20_balances_tools::calculated_retention::journal::BYTES,
            "independently regenerated journal differs from committed regression fixture"
        );
        ensure!(inventory(repo)? == before, "source inventory changed during replay");
        report["source_inputs_unchanged"] = json!(true);
        report["source_inventory_sha256"] = json!(binding::sha(&fs::read(out.join("source-inputs.json"))?));
        report["registered_token_holder_pairs"] = json!(64);
        report["observations"] = json!(comparisons);
        report["reference_observation_scope"] = json!({"parent":64,"in_interval":859,"LBP":{"parent":33,"in_interval":561,"amount_and_pending_scalar_comparisons":1188},"reflection":{"parent":31,"in_interval":298}});
        report["initial_evaluations_compared"] = json!(64);
        report["reflection_storage_comparisons"] = json!(storage);
        report["retained_writes"] = json!(retained);
        report["changed_evaluations"] = json!(changed);
        report["physical_storage_effects"] = json!(physical);
        report["protected_code_effects"] = json!(code_effects);
        report["no_protected_storage_blocks"] = json!(idle);
        report["evaluations"] = json!(ledgers.iter().map(|l| l.counters().evaluations).sum::<u64>());
        report["snapshot_resume_blocks"] = json!(512);
        report["undo_replayed_blocks"] = json!(16);
        report["final_counters"] = json!(ledgers.iter().map(|l| (l.binding().model.label(), l.counters())).collect::<BTreeMap<_, _>>());
        Ok(())
    }
    pub fn main() -> Result<()> {
        let args: Vec<_> = std::env::args().collect();
        ensure!(args.len() == 3, "usage: replay_calculated_retention CACHE_PACKAGE FRESH_OUTPUT");
        let cache = Path::new(&args[1]).canonicalize()?;
        let out = Path::new(&args[2]);
        fs::create_dir(out).context("fresh output directory required")?;
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize()?;
        let start = Instant::now();
        let mut report = json!({"schema":1,"status":"running","qualified":false,"scope":"Host-only finite calculated getter retention; no production Balance rows, current deployment qualification, RPC expectation seeding or global-holder discovery.","start":122288006,"stop":122289030,"completed_blocks":0,"source_block_manifest_sha256":BLOCK_MANIFEST,"parent_metadata_limit":"Historical checkpoint has known number/hash/time only; parent-of-parent and producer version absent. First actual Extended block binds version; every applied block has full header metadata.","ordering":"Distinct physical keys commute at EOB. Each same-key chain must have positive strictly increasing ordinals and exact old/new continuity; equal-value writes remain evidence."});
        let result = run(&repo, &cache, out, &mut report);
        report["elapsed_seconds"] = json!(start.elapsed().as_secs_f64());
        match &result {
            Ok(()) => report["status"] = json!("passed"),
            Err(e) => {
                report["status"] = json!("failed");
                report["error"] = json!(format!("{e:#}"));
            }
        }
        let mut artifacts = Vec::new();
        for e in fs::read_dir(out)? {
            let p = e?.path();
            if p.is_file() {
                let raw = fs::read(&p)?;
                artifacts.push(json!({"path":p.file_name().unwrap().to_str(),"sha256":binding::sha(&raw),"bytes":raw.len()}));
            }
        }
        artifacts.sort_by_key(|a| a["path"].as_str().unwrap().to_owned());
        report["artifacts"] = json!(artifacts);
        save(&out.join("report.json"), &report)?;
        println!(
            "{}: {} completed blocks in {:.2}s",
            report["status"],
            report["completed_blocks"],
            start.elapsed().as_secs_f64()
        );
        result
    }
}
