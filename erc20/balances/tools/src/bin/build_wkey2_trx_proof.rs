#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::wkey2_trx_proof as p;
    use serde_json::{json, Value};
    use std::{
        fs,
        io::{Read, Write},
        os::unix::fs::PermissionsExt,
        path::Path,
        process::{Command, Stdio},
    };
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: build_wkey2_trx_proof FIXTURE FRESH_OUTPUT");
    let input = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir(out).context("fresh output directory required")?;
    let run = (|| -> anyhow::Result<Value> {
        let inventory = p::snapshot_sources(out)?;
        let fetch = |url: &str, path: &Path| -> anyhow::Result<Vec<u8>> {
            let mut raw = vec![];
            ureq::get(url).call()?.into_reader().read_to_end(&mut raw)?;
            fs::write(path, &raw)?;
            Ok(raw)
        };
        let manifest_url = format!("https://raw.githubusercontent.com/ethereum/solc-bin/{}/macosx-amd64/list.json", p::SOLC_PIN);
        let manifest = fetch(&manifest_url, &out.join("solc-list.json"))?;
        let license_url = format!("https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{}/LICENSE", p::OZ_PIN);
        ensure!(
            p::sha(&fetch(&license_url, &out.join("LICENSE-openzeppelin"))?) == p::LICENSE_SHA,
            "exact MIT license"
        );
        let mut targets = vec![];
        for t in p::Target::ALL {
            p::verify_manifest(&manifest, t)?;
            let dir = out.join(t.label());
            fs::create_dir(&dir)?;
            let binary_url = format!(
                "https://raw.githubusercontent.com/ethereum/solc-bin/{}/macosx-amd64/{}",
                p::SOLC_PIN,
                t.solc_path()
            );
            let binary = fetch(&binary_url, &dir.join("solc"))?;
            ensure!(p::sha(&binary) == t.solc_sha() && p::kh(&binary) == t.solc_keccak(), "official compiler bytes");
            fs::set_permissions(dir.join("solc"), fs::Permissions::from_mode(0o755))?;
            let compiler = fs::canonicalize(dir.join("solc"))?;
            let version = Command::new(&compiler).arg("--version").output()?;
            fs::write(dir.join("compiler-version.txt"), &version.stdout)?;
            ensure!(version.status.success(), "compiler --version");
            let raw = fs::read(input.join(format!("{}-capture.json", t.label())))?;
            fs::write(dir.join("capture.json"), &raw)?;
            let c = p::verify_capture(&raw, t)?;
            let mut primaries = vec![];
            for (name, source) in c["sources"].as_object().context("sources")? {
                let body = source["content"].as_str().context("raw body")?;
                if let Some(url) = p::primary_url(name, t)? {
                    let raw = fetch(&url, &dir.join(format!("primary-{}.sol", primaries.len())))?;
                    ensure!(raw == body.as_bytes(), "whole primary dependency {name}");
                    primaries.push(json!({"path":name,"url":url,"classification":"exact_full_file","capture_sha256":p::sha(body.as_bytes()),"sha256":p::sha(&raw),"content":String::from_utf8(raw)?}));
                } else {
                    primaries.push(json!({"path":name,"url":null,"classification":"unestablished_full_file","capture_sha256":p::sha(body.as_bytes()),"sha256":null,"content":null}));
                }
            }
            let mut declarations = vec![];
            if t == p::Target::Trx {
                let flattened = c["sources"][t.source()]["content"].as_str().context("flattened source")?;
                for (name, path) in p::DECLARATIONS {
                    let url = format!(
                        "https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{}/contracts/{path}",
                        p::OZ_PIN
                    );
                    let raw = fetch(&url, &dir.join(format!("declaration-primary-{}.sol", declarations.len())))?;
                    let primary = String::from_utf8(raw)?;
                    let selected = p::declaration(flattened, name)?;
                    ensure!(selected == p::declaration(&primary, name)?, "complete normalized declaration");
                    declarations.push(json!({"name":name,"url":url,"primary_content":primary,"primary_sha256":p::sha(primary.as_bytes()),"normalized_declaration":selected,"normalized_sha256":p::sha(selected.as_bytes()),"normalization":"CRLF to LF only; complete declaration, not flattened file"}));
                }
            }
            let primary = json!({"target":t.label(),"source_gap":p::PRIMARY_GAP,"sources":primaries,"declarations":declarations});
            p::verify_primary(&c, &primary, t)?;
            fs::write(dir.join("primary-sources.json"), serde_json::to_vec_pretty(&primary)?)?;
            fs::write(dir.join("source-licenses.json"), serde_json::to_vec_pretty(&p::source_licenses(&c, t)?)?)?;
            fs::write(dir.join("compiler-input-original.json"), serde_json::to_vec(&c["stdJsonInput"])?)?;
            let raw = serde_json::to_vec(&p::input(&c)?)?;
            fs::write(dir.join("compiler-input.json"), &raw)?;
            let mut child = Command::new(&compiler)
                .arg("--standard-json")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            child.stdin.take().context("compiler stdin")?.write_all(&raw)?;
            let result = child.wait_with_output()?;
            fs::write(dir.join("compiler-output.json"), &result.stdout)?;
            fs::write(dir.join("compiler-stderr.txt"), &result.stderr)?;
            ensure!(result.status.success(), "compiler status");
            let compiled = p::verify_compiled(&c, &result.stdout, t)?;
            let original = c["stdJsonOutput"]["contracts"][t.source()][t.name()]["metadata"]
                .as_str()
                .context("saved metadata")?;
            let fresh = compiled["contracts"][t.source()][t.name()]["metadata"].as_str().context("fresh metadata")?;
            fs::write(dir.join("metadata-original.json"), original)?;
            fs::write(dir.join("metadata-fresh.json"), fresh)?;
            p::verify_auxiliary(&dir, &c, t)?;
            targets.push(json!({"target":t.label(),"address":t.address(),"capture_sha256":t.capture_sha(),"whole_compiler_output_sha256":p::sha(&result.stdout),"compiler_url":binary_url,"compiler_sha256":t.solc_sha(),"compiler_keccak256":t.solc_keccak(),"compiler_arguments":["--standard-json"],"runtime_bytes":t.runtime_len(),"runtime_cbor_offset":t.cbor_offset(false),"creation_bytes":t.creation_len(),"creation_cbor_offset":t.cbor_offset(true),"creation_bytes_after_cbor":t.creation_len()-t.cbor_offset(true)-53,"argument_bytes":t.argument_len(),"captured_runtime_keccak256":t.runtime_hash(),"compiler_runtime_keccak256":p::kh(&p::bytes(&c["runtimeBytecode"]["recompiledBytecode"])?),"full_runtime_creation_reconstruction":true,"original_match_labels":"match","raw_metadata_equal":original==fresh,"exact_primary_files":if t==p::Target::Wkeydao2 {4}else{0},"crlf_normalized_declarations":if t==p::Target::Trx {2}else{0}}));
        }
        let mut artifacts = vec![];
        for file in ["solc-list.json", "LICENSE-openzeppelin"] {
            artifacts.push(json!({"file":file,"sha256":p::sha(&fs::read(out.join(file))?)}));
        }
        for t in p::Target::ALL {
            for name in [
                "capture.json",
                "compiler-input-original.json",
                "compiler-input.json",
                "compiler-output.json",
                "compiler-version.txt",
                "metadata-original.json",
                "metadata-fresh.json",
                "primary-sources.json",
                "source-licenses.json",
            ] {
                let file = format!("{}/{name}", t.label());
                artifacts.push(json!({"file":file,"sha256":p::sha(&fs::read(out.join(&file))?)}));
            }
        }
        Ok(
            json!({"status":"passed","qualified":false,"scope":p::LIMITS,"source_gap":p::PRIMARY_GAP,"args":args,"source_inventory_sha256":inventory,"compiler_manifest_url":manifest_url,"upstream_license_url":license_url,"targets":targets,"artifacts":artifacts,"chain_calls":0}),
        )
    })();
    let (report, status) = match run {
        Ok(v) => (v, Ok(())),
        Err(e) => (json!({"status":"failed","qualified":false,"args":args,"error":format!("{e:#}")}), Err(e)),
    };
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    status
}
