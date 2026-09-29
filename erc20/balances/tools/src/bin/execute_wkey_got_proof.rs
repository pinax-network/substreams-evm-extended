#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::wkey_got_proof::{self as p, cases::Proof};
    use serde_json::{json, Value};
    use std::{collections::BTreeMap, fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: execute_wkey_got_proof SOURCE_DIRECTORY FRESH_OUTPUT");
    let source = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir(out).context("fresh output required")?;
    let mut raw_inventory = vec![];
    let mut cases = vec![];
    let mut compact = vec![];
    let result = (|| -> anyhow::Result<Value> {
        let source_inventory = p::snapshot_sources(out)?;
        let sr: Value = serde_json::from_slice(&fs::read(source.join("report.json"))?)?;
        ensure!(
            sr["status"] == "passed" && sr["source_inventory_sha256"] == p::sha(&fs::read(source.join("source-inventory.json"))?),
            "source report/inventory identity"
        );
        let bindings = p::verify_source_directory(source)?;
        let mut all = vec![];
        let mut targets = vec![];
        let mut opcodes = BTreeMap::<String, usize>::new();
        let mut mapped_effects = 0usize;
        for (target, capture, compiled) in bindings {
            let runtime = p::runtime(&capture)?;
            let creation = p::bytes(&capture["creationBytecode"]["recompiledBytecode"])?;
            let mut maps = BTreeMap::new();
            for kind in ["bytecode", "deployedBytecode"] {
                let v = &compiled["contracts"][p::SOURCE][target.name()]["evm"][kind];
                maps.insert(
                    kind,
                    (
                        p::source_map::decode(&p::bytes(&v["object"])?, v["sourceMap"].as_str().context("map")?)?,
                        p::mapped_sources(&capture, &compiled, target, kind)?,
                    ),
                );
            }
            let mut save = |_n: usize, v: &Value| -> anyhow::Result<()> {
                let n = raw_inventory.len() + 1;
                let raw_file = format!("case-{n:04}-raw.json");
                let raw = serde_json::to_vec(v)?;
                fs::write(out.join(&raw_file), &raw)?;
                let raw_entry = json!({"file":raw_file,"sha256":p::sha(&raw),"bytes":raw.len(),"name":v["name"],"target":target.label()});
                raw_inventory.push(raw_entry.clone());
                let kind = v["code_kind"].as_str().context("code kind")?;
                ensure!(["bytecode", "deployedBytecode"].contains(&kind), "explicit code kind");
                let code = if kind == "bytecode" {
                    let code = hex::decode(v["constructor_code"].as_str().context("constructor code")?)?;
                    ensure!(
                        code.len() == creation.len() + 96 && code.starts_with(&creation),
                        "exact creation prefix and bounded explicit append"
                    );
                    code
                } else {
                    runtime.clone()
                };
                ensure!(v["code_sha256"] == p::sha(&code), "actual executed code binding");
                let (map, sources) = &maps[kind];
                let mut record = v.clone();
                record["target"] = json!(target.label());
                for field in ["writes", "reads", "logs", "keccaks"] {
                    for e in record["execution"][field].as_array_mut().context("effect array")? {
                        mapped_effects += 1;
                        e["source"] = p::source_map::describe(e["pc"].as_u64().context("PC")? as usize, map, sources)?;
                        ensure!(e["source"]["path"].is_string(), "source annotation must resolve each effect");
                    }
                }
                for step in record["execution"]["trace"].as_array().context("trace")? {
                    *opcodes.entry(format!("0x{:02x}", step["opcode"].as_u64().context("opcode")?)).or_default() += 1;
                }
                let raw = serde_json::to_vec(&record)?;
                let file = format!("case-{n:04}.json");
                fs::write(out.join(&file), &raw)?;
                let entry = json!({"file":file,"raw_artifact":raw_entry,"target":target.label(),"name":v["name"],"scope":v["scope"],"sha256":p::sha(&raw),"bytes":raw.len(),"exit":v["execution"]["exit"],"steps":v["execution"]["trace"].as_array().context("trace")?.len(),"writes":v["execution"]["writes"].as_array().context("writes")?.len(),"logs":v["execution"]["logs"].as_array().context("logs")?.len()});
                cases.push(entry.clone());
                record["full_trace_artifact"] = entry;
                record["execution"].as_object_mut().context("execution")?.remove("trace");
                let getter = v["signature"].as_str().is_some_and(|s| {
                    s.starts_with("getRole")
                        || s.starts_with("hasRole")
                        || [
                            "balanceOf(address)",
                            "allowance(address,address)",
                            "totalSupply()",
                            "MaxSupply()",
                            "nonces(address)",
                            "DOMAIN_SEPARATOR()",
                            "feeRatio()",
                            "buyFeeRatio()",
                            "feeReceiver()",
                            "buyFeeReceiver()",
                            "mainPair()",
                        ]
                        .contains(&s)
                });
                if !getter || v["execution"]["exit"]["kind"] != "return" {
                    compact.push(record);
                }
                Ok(())
            };
            let mut proof = Proof::new(&runtime, target);
            proof.constructor(&capture, &mut save)?;
            proof.matrix(&mut save)?;
            for v in &proof.cases {
                ensure!(
                    (v["execution"]["exit"]["kind"] == "harness_failure") == (v["scope"] == "unsupported_path_control")
                        && (v["execution"]["exit"]["kind"] == "invalid") == (v["scope"] == "source_invalid_control"),
                    "exact per-case boundary classification"
                );
            }
            let count = |kind: &str| proof.cases.iter().filter(|v| v["execution"]["exit"]["kind"] == kind).count();
            targets.push(json!({"target":target.label(),"address":target.address(),"runtime_keccak256":target.runtime_hash(),"runtime_bytes":target.runtime_len(),"whole_compiler_output_sha256":target.compiled_sha(),"calls":proof.calls,"return":count("return"),"revert":count("revert"),"explicit_source_invalid":count("invalid"),"explicit_unsupported":count("harness_failure")}));
            all.extend(proof.cases);
        }
        let iraw = serde_json::to_vec_pretty(&cases)?;
        fs::write(out.join("cases-inventory.json"), &iraw)?;
        let traw = serde_json::to_vec_pretty(&compact)?;
        fs::write(out.join("operation-transcripts.json"), &traw)?;
        let mut artifacts = vec![];
        for a in sr["artifacts"].as_array().context("artifacts")? {
            let file = a["file"].as_str().context("artifact")?;
            let digest = p::sha(&fs::read(source.join(file))?);
            ensure!(a["sha256"] == digest, "source artifact unchanged {file}");
            artifacts.push(json!({"file":file,"sha256":digest}));
        }
        artifacts.push(json!({"file":"report.json","sha256":p::sha(&fs::read(source.join("report.json"))?)}));
        let count = |kind: &str| all.iter().filter(|v| v["execution"]["exit"]["kind"] == kind).count();
        let steps: usize = opcodes.values().sum();
        Ok(
            json!({"status":"passed","qualified":false,"scope":p::LIMITS,"source_gap":p::PRIMARY_GAP,"args":args,"source_inventory_sha256":source_inventory,"compiler_source_inventory_sha256":sr["source_inventory_sha256"],"source_trees_equal":source_inventory==sr["source_inventory_sha256"],"source_artifacts":artifacts,"targets":targets,"calls":all.len(),"returned_calls":count("return"),"reverted_calls":count("revert"),"explicit_source_invalid_controls":count("invalid"),"explicit_unsupported_path_controls":count("harness_failure"),"unexpected_invalid_or_harness_failure_calls":0,"compact_transcripts":compact.len(),"cases_inventory_sha256":p::sha(&iraw),"transcripts_sha256":p::sha(&traw),"full_opcode_records":steps,"mapped_effect_annotations":mapped_effects,"executed_opcode_counts":opcodes,"chain_calls":0}),
        )
    })();
    fs::write(out.join("raw-cases-inventory.json"), serde_json::to_vec_pretty(&raw_inventory)?)?;
    let (report, status) = match result {
        Ok(v) => (v, Ok(())),
        Err(e) => (
            json!({"status":"failed","qualified":false,"args":args,"attempted_calls":raw_inventory.len(),"annotated_calls":cases.len(),"raw_inventory_sha256":p::sha(&fs::read(out.join("raw-cases-inventory.json"))?),"error":format!("{e:#}")}),
            Err(e),
        ),
    };
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    status
}
