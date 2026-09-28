#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::ptoken_proof as proof;
    use serde_json::{json, Value};
    use std::{
        fs,
        io::{Read, Write},
        path::Path,
        process::{Command, Stdio},
    };
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 4, "usage: build_ptoken_proof ORIGINAL_CAPTURE OFFICIAL_MACOS_SOLC FRESH_OUTPUT");
    let out = Path::new(&args[3]);
    fs::create_dir(out).context("fresh output required")?;
    let result = (|| -> anyhow::Result<Value> {
        let inventory_sha = proof::snapshot_sources(out)?;
        let raw = fs::read(&args[1])?;
        let capture = proof::verify_capture(&raw)?;
        fs::write(out.join("capture.json"), raw)?;
        let compiler = fs::canonicalize(&args[2])?;
        ensure!(proof::sha(&fs::read(&compiler)?) == proof::SOLC_SHA, "official compiler SHA256");
        let fetch = |url: &str, path: &Path| -> anyhow::Result<Vec<u8>> {
            let mut bytes = vec![];
            ureq::get(url).call()?.into_reader().read_to_end(&mut bytes)?;
            fs::write(path, &bytes)?;
            Ok(bytes)
        };
        let manifest_url = format!("https://raw.githubusercontent.com/ethereum/solc-bin/{}/macosx-amd64/list.json", proof::SOLC_PIN);
        let manifest_raw = fetch(&manifest_url, &out.join("solc-list.json"))?;
        let manifest: Value = serde_json::from_slice(&manifest_raw)?;
        let entry: Vec<_> = manifest["builds"]
            .as_array()
            .context("compiler builds")?
            .iter()
            .filter(|b| b["longVersion"] == proof::SOLC_VERSION)
            .collect();
        ensure!(
            entry.len() == 1
                && entry[0]["sha256"] == format!("0x{}", proof::SOLC_SHA)
                && entry[0]["path"] == format!("solc-macosx-amd64-v{}", proof::SOLC_VERSION),
            "official release association"
        );
        let license_url = format!(
            "https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{}/LICENSE",
            proof::OZ_PIN
        );
        let license = fetch(&license_url, &out.join("LICENSE-OZ"))?;
        let mut primary = serde_json::Map::new();
        for (name, s) in capture["sources"].as_object().unwrap() {
            let content = s["content"].as_str().context("source content")?;
            let path = out.join("sources").join(name);
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(&path, content)?;
            if name != proof::SOURCE {
                let suffix = name.strip_prefix("lib/openzeppelin-contracts/").context("exact captured dependency prefix")?;
                let url = format!(
                    "https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{}/{suffix}",
                    proof::OZ_PIN
                );
                let path = out.join("primary").join(suffix);
                fs::create_dir_all(path.parent().unwrap())?;
                let bytes = fetch(&url, &path)?;
                ensure!(bytes == content.as_bytes(), "exact primary dependency {name}");
                primary.insert(name.clone(), json!({"url":url,"sha256":proof::sha(&bytes),"content":content}));
            }
        }
        ensure!(primary.len() == 20, "complete primary dependency set");
        fs::write(
            out.join("primary-sources.json"),
            serde_json::to_vec_pretty(&json!({"token_gap":proof::PRIMARY_GAP,"sources":primary}))?,
        )?;
        let version = Command::new(&compiler).arg("--version").output()?;
        ensure!(version.status.success(), "compiler version");
        fs::write(out.join("compiler-version.txt"), &version.stdout)?;
        ensure!(String::from_utf8(version.stdout)?.contains(proof::SOLC_VERSION), "compiler version identity");
        let original = capture["stdJsonInput"].clone();
        let mut input = original.clone();
        ensure!(input["settings"].get("outputSelection").is_none(), "original has no outputSelection");
        input["settings"]["outputSelection"] =
            json!({"*":{"*":["abi","metadata","devdoc","userdoc","storageLayout","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}});
        let mut restored = input.clone();
        restored["settings"].as_object_mut().unwrap().remove("outputSelection");
        ensure!(restored == original, "outputSelection-only addition");
        let input_bytes = serde_json::to_vec(&input)?;
        fs::write(out.join("compiler-input-original.json"), serde_json::to_vec(&original)?)?;
        fs::write(out.join("compiler-input.json"), &input_bytes)?;
        let mut child = Command::new(&compiler)
            .arg("--standard-json")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        child.stdin.take().unwrap().write_all(&input_bytes)?;
        let compiled = child.wait_with_output()?;
        fs::write(out.join("compiler-output.json"), &compiled.stdout)?;
        fs::write(out.join("compiler-stderr.txt"), &compiled.stderr)?;
        ensure!(compiled.status.success(), "compiler process failed");
        let compiled = proof::verify_compiled(&capture, &compiled.stdout)?;
        ensure!(
            !compiled["errors"].as_array().into_iter().flatten().any(|e| e["severity"] == "error"),
            "compiler diagnostic error"
        );
        ensure!(compiled["sources"] == capture["sourceIds"], "fresh complete source IDs");
        let actual = &compiled["contracts"][proof::SOURCE][proof::NAME];
        let saved = &capture["stdJsonOutput"]["contracts"][proof::SOURCE][proof::NAME];
        for field in ["abi", "devdoc", "userdoc", "storageLayout"] {
            ensure!(actual[field] == saved[field], "fresh output {field}");
        }
        let fresh_metadata: Value = serde_json::from_str(actual["metadata"].as_str().context("fresh metadata")?)?;
        ensure!(
            fresh_metadata == capture["metadata"],
            "exact parsed compiler metadata (serialization order may differ)"
        );
        for (field, kind) in [("runtimeBytecode", "deployedBytecode"), ("creationBytecode", "bytecode")] {
            let code = proof::bytes(&actual["evm"][kind]["object"])?;
            ensure!(code == proof::bytes(&capture[field]["recompiledBytecode"])?, "full fresh {kind} equality");
            ensure!(actual["evm"][kind]["sourceMap"] == capture[field]["sourceMap"], "fresh source map {kind}");
            fs::write(out.join(format!("{kind}.hex")), format!("{}\n", hex::encode(&code)))?;
        }
        Ok(
            json!({"source_inventory_sha256":inventory_sha,"status":"exact_source_compiler_regeneration_passed","qualified":false,"phase":"host operation proof only","network_chain_calls":0,"capture_sha256":proof::CAPTURE,"compiler":proof::SOLC_VERSION,"compiler_sha256":proof::SOLC_SHA,"compiler_manifest_url":manifest_url,"compiler_manifest_sha256":proof::sha(&manifest_raw),"compiler_output_selection_added":input["settings"]["outputSelection"],"original_settings_and_sources_unchanged":true,"source_files":21,"primary_dependencies":20,"primary_token_gap":proof::PRIMARY_GAP,"runtime_bytes":6065,"runtime_keccak256":proof::RUNTIME,"creation_compiler_bytes":7220,"constructor_argument_bytes":256,"creation_saved_bytes":7476,"compiler_output_sha256":proof::COMPILED_SHA,"fresh_metadata_raw_sha256":proof::sha(actual["metadata"].as_str().unwrap().as_bytes()),"saved_metadata_raw_sha256":proof::sha(saved["metadata"].as_str().unwrap().as_bytes()),"metadata_comparison":"parsed JSON equality; both original raw strings retained; serialization ordering differs","oz_license_url":license_url,"oz_license_sha256":proof::sha(&license)}),
        )
    })();
    match result {
        Ok(report) => {
            fs::write(out.join("report.json"), format!("{}\n", serde_json::to_string_pretty(&report)?))?;
            println!("Exact compiler regeneration passed: {}", out.display());
            Ok(())
        }
        Err(error) => {
            fs::write(
                out.join("report.json"),
                serde_json::to_vec_pretty(&json!({"status":"failed","error":format!("{error:#}"),"qualified":false}))?,
            )?;
            Err(error)
        }
    }
}
