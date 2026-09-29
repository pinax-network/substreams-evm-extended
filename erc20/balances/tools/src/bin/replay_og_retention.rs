#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    host::main()
}
#[cfg(not(target_arch = "wasm32"))]
mod host {
    use anyhow::{ensure, Context, Result};
    use erc20_balances_tools::{
        calculated_retention::binding::sha,
        og_retention::{binding as b, historical as h, journal as j, *},
    };
    use serde_json::{json, Value};
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs::{self, File},
        io::{BufWriter, Write},
        path::Path,
    };
    fn save(p: &Path, v: &impl serde::Serialize) -> Result<()> {
        fs::write(p, serde_json::to_vec_pretty(v)?)?;
        Ok(())
    }
    fn line(w: &mut BufWriter<File>, v: &impl serde::Serialize) -> Result<()> {
        serde_json::to_writer(&mut *w, v)?;
        writeln!(w)?;
        w.flush()?;
        Ok(())
    }
    fn inventory(repo: &Path) -> Result<Value> {
        snapshots::verify(repo)?;
        let mut paths: BTreeSet<_> = snapshots::inputs().into_iter().map(|(p, _)| p.to_string()).collect();
        paths.extend(
            [
                "docs/follow-up.md",
                "docs/handoff.md",
                "docs/research/README.md",
                "erc20/balances/docs/role-shape-audit.md",
            ]
            .map(str::to_string),
        );
        Ok(
            json!({"schema":1,"files":paths.into_iter().map(|path|{let raw=fs::read(repo.join(&path))?;Ok(json!({"path":path,"sha256":sha(&raw),"bytes":raw.len()}))}).collect::<Result<Vec<_>>>()?}),
        )
    }
    fn status<T>(m: &Metric<T>) -> &'static str {
        match m {
            Metric::Known(_) => "known",
            Metric::Unknown { .. } => "unknown",
            Metric::ModelRefusal { .. } => "model_refusal",
            Metric::ScopeRefusal { .. } => "scope_refusal",
            Metric::Suspended { .. } => "suspended",
        }
    }
    fn compare<T>(m: &Metric<T>, expected: &Value, values: impl FnOnce(&T) -> Vec<String>) -> Result<&'static str> {
        if expected.get("values").is_some() {
            let Metric::Known(v) = m else {
                anyhow::bail!("captured value versus {}", status(m))
            };
            ensure!(
                values(v) == h::expected_values(expected)?.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "captured getter mismatch"
            );
            Ok("value")
        } else if expected["rpc_error"].get("data").is_none() {
            ensure!(
                expected["rpc_error"]["code"] == 3
                    && expected["rpc_error"]["message"] == "execution reverted"
                    && matches!(m,Metric::ScopeRefusal{reason} if reason.starts_with("unmodeled recursive pool")),
                "recursive observation scope mismatch"
            );
            Ok("recursive_observation")
        } else {
            ensure!(
                expected["rpc_error"]["data"] == format!("0x4e487b71{:064x}", 0x11)
                    && matches!(m,Metric::ModelRefusal{reason} if ["uint256 addition overflow","uint256 multiplication overflow","uint256 subtraction underflow"].contains(&reason.as_str())),
                "captured arithmetic refusal mismatch"
            );
            Ok("panic11_observation")
        }
    }
    fn compare_case(cp: &Checkpoint, holder: Address, expected: &Value, attempts: &mut BufWriter<File>) -> Result<Value> {
        let l = Ledger::from_checkpoint(cp.clone(), Limits::default())?;
        let e = l.evaluate(holder)?;
        let pool = expected.get("pool_balance").map(|_| decode::pool_terminal(&l));
        line(attempts, &json!({"checkpoint":cp,"evaluation":e,"pool_terminal":pool,"expected":expected}))?;
        let prefix = if expected.get("holder_balance").is_some() { "holder_" } else { "" };
        let mut outcomes = vec![
            compare(&e.hourly, &expected[format!("{prefix}hourly")], |v| {
                vec![v.amount.clone(), v.stopping_hour.clone()]
            })?,
            compare(&e.daily, &expected[format!("{prefix}daily")], |v| vec![v.clone()])?,
            compare(&e.observable, &expected[format!("{prefix}balance")], |v| vec![v.clone()])?,
        ];
        if let (Some(m), Some(want)) = (&pool, expected.get("pool_balance")) {
            outcomes.push(compare(m, want, |v| vec![v.clone()])?);
        }
        Ok(json!({"checkpoint_sha256":sha(&serde_json::to_vec(cp)?),"evaluation":e,"pool_terminal":pool,"outcomes":outcomes,"expected":expected}))
    }
    fn captured(out: &Path) -> Result<Value> {
        let mut records = BufWriter::new(File::create(out.join("captured-comparisons.jsonl"))?);
        let mut attempts = BufWriter::new(File::create(out.join("captured-attempts.jsonl"))?);
        let originals = b::original("historical.json")?;
        let mut counts = BTreeMap::<String, u64>::new();
        let mut add = |label: &str, row: Value| -> Result<()> {
            for v in row["outcomes"].as_array().unwrap() {
                *counts.entry(v.as_str().unwrap().into()).or_default() += 1;
            }
            line(&mut records, &json!({"label":label,"comparison":row}))
        };
        for (cp, original) in h::checkpoints()?.iter().zip(originals.as_array().unwrap()) {
            add(
                &format!("checkpoint{}", original["case"]),
                compare_case(cp, address(original["holder"].as_str().unwrap())?, &original["expected"], &mut attempts)?,
            )?;
        }
        for control in h::controls()? {
            let holder = address(control.snapshot["holder"].as_str().unwrap())?;
            add(&control.label, compare_case(&control.checkpoint, holder, &control.expected, &mut attempts)?)?;
            for (i, x) in control.extra_gas_controls.iter().enumerate() {
                add(
                    &format!("{}-extra-gas-{i}", control.label),
                    compare_case(&control.checkpoint, holder, &x["expected"], &mut attempts)?,
                )?;
            }
        }
        ensure!(
            counts.get("value") == Some(&303) && counts.get("panic11_observation") == Some(&40) && counts.get("recursive_observation") == Some(&48),
            "external observation cohort"
        );
        Ok(json!(counts))
    }
    fn run(repo: &Path, cache: &Path, out: &Path, report: &mut Value) -> Result<()> {
        b::verify()?;
        let before = inventory(repo)?;
        save(&out.join("source-inputs.json"), &before)?;
        for entry in before["files"].as_array().unwrap() {
            let path = entry["path"].as_str().unwrap();
            let dst = out.join("source").join(path);
            fs::create_dir_all(dst.parent().unwrap())?;
            fs::copy(repo.join(path), dst)?;
        }
        report["source_inventory_sha256"] = json!(sha(&fs::read(out.join("source-inputs.json"))?));
        report["captured_external_observations"] = captured(out)?;
        let checkpoints = h::checkpoints()?;
        save(&out.join("five-independent-checkpoints.json"), &checkpoints)?;
        let pool = j::pool_observations()?;
        let observations: Vec<_> = pool.iter().filter(|p| p.at.number >= j::START - 1).collect();
        ensure!(observations.len() == 923, "pool continuation boundary count");
        let joined = j::join_initial(&checkpoints[0], observations[0])?;
        save(&out.join("initial-join-provenance.json"), &joined.provenance)?;
        let parent = joined.checkpoint;
        save(&out.join("parent-checkpoint.json"), &parent)?;
        let mut l = Ledger::from_checkpoint(parent.clone(), Limits::default())?;
        j::check_pool(&l, observations[0])?;
        let manifest_raw = fs::read(cache.join("out/reflection-replay/blocks.json"))?;
        ensure!(
            manifest_raw == b::artifact_bytes("original-block-manifest.json")?,
            "original manifest byte identity"
        );
        let manifest: Vec<Value> = serde_json::from_slice(&manifest_raw)?;
        let mut journals = BufWriter::new(File::create(out.join("journal.jsonl"))?);
        let mut evals = BufWriter::new(File::create(out.join("evaluations.jsonl"))?);
        let mut blocks = BufWriter::new(File::create(out.join("blocks.jsonl"))?);
        let mut pools = BufWriter::new(File::create(out.join("pool-comparisons.jsonl"))?);
        line(
            &mut pools,
            &json!({"at":l.at(),"matched_raw_words":21,"raw_expectation":observations[0].expected,"snapshot_sha256":sha(&l.snapshot()?)}),
        )?;
        let mut counts = BTreeMap::<String, u64>::new();
        let mut per_holder = BTreeMap::<String, BTreeMap<String, u64>>::new();
        let mut inputs = vec![];
        let mut first_suspension = None;
        let mut physical = 0;
        let mut code_effects = 0;
        for (i, m) in manifest.iter().filter(|v| v["block"].as_u64().is_some_and(|n| n >= j::START)).enumerate() {
            let number = j::START + i as u64;
            ensure!(m["block"] == number, "manifest ordering");
            let raw = fs::read(cache.join(format!("out/top50-full-holder-blocks/{number}.pb")))?;
            let digest = m["sha256"].as_str().context("manifest PB digest")?;
            let input = collect::decode_original(&raw, number, b::hexword(&m["hash"])?, digest)?;
            // Save the independently decoded attempt before apply or later assertions.
            line(&mut journals, &input)?;
            let applied = l.apply(&input)?;
            physical += input.writes.len();
            code_effects += input.codes.len();
            if first_suspension.is_none() && l.suspension().is_some() {
                first_suspension = Some(json!({"at":l.at(),"reason":l.suspension(),"input":input}));
                save(&out.join("first-suspension.json"), &first_suspension)?;
            }
            for e in &applied.evaluations {
                let holder = format!("0x{}", hex::encode(e.holder));
                let mut row = serde_json::to_value(e)?;
                row.as_object_mut().unwrap().remove("used");
                row["used_facts_sha256"] = json!(sha(&serde_json::to_vec(&e.used)?));
                row["used_facts_count"] = json!(e.used.len());
                line(&mut evals, &row)?;
                for (name, state) in [
                    ("raw_basis", status(&e.raw_basis)),
                    ("hourly", status(&e.hourly)),
                    ("daily", status(&e.daily)),
                    ("observable", status(&e.observable)),
                ] {
                    let key = format!("{name}.{state}");
                    *counts.entry(key.clone()).or_default() += 1;
                    *per_holder.entry(holder.clone()).or_default().entry(key).or_default() += 1;
                }
            }
            j::check_pool(&l, observations[i + 1])?;
            line(
                &mut pools,
                &json!({"at":l.at(),"matched_raw_words":21,"raw_expectation":observations[i+1].expected,"snapshot_sha256":sha(&l.snapshot()?)}),
            )?;
            line(
                &mut blocks,
                &json!({"at":input.at,"source_sha256":digest,"persisted_storage_effects":input.writes.len(),"persisted_code_effects":input.codes.len(),"retained_writes":applied.retained_writes}),
            )?;
            report["completed_blocks"] = json!(i + 1);
            report["outcomes"] = json!(counts);
            report["per_holder"] = json!(per_holder);
            report["first_suspension"] = json!(first_suspension);
            inputs.push(input);
            if i == 460 {
                fs::write(out.join("midpoint-snapshot.json"), l.snapshot()?)?;
            }
            if i % 64 == 0 {
                save(&out.join("report.json"), report)?;
            }
        }
        ensure!(inputs.len() == 922 && l.at().number == j::STOP - 1, "full continuation");
        let final_snapshot = l.snapshot()?;
        fs::write(out.join("final-snapshot.json"), &final_snapshot)?;
        let mid = fs::read(out.join("midpoint-snapshot.json"))?;
        let mut resumed = Ledger::restore(&mid, &sha(&mid), l.binding())?;
        for input in inputs.iter().filter(|i| i.at.number > resumed.at().number).cloned().collect::<Vec<_>>() {
            resumed.apply(&input)?;
        }
        ensure!(resumed.snapshot()? == final_snapshot, "midpoint full-state resume mismatch");
        let last = inputs.last().unwrap();
        let before_undo = l.clone();
        let previous = &inputs[inputs.len() - 2].at;
        l.undo(previous.number, previous.hash)?;
        l.apply(last)?;
        ensure!(l == before_undo, "undo/replay whole ledger differs");
        let raw = fs::read(out.join("journal.jsonl"))?;
        let decoded = j::verify(&raw)?;
        ensure!(decoded == inputs, "portable journal roundtrip");
        let mut portable = Ledger::from_checkpoint(parent, Limits::default())?;
        for input in &decoded {
            portable.apply(input)?;
        }
        ensure!(portable == l, "portable full-ledger replay mismatch");
        report["persisted_storage_effects"] = json!(physical);
        report["persisted_code_effects"] = json!(code_effects);
        report["ledger_counters"] = json!(l.counters());
        report["pool_boundaries"] = json!(923);
        report["pool_raw_word_comparisons"] = json!(923 * 21);
        report["initial_registered_pairs"] = json!(2);
        report["scheduled_interval_evaluations"] = json!(1844);
        report["midpoint_restore_full_state"] = json!(true);
        report["undo_replay_full_ledger"] = json!(true);
        report["portable_journal_full_ledger"] = json!(true);
        ensure!(inventory(repo)? == before, "source inventory changed during replay");
        ensure!(
            fs::read(cache.join("out/reflection-replay/blocks.json"))? == manifest_raw,
            "manifest changed during replay"
        );
        let mut names = vec![
            "source-inputs.json",
            "captured-comparisons.jsonl",
            "captured-attempts.jsonl",
            "five-independent-checkpoints.json",
            "parent-checkpoint.json",
            "initial-join-provenance.json",
            "journal.jsonl",
            "evaluations.jsonl",
            "blocks.jsonl",
            "pool-comparisons.jsonl",
            "midpoint-snapshot.json",
            "final-snapshot.json",
        ];
        if first_suspension.is_some() {
            names.push("first-suspension.json");
        }
        report["artifacts"] = json!(names
            .into_iter()
            .map(|name| {
                let raw = fs::read(out.join(name))?;
                Ok(json!({"path":name,"bytes":raw.len(),"sha256":sha(&raw)}))
            })
            .collect::<Result<Vec<_>>>()?);
        report["status"] = json!("passed_offline_unqualified");
        Ok(())
    }
    pub fn main() -> Result<()> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        ensure!(args.len() == 3, "usage: replay_og_retention REPO CACHE_BALANCES FRESH_OUT");
        let repo = Path::new(&args[0]);
        let cache = Path::new(&args[1]);
        let out = Path::new(&args[2]);
        ensure!(!out.exists(), "output must be fresh");
        fs::create_dir_all(out)?;
        let mut report = json!({"schema":1,"status":"attempting","qualified":false,"start":j::START,"stop_exclusive":j::STOP,"completed_blocks":0,"scope":"Two finite initialized token-holder pairs; separate raw/hourly/daily/observable metrics. Pool comparisons are raw storage only. No continuous holder getter parity, fresh RPC, source qualification, or deployment admission."});
        let result = run(repo, cache, out, &mut report);
        if let Err(e) = &result {
            report["status"] = json!("failed");
            report["error"] = json!(format!("{e:#}"));
        }
        save(&out.join("report.json"), &report)?;
        result
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn failed_captured_comparison_keeps_computed_raw_attempt() {
            let row = &b::original("historical.json").unwrap()[0];
            let cp = h::checkpoints().unwrap().remove(0);
            let mut expected = row["expected"].clone();
            expected["hourly"]["values"][0] = json!("1");
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("attempt.jsonl");
            let mut out = BufWriter::new(File::create(&path).unwrap());
            assert!(compare_case(&cp, address(row["holder"].as_str().unwrap()).unwrap(), &expected, &mut out).is_err());
            let attempt: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
            assert_eq!(attempt["checkpoint"], serde_json::to_value(cp).unwrap());
            assert_eq!(attempt["expected"], expected);
            assert_ne!(attempt["evaluation"]["hourly"]["Known"]["amount"], expected["hourly"]["values"][0]);
        }
    }
}
