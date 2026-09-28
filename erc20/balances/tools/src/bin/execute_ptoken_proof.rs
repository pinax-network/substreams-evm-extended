#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::ensure;
    use erc20_balances_tools::ptoken_proof::{self as proof, cases::Proof};
    use serde_json::{json, Value};
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: execute_ptoken_proof SOURCE_PROOF_DIRECTORY FRESH_OUTPUT");
    let source = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir(out)?;
    let mut attempted_calls = 0;
    let result = (|| -> anyhow::Result<Value> {
        let inventory_sha = proof::snapshot_sources(out)?;
        let capture = proof::verify_capture(&fs::read(source.join("capture.json"))?)?;
        let raw = fs::read(source.join("compiler-output.json"))?;
        let compiled = proof::verify_compiled(&capture, &raw)?;
        let runtime = proof::bytes(&compiled["contracts"][proof::SOURCE][proof::NAME]["evm"]["deployedBytecode"]["object"])?;
        ensure!(runtime == proof::bytes(&capture["runtimeBytecode"]["onchainBytecode"])?, "exact runtime");
        let contract = &compiled["contracts"][proof::SOURCE][proof::NAME];
        let mut maps = std::collections::BTreeMap::new();
        for kind in ["bytecode", "deployedBytecode"] {
            let code = proof::bytes(&contract["evm"][kind]["object"])?;
            maps.insert(
                kind,
                (
                    proof::source_map::decode(&code, contract["evm"][kind]["sourceMap"].as_str().unwrap())?,
                    proof::source_map::sources(&capture, &compiled, kind)?,
                ),
            );
        }
        let mut case_inventory = vec![];
        let mut compact = vec![];
        let mut opcodes = std::collections::BTreeMap::<String, usize>::new();
        let mut save = |n: usize, v: &Value| -> anyhow::Result<()> {
            attempted_calls = n;
            // Save the raw attempted execution even if source annotation fails.
            fs::write(out.join(format!("case-{n:04}-raw.json")), format!("{}\n", serde_json::to_string(v)?))?;
            let kind = if v["name"] == "captured_creation_with_exact_arguments" {
                "bytecode"
            } else {
                "deployedBytecode"
            };
            let (map, sources) = &maps[kind];
            let mut record = v.clone();
            record["code_kind"] = json!(kind);
            for field in ["writes", "reads", "logs", "keccaks"] {
                for effect in record["execution"][field].as_array_mut().unwrap() {
                    effect["source"] = proof::source_map::describe(effect["pc"].as_u64().unwrap() as usize, map, sources)?;
                }
            }
            for step in record["execution"]["trace"].as_array().unwrap() {
                *opcodes.entry(format!("0x{:02x}", step["opcode"].as_u64().unwrap())).or_default() += 1;
            }
            let raw = format!("{}\n", serde_json::to_string(&record)?).into_bytes();
            let filename = format!("case-{n:04}.json");
            fs::write(out.join(&filename), &raw)?;
            let entry = json!({"file":filename,"name":v["name"],"sha256":proof::sha(&raw),"bytes":raw.len(),"steps":v["execution"]["trace"].as_array().unwrap().len(),"exit":v["execution"]["exit"],"writes":v["execution"]["writes"].as_array().unwrap().len(),"logs":v["execution"]["logs"].as_array().unwrap().len()});
            case_inventory.push(entry.clone());
            record["full_trace_artifact"] = entry;
            record["execution"].as_object_mut().unwrap().remove("trace");
            // Keep effect source spans/preimages and committed results for all
            // mutating/refusal cases; getter traces remain in immutable artifacts.
            if !v["signature"]
                .as_str()
                .is_some_and(|s| s.starts_with("getRole") || s.starts_with("hasRole") || s.starts_with("balanceOf") || s == "totalSupply()")
            {
                compact.push(record);
            }
            Ok(())
        };
        let mut p = Proof::new(&runtime);
        p.constructor(&proof::bytes(&capture["creationBytecode"]["onchainBytecode"])?, &mut save)?;
        p.matrix(&mut save)?;
        let cases_raw = serde_json::to_vec_pretty(&case_inventory)?;
        fs::write(out.join("cases-inventory.json"), &cases_raw)?;
        fs::write(out.join("operation-transcripts.json"), serde_json::to_vec_pretty(&compact)?)?;
        Ok(
            json!({"source_inventory_sha256":inventory_sha,"cases_inventory_sha256":proof::sha(&cases_raw),"executed_opcode_counts":opcodes,"status":"passed","qualified":false,"scope":"synthetic account operation proof only; no ingestion candidate or chain execution","calls":p.calls,"returned_calls":p.cases.iter().filter(|c|c["execution"]["exit"]["kind"]=="return").count(),"reverted_calls":p.cases.iter().filter(|c|c["execution"]["exit"]["kind"]=="revert").count(),"invalid_or_unsupported_calls":0,"capture_sha256":proof::CAPTURE,"runtime_keccak256":proof::RUNTIME,"compiler_output_sha256":proof::sha(&raw)}),
        )
    })();
    match result {
        Ok(report) => {
            fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
            println!("{report}");
            Ok(())
        }
        Err(e) => {
            fs::write(
                out.join("report.json"),
                serde_json::to_vec_pretty(&json!({"status":"failed","attempted_calls":attempted_calls,"error":format!("{e:#}")}))?,
            )?;
            Err(e)
        }
    }
}
