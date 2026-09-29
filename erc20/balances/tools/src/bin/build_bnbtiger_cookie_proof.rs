#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::bnbtiger_cookie_proof as p;
    use serde_json::{json, Value};
    use std::{
        fs,
        io::{Read, Write},
        os::unix::fs::PermissionsExt,
        path::Path,
        process::{Command, Stdio},
    };
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: build_bnbtiger_cookie_proof FIXTURE FRESH_OUTPUT");
    let fixture = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir(out).context("fresh output required")?;
    let run = (|| -> anyhow::Result<Value> {
        let inventory = p::snapshot_sources(out)?;
        let fetch = |url: &str, file: &Path| -> anyhow::Result<Vec<u8>> {
            let mut b = vec![];
            ureq::get(url).call()?.into_reader().read_to_end(&mut b)?;
            fs::write(file, &b)?;
            Ok(b)
        };
        let manifest_url = format!("https://raw.githubusercontent.com/ethereum/solc-bin/{}/macosx-amd64/list.json", p::SOLC_PIN);
        let manifest = fetch(&manifest_url, &out.join("solc-list.json"))?;
        let mut licenses = vec![];
        for (name, repo, pin, hash) in p::LICENSES {
            let url = format!("https://raw.githubusercontent.com/{repo}/{pin}/LICENSE");
            let file = format!("LICENSE-{name}");
            let body = fetch(&url, &out.join(&file))?;
            ensure!(p::sha(&body) == hash, "exact upstream LICENSE");
            licenses.push(json!({"file":file,"url":url,"sha256":p::sha(&body)}));
        }
        let mut targets = vec![];
        for t in p::Target::ALL {
            p::verify_manifest(&manifest, t)?;
            let dir = out.join(t.label());
            fs::create_dir(&dir)?;
            let raw = fs::read(fixture.join(format!("{}-capture.json", t.label())))?;
            fs::write(dir.join("capture.json"), &raw)?;
            let c = p::verify_capture(&raw, t)?;
            let mut primary = vec![];
            for (path, s) in c["sources"].as_object().context("sources")? {
                let body = s["content"].as_str().context("source")?;
                let url = p::primary_url(path);
                let raw = match &url {
                    Some(url) => Some(fetch(url, &dir.join(format!("primary-{}.sol", primary.len())))?),
                    None => None,
                };
                primary.push(json!({"path":path,"capture_sha256":p::sha(body.as_bytes()),"url":url,"classification":match &raw {None=>"unestablished_full_file",Some(b) if b==body.as_bytes()=>"exact_full_file",_=>"different_full_file"},"primary_sha256":raw.as_ref().map(|b|p::sha(b)),"content":raw.map(String::from_utf8).transpose()?}));
            }
            let primary = json!({"target":t.label(),"source_gap":p::PRIMARY_GAP,"sources":primary});
            fs::write(dir.join("primary-sources.json"), serde_json::to_vec_pretty(&primary)?)?;
            p::verify_primary(&c, &primary, t)?;
            fs::write(dir.join("source-licenses.json"), serde_json::to_vec_pretty(&p::source_licenses(&c)?)?)?;
            fs::write(dir.join("writer-review.json"), serde_json::to_vec_pretty(&p::writers::review(&c, t)?)?)?;
            let url = format!(
                "https://raw.githubusercontent.com/ethereum/solc-bin/{}/macosx-amd64/{}",
                p::SOLC_PIN,
                t.solc_path()
            );
            let bin = fetch(&url, &dir.join("solc"))?;
            ensure!(p::sha(&bin) == t.solc_sha() && p::kh(&bin) == t.solc_keccak(), "exact official compiler bytes");
            fs::set_permissions(dir.join("solc"), fs::Permissions::from_mode(0o755))?;
            let compiler = fs::canonicalize(dir.join("solc"))?;
            let version = Command::new(&compiler).arg("--version").output()?;
            fs::write(dir.join("compiler-version.txt"), &version.stdout)?;
            ensure!(version.status.success(), "compiler version");
            fs::write(dir.join("compiler-input-original.json"), serde_json::to_vec(&c["stdJsonInput"])?)?;
            let input = serde_json::to_vec(&p::input(&c)?)?;
            fs::write(dir.join("compiler-input.json"), &input)?;
            let mut child = Command::new(&compiler)
                .arg("--standard-json")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            child.stdin.take().context("stdin")?.write_all(&input)?;
            let result = child.wait_with_output()?;
            fs::write(dir.join("compiler-output.json"), &result.stdout)?;
            fs::write(dir.join("compiler-stderr.txt"), &result.stderr)?;
            ensure!(result.status.success(), "compiler exit");
            let parsed: Value = serde_json::from_slice(&result.stdout)?;
            for (name, value) in [
                ("metadata-original.json", p::selected(&c["stdJsonOutput"], t)),
                ("metadata-fresh.json", p::selected(&parsed, t)),
            ] {
                fs::write(dir.join(name), value["metadata"].as_str().context("raw metadata")?)?;
            }
            p::verify_compiled(&c, &result.stdout, t)?;
            targets.push(json!({"target":t.label(),"capture_sha256":t.capture_sha(),"compiler_output_sha256":p::sha(&result.stdout),"compiler_url":url,"runtime_keccak256":t.runtime_hash(),"source_files":c["sources"].as_object().context("sources")?.len()}));
        }
        p::verify_source_directory(out)?;
        let artifacts = p::source_artifacts(out)?;
        let report = json!({"status":"passed","qualified":false,"scope":p::LIMITS,"source_gap":p::PRIMARY_GAP,"args":args,"source_inventory_sha256":inventory,"targets":targets,"licenses":licenses,"artifacts":artifacts,"chain_calls":0});
        p::validate_source_report(&report, &artifacts, &fs::read(out.join("source-inventory.json"))?, &inventory)?;
        Ok(report)
    })();
    let (report, status) = match run {
        Ok(v) => (v, Ok(())),
        Err(e) => (json!({"status":"failed","qualified":false,"args":args,"error":format!("{e:#}")}), Err(e)),
    };
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    status
}
