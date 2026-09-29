#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::tops_runtime_proof as p;
    use serde_json::{json, Value};
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: execute_tops_runtime_proof SOURCE_DIRECTORY FRESH_OUTPUT");
    let source = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir(out).context("fresh output required")?;
    let mut raw_inventory = vec![];
    let mut inventory = vec![];
    let mut compact = vec![];
    let mut opcodes = std::collections::BTreeMap::<String, usize>::new();
    let result = (|| -> anyhow::Result<Value> {
        let digest = p::snapshot_sources(out)?;
        let (capture, compiled, source_report) = p::verify_report(source, &digest)?;
        let code = p::bytes(&capture["runtimeBytecode"]["onchainBytecode"])?;
        let target = &compiled["contracts"][p::SOURCE][p::NAME]["evm"]["deployedBytecode"];
        let map = p::source_map::decode(&code, target["sourceMap"].as_str().context("map")?)?;
        let sources = p::mapped_sources(&capture["stdJsonInput"], &compiled, p::SOURCE, p::NAME, "deployedBytecode")?;
        let mut save = |v: &Value| -> anyhow::Result<()> {
            let n = raw_inventory.len() + 1;
            let rawfile = format!("case-{n:04}-raw.json");
            let file = format!("case-{n:04}.json");
            let raw = serde_json::to_vec(v)?;
            fs::write(out.join(&rawfile), &raw)?;
            let rawentry = json!({"file":rawfile,"sha256":p::sha(&raw),"bytes":raw.len(),"name":v["name"]});
            raw_inventory.push(rawentry.clone());
            let mut v = v.clone();
            for field in ["writes", "reads", "logs", "keccaks"] {
                for e in v["execution"][field].as_array_mut().context("effects")? {
                    e["source"] = p::source_map::describe(e["pc"].as_u64().context("pc")? as usize, &map, &sources)?;
                    ensure!(e["source"]["path"].is_string(), "unmapped local effect");
                }
            }
            for field in ["calls", "origin_reads", "gas_reads"] {
                for e in v["context_witness"][field].as_array_mut().context("context effects")? {
                    e["source"] = p::source_map::describe(e["pc"].as_u64().context("pc")? as usize, &map, &sources)?;
                    ensure!(e["source"]["path"].is_string(), "unmapped context effect");
                }
            }
            for step in v["execution"]["trace"].as_array().context("trace")? {
                *opcodes.entry(format!("0x{:02x}", step["opcode"].as_u64().context("opcode")?)).or_default() += 1;
            }
            let raw = serde_json::to_vec(&v)?;
            fs::write(out.join(&file), &raw)?;
            let entry = json!({"file":file,"sha256":p::sha(&raw),"bytes":raw.len(),"raw_artifact":rawentry,"name":v["name"],"exit":v["execution"]["exit"],"steps":v["execution"]["trace"].as_array().unwrap().len(),"writes":v["execution"]["writes"].as_array().unwrap().len(),"logs":v["execution"]["logs"].as_array().unwrap().len(),"static_calls":v["context_witness"]["calls"].as_array().unwrap().len()});
            inventory.push(entry.clone());
            if !v["name"].as_str().unwrap().starts_with("post_") {
                v["full_trace_artifact"] = entry;
                v["execution"].as_object_mut().unwrap().remove("trace");
                compact.push(v);
            }
            Ok(())
        };
        let mut proof = p::cases::Proof::new(&code);
        proof.matrix(&mut save)?;
        let cases = serde_json::to_vec_pretty(&inventory)?;
        let transcripts = serde_json::to_vec_pretty(&compact)?;
        fs::write(out.join("operation-transcripts.json"), &transcripts)?;
        // A second fresh snapshot proves that current disk inputs stayed fixed throughout execution.
        let after = out.join("after");
        fs::create_dir(&after)?;
        ensure!(p::snapshot_sources(&after)? == digest, "source changed during proof");
        p::verify_report(source, &digest)?;
        Ok(
            json!({"status":"passed","qualified":false,"scope":p::LIMITS,"chain_calls":0,"args":args,"source_inventory_sha256":digest,"preparation_source_inventory_sha256":source_report["source_inventory_sha256"],"source_trees_equal":true,"source_report_sha256":p::sha(&fs::read(source.join("report.json"))?),"source_artifacts":source_report["artifacts"],"historical_compiler_report_sha256":p::HISTORICAL_REPORT_SHA,"historical_compiler_inventory_sha256":p::HISTORICAL_INVENTORY_SHA,"historical_source_gap":erc20_balances_tools::tops_proof::PRIMARY_GAP,"calls":inventory.len(),"returned_calls":inventory.iter().filter(|v|v["exit"]["kind"]=="return").count(),"reverted_calls":inventory.iter().filter(|v|v["exit"]["kind"]=="revert").count(),"explicit_unsupported_calls":inventory.iter().filter(|v|v["exit"]["kind"]=="harness_failure").count(),"invalid_calls":inventory.iter().filter(|v|v["exit"]["kind"]=="invalid").count(),"compact_transcripts":compact.len(),"cases_inventory_sha256":p::sha(&cases),"transcripts_sha256":p::sha(&transcripts),"executed_opcode_counts":opcodes}),
        )
    })();
    fs::write(out.join("raw-cases-inventory.json"), serde_json::to_vec_pretty(&raw_inventory)?)?;
    fs::write(out.join("cases-inventory.json"), serde_json::to_vec_pretty(&inventory)?)?;
    let (report, status) = match result {
        Ok(v) => (v, Ok(())),
        Err(e) => (
            json!({"status":"failed","qualified":false,"attempted_calls":raw_inventory.len(),"annotated_calls":inventory.len(),"error":format!("{e:#}")}),
            Err(e),
        ),
    };
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    status
}
