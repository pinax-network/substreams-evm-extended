#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::tops_proof as p;
    use serde_json::{json, Value};
    use std::{
        fs,
        io::{Read, Write},
        path::Path,
        process::{Command, Stdio},
    };
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 4, "usage: build_tops_proof CAPTURE OFFICIAL_MACOS_SOLC FRESH_OUTPUT");
    let out = Path::new(&args[3]);
    fs::create_dir(out).context("fresh output required")?;
    let result = (|| -> anyhow::Result<Value> {
        let inventory = p::snapshot_sources(out)?;
        let raw = fs::read(&args[1])?;
        fs::write(out.join("capture.json"), &raw)?;
        let capture = p::verify_capture(&raw)?;
        let compiler = fs::canonicalize(&args[2])?;
        let binary = fs::read(&compiler)?;
        ensure!(p::sha(&binary) == p::SOLC_SHA, "official compiler executable SHA");
        let fetch = |url: &str, name: &str| -> anyhow::Result<Vec<u8>> {
            let mut raw = vec![];
            ureq::get(url).call()?.into_reader().read_to_end(&mut raw)?;
            fs::write(out.join(name), &raw)?;
            Ok(raw)
        };
        let manifest_url = format!("https://raw.githubusercontent.com/ethereum/solc-bin/{}/macosx-amd64/list.json", p::SOLC_PIN);
        let mraw = fetch(&manifest_url, "solc-list.json")?;
        let manifest: Value = serde_json::from_slice(&mraw)?;
        let builds: Vec<_> = manifest["builds"]
            .as_array()
            .context("builds")?
            .iter()
            .filter(|x| x["longVersion"] == p::SOLC_VERSION)
            .collect();
        ensure!(
            builds.len() == 1
                && builds[0]["sha256"] == format!("0x{}", p::SOLC_SHA)
                && builds[0]["keccak256"] == p::kh(&binary)
                && manifest["releases"]["0.8.28"] == builds[0]["path"],
            "immutable official release association"
        );
        let version = Command::new(&compiler).arg("--version").output()?;
        fs::write(out.join("compiler-version.txt"), &version.stdout)?;
        ensure!(
            version.status.success() && String::from_utf8_lossy(&version.stdout).contains(p::SOLC_VERSION),
            "compiler version"
        );
        let spec_url = format!(
            "https://raw.githubusercontent.com/ethereum/execution-specs/{}/src/ethereum/forks/cancun/vm/instructions/block.py",
            p::TIMESTAMP_SPEC_PIN
        );
        let spec = fetch(&spec_url, "execution-spec-block.py")?;
        ensure!(p::sha(&spec) == p::TIMESTAMP_SPEC_SHA, "pinned primary TIMESTAMP source");
        let spec_license_url = format!(
            "https://raw.githubusercontent.com/ethereum/execution-specs/{}/LICENSE.md",
            p::TIMESTAMP_SPEC_PIN
        );
        let spec_license = fetch(&spec_license_url, "LICENSE-execution-specs")?;
        ensure!(p::sha(&spec_license) == p::TIMESTAMP_LICENSE_SHA, "specification CC0 license");
        let mut primaries = vec![];
        for (name, src) in capture["sources"].as_object().context("sources")? {
            if name == p::SOURCE {
                continue;
            }
            let path = name.strip_prefix("@openzeppelin/").context("OZ path")?;
            let url = format!("https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{}/{path}", p::OZ_PIN);
            let raw = fetch(&url, &format!("primary-{}.sol", primaries.len()))?;
            ensure!(raw == src["content"].as_str().context("content")?.as_bytes(), "exact primary {name}");
            primaries.push(json!({"path":name,"url":url,"sha256":p::sha(&raw),"content":String::from_utf8(raw)?,"status":"exact"}));
        }
        let primary = json!({"custom_gap":p::PRIMARY_GAP,"sources":primaries});
        fs::write(out.join("primary-sources.json"), serde_json::to_vec_pretty(&primary)?)?;
        let license_url = format!("https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{}/LICENSE", p::OZ_PIN);
        let license = fetch(&license_url, "LICENSE-OpenZeppelin")?;
        let compile = |input: &Value, label: &str| -> anyhow::Result<Vec<u8>> {
            let raw = serde_json::to_vec(input)?;
            fs::write(out.join(format!("{label}-input.json")), &raw)?;
            let mut child = Command::new(&compiler)
                .arg("--standard-json")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            child.stdin.take().unwrap().write_all(&raw)?;
            let result = child.wait_with_output()?;
            fs::write(out.join(format!("{label}-output.json")), &result.stdout)?;
            fs::write(out.join(format!("{label}-stderr.txt")), &result.stderr)?;
            ensure!(result.status.success(), "compiler process {label}");
            Ok(result.stdout)
        };
        let input = p::compiler_input(&capture)?;
        fs::write(out.join("original-input.json"), serde_json::to_vec(&capture["stdJsonInput"])?)?;
        let raw = compile(&input, "full")?;
        let compiled = p::verify_compiled(&capture, &raw)?;
        let original_meta = capture["stdJsonOutput"]["contracts"][p::SOURCE][p::NAME]["metadata"]
            .as_str()
            .context("original metadata")?;
        let fresh_meta = compiled["contracts"][p::SOURCE][p::NAME]["metadata"].as_str().context("fresh metadata")?;
        fs::write(out.join("metadata-original.json"), original_meta)?;
        fs::write(out.join("metadata-fresh.json"), fresh_meta)?;
        let (harness, extraction) = p::harness(&capture)?;
        fs::write(out.join(p::HARNESS_SOURCE), &harness)?;
        fs::write(out.join("harness-extraction.json"), serde_json::to_vec_pretty(&extraction)?)?;
        let mut hinput = input.clone();
        hinput["sources"] = json!({p::HARNESS_SOURCE:{"content":harness}});
        let hraw = compile(&hinput, "harness")?;
        ensure!(p::sha(&hraw) == p::HARNESS_COMPILED_SHA, "whole pinned harness compiler output");
        let hout: Value = serde_json::from_slice(&hraw)?;
        ensure!(
            !hout["errors"].as_array().into_iter().flatten().any(|e| e["severity"] == "error"),
            "harness compilation"
        );
        p::verify_layout(&hout["contracts"][p::HARNESS_SOURCE][p::HARNESS_NAME]["storageLayout"], true)?;
        let artifacts = [
            "capture.json",
            "original-input.json",
            "full-input.json",
            "full-output.json",
            "harness-input.json",
            "harness-output.json",
            "TOPSCleanup.sol",
            "harness-extraction.json",
            "primary-sources.json",
            "solc-list.json",
            "compiler-version.txt",
            "LICENSE-OpenZeppelin",
            "metadata-original.json",
            "metadata-fresh.json",
            "execution-spec-block.py",
            "LICENSE-execution-specs",
        ]
        .into_iter()
        .map(|name| Ok(json!({"file":name,"sha256":p::sha(&fs::read(out.join(name))?)})))
        .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(
            json!({"status":"passed","qualified":false,"scope":p::LIMITS,"custom_primary_gap":p::PRIMARY_GAP,"args":args,"source_inventory_sha256":inventory,"capture_sha256":p::CAPTURE_SHA,"compiler_sha256":p::SOLC_SHA,"compiler_manifest_url":manifest_url,"compiler_output_sha256":p::sha(&raw),"harness_output_sha256":p::sha(&hraw),"metadata_parsed_equal":true,"metadata_raw_equal":original_meta==fresh_meta,"primary_exact":8,"timestamp_spec":{"url":spec_url,"sha256":p::TIMESTAMP_SPEC_SHA,"license_url":spec_license_url,"license_sha256":p::TIMESTAMP_LICENSE_SHA,"scope":"opcode stack semantics only; gas/fork model excluded"},"creation_bytes":26671,"runtime_bytes":22323,"runtime_keccak256":p::RUNTIME,"immutable_ids":13,"immutable_sites":91,"distributors_create_nonce_range":[1,10],"main_pair":"capture-bound external factory result, not independently qualified","upstream_license":{"url":license_url,"sha256":p::sha(&license)},"token_license":"Unlicensed original notice; no relicense","artifacts":artifacts,"chain_calls":0}),
        )
    })();
    let (report, status) = match result {
        Ok(v) => (v, Ok(())),
        Err(e) => (json!({"status":"failed","qualified":false,"error":format!("{e:#}"),"args":args}), Err(e)),
    };
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    status
}
