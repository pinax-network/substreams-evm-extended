#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::btr_proof::{self as proof, cases::Proof};
    use serde_json::{json, Value};
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: execute_btr_proof SOURCE_PROOF_DIRECTORY FRESH_OUTPUT");
    let source = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir(out).context("fresh output required")?;
    let mut attempted = 0;
    let result = (|| -> anyhow::Result<Value> {
        let source_inventory = proof::snapshot_sources(out)?;
        let mut captures = vec![];
        let mut outputs = vec![];
        let mut bindings = vec![];
        for p in &proof::CAPTURES {
            let raw = fs::read(source.join(format!("{}-capture.json", p.label)))?;
            let c = proof::verify_capture(&raw, p)?;
            let raw = fs::read(source.join(format!("{}-compiler-output.json", p.label)))?;
            let o = proof::verify_compiled(&c, &raw, p)?;
            bindings.push(json!({"capture":p.label,"capture_sha256":p.sha,"compiler_output_sha256":proof::sha(&raw),"runtime_keccak256":p.runtime_hash}));
            captures.push(c);
            outputs.push(o);
        }
        let primary: Value = serde_json::from_slice(&fs::read(source.join("primary-sources.json"))?)?;
        proof::verify_primary(&captures, &primary)?;
        let p = &proof::CAPTURES[0];
        let capture = &captures[0];
        let compiled = &outputs[0];
        let runtime = proof::runtime(capture)?;
        let contract = &compiled["contracts"][p.source][p.name];
        let mut maps = std::collections::BTreeMap::new();
        for kind in ["bytecode", "deployedBytecode"] {
            let code = proof::bytes(&contract["evm"][kind]["object"])?;
            maps.insert(
                kind,
                (
                    proof::source_map::decode(&code, contract["evm"][kind]["sourceMap"].as_str().context("source map")?)?,
                    proof::mapped_sources(capture, compiled, p, kind)?,
                ),
            );
        }
        let mut case_inventory = vec![];
        let mut compact = vec![];
        let mut opcodes = std::collections::BTreeMap::<String, usize>::new();
        let mut save = |n: usize, v: &Value| -> anyhow::Result<()> {
            attempted = n;
            // Raw attempted effects survive annotation or expectation failure.
            fs::write(out.join(format!("case-{n:04}-raw.json")), format!("{}\n", serde_json::to_string(v)?))?;
            let kind = if v["name"] == "implementation_constructor" {
                "bytecode"
            } else {
                "deployedBytecode"
            };
            let (map, sources) = &maps[kind];
            let mut record = v.clone();
            record["code_kind"] = json!(kind);
            for field in ["writes", "reads", "logs", "keccaks"] {
                for effect in record["execution"][field].as_array_mut().context("effect array")? {
                    effect["source"] = proof::source_map::describe(effect["pc"].as_u64().context("effect pc")? as usize, map, sources)?;
                }
            }
            for step in record["execution"]["trace"].as_array().context("trace")? {
                *opcodes.entry(format!("0x{:02x}", step["opcode"].as_u64().context("opcode")?)).or_default() += 1;
            }
            let raw = format!("{}\n", serde_json::to_string(&record)?).into_bytes();
            let filename = format!("case-{n:04}.json");
            fs::write(out.join(&filename), &raw)?;
            let entry = json!({"file":filename,"name":v["name"],"sha256":proof::sha(&raw),"bytes":raw.len(),"steps":v["execution"]["trace"].as_array().unwrap().len(),"exit":v["execution"]["exit"],"writes":v["execution"]["writes"].as_array().unwrap().len(),"logs":v["execution"]["logs"].as_array().unwrap().len()});
            case_inventory.push(entry.clone());
            record["full_trace_artifact"] = entry;
            record["execution"].as_object_mut().unwrap().remove("trace");
            if !v["signature"].as_str().is_some_and(|s| {
                s.starts_with("getRole")
                    || s.starts_with("hasRole")
                    || s.starts_with("balanceOf")
                    || s == "totalSupply()"
                    || s == "owner()"
                    || s == "queryWhitelisted()"
            }) {
                compact.push(record);
            }
            Ok(())
        };
        let mut proof_run = Proof::new(&runtime);
        proof_run.constructor(&proof::bytes(&capture["creationBytecode"]["onchainBytecode"])?, &mut save)?;
        proof_run.matrix(&mut save)?;
        let cases_raw = serde_json::to_vec_pretty(&case_inventory)?;
        fs::write(out.join("cases-inventory.json"), &cases_raw)?;
        let transcripts = serde_json::to_vec_pretty(&compact)?;
        fs::write(out.join("operation-transcripts.json"), &transcripts)?;
        let mut source_artifacts = vec![];
        for p in &proof::CAPTURES {
            for suffix in ["capture.json", "compiler-input-original.json", "compiler-input.json", "compiler-output.json"] {
                let path = source.join(format!("{}-{suffix}", p.label));
                source_artifacts.push(json!({"path":path,"sha256":proof::sha(&fs::read(&path)?)}));
            }
        }
        for name in [
            "report.json",
            "primary-sources.json",
            "solc-list.json",
            "compiler-version.txt",
            "LICENSE-implementation",
            "LICENSE-proxy",
        ] {
            let path = source.join(name);
            source_artifacts.push(json!({"path":path,"sha256":proof::sha(&fs::read(&path)?)}));
        }
        Ok(
            json!({"status":"passed","qualified":false,"scope":"BTR exact implementation execution in synthetic local accounts; no actual proxy dispatch, current chain state, ingestion candidate or package qualification","source_inventory_sha256":source_inventory,"cases_inventory_sha256":proof::sha(&cases_raw),"transcripts_sha256":proof::sha(&transcripts),"source_artifacts":source_artifacts,"bindings":bindings,"calls":proof_run.calls,"returned_calls":proof_run.cases.iter().filter(|v|v["execution"]["exit"]["kind"]=="return").count(),"reverted_calls":proof_run.cases.iter().filter(|v|v["execution"]["exit"]["kind"]=="revert").count(),"invalid_or_harness_failure_calls":0,"executed_opcode_counts":opcodes,"local_contexts":{"constructor":{"address":proof::IMPLEMENTATION,"self_code_size":0},"deployed_implementation":{"address":proof::IMPLEMENTATION,"self_code_size":15308},"synthetic_proxy":{"address":proof::PROXY,"self_code_size":183}},"unsupported":"external account context, calls/delegatecall/staticcall, UUPS execution, permit/ecrecover, gas/refunds/fork cost model","chain_calls":0,"token_primary_gap":proof::PRIMARY_GAP}),
        )
    })();
    let (report, result) = match result {
        Ok(report) => (report, Ok(())),
        Err(e) => (
            json!({"status":"failed","qualified":false,"attempted_calls":attempted,"error":format!("{e:#}")}),
            Err(e),
        ),
    };
    fs::write(out.join("report.json"), format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    result
}
