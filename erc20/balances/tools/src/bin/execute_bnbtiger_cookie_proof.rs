#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::bnbtiger_cookie_proof as p;
    use serde_json::{json, Value};
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: execute_bnbtiger_cookie_proof SOURCE_DIRECTORY FRESH_OUTPUT");
    let input = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir(out).context("fresh output required")?;
    let mut records = vec![];
    let mut compact = vec![];
    let mut targets = vec![];
    let mut steps = 0usize;
    let mut mapped = 0usize;
    let mut unmapped = 0usize;
    let mut returned = 0usize;
    let mut reverted = 0usize;
    let run = (|| -> anyhow::Result<Value> {
        let inventory = p::snapshot_sources(out)?;
        let source_report = p::verify_source_report(input, &inventory)?;
        for (t, c, compiled) in p::verify_source_directory(input)? {
            let compiler_raw = fs::read(input.join(t.label()).join("compiler-output.json"))?;
            let code = p::bytes(&p::selected(&compiled, t)["evm"]["deployedBytecode"]["object"])?;
            let map = p::source_map::decode(
                &code,
                p::selected(&compiled, t)["evm"]["deployedBytecode"]["sourceMap"].as_str().context("map")?,
            )?;
            let sources = p::mapped_sources(&c, &compiled, t, "deployedBytecode")?;
            let dir = out.join(t.label());
            fs::create_dir(&dir)?;
            let counts = p::cases::run(&c, &compiled, t, &mut |case: &Value| {
                let n = records.len();
                let raw = serde_json::to_vec_pretty(case)?;
                let name = format!("{}/{n:04}.json", t.label());
                fs::write(out.join(&name), &raw)?;
                records.push(json!({"file":name,"sha256":p::sha(&raw),"target":t.label(),"case":case["case"],"program":case["program"],"exit":case["execution"]["exit"]}));
                let mut annotated = case.clone();
                for field in ["reads", "writes", "keccaks", "logs"] {
                    for effect in annotated["execution"][field].as_array_mut().context("effects")? {
                        let pc = effect["pc"].as_u64().context("PC")? as usize;
                        effect["source"] = p::source_map::describe(pc, &map, &sources)?;
                        if effect["source"]["path"].is_string() {
                            mapped += 1;
                        } else {
                            unmapped += 1;
                        }
                    }
                }
                let mut reduced = annotated.clone();
                reduced["execution"].as_object_mut().context("execution")?.remove("trace");
                reduced["trace_steps"] = json!(case["execution"]["trace"].as_array().context("trace")?.len());
                steps += case["execution"]["trace"].as_array().context("trace")?.len();
                match case["execution"]["exit"]["kind"].as_str() {
                    Some("return") => returned += 1,
                    Some("revert") => reverted += 1,
                    _ => {}
                }
                let file = format!("{}/{n:04}-annotated.json", t.label());
                let raw = serde_json::to_vec_pretty(&annotated)?;
                fs::write(out.join(&file), &raw)?;
                records.last_mut().context("record")?["annotated"] = json!({"file":file,"sha256":p::sha(&raw)});
                reduced["full_trace_artifact"] = records.last().context("record")?.clone();
                compact.push(reduced);
                Ok(())
            })?;
            targets.push(json!({"target":t.label(),"counts":counts.json(),"compiler_output_sha256":p::sha(&compiler_raw),"capture_sha256":t.capture_sha(),"captured_runtime_keccak256":t.runtime_hash(),"call_value":"zero only; VM has no nonzero callvalue context","writer_review_sha256":p::sha(&fs::read(input.join(t.label()).join("writer-review.json"))?)}));
        }
        Ok(
            json!({"status":"passed","qualified":false,"scope":p::LIMITS,"source_gap":p::PRIMARY_GAP,"args":args,"source_inventory_sha256":inventory,"compiler_source_inventory_sha256":source_report["source_inventory_sha256"],"source_trees_equal":true,"compiler_report_sha256":p::sha(&fs::read(input.join("report.json"))?),"source_artifacts":source_report["artifacts"],"targets":targets,"calls":records.len(),"returned_calls":returned,"reverted_calls":reverted,"unexpected_invalid_or_harness_failure_calls":0,"compiler_captured_case_pairs":records.len()/2,"all_case_pairs_exact":true,"full_opcode_records":steps,"mapped_effects":mapped,"unmapped_effects":unmapped,"chain_calls":0}),
        )
    })();
    let transcripts = serde_json::to_vec_pretty(&compact)?;
    fs::write(out.join("transcripts.json"), &transcripts)?;
    let cases = serde_json::to_vec_pretty(&records)?;
    fs::write(out.join("cases-inventory.json"), &cases)?;
    let (mut report, status) = match run {
        Ok(v) => (v, Ok(())),
        Err(e) => (
            json!({"status":"failed","qualified":false,"error":format!("{e:#}"),"completed_targets":targets,"attempted_cases":records}),
            Err(e),
        ),
    };
    report["transcripts_sha256"] = json!(p::sha(&transcripts));
    report["cases_inventory_sha256"] = json!(p::sha(&cases));
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    status
}
