#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::securities_proof::{self as proof, cases::Proof};
    use serde_json::{json, Value};
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: execute_securities_proof SOURCE_PROOF_DIRECTORY FRESH_OUTPUT");
    let source = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir(out).context("fresh output required")?;
    let mut attempted = 0;
    let result = (|| -> anyhow::Result<Value> {
        let source_inventory = proof::snapshot_sources(out)?;
        let capture = proof::verify_capture(&fs::read(source.join("capture.json"))?)?;
        proof::verify_auxiliary(source, &capture)?;
        proof::verify_metadata_files(source)?;
        let compiled = proof::verify_compiled(&capture, &fs::read(source.join("compiler-output.json"))?)?;
        let primary: Value = serde_json::from_slice(&fs::read(source.join("primary-sources.json"))?)?;
        proof::verify_primary(&capture, &primary)?;
        let runtime = proof::runtime(&capture)?;
        let contract = &compiled["contracts"][proof::SOURCE][proof::NAME];
        let mut maps = std::collections::BTreeMap::new();
        for kind in ["bytecode", "deployedBytecode"] {
            let code = proof::bytes(&contract["evm"][kind]["object"])?;
            maps.insert(
                kind,
                (
                    proof::source_map::decode(&code, contract["evm"][kind]["sourceMap"].as_str().context("source map")?)?,
                    proof::mapped_sources(&capture, &compiled, kind)?,
                ),
            );
        }
        let mut inventory = vec![];
        let mut compact = vec![];
        let mut opcodes = std::collections::BTreeMap::<String, usize>::new();
        let mut save = |n: usize, v: &Value| -> anyhow::Result<()> {
            attempted = n;
            fs::write(out.join(format!("case-{n:04}-raw.json")), format!("{}\n", serde_json::to_string(v)?))?;
            let kind = if v["name"] == "synthetic_implementation_constructor" {
                "bytecode"
            } else {
                "deployedBytecode"
            };
            let (map, sources) = &maps[kind];
            let mut record = v.clone();
            record["code_kind"] = json!(kind);
            for field in ["writes", "reads", "logs", "keccaks"] {
                for effect in record["execution"][field].as_array_mut().context("effect array")? {
                    effect["source"] = proof::source_map::describe(effect["pc"].as_u64().context("effect PC")? as usize, map, sources)?;
                    ensure!(effect["source"]["source"] != "unmapped instruction", "executed effect without source-map entry");
                }
            }
            for step in record["execution"]["trace"].as_array().context("trace")? {
                *opcodes.entry(format!("0x{:02x}", step["opcode"].as_u64().context("opcode")?)).or_default() += 1;
            }
            let raw = format!("{}\n", serde_json::to_string(&record)?).into_bytes();
            let filename = format!("case-{n:04}.json");
            fs::write(out.join(&filename), &raw)?;
            let entry = json!({"file":filename,"name":v["name"],"scope":v["scope"],"sha256":proof::sha(&raw),"bytes":raw.len(),"steps":v["execution"]["trace"].as_array().unwrap().len(),"exit":v["execution"]["exit"],"writes":v["execution"]["writes"].as_array().unwrap().len(),"logs":v["execution"]["logs"].as_array().unwrap().len()});
            inventory.push(entry.clone());
            record["full_trace_artifact"] = entry;
            record["execution"].as_object_mut().unwrap().remove("trace");
            // Complete getter traces remain in the hashed case inventory; the
            // committed compact evidence keeps mutations/failures/exclusions.
            let getter = v["signature"].as_str().is_some_and(|s| {
                s.starts_with("getRole") || s.starts_with("hasRole") || s == "balanceOf(address)" || s == "allowance(address,address)" || s == "totalSupply()"
            });
            if !getter || v["scope"] == "unsupported_path_control" {
                compact.push(record);
            }
            Ok(())
        };
        let mut run = Proof::new(&runtime);
        run.constructor(&proof::bytes(&capture["creationBytecode"]["recompiledBytecode"])?, &mut save)?;
        run.matrix(&mut save)?;
        let cases_raw = serde_json::to_vec_pretty(&inventory)?;
        fs::write(out.join("cases-inventory.json"), &cases_raw)?;
        let transcripts = serde_json::to_vec_pretty(&compact)?;
        fs::write(out.join("operation-transcripts.json"), &transcripts)?;
        let mut artifacts = vec![];
        for name in [
            "capture.json",
            "compiler-input-original.json",
            "compiler-input.json",
            "compiler-output.json",
            "metadata-fresh.json",
            "metadata-saved.json",
            "report.json",
            "primary-sources.json",
            "solc-list.json",
            "compiler-version.txt",
            "LICENSE-openzeppelin",
            "LICENSE-openzeppelin-upgradeable",
            "LICENSE-bep677",
        ] {
            let path = source.join(name);
            artifacts.push(json!({"path":path,"sha256":proof::sha(&fs::read(&path)?)}));
        }
        let count = |kind: &str| run.cases.iter().filter(|v| v["execution"]["exit"]["kind"] == kind).count();
        let unsupported = run.cases.iter().filter(|v| v["scope"] == "unsupported_path_control").count();
        ensure!(
            count("harness_failure") == unsupported && count("invalid") == 0,
            "only explicitly excluded paths may fail the harness"
        );
        Ok(
            json!({"status":"passed","qualified":false,"scope":"selected SecuritiesToken compiled implementation in synthetic local accounts; no proxy/beacon dispatch, deployed initialization/history, ingestion candidate or qualification","source_inventory_sha256":source_inventory,"cases_inventory_sha256":proof::sha(&cases_raw),"transcripts_sha256":proof::sha(&transcripts),"source_artifacts":artifacts,"capture_sha256":proof::CAPTURE_SHA,"compiler_output_sha256":proof::COMPILED_SHA,"runtime_keccak256":proof::RUNTIME_HASH,"calls":run.calls,"returned_calls":count("return"),"reverted_calls":count("revert"),"explicit_unsupported_path_controls":unsupported,"unexpected_invalid_or_harness_failure_calls":0,"executed_opcode_counts":opcodes,"creation":"synthetic constructor from fresh compiler object only; all saved on-chain creation/deployment fields are null","source_gap":proof::PRIMARY_GAP,"unsupported":"initializer success, external client validation, transfers/mint/burn success, timed UI conversion, beacon/proxy calls, external account context, gas/refunds/fork costs","chain_calls":0}),
        )
    })();
    let (report, result) = match result {
        Ok(v) => (v, Ok(())),
        Err(e) => (
            json!({"status":"failed","qualified":false,"attempted_calls":attempted,"error":format!("{e:#}")}),
            Err(e),
        ),
    };
    fs::write(out.join("report.json"), format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    result
}
