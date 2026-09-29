#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::erc20tokenx_proof::{self as proof, cases::Proof};
    use serde_json::{json, Value};
    use std::{collections::BTreeMap, fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: execute_erc20tokenx_proof SOURCE_PROOF_DIRECTORY FRESH_OUTPUT");
    let source = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir(out).context("fresh output required")?;
    let mut attempted = 0;
    let result = (|| -> anyhow::Result<Value> {
        let inventory = proof::snapshot_sources(out)?;
        let mut captures = vec![];
        for (i, (label, _, _)) in proof::CAPTURES.iter().enumerate() {
            captures.push(proof::verify_capture(&fs::read(source.join(format!("{label}-capture.json")))?, i)?);
        }
        let c = &captures[0];
        let compiled_raw = fs::read(source.join("compiler-output.json"))?;
        let compiled = proof::verify_compiled(c, &compiled_raw)?;
        let primary: Value = serde_json::from_slice(&fs::read(source.join("primary-sources.json"))?)?;
        for capture in &captures {
            proof::verify_auxiliary(source, capture)?;
            proof::verify_primary(capture, &primary)?;
            proof::verify_compiled(capture, &compiled_raw)?;
        }
        let runtime = proof::runtime(c)?;
        proof::verify_phi(source, &runtime)?;
        ensure!(
            runtime == proof::bytes(&compiled["contracts"][proof::SOURCE][proof::NAME]["evm"]["deployedBytecode"]["object"])?,
            "entire executable bytes exact"
        );
        let mut maps = BTreeMap::new();
        for kind in ["bytecode", "deployedBytecode"] {
            let v = &compiled["contracts"][proof::SOURCE][proof::NAME]["evm"][kind];
            maps.insert(
                kind,
                (
                    proof::source_map::decode(&proof::bytes(&v["object"])?, v["sourceMap"].as_str().context("map")?)?,
                    proof::mapped_sources(c, &compiled, kind)?,
                ),
            );
        }
        let mut cases = vec![];
        let mut compact = vec![];
        let mut opcodes = BTreeMap::<String, usize>::new();
        let mut mapped_effects = 0usize;
        let mut save = |n: usize, v: &Value| -> anyhow::Result<()> {
            attempted = n;
            fs::write(out.join(format!("case-{n:04}-raw.json")), format!("{}\n", serde_json::to_string(v)?))?;
            let kind = if v["code_kind"] == "bytecode" { "bytecode" } else { "deployedBytecode" };
            let (map, sources) = &maps[kind];
            let mut record = v.clone();
            record["code_kind"] = json!(kind);
            for field in ["writes", "reads", "logs", "keccaks"] {
                for e in record["execution"][field].as_array_mut().context("effect array")? {
                    mapped_effects += 1;
                    e["source"] = proof::source_map::describe(e["pc"].as_u64().context("PC")? as usize, map, sources)?;
                    ensure!(e["source"]["source"] != "unmapped instruction", "effect outside source-map");
                }
            }
            for step in record["execution"]["trace"].as_array().context("trace")? {
                *opcodes.entry(format!("0x{:02x}", step["opcode"].as_u64().context("opcode")?)).or_default() += 1;
            }
            let raw = format!("{}\n", serde_json::to_string(&record)?).into_bytes();
            let file = format!("case-{n:04}.json");
            fs::write(out.join(&file), &raw)?;
            let entry = json!({"file":file,"name":v["name"],"scope":v["scope"],"sha256":proof::sha(&raw),"bytes":raw.len(),"steps":v["execution"]["trace"].as_array().unwrap().len(),"exit":v["execution"]["exit"],"writes":v["execution"]["writes"].as_array().unwrap().len(),"logs":v["execution"]["logs"].as_array().unwrap().len()});
            cases.push(entry.clone());
            record["full_trace_artifact"] = entry;
            record["execution"].as_object_mut().unwrap().remove("trace");
            let getter = v["signature"].as_str().is_some_and(|s| {
                s.starts_with("getRole") || s.starts_with("hasRole") || s == "balanceOf(address)" || s == "allowance(address,address)" || s == "totalSupply()"
            });
            if !getter || v["execution"]["exit"]["kind"] != "return" {
                compact.push(record);
            }
            Ok(())
        };
        let mut p = Proof::new(&runtime);
        for c in &captures {
            p.constructor(c, &mut save)?;
        }
        p.matrix(&mut save)?;
        let cases_raw = serde_json::to_vec_pretty(&cases)?;
        fs::write(out.join("cases-inventory.json"), &cases_raw)?;
        let transcripts = serde_json::to_vec_pretty(&compact)?;
        fs::write(out.join("operation-transcripts.json"), &transcripts)?;
        let count = |kind: &str| p.cases.iter().filter(|v| v["execution"]["exit"]["kind"] == kind).count();
        let unsupported = p.cases.iter().filter(|v| v["scope"] == "unsupported_path_control").count();
        let invalid = p.cases.iter().filter(|v| v["scope"] == "source_invalid_control").count();
        for record in &p.cases {
            ensure!(
                (record["execution"]["exit"]["kind"] == "harness_failure") == (record["scope"] == "unsupported_path_control")
                    && (record["execution"]["exit"]["kind"] == "invalid") == (record["scope"] == "source_invalid_control"),
                "each excluded or bounds outcome needs its exact independent case classification"
            );
        }
        ensure!(
            count("harness_failure") == unsupported && count("invalid") == invalid,
            "only explicit exact excluded/bounds controls can fail execution"
        );
        let mut artifacts = vec![];
        for name in [
            "ori-capture.json",
            "fna-capture.json",
            "phi-source-request.json",
            "phi-runtime.hex",
            "phi-runtime-report.json",
            "compiler-input-original.json",
            "compiler-input.json",
            "compiler-output.json",
            "report.json",
            "primary-sources.json",
            "solc-list.json",
            "compiler-version.txt",
            "LICENSE-openzeppelin",
        ] {
            let path = source.join(name);
            artifacts.push(json!({"path":path,"sha256":proof::sha(&fs::read(&path)?)}));
        }
        let opcode_steps: usize = opcodes.values().sum();
        Ok(
            json!({"status":"passed","qualified":false,"scope":"ERC20TokenX exact selected runtime in synthetic local state; ORI/FNA complete capture bindings and PHI exact-runtime-only attribution; no production admission","source_inventory_sha256":inventory,"cases_inventory_sha256":proof::sha(&cases_raw),"transcripts_sha256":proof::sha(&transcripts),"source_artifacts":artifacts,"compiler_output_sha256":proof::COMPILED_SHA,"runtime_keccak256":proof::RUNTIME_HASH,"runtime_bytes":7896,"runtime_substitutions":0,"calls":p.calls,"returned_calls":count("return"),"reverted_calls":count("revert"),"explicit_source_invalid_controls":invalid,"explicit_unsupported_path_controls":unsupported,"unexpected_invalid_or_harness_failure_calls":0,"full_opcode_records":opcode_steps,"mapped_effect_annotations":mapped_effects,"executed_opcode_counts":opcodes,"source_gap":proof::PRIMARY_GAP,"creation":"complete ORI/FNA creation and argument bytes bound, execution stops at unsupported CHAINID; no initialized-set proof","unsupported":"chain context, successful fee-receiver external calls, permit success, gas/refunds/fork costs, live producer/preimage/noop visibility, initial set coherence, runtime continuity, ingestion candidate","chain_calls":0}),
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
