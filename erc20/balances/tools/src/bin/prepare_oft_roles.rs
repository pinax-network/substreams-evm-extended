#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::oft_roles as bound;
    use serde_json::{json, Value};
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: prepare_oft_roles <original package root> <fresh output>");
    let output = Path::new(&args[2]);
    fs::create_dir(output).context("fresh output required")?;
    let tools = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut implementation = serde_json::Map::new();
    for file in ["src/oft_roles.rs", "src/bin/prepare_oft_roles.rs"] {
        let raw = fs::read(tools.join(file))?;
        let dest = output.join("implementation").join(file);
        fs::create_dir_all(dest.parent().unwrap())?;
        fs::write(dest, &raw)?;
        implementation.insert(file.into(), json!(bound::sha(&raw)));
    }
    let write = |name: &str, v: &Value| -> anyhow::Result<()> {
        fs::write(output.join(name), format!("{}\n", serde_json::to_string_pretty(v)?))?;
        Ok(())
    };
    write(
        "preparation.json",
        &json!({"qualified":false,"command_arguments":args[1..],"implementation_sha256":implementation,"live_chain_calls":false,"public_source_requests":"Only exact immutable GitHub source URLs produced by the binding helper"}),
    )?;
    let mut captures = Vec::new();
    for p in &bound::CAPTURES {
        let raw = fs::read(Path::new(&args[1]).join(p.cache))?;
        fs::write(output.join(format!("{}.json", p.label)), &raw)?;
        captures.push(bound::verify_capture(&raw, p)?);
    }
    let vendored = ureq::get(bound::VENDORED_URL).call()?.into_string()?;
    fs::write(output.join("vendored-input.json"), &vendored)?;
    ensure!(bound::sha(vendored.as_bytes()) == bound::VENDORED_SHA, "pinned complete vendored input digest");
    let input: Value = serde_json::from_str(&vendored)?;
    let mut sources = Vec::new();
    for (v, p) in captures.iter().zip(&bound::CAPTURES) {
        for (name, s) in v["sources"].as_object().unwrap() {
            let status = bound::source_status(p, name);
            let captured = s["content"].as_str().unwrap();
            let (url, content) = match status {
                "independent_custom_primary_gap" => (None, None),
                "exact_vendored_snapshot_upstream_differs" => (
                    Some(bound::VENDORED_URL.to_string()),
                    Some(input["sources"][name]["content"].as_str().context("vendored interface")?.to_string()),
                ),
                _ => {
                    let url = if status == "nonmatching_public_token_whitespace_not_normalized" {
                        bound::NEAR_URL.to_string()
                    } else {
                        bound::primary_url(name)?
                    };
                    let content = ureq::get(&url).call()?.into_string()?;
                    (Some(url), Some(content))
                }
            };
            if let Some(content) = &content {
                let dest = output.join("primary").join(p.label).join(name);
                fs::create_dir_all(dest.parent().unwrap())?;
                fs::write(dest, content)?;
            }
            sources.push(json!({"capture":p.label,"path":name,"status":status,"captured_sha256":bound::sha(captured.as_bytes()),"url":url,"sha256":content.as_ref().map(|s|bound::sha(s.as_bytes())),"content":content}));
        }
    }
    let primary = json!({"qualified":false,"vendored_input":{"url":bound::VENDORED_URL,"sha256":bound::VENDORED_SHA},"sources":sources});
    write("primary-sources.json", &primary)?;
    bound::verify_primary(&captures, &primary, vendored.as_bytes())?;
    write("source-review.json", &bound::review(&captures)?)?;
    write(
        "layouts.json",
        &bound::candidate(&fs::read(tools.parent().unwrap().join("tests/fixtures/bsc-refined450-layouts.json"))?)?,
    )?;
    println!("Prepared two NOT-QUALIFIED OFT role candidates in {}", output.display());
    Ok(())
}
