#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::apd_dsg_controls::{self as p, cases::Proof, Target};
    use serde_json::{json, Value};
    use std::{collections::BTreeMap, fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 2, "usage: execute_apd_dsg_controls FRESH_OUTPUT");
    let out = Path::new(&args[1]);
    fs::create_dir(out).context("fresh output required")?;
    let mut raw_inventory = vec![];
    let mut cases = vec![];
    let mut compact = vec![];
    let result = (|| -> anyhow::Result<Value> {
        let inventory = p::snapshot(out)?;
        let mut bindings = vec![];
        let mut projections = vec![];
        let mut opcodes = BTreeMap::<String, usize>::new();
        let mut resolved = 0;
        let mut unresolved = 0;
        for t in [Target::Apd, Target::Dsg] {
            let (c, code) = p::bound(t)?;
            let map = p::source_map::decode(&code, c["runtimeBytecode"]["sourceMap"].as_str().context("map")?)?;
            bindings.push(json!({"target":t.label(),"address":t.address(),"chain_id":56,"capture_sha256":t.capture_sha(),"source_sha256":t.source_sha(),"runtime_keccak256":t.runtime_hash(),"compiler":t.compiler(),"compiler_settings":c["compilation"]["compilerSettings"],"creation_arguments":c["creationBytecode"]["transformationValues"]["constructorArguments"],"runtime_transformations":c["runtimeBytecode"]["transformations"],"runtime_transformation_values":c["runtimeBytecode"]["transformationValues"],"creation_transformations":c["creationBytecode"]["transformations"],"creation_transformation_values":c["creationBytecode"]["transformationValues"],"boundary_heights":[122288005,122289029],"boundary_files_sha256":t.runtime_file_sha(),"whole_compiler_artifact_sha256":p::sha(&serde_json::to_vec(&c["stdJsonOutput"])?),"fresh_compilation":false,"qualified":false}));
            let mut save = |v: &Value| -> anyhow::Result<()> {
                let n = raw_inventory.len() + 1;
                let raw = serde_json::to_vec(v)?;
                let file = format!("case-{n:04}-raw.json");
                fs::write(out.join(&file), &raw)?;
                let re = json!({"file":file,"sha256":p::sha(&raw),"bytes":raw.len(),"name":v["name"]});
                raw_inventory.push(re.clone());
                ensure!(v["code_sha256"] == p::sha(&code), "actual code bytes");
                let mut record = v.clone();
                let mut resolutions = BTreeMap::<String, Value>::new();
                for field in ["reads", "writes", "logs", "keccaks"] {
                    let mut yes = 0;
                    let mut no = 0;
                    for e in record["execution"][field].as_array_mut().context("effects")? {
                        let a = p::annotate(&c, e["pc"].as_u64().context("PC")? as usize, &map)?;
                        if a["path"].is_string() {
                            yes += 1;
                            resolved += 1;
                        } else {
                            no += 1;
                            unresolved += 1;
                        }
                        e["source"] = a;
                    }
                    resolutions.insert(field.into(), json!({"resolved":yes,"unresolved":no}));
                }
                record["source_coverage"] = json!(resolutions);
                for e in record["execution"]["trace"].as_array().context("trace")? {
                    *opcodes.entry(format!("0x{:02x}", e["opcode"].as_u64().context("opcode")?)).or_default() += 1;
                }
                let raw = serde_json::to_vec(&record)?;
                let file = format!("case-{n:04}.json");
                fs::write(out.join(&file), &raw)?;
                let entry = json!({"file":file,"sha256":p::sha(&raw),"bytes":raw.len(),"raw_artifact":re,"target":t.label(),"name":v["name"],"exit":v["execution"]["exit"],"source_coverage":resolutions});
                cases.push(entry.clone());
                record["full_trace_artifact"] = entry;
                record["execution"].as_object_mut().context("execution")?.remove("trace");
                if !v["scope"].as_str().unwrap_or("").contains("getter") {
                    compact.push(record);
                }
                Ok(())
            };
            let mut proof = Proof::new(t, &code);
            proof.getters(&mut save)?;
            if t == Target::Apd {
                proof.apd_operations(&mut save)?;
            } else {
                proof.dsg_operations(&mut save)?;
                proof.dsg_saved(&mut save)?;
            }
            projections.extend(proof.projections(&mut save)?);
        }
        let iraw = serde_json::to_vec_pretty(&cases)?;
        fs::write(out.join("cases-inventory.json"), &iraw)?;
        let traw = serde_json::to_vec_pretty(&compact)?;
        fs::write(out.join("operation-transcripts.json"), &traw)?;
        let praw = serde_json::to_vec_pretty(&projections)?;
        fs::write(out.join("projector-controls.json"), &praw)?;
        let count = |kind: &str| cases.iter().filter(|c| c["exit"]["kind"] == kind).count();
        Ok(
            json!({"status":"passed","qualified":false,"scope":p::LIMITS,"source_gap":p::SOURCE_GAP,"args":args,"source_inventory_sha256":inventory,"bindings":bindings,"compiler_artifact_digest_encoding":"serde_json::to_vec of the complete saved stdJsonOutput object; raw original record separately pinned, no new compiler stdout","calls":cases.len(),"returned_calls":count("return"),"reverted_calls":count("revert"),"unexpected_invalid_or_harness_failure_calls":0,"compact_transcripts":compact.len(),"projector_controls":projections.len(),"raw_metadata_projector_controls":projections.iter().filter(|p|p["kind"].is_string()).count(),"measured_operation_projector_controls":projections.iter().filter(|p|p["kind"].is_null()).count(),"mixed_balance_getter_combinations":projections.iter().map(|p|p["mixed_balance_getter_combinations"].as_u64().unwrap_or(0)).sum::<u64>(),"cases_inventory_sha256":p::sha(&iraw),"transcripts_sha256":p::sha(&traw),"projector_controls_sha256":p::sha(&praw),"resolved_effects":resolved,"unresolved_effects":unresolved,"full_opcode_records":opcodes.values().sum::<usize>(),"executed_opcode_counts":opcodes,"chain_calls":0,"fresh_compilation":false}),
        )
    })();
    fs::write(out.join("raw-cases-inventory.json"), serde_json::to_vec_pretty(&raw_inventory)?)?;
    let (report, status) = match result {
        Ok(v) => (v, Ok(())),
        Err(e) => (
            json!({"status":"failed","qualified":false,"args":args,"attempted_calls":raw_inventory.len(),"annotated_calls":cases.len(),"error":format!("{e:#}")}),
            Err(e),
        ),
    };
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    status
}
