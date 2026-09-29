#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::erc20tokenx_proof as proof;
    use serde_json::{json, Value};
    use std::{
        fs,
        io::{Read, Write},
        path::Path,
        process::{Command, Stdio},
    };
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 4,
        "usage: build_erc20tokenx_proof CAPTURE_FIXTURE_DIRECTORY OFFICIAL_MACOS_SOLC FRESH_OUTPUT"
    );
    let source = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir(out).context("fresh output required")?;
    let result = (|| -> anyhow::Result<Value> {
        let inventory = proof::snapshot_sources(out)?;
        let mut captures = vec![];
        for (i, (label, _, _)) in proof::CAPTURES.iter().enumerate() {
            let name = format!("{label}-capture.json");
            let raw = fs::read(source.join(&name))?;
            fs::write(out.join(name), &raw)?;
            captures.push(proof::verify_capture(&raw, i)?);
        }
        let capture = &captures[0];
        ensure!(
            captures[0]["stdJsonInput"] == captures[1]["stdJsonInput"],
            "both complete captures bind same original source/settings"
        );
        ensure!(proof::runtime(&captures[0])? == proof::runtime(&captures[1])?, "ORI/FNA exact runtime equality");
        for name in ["phi-source-request.json", "phi-runtime.hex", "phi-runtime-report.json"] {
            fs::copy(source.join(name), out.join(name))?;
        }
        proof::verify_phi(out, &proof::runtime(capture)?)?;
        let compiler = fs::canonicalize(&args[2])?;
        let binary = fs::read(&compiler)?;
        ensure!(
            proof::sha(&binary) == proof::SOLC_SHA && proof::kh(&binary) == proof::SOLC_KECCAK,
            "official compiler binary"
        );
        let fetch = |url: &str, path: &Path| -> anyhow::Result<Vec<u8>> {
            let mut b = vec![];
            ureq::get(url).call()?.into_reader().read_to_end(&mut b)?;
            fs::write(path, &b)?;
            Ok(b)
        };
        let url = format!("https://raw.githubusercontent.com/ethereum/solc-bin/{}/macosx-amd64/list.json", proof::SOLC_PIN);
        let manifest_raw = fetch(&url, &out.join("solc-list.json"))?;
        ensure!(proof::sha(&manifest_raw) == proof::MANIFEST_SHA, "immutable compiler manifest");
        let manifest: Value = serde_json::from_slice(&manifest_raw)?;
        let builds: Vec<_> = manifest["builds"]
            .as_array()
            .context("manifest builds")?
            .iter()
            .filter(|b| b["longVersion"] == proof::SOLC_VERSION)
            .collect();
        ensure!(
            builds.len() == 1
                && builds[0]["sha256"] == format!("0x{}", proof::SOLC_SHA)
                && builds[0]["keccak256"] == proof::SOLC_KECCAK
                && builds[0]["path"] == manifest["releases"]["0.7.5"],
            "selected release hashes"
        );
        let version = Command::new(&compiler).arg("--version").output()?;
        fs::write(out.join("compiler-version.txt"), &version.stdout)?;
        ensure!(version.status.success(), "version status");
        let license_url = format!(
            "https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{}/LICENSE",
            proof::OZ_PIN
        );
        let license = fetch(&license_url, &out.join("LICENSE-openzeppelin"))?;
        let mut sources = vec![];
        for (name, s) in capture["sources"].as_object().context("sources")? {
            let body = s["content"].as_str().context("body")?;
            let path = out.join("sources").join(name);
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(path, body)?;
            let url = proof::primary_url(name)?;
            let mut r = json!({"path":name,"capture_sha256":proof::sha(body.as_bytes()),"url":url,"classification":if url.is_some(){"exact"}else{"unestablished"},"content":null,"sha256":null});
            if let Some(url) = url {
                let path = out.join("primary").join(name);
                fs::create_dir_all(path.parent().unwrap())?;
                let b = fetch(&url, &path)?;
                r["sha256"] = json!(proof::sha(&b));
                r["content"] = json!(String::from_utf8(b)?);
            }
            sources.push(r);
        }
        let primary = json!({"source_gap":proof::PRIMARY_GAP,"sources":sources});
        proof::verify_primary(capture, &primary)?;
        fs::write(out.join("primary-sources.json"), serde_json::to_vec_pretty(&primary)?)?;
        let input = serde_json::to_vec(&proof::input(capture)?)?;
        fs::write(out.join("compiler-input-original.json"), serde_json::to_vec(&capture["stdJsonInput"])?)?;
        fs::write(out.join("compiler-input.json"), &input)?;
        proof::verify_auxiliary(out, capture)?;
        let mut child = Command::new(&compiler)
            .arg("--standard-json")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        child.stdin.take().unwrap().write_all(&input)?;
        let compiled = child.wait_with_output()?;
        fs::write(out.join("compiler-output.json"), &compiled.stdout)?;
        fs::write(out.join("compiler-stderr.txt"), &compiled.stderr)?;
        ensure!(compiled.status.success(), "compiler status");
        // The preserved initial regeneration established this artifact hash.
        // Every final build and operation attempt requires the whole raw output.
        for capture in &captures {
            proof::verify_compiled(capture, &compiled.stdout)?;
        }
        Ok(
            json!({"status":"exact_source_compiler_regeneration_passed","qualified":false,"source_inventory_sha256":inventory,"captures":proof::CAPTURES.iter().map(|(label,address,sha)|json!({"label":label,"address":address,"sha256":sha})).collect::<Vec<_>>(),"runtime_bytes":7896,"runtime_keccak256":proof::RUNTIME_HASH,"full_compiled_captured_runtime_equal":true,"runtime_substitutions":0,"creation_bytes":9347,"both_captured_creation_appends_exact":true,"phi_attribution":"exact full historical runtime only; no individual source/creation/deployment evidence","compiler":proof::SOLC_VERSION,"compiler_sha256":proof::SOLC_SHA,"compiler_keccak256":proof::SOLC_KECCAK,"compiler_manifest_url":url,"compiler_manifest_sha256":proof::MANIFEST_SHA,"compiler_input_sha256":proof::sha(&input),"compiler_output_sha256":proof::sha(&compiled.stdout),"only_output_selection_added":true,"source_files":5,"exact_primary_dependencies":4,"custom_primary_gap":proof::PRIMARY_GAP,"license_url":license_url,"license_sha256":proof::sha(&license),"chain_calls":0}),
        )
    })();
    let (report, result) = match result {
        Ok(v) => (v, Ok(())),
        Err(e) => (json!({"status":"failed","qualified":false,"error":format!("{e:#}")}), Err(e)),
    };
    fs::write(out.join("report.json"), format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    result
}
