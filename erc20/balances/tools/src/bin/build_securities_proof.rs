#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::securities_proof as proof;
    use serde_json::{json, Value};
    use std::{
        fs,
        io::{Read, Write},
        path::Path,
        process::{Command, Stdio},
    };
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 4, "usage: build_securities_proof CAPTURE OFFICIAL_MACOS_SOLC FRESH_OUTPUT");
    let out = Path::new(&args[3]);
    fs::create_dir(out).context("fresh output required")?;
    let result = (|| -> anyhow::Result<Value> {
        let source_inventory = proof::snapshot_sources(out)?;
        let raw = fs::read(&args[1])?;
        fs::write(out.join("capture.json"), &raw)?;
        let capture = proof::verify_capture(&raw)?;
        let compiler = fs::canonicalize(&args[2])?;
        let binary = fs::read(&compiler)?;
        ensure!(
            proof::sha(&binary) == proof::SOLC_SHA && proof::kh(&binary) == proof::SOLC_KECCAK,
            "official compiler binary hashes"
        );
        let fetch = |url: &str, path: &Path| -> anyhow::Result<Vec<u8>> {
            let mut raw = vec![];
            ureq::get(url).call()?.into_reader().read_to_end(&mut raw)?;
            fs::write(path, &raw)?;
            Ok(raw)
        };
        let manifest_url = format!("https://raw.githubusercontent.com/ethereum/solc-bin/{}/macosx-amd64/list.json", proof::SOLC_PIN);
        let manifest_raw = fetch(&manifest_url, &out.join("solc-list.json"))?;
        let manifest: Value = serde_json::from_slice(&manifest_raw)?;
        let entries: Vec<_> = manifest["builds"]
            .as_array()
            .context("builds")?
            .iter()
            .filter(|e| e["longVersion"] == proof::SOLC_VERSION)
            .collect();
        ensure!(
            entries.len() == 1
                && entries[0]["sha256"] == format!("0x{}", proof::SOLC_SHA)
                && entries[0]["keccak256"] == proof::SOLC_KECCAK
                && entries[0]["path"] == format!("solc-macosx-amd64-v{}", proof::SOLC_VERSION)
                && manifest["releases"]["0.8.24"] == entries[0]["path"],
            "immutable official manifest"
        );
        let version = Command::new(&compiler).arg("--version").output()?;
        fs::write(out.join("compiler-version.txt"), &version.stdout)?;
        ensure!(
            version.status.success() && String::from_utf8(version.stdout)?.contains(proof::SOLC_VERSION),
            "compiler version"
        );
        let mut licenses = vec![];
        for (label, repo, pin) in proof::LICENSES {
            let url = format!("https://raw.githubusercontent.com/{repo}/{pin}/LICENSE");
            let raw = fetch(&url, &out.join(format!("LICENSE-{label}")))?;
            licenses.push(json!({"label":label,"url":url,"sha256":proof::sha(&raw)}));
        }
        let mut primary = vec![];
        for (name, s) in capture["sources"].as_object().context("sources")? {
            let content = s["content"].as_str().context("content")?;
            let path = out.join("sources").join(name);
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(path, content)?;
            let (url, status, _) = proof::primary_spec(name)?;
            let mut record =
                json!({"path":name,"capture_sha256":proof::sha(content.as_bytes()),"url":url,"classification":status,"content":null,"sha256":null});
            if let Some(url) = url {
                let path = out.join("primary").join(name);
                fs::create_dir_all(path.parent().unwrap())?;
                let raw = fetch(&url, &path)?;
                record["sha256"] = json!(proof::sha(&raw));
                record["content"] = json!(String::from_utf8(raw)?);
            }
            primary.push(record);
        }
        let primary = json!({"source_gap":proof::PRIMARY_GAP,"sources":primary});
        let primary_raw = serde_json::to_vec_pretty(&primary)?;
        fs::write(out.join("primary-sources.json"), &primary_raw)?;
        proof::verify_primary(&capture, &primary)?;
        let original = capture["stdJsonInput"].clone();
        let mut input = original.clone();
        ensure!(input["settings"].get("outputSelection").is_none(), "original outputSelection absent");
        input["settings"]["outputSelection"] =
            json!({"*":{"*":["abi","metadata","devdoc","userdoc","storageLayout","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}});
        let mut restored = input.clone();
        restored["settings"].as_object_mut().unwrap().remove("outputSelection");
        ensure!(restored == original, "only outputSelection added");
        let input_raw = serde_json::to_vec(&input)?;
        fs::write(out.join("compiler-input-original.json"), serde_json::to_vec(&original)?)?;
        fs::write(out.join("compiler-input.json"), &input_raw)?;
        proof::verify_auxiliary(out, &capture)?;
        let mut child = Command::new(&compiler)
            .arg("--standard-json")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        child.stdin.take().unwrap().write_all(&input_raw)?;
        let compiled = child.wait_with_output()?;
        fs::write(out.join("compiler-output.json"), &compiled.stdout)?;
        fs::write(out.join("compiler-stderr.txt"), &compiled.stderr)?;
        ensure!(compiled.status.success(), "compiler status");
        let result = proof::verify_compiled(&capture, &compiled.stdout)?;
        let selected = &result["contracts"][proof::SOURCE][proof::NAME];
        fs::write(out.join("metadata-fresh.json"), selected["metadata"].as_str().context("fresh metadata")?)?;
        fs::write(
            out.join("metadata-saved.json"),
            capture["stdJsonOutput"]["contracts"][proof::SOURCE][proof::NAME]["metadata"]
                .as_str()
                .context("saved metadata")?,
        )?;
        proof::verify_metadata_files(out)?;
        Ok(
            json!({"status":"exact_source_compiler_regeneration_passed","qualified":false,"scope":"selected SecuritiesToken implementation source/compiler equality; synthetic constructor only; on-chain creation and all deployment fields absent","source_inventory_sha256":source_inventory,"capture_sha256":proof::CAPTURE_SHA,"compiler":proof::SOLC_VERSION,"compiler_sha256":proof::SOLC_SHA,"compiler_keccak256":proof::SOLC_KECCAK,"compiler_manifest_url":manifest_url,"compiler_manifest_sha256":proof::sha(&manifest_raw),"compiler_arguments":["--standard-json"],"original_settings_sources_unchanged":true,"only_output_selection_added":true,"compiler_input_sha256":proof::sha(&input_raw),"compiler_output_sha256":proof::sha(&compiled.stdout),"runtime_bytes":10836,"runtime_keccak256":proof::RUNTIME_HASH,"full_saved_runtime_equal":true,"compiler_creation_bytes":11063,"fresh_saved_compiler_creation_equal":true,"onchain_creation_available":false,"deployment_evidence_available":false,"fresh_metadata_raw_sha256":proof::sha(selected["metadata"].as_str().unwrap().as_bytes()),"saved_metadata_raw_sha256":proof::sha(capture["stdJsonOutput"]["contracts"][proof::SOURCE][proof::NAME]["metadata"].as_str().unwrap().as_bytes()),"parsed_metadata_equal":true,"source_files":31,"exact_primary_files":23,"near_not_exact_files":3,"unrecovered_primary_files":5,"primary_sha256":proof::sha(&primary_raw),"source_gap":proof::PRIMARY_GAP,"licenses":licenses,"chain_calls":0,"fresh_public_source_requests_only":true}),
        )
    })();
    let (mut report, result) = match result {
        Ok(v) => (v, Ok(())),
        Err(e) => (json!({"status":"failed","qualified":false,"error":format!("{e:#}")}), Err(e)),
    };
    if result.is_ok() {
        report["raw_metadata_equal"] = json!(false);
        report["metadata_serialization_difference"] = json!("The capture contains literal Unicode while official solc uses Unicode escapes. Both complete raw strings are separately SHA256-pinned; full parsed metadata is equal, with no normalization or source change.");
    }
    fs::write(out.join("report.json"), format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    result
}
