#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::tops_proof::{self as p, cases::Proof};
    use serde_json::{json, Value};
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: execute_tops_proof SOURCE_DIRECTORY FRESH_OUTPUT");
    let source = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir(out).context("fresh output required")?;
    let mut inventory = vec![];
    let mut raw_inventory = vec![];
    let mut compact = vec![];
    let mut opcode_counts = std::collections::BTreeMap::<String, usize>::new();
    let result = (|| -> anyhow::Result<Value> {
        let source_inventory = p::snapshot_sources(out)?;
        let (capture, compiled, hinput, hout) = p::verify_source_directory(source)?;
        let sr: Value = serde_json::from_slice(&fs::read(source.join("report.json"))?)?;
        ensure!(sr["status"] == "passed", "compiler report passed");
        ensure!(
            sr["source_inventory_sha256"] == p::sha(&fs::read(source.join("source-inventory.json"))?),
            "compiler source inventory binding"
        );
        let mut all = vec![];
        for (variant, input, output, path, name, kind, code) in [
            (
                "captured_full_runtime",
                capture["stdJsonInput"].clone(),
                &compiled,
                p::SOURCE,
                p::NAME,
                "deployedBytecode",
                p::bytes(&capture["runtimeBytecode"]["onchainBytecode"])?,
            ),
            (
                "source_cleanup_harness",
                hinput.clone(),
                &hout,
                p::HARNESS_SOURCE,
                p::HARNESS_NAME,
                "deployedBytecode",
                p::bytes(&hout["contracts"][p::HARNESS_SOURCE][p::HARNESS_NAME]["evm"]["deployedBytecode"]["object"])?,
            ),
            (
                "original_creation_unsupported",
                capture["stdJsonInput"].clone(),
                &compiled,
                p::SOURCE,
                p::NAME,
                "bytecode",
                p::bytes(&capture["creationBytecode"]["onchainBytecode"])?,
            ),
        ] {
            let t = &output["contracts"][path][name]["evm"][kind];
            let map = p::source_map::decode(&code, t["sourceMap"].as_str().context("source map")?)?;
            let sources = p::mapped_sources(&input, output, path, name, kind)?;
            let mut save = |v: &Value| -> anyhow::Result<()> {
                let n = raw_inventory.len() + 1;
                let file = format!("case-{n:04}.json");
                let raw_file = format!("case-{n:04}-raw.json");
                let attempted_raw = serde_json::to_vec(v)?;
                fs::write(out.join(&raw_file), &attempted_raw)?;
                let raw_entry = json!({"file":raw_file,"sha256":p::sha(&attempted_raw),"bytes":attempted_raw.len(),"variant":variant,"name":v["name"]});
                raw_inventory.push(raw_entry.clone());
                let mut v = v.clone();
                v["variant"] = json!(variant);
                v["code_sha256"] = json!(p::sha(&code));
                for field in ["writes", "reads", "logs", "keccaks"] {
                    for effect in v["execution"][field].as_array_mut().context("effects")? {
                        effect["source"] = p::source_map::describe(effect["pc"].as_u64().context("pc")? as usize, &map, &sources)?;
                        ensure!(effect["source"]["path"].is_string(), "unmapped effect source");
                    }
                }
                for step in v["execution"]["trace"].as_array().context("trace")? {
                    *opcode_counts
                        .entry(format!("0x{:02x}", step["opcode"].as_u64().context("opcode")?))
                        .or_default() += 1;
                }
                let raw = serde_json::to_vec(&v)?;
                fs::write(out.join(&file), &raw)?;
                let entry = json!({"file":file,"raw_artifact":raw_entry,"sha256":p::sha(&raw),"bytes":raw.len(),"variant":variant,"name":v["name"],"exit":v["execution"]["exit"],"steps":v["execution"]["trace"].as_array().unwrap().len(),"writes":v["execution"]["writes"].as_array().unwrap().len(),"logs":v["execution"]["logs"].as_array().unwrap().len()});
                inventory.push(entry.clone());
                if v["signature"] == "process(address)" || v["signature"] == "createLPInfo(address,uint256)" || v["signature"] == "unsupported_boundary" {
                    v["full_trace_artifact"] = entry;
                    v["execution"].as_object_mut().unwrap().remove("trace");
                    compact.push(v);
                }
                Ok(())
            };
            let mut proof = Proof::new(&code);
            match variant {
                "captured_full_runtime" => {
                    proof.full_matrix(&mut save)?;
                    let pre = p::cases::seed(55.into(), &[], 0.into());
                    proof.boundary(
                        "original_transfer_unsupported",
                        &erc20_balances_tools::ptoken_proof::cases::call("transfer(address,uint256)", &[56.into(), 0.into()]),
                        &pre,
                        "unsupported opcode 0x5a at pc 7757",
                        &mut save,
                    )?;
                }
                "source_cleanup_harness" => proof.harness_matrix(&mut save)?,
                _ => proof.boundary(
                    "original_constructor_unsupported",
                    &[],
                    &p::vm::State::new(),
                    "unsupported opcode 0x5a at pc 913",
                    &mut save,
                )?,
            }
            all.extend(proof.cases);
        }
        let iraw = serde_json::to_vec_pretty(&inventory)?;
        fs::write(out.join("cases-inventory.json"), &iraw)?;
        let traw = serde_json::to_vec_pretty(&compact)?;
        fs::write(out.join("operation-transcripts.json"), &traw)?;
        let mut artifacts = vec![];
        for a in sr["artifacts"].as_array().context("source artifacts")? {
            let name = a["file"].as_str().context("file")?;
            let digest = p::sha(&fs::read(source.join(name))?);
            ensure!(a["sha256"] == digest, "source artifact unchanged {name}");
            artifacts.push(json!({"file":name,"sha256":digest}));
        }
        Ok(
            json!({"status":"passed","qualified":false,"scope":p::LIMITS,"custom_primary_gap":p::PRIMARY_GAP,"args":args,"source_inventory_sha256":source_inventory,"compiler_source_inventory_sha256":sr["source_inventory_sha256"],"source_trees_equal":sr["source_inventory_sha256"]==source_inventory,"source_artifacts":artifacts,"calls":all.len(),"per_variant_calls":{"captured_full_runtime":inventory.iter().filter(|v|v["variant"]=="captured_full_runtime").count(),"source_cleanup_harness":inventory.iter().filter(|v|v["variant"]=="source_cleanup_harness").count(),"original_creation_unsupported":inventory.iter().filter(|v|v["variant"]=="original_creation_unsupported").count()},"returned_calls":all.iter().filter(|v|v["execution"]["exit"]["kind"]=="return").count(),"reverted_calls":all.iter().filter(|v|v["execution"]["exit"]["kind"]=="revert").count(),"invalid_calls":all.iter().filter(|v|v["execution"]["exit"]["kind"]=="invalid").count(),"explicit_unsupported_calls":all.iter().filter(|v|v["execution"]["exit"]["kind"]=="harness_failure").count(),"compact_transcripts":compact.len(),"cases_inventory_sha256":p::sha(&iraw),"transcripts_sha256":p::sha(&traw),"executed_opcode_counts":opcode_counts,"chain_calls":0}),
        )
    })();
    // Partial raw cases and a partial inventory survive annotation and expectation failures.
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
