#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::wkey_got_proof as p;
    use serde_json::{json, Value};
    use std::{
        fs,
        io::{Read, Write},
        path::Path,
        process::{Command, Stdio},
    };
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 4, "usage: build_wkey_got_proof FIXTURE OFFICIAL_MACOS_SOLC FRESH_OUTPUT");
    let input = Path::new(&args[1]);
    let compiler = fs::canonicalize(&args[2])?;
    let out = Path::new(&args[3]);
    fs::create_dir(out).context("fresh output directory required")?;
    let run = (|| -> anyhow::Result<Value> {
        let inventory = p::snapshot_sources(out)?;
        let binary = fs::read(&compiler)?;
        ensure!(
            p::sha(&binary) == p::SOLC_SHA && p::kh(&binary) == p::SOLC_KECCAK,
            "exact official compiler binary"
        );
        let fetch = |url: &str, path: &Path| -> anyhow::Result<Vec<u8>> {
            let mut raw = vec![];
            ureq::get(url).call()?.into_reader().read_to_end(&mut raw)?;
            fs::write(path, &raw)?;
            Ok(raw)
        };
        let manifest_url = format!("https://raw.githubusercontent.com/ethereum/solc-bin/{}/macosx-amd64/list.json", p::SOLC_PIN);
        let raw = fetch(&manifest_url, &out.join("solc-list.json"))?;
        ensure!(p::sha(&raw) == p::MANIFEST_SHA, "whole pinned manifest");
        let manifest: Value = serde_json::from_slice(&raw)?;
        let builds: Vec<_> = manifest["builds"]
            .as_array()
            .context("builds")?
            .iter()
            .filter(|v| v["longVersion"] == p::SOLC_VERSION)
            .collect();
        ensure!(
            builds.len() == 1
                && builds[0]["sha256"] == format!("0x{}", p::SOLC_SHA)
                && builds[0]["keccak256"] == p::SOLC_KECCAK
                && manifest["releases"]["0.7.5"] == builds[0]["path"],
            "official release association"
        );
        let version = Command::new(&compiler).arg("--version").output()?;
        fs::write(out.join("compiler-version.txt"), &version.stdout)?;
        ensure!(
            version.status.success() && String::from_utf8_lossy(&version.stdout).contains(p::SOLC_VERSION),
            "compiler version"
        );
        let license_url = format!("https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{}/LICENSE", p::OZ_PIN);
        let license = fetch(&license_url, &out.join("LICENSE-openzeppelin"))?;
        ensure!(p::sha(&license) == p::LICENSE_SHA, "exact original MIT license");
        let mut targets = vec![];
        for t in p::Target::ALL {
            let dir = out.join(t.label());
            fs::create_dir(&dir)?;
            let raw = fs::read(input.join(format!("{}-capture.json", t.label())))?;
            fs::write(dir.join("capture.json"), &raw)?;
            let capture = p::verify_capture(&raw, t)?;
            let mut primaries = vec![];
            for (name, source) in capture["sources"].as_object().context("sources")? {
                let body = source["content"].as_str().context("body")?;
                if let Some(url) = p::primary_url(name, t)? {
                    let raw = fetch(&url, &dir.join(format!("primary-{}.sol", primaries.len())))?;
                    ensure!(raw == body.as_bytes(), "exact pinned dependency {name}");
                    primaries.push(json!({"path":name,"url":url,"classification":"exact","capture_sha256":p::sha(body.as_bytes()),"sha256":p::sha(&raw),"content":String::from_utf8(raw)?}));
                } else {
                    primaries.push(
                        json!({"path":name,"url":null,"classification":"unestablished","capture_sha256":p::sha(body.as_bytes()),"sha256":null,"content":null}),
                    );
                }
            }
            let primary = json!({"target":t.label(),"source_gap":p::PRIMARY_GAP,"sources":primaries});
            p::verify_primary(&capture, &primary, t)?;
            fs::write(dir.join("primary-sources.json"), serde_json::to_vec_pretty(&primary)?)?;
            fs::write(dir.join("compiler-input-original.json"), serde_json::to_vec(&capture["stdJsonInput"])?)?;
            let raw = serde_json::to_vec(&p::input(&capture)?)?;
            fs::write(dir.join("compiler-input.json"), &raw)?;
            let mut child = Command::new(&compiler)
                .arg("--standard-json")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            child.stdin.take().unwrap().write_all(&raw)?;
            let result = child.wait_with_output()?;
            fs::write(dir.join("compiler-output.json"), &result.stdout)?;
            fs::write(dir.join("compiler-stderr.txt"), &result.stderr)?;
            ensure!(result.status.success(), "compiler process");
            let compiled = p::verify_compiled(&capture, &result.stdout, t)?;
            let original = capture["stdJsonOutput"]["contracts"][p::SOURCE][t.name()]["metadata"]
                .as_str()
                .context("original metadata")?;
            let fresh = compiled["contracts"][p::SOURCE][t.name()]["metadata"].as_str().context("fresh metadata")?;
            fs::write(dir.join("metadata-original.json"), original)?;
            fs::write(dir.join("metadata-fresh.json"), fresh)?;
            p::verify_auxiliary(&dir, &capture, t)?;
            targets.push(json!({"target":t.label(),"address":t.address(),"capture_sha256":t.capture_sha(),"whole_compiler_output_sha256":p::sha(&result.stdout),"runtime_keccak256":t.runtime_hash(),"runtime_bytes":t.runtime_len(),"creation_bytes":t.creation_len(),"constructor_append_bytes":96,"role_root":t.role_root().to_string(),"primary_exact":if t==p::Target::Wkeydao{4}else{6},"custom_source_license":"AGPL-3.0-or-later; original notice retained","raw_metadata_equal":original==fresh}));
        }
        let mut artifacts = vec![];
        for name in ["solc-list.json", "compiler-version.txt", "LICENSE-openzeppelin"] {
            artifacts.push(json!({"file":name,"sha256":p::sha(&fs::read(out.join(name))?)}));
        }
        for t in p::Target::ALL {
            for name in [
                "capture.json",
                "compiler-input-original.json",
                "compiler-input.json",
                "compiler-output.json",
                "metadata-original.json",
                "metadata-fresh.json",
                "primary-sources.json",
            ] {
                let rel = format!("{}/{name}", t.label());
                artifacts.push(json!({"file":rel,"sha256":p::sha(&fs::read(out.join(&rel))?)}));
            }
        }
        Ok(
            json!({"status":"passed","qualified":false,"scope":p::LIMITS,"source_gap":p::PRIMARY_GAP,"args":args,"source_inventory_sha256":inventory,"compiler_sha256":p::SOLC_SHA,"compiler_manifest_url":manifest_url,"upstream_license_url":license_url,"targets":targets,"artifacts":artifacts,"chain_calls":0}),
        )
    })();
    let (report, result) = match run {
        Ok(v) => (v, Ok(())),
        Err(e) => (json!({"status":"failed","qualified":false,"args":args,"error":format!("{e:#}")}), Err(e)),
    };
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    result
}
