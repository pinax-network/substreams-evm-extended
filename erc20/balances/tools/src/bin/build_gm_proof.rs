#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::gm_proof as proof;
    use serde_json::{json, Value};
    use std::{
        fs,
        io::{Read, Write},
        path::Path,
        process::{Command, Stdio},
    };
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 6,
        "usage: build_gm_proof IMPLEMENTATION_CAPTURE PROXY_A_CAPTURE PROXY_B_CAPTURE OFFICIAL_MACOS_SOLC FRESH_OUTPUT"
    );
    let out = Path::new(&args[5]);
    fs::create_dir(out).context("fresh output required")?;
    let result = (|| -> anyhow::Result<Value> {
        let source_inventory = proof::snapshot_sources(out)?;
        let mut captures = vec![];
        for (i, p) in proof::CAPTURES.iter().enumerate() {
            let raw = fs::read(&args[i + 1])?;
            fs::write(out.join(format!("{}-capture.json", p.label)), &raw)?;
            captures.push(proof::verify_capture(&raw, p)?);
        }
        let compiler = fs::canonicalize(&args[4])?;
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
                && manifest["releases"]["0.8.16"] == entries[0]["path"],
            "official immutable release manifest"
        );
        let version = Command::new(&compiler).arg("--version").output()?;
        fs::write(out.join("compiler-version.txt"), &version.stdout)?;
        ensure!(
            version.status.success() && String::from_utf8(version.stdout)?.contains(proof::SOLC_VERSION),
            "compiler version"
        );
        // Both proxy source/compiler inputs are identical but captures retain
        // independent address/digest/null-creation bindings.
        ensure!(captures[1]["stdJsonInput"] == captures[2]["stdJsonInput"], "identical proxy inputs");
        let mut primary = vec![];

        let license_url = format!(
            "https://raw.githubusercontent.com/{}/{}/LICENSE",
            proof::CAPTURES[1].repo,
            proof::CAPTURES[1].pin
        );
        let license = fetch(&license_url, &out.join("LICENSE-proxy-upstream"))?;
        let mut source_licenses = vec![];
        for (v, p) in captures.iter().zip(&proof::CAPTURES) {
            for (name, s) in v["sources"].as_object().unwrap() {
                let content = s["content"].as_str().context("source")?;
                let path = out.join("sources").join(p.label).join(name);
                fs::create_dir_all(path.parent().unwrap())?;
                fs::write(path, content)?;
                let url = if proof::custom(name) {
                    proof::custom_url(name)
                } else {
                    proof::primary_url(p, name)?
                };
                let path = out.join("primary").join(p.label).join(name);
                fs::create_dir_all(path.parent().unwrap())?;
                let raw = fetch(&url, &path)?;
                let status = if proof::custom(name) {
                    ensure!(
                        raw != content.as_bytes(),
                        "custom contextual source unexpectedly matches; review gap classification"
                    );
                    "contextual_nonmatch_primary_gap"
                } else {
                    ensure!(raw == content.as_bytes(), "exact dependency {name}");
                    if p.label == "implementation" {
                        "exact_ondo_vendor"
                    } else {
                        "exact_upstream_oz"
                    }
                };
                primary.push(json!({"capture":p.label,"path":name,"url":url,"sha256":proof::sha(&raw),"captured_sha256":proof::sha(content.as_bytes()),"content":String::from_utf8(raw)?,"status":status}));
                source_licenses.push(json!({"capture":p.label,"path":name,"sha256":proof::sha(content.as_bytes()),"license":v["metadata"]["sources"][name]["license"],"original_first_line":content.lines().next()}));
            }
        }
        let primary = json!({"custom_gap":proof::PRIMARY_GAP,"sources":primary});
        fs::write(out.join("primary-sources.json"), serde_json::to_vec_pretty(&primary)?)?;
        proof::verify_primary(&captures, &primary)?;
        let licenses = json!({"sources":source_licenses,"upstream_proxy_license":{"url":license_url,"sha256":proof::sha(&license)},"vendor_and_custom_notice":"Original per-source MIT/BUSL-1.1 headers and compiler metadata retained; no exact custom-source public license artifact or separate vendor root license is claimed. The proxy upstream MIT text does not relabel custom or vendored sources."});
        fs::write(out.join("source-licenses.json"), serde_json::to_vec_pretty(&licenses)?)?;
        let mut compiled_records = vec![];
        for (capture, p) in captures.iter().zip(&proof::CAPTURES) {
            let original = capture["stdJsonInput"].clone();
            let mut input = original.clone();
            ensure!(input["settings"].get("outputSelection").is_none(), "original output selection absent");
            input["settings"]["outputSelection"] =
                json!({"*":{"*":["abi","metadata","devdoc","userdoc","storageLayout","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}});
            let mut restored = input.clone();
            restored["settings"].as_object_mut().unwrap().remove("outputSelection");
            ensure!(restored == original, "outputSelection is the only change");
            let raw = serde_json::to_vec(&input)?;
            fs::write(out.join(format!("{}-compiler-input-original.json", p.label)), serde_json::to_vec(&original)?)?;
            fs::write(out.join(format!("{}-compiler-input.json", p.label)), &raw)?;
            let mut child = Command::new(&compiler)
                .arg("--standard-json")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            child.stdin.take().unwrap().write_all(&raw)?;
            let compiled = child.wait_with_output()?;
            fs::write(out.join(format!("{}-compiler-output.json", p.label)), &compiled.stdout)?;
            fs::write(out.join(format!("{}-compiler-stderr.txt", p.label)), &compiled.stderr)?;
            ensure!(compiled.status.success(), "compiler process status");
            let result = proof::verify_compiled(capture, &compiled.stdout, p)?;
            let selected = &result["contracts"][p.source][p.name];
            ensure!(
                p.label != "implementation"
                    || selected["evm"]["methodIdentifiers"]["initialize(string,string,address,address)"]
                        == hex::encode(&erc20_balances::hash(b"initialize(string,string,address,address)")[..4]),
                "initializer selector"
            );
            compiled_records.push(json!({"capture":p.label,"capture_sha256":p.sha,"files":p.files,"compiler_input_sha256":proof::sha(&raw),"compiler_output_sha256":proof::sha(&compiled.stdout),"runtime_bytes":p.runtime_bytes,"captured_runtime_keccak256":p.runtime_hash,"compiler_runtime_keccak256":proof::kh(&proof::bytes(&capture["runtimeBytecode"]["recompiledBytecode"])?),"creation_compiler_bytes":p.creation_bytes,"creation_match":null,"creation_onchain_bytes":null,"runtime_transformations":capture["runtimeBytecode"]["transformations"],"runtime_transformation_values":capture["runtimeBytecode"]["transformationValues"],"fresh_metadata_raw_sha256":proof::sha(selected["metadata"].as_str().unwrap().as_bytes()),"saved_metadata_raw_sha256":proof::sha(capture["stdJsonOutput"]["contracts"][p.source][p.name]["metadata"].as_str().unwrap().as_bytes())}));
        }
        Ok(
            json!({"status":"exact_source_compiler_regeneration_passed","qualified":false,"scope":"GM host-only operation proof; no proxy/delegatecall/deployment or ingestion qualification","source_inventory_sha256":source_inventory,"compiler":proof::SOLC_VERSION,"compiler_sha256":proof::SOLC_SHA,"compiler_keccak256":proof::SOLC_KECCAK,"compiler_manifest_url":manifest_url,"compiler_manifest_sha256":proof::sha(&manifest_raw),"compiler_arguments":["--standard-json"],"original_settings_sources_unchanged":true,"only_output_selection_added":true,"compiled":compiled_records,"source_records":34,"unique_source_profiles":27,"exact_ondo_vendored_dependencies":15,"unique_exact_upstream_proxy_dependencies":7,"custom_primary_gaps":5,"custom_gap":proof::PRIMARY_GAP,"history_limit":proof::HISTORY_LIMIT,"license_inventory_sha256":proof::sha(&serde_json::to_vec_pretty(&licenses)?),"network_chain_calls":0,"fresh_public_source_requests_only":true}),
        )
    })();
    let (report, result) = match result {
        Ok(report) => (report, Ok(())),
        Err(e) => (json!({"status":"failed","qualified":false,"error":format!("{e:#}")}), Err(e)),
    };
    fs::write(out.join("report.json"), format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    result
}
