#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    host::main()
}
#[cfg(not(target_arch = "wasm32"))]
mod host {
    use anyhow::{ensure, Result};
    use erc20_balances_tools::{
        calculated_retention::binding::sha,
        ybc_retention::{binding as b, historical as h, *},
    };
    use serde_json::{json, Value};
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs::{self, File},
        io::{BufWriter, Write},
        path::Path,
    };
    const BLOCK_PIN: &str = "56397f18ea7371231b261baefb10122c1439e604d130be2cbc97fca598e38ba6";
    fn save(path: &Path, v: &impl serde::Serialize) -> Result<()> {
        fs::write(path, serde_json::to_vec_pretty(v)?)?;
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
        let entries = paths
            .into_iter()
            .map(|path| {
                let raw = fs::read(repo.join(&path))?;
                Ok(json!({"path":path,"bytes":raw.len(),"sha256":sha(&raw)}))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(json!({"schema":1,"files":entries}))
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
    fn run(repo: &Path, cache: &Path, out: &Path, report: &mut Value) -> Result<()> {
        let before = inventory(repo)?;
        save(&out.join("source-inputs.json"), &before)?;
        for f in before["files"].as_array().unwrap() {
            let path = f["path"].as_str().unwrap();
            let dest = out.join("source").join(path);
            fs::create_dir_all(dest.parent().unwrap())?;
            fs::copy(repo.join(path), dest)?;
        }
        report["source_inventory_sha256"] = json!(sha(&fs::read(out.join("source-inputs.json"))?));
        let captures = [
            (
                "token-source.json",
                "out/ranks201-250-source-review/0xebc2d768147f2d058f4266bb57e34ca1b6ef1319.json",
            ),
            ("prestate.json", "out/ybc-compact-trace/prestateTracer.json"),
            ("calls.json", "out/ybc-arithmetic-review/calls.json"),
            ("parent-storage.json", "out/ybc-model-historical/122288005-storage.json"),
            ("final-storage.json", "out/ybc-model-historical/122289029-storage.json"),
            ("historical.json", "out/ybc-model-historical/report.json"),
            ("overrides.json", "out/ybc-model-controls/report.json"),
        ];
        let mut input_bindings = Vec::new();
        for (file, path) in captures {
            let raw = fs::read(cache.join(path))?;
            b::verify_original(file, &raw)?;
            input_bindings.push(json!({"path":path,"sha256":sha(&raw),"bytes":raw.len()}));
        }
        report["original_inputs"] = json!(input_bindings);
        save(&out.join("captured-comparisons.json"), &h::check_captured()?)?;
        let parent = h::checkpoint(h::PARENT)?;
        let final_cp = h::checkpoint(h::FINAL)?;
        save(&out.join("parent-checkpoint.json"), &parent)?;
        save(&out.join("independent-final-checkpoint.json"), &final_cp)?;
        let mut ledger = Ledger::from_checkpoint(parent, Limits::default())?;
        let raw = fs::read(cache.join("out/reflection-replay/blocks.json"))?;
        ensure!(sha(&raw) == BLOCK_PIN, "original block manifest digest");
        fs::write(out.join("original-block-manifest.json"), &raw)?;
        let manifest: Vec<Value> = serde_json::from_slice(&raw)?;
        ensure!(manifest.len() == 1024, "block manifest count");
        let mut journal = BufWriter::new(File::create(out.join("journal.jsonl"))?);
        let mut evals = BufWriter::new(File::create(out.join("evaluations.jsonl"))?);
        let mut blocks = BufWriter::new(File::create(out.join("blocks.jsonl"))?);
        let mut counts = BTreeMap::<String, u64>::new();
        let mut per_holder = BTreeMap::<String, BTreeMap<String, u64>>::new();
        let mut versions = BTreeMap::<u32, u64>::new();
        for (i, m) in manifest.iter().enumerate() {
            let number = h::PARENT + 1 + i as u64;
            ensure!(m["block"] == number, "manifest ordering");
            let raw = fs::read(cache.join(format!("out/top50-full-holder-blocks/{number}.pb")))?;
            let digest = sha(&raw);
            ensure!(m["sha256"] == digest, "original PB digest differs at {number}");
            let input = collect::decode_original(&raw, number, b::hexword(&m["hash"])?, &digest)?;
            let applied = ledger.apply(&input)?;
            // Commit journal/counters only after whole-block validation and evaluation.
            line(&mut journal, &input)?;
            for e in applied.evaluations {
                let holder = format!("0x{}", hex::encode(e.holder));
                let mut summary = serde_json::to_value(&e)?;
                summary.as_object_mut().unwrap().remove("used");
                summary["used_facts_sha256"] = json!(sha(&serde_json::to_vec(&e.used)?));
                summary["used_facts_count"] = json!(e.used.len());
                line(&mut evals, &summary)?;
                for (metric, label) in [
                    ("pending", status(&e.pending)),
                    ("observable", status(&e.observable)),
                    ("raw_basis", status(&e.raw_basis)),
                ] {
                    let key = format!("{metric}.{label}");
                    *counts.entry(key.clone()).or_default() += 1;
                    *per_holder.entry(holder.clone()).or_default().entry(key).or_default() += 1;
                }
            }
            line(
                &mut blocks,
                &json!({"at":input.at,"source_sha256":digest,"persisted_storage_effects":input.writes.len(),"persisted_code_effects":input.codes.len(),"retained_writes":applied.retained_writes}),
            )?;
            *versions.entry(input.at.producer_version.unwrap()).or_default() += 1;
            report["completed_blocks"] = json!(i + 1);
            report["counts"] = json!(counts);
            report["per_holder"] = json!(per_holder);
            report["producer_versions"] = json!(versions);
            if i % 64 == 0 {
                save(&out.join("report.json"), report)?;
            }
        }
        ensure!(
            ledger.at().number == h::FINAL && ledger.at().hash == final_cp.at.hash && ledger.at().timestamp == final_cp.at.timestamp,
            "final boundary differs"
        );
        let mut known = Vec::new();
        let mut absent = Vec::new();
        for f in &final_cp.facts {
            if let Some(retained) = ledger.fact(&f.slot) {
                ensure!(retained.word == f.word, "independent final raw fact differs: {:?}", f.slot);
                known.push(json!({"retained":retained,"independent":f}));
            } else {
                absent.push(f.slot.clone());
            }
        }
        save(&out.join("final-storage-comparisons.json"), &json!({"matches":known,"unknown":absent}))?;
        report["final_raw_word_matches"] = json!(known.len());
        report["final_raw_words_unknown"] = json!(absent.len());
        let expectations = b::original("historical.json")?;
        let mut final_evals = Vec::new();
        let mut final_known = 0;
        for row in expectations["snapshots"].as_array().unwrap().iter().filter(|v| v["block"] == h::FINAL) {
            let holder = address(row["holder"].as_str().unwrap())?;
            let e = ledger.evaluate(holder)?;
            if let Metric::Known(v) = &e.observable {
                ensure!(row["expected_balance"] == *v, "independent final getter mismatch");
                final_known += 1;
            }
            if let Metric::Known(v) = &e.pending {
                ensure!(
                    row["expected_reward"] == v.pending_reward && row["expected_stopping_hour"] == v.stopping_hour,
                    "independent final reward mismatch"
                );
            }
            final_evals.push(json!({"retained":e,"independent_expected_balance":row["expected_balance"],"independent_expected_reward":row["expected_reward"],"independent_expected_stopping_hour":row["expected_stopping_hour"]}));
        }
        save(&out.join("final-getter-comparisons.json"), &final_evals)?;
        report["final_evaluable_balances"] = json!(final_known);
        report["ledger_counters"] = json!(ledger.counters());
        fs::write(out.join("final-snapshot.json"), ledger.snapshot()?)?;
        ensure!(fs::read(out.join("journal.jsonl"))? == journal::BYTES, "regenerated portable journal differs");
        ensure!(before == inventory(repo)?, "source inventory changed during replay");
        let artifacts = [
            "captured-comparisons.json",
            "parent-checkpoint.json",
            "independent-final-checkpoint.json",
            "original-block-manifest.json",
            "journal.jsonl",
            "evaluations.jsonl",
            "blocks.jsonl",
            "final-storage-comparisons.json",
            "final-getter-comparisons.json",
            "final-snapshot.json",
        ]
        .into_iter()
        .map(|path| {
            let raw = fs::read(out.join(path))?;
            Ok(json!({"path":path,"sha256":sha(&raw),"bytes":raw.len()}))
        })
        .collect::<Result<Vec<_>>>()?;
        report["artifacts"] = json!(artifacts);
        report["status"] = json!("passed");
        Ok(())
    }
    pub fn main() -> Result<()> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        ensure!(args.len() == 3, "usage: replay_ybc_retention REPO ORIGINAL_PACKAGE_CACHE FRESH_OUTPUT");
        let (repo, cache, out) = (Path::new(&args[0]), Path::new(&args[1]), Path::new(&args[2]));
        ensure!(!out.exists(), "fresh output required");
        fs::create_dir_all(out)?;
        let mut report = json!({"status":"incomplete","qualified":false,"mode":"offline_saved_data_only","interval":[122288006,122289030],"registered_holders":12,"completed_blocks":0,"limits":"Finite raw-key registry and <=240 required hours. End checkpoint never initializes replay. Historical getter parity exists only at independent captured boundaries and overrides; no intermediate getter capture or production qualification."});
        save(&out.join("report.json"), &report)?;
        let result = run(repo, cache, out, &mut report);
        if let Err(e) = &result {
            report["status"] = json!("failed");
            report["error"] = json!(format!("{e:#}"));
        }
        save(&out.join("report.json"), &report)?;
        result
    }
}
