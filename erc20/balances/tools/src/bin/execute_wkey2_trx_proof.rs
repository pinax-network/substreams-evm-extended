#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::wkey2_trx_proof::{self as p, cases::Proof};
    use serde_json::{json, Value};
    use std::{collections::BTreeMap, fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: execute_wkey2_trx_proof SOURCE_DIRECTORY FRESH_OUTPUT");
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
        let mut targets = vec![];
        let mut opcodes = BTreeMap::<String, usize>::new();
        let mut exits = BTreeMap::<String, usize>::new();
        let mut mapped_effects = 0usize;
        let mut generated_effects = 0usize;
        let mut paired_calls = 0usize;
        for (target, capture, compiled) in bindings {
            let contract = &compiled["contracts"][target.source()][target.name()];
            let mut maps = BTreeMap::new();
            for kind in ["bytecode", "deployedBytecode"] {
                let v = &contract["evm"][kind];
                maps.insert(
                    kind,
                    (
                        p::source_map::decode(&p::bytes(&v["object"])?, v["sourceMap"].as_str().context("map")?)?,
                        p::mapped_sources(&capture, &compiled, target, kind)?,
                    ),
                );
            }
            let compiler_runtime = p::bytes(&contract["evm"]["deployedBytecode"]["object"])?;
            let compiler_creation = [p::bytes(&contract["evm"]["bytecode"]["object"])?, p::arguments(target)].concat();
            let captured_runtime = p::runtime(&capture)?;
            let captured_creation = p::bytes(&capture["creationBytecode"]["onchainBytecode"])?;
            let mut compiler_cases = None;
            let mut variant_reports = vec![];
            for (variant, runtime, creation) in [
                ("compiled", &compiler_runtime, &compiler_creation),
                ("captured", &captured_runtime, &captured_creation),
            ] {
                let mut save = |_n: usize, v: &Value| -> anyhow::Result<()> {
                    let n = raw_inventory.len() + 1;
                    let raw_file = format!("case-{n:04}-raw.json");
                    let raw = serde_json::to_vec(v)?;
                    fs::write(out.join(&raw_file), &raw)?;
                    let raw_entry = json!({"file":raw_file,"sha256":p::sha(&raw),"bytes":raw.len(),"name":v["name"],"target":target.label(),"variant":variant});
                    raw_inventory.push(raw_entry.clone());
                    let kind = v["code_kind"].as_str().context("explicit code kind")?;
                    let code = match kind {
                        "bytecode" => {
                            ensure!(
                                hex::decode(v["constructor_code"].as_str().context("full constructor code")?)? == *creation,
                                "exact complete creation and independent constructor append"
                            );
                            creation
                        }
                        "deployedBytecode" => runtime,
                        _ => anyhow::bail!("unknown code kind"),
                    };
                    ensure!(v["code_sha256"] == p::sha(code), "actual executed code binding");
                    p::verify_trace(&v["execution"], target, kind == "bytecode")?;
                    let (map, sources) = &maps[kind];
                    let mut record = v.clone();
                    record["target"] = json!(target.label());
                    record["variant"] = json!(variant);
                    for field in ["writes", "reads", "logs", "keccaks"] {
                        for e in record["execution"][field].as_array_mut().context("effect array")? {
                            e["source"] = p::annotate_effect(e["pc"].as_u64().context("PC")? as usize, field, target, kind == "bytecode", code, map, sources)?;
                            if e["source"]["attribution"] == "exact_solidity_source" {
                                mapped_effects += 1;
                            } else {
                                generated_effects += 1;
                            }
                        }
                    }
                    for step in record["execution"]["trace"].as_array().context("trace")? {
                        *opcodes.entry(format!("0x{:02x}", step["opcode"].as_u64().context("opcode")?)).or_default() += 1;
                    }
                    let raw = serde_json::to_vec(&record)?;
                    let file = format!("case-{n:04}.json");
                    fs::write(out.join(&file), &raw)?;
                    let entry = json!({"file":file,"raw_artifact":raw_entry,"target":target.label(),"variant":variant,"name":v["name"],"scope":v["scope"],"sha256":p::sha(&raw),"bytes":raw.len(),"exit":v["execution"]["exit"],"steps":v["execution"]["trace"].as_array().context("trace")?.len(),"writes":v["execution"]["writes"].as_array().context("writes")?.len(),"logs":v["execution"]["logs"].as_array().context("logs")?.len()});
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
                                "nonces(address)",
                                "getNonce(address)",
                                "DOMAIN_SEPARATOR()",
                                "getDomainSeperator()",
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
                let mut proof = Proof::new(runtime, target);
                proof.constructor(&capture, creation, &mut save)?;
                proof.matrix(&mut save)?;
                ensure!(proof.calls == proof.cases.len(), "one record for every attempted call");
                for v in &proof.cases {
                    let kind = v["execution"]["exit"]["kind"].as_str().context("exit kind")?;
                    ensure!(["return", "revert", "invalid", "harness_failure"].contains(&kind), "known exit kind");
                    ensure!(
                        (kind == "harness_failure") == (v["scope"] == "unsupported_path_control")
                            && (kind == "invalid") == (v["scope"] == "source_invalid_control"),
                        "exact per-case boundary classification"
                    );
                    *exits.entry(kind.into()).or_default() += 1;
                }
                let count = |kind: &str| proof.cases.iter().filter(|v| v["execution"]["exit"]["kind"] == kind).count();
                variant_reports.push(json!({"variant":variant,"calls":proof.calls,"return":count("return"),"revert":count("revert"),"explicit_source_invalid":count("invalid"),"explicit_unsupported":count("harness_failure")}));
                // Only the bytes identifying the two programs differ. Every
                // original calldata, context, step, effect and exit must agree.
                for v in &mut proof.cases {
                    let object = v.as_object_mut().context("case")?;
                    object.remove("code_sha256");
                    object.remove("constructor_code");
                }
                if variant == "compiled" {
                    compiler_cases = Some(proof.cases);
                } else {
                    ensure!(
                        compiler_cases.as_ref().context("compiler cases")? == &proof.cases,
                        "exact compiler/captured full execution equality"
                    );
                    paired_calls += proof.calls;
                }
            }
            targets.push(json!({"target":target.label(),"address":target.address(),"runtime_keccak256":target.runtime_hash(),"runtime_bytes":target.runtime_len(),"whole_compiler_output_sha256":target.compiled_sha(),"variants":variant_reports}));
        }
        let iraw = serde_json::to_vec_pretty(&cases)?;
        fs::write(out.join("cases-inventory.json"), &iraw)?;
        let traw = serde_json::to_vec_pretty(&compact)?;
        fs::write(out.join("operation-transcripts.json"), &traw)?;
        let mut artifacts = vec![];
        let expected: Vec<String> = ["solc-list.json", "LICENSE-openzeppelin"]
            .iter()
            .map(|s| (*s).into())
            .chain(p::Target::ALL.into_iter().flat_map(|t| {
                [
                    "capture.json",
                    "compiler-input-original.json",
                    "compiler-input.json",
                    "compiler-output.json",
                    "compiler-version.txt",
                    "metadata-original.json",
                    "metadata-fresh.json",
                    "primary-sources.json",
                    "source-licenses.json",
                ]
                .into_iter()
                .map(move |n| format!("{}/{n}", t.label()))
            }))
            .collect();
        let reported = sr["artifacts"].as_array().context("artifacts")?;
        ensure!(reported.len() == expected.len(), "complete fixed source artifact inventory");
        for (a, file) in reported.iter().zip(expected) {
            ensure!(a["file"] == file, "exact source artifact name");
            let digest = p::sha(&fs::read(source.join(&file))?);
            ensure!(a["sha256"] == digest, "source artifact unchanged {file}");
            artifacts.push(json!({"file":file,"sha256":digest}));
        }
        for file in ["report.json", "source-inventory.json"] {
            artifacts.push(json!({"file":file,"sha256":p::sha(&fs::read(source.join(file))?)}));
        }
        let steps: usize = opcodes.values().sum();
        Ok(
            json!({"status":"passed","qualified":false,"scope":p::LIMITS,"source_gap":p::PRIMARY_GAP,"args":args,"source_inventory_sha256":source_inventory,"compiler_source_inventory_sha256":sr["source_inventory_sha256"],"source_trees_equal":source_inventory==sr["source_inventory_sha256"],"source_artifacts":artifacts,"targets":targets,"calls":cases.len(),"returned_calls":exits.get("return").copied().unwrap_or(0),"reverted_calls":exits.get("revert").copied().unwrap_or(0),"explicit_source_invalid_controls":exits.get("invalid").copied().unwrap_or(0),"explicit_unsupported_path_controls":exits.get("harness_failure").copied().unwrap_or(0),"unexpected_invalid_or_harness_failure_calls":0,"compiler_captured_case_pairs":paired_calls,"all_case_pairs_exact":true,"cbor_excluded_from_executed_pc_immediates_codecopy":true,"constructor_initialization_complete":false,"compact_transcripts":compact.len(),"cases_inventory_sha256":p::sha(&iraw),"transcripts_sha256":p::sha(&traw),"full_opcode_records":steps,"mapped_effect_annotations":mapped_effects,"pinned_generated_effect_annotations_without_source_text":generated_effects,"total_effect_annotations":mapped_effects+generated_effects,"executed_opcode_counts":opcodes,"chain_calls":0}),
        )
    })();
    fs::write(out.join("raw-cases-inventory.json"), serde_json::to_vec_pretty(&raw_inventory)?)?;
    // Retain the completed annotated prefix even when a later expectation fails.
    fs::write(out.join("cases-inventory.json"), serde_json::to_vec_pretty(&cases)?)?;
    fs::write(out.join("operation-transcripts.json"), serde_json::to_vec_pretty(&compact)?)?;
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
