#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::bas_role as bound;
    use serde_json::json;
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: prepare_bas_role <original package root> <fresh output>");
    let output = Path::new(&args[2]);
    fs::create_dir(output).context("output must be fresh")?;
    let tools = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut implementation = serde_json::Map::new();
    for file in ["src/bas_role.rs", "src/bin/prepare_bas_role.rs"] {
        let bytes = fs::read(tools.join(file))?;
        let dest = output.join("implementation").join(file);
        fs::create_dir_all(dest.parent().unwrap())?;
        fs::write(dest, &bytes)?;
        implementation.insert(file.into(), json!(bound::sha(&bytes)));
    }
    fs::write(
        output.join("preparation.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&json!({
                "qualified":false,"command_arguments":args[1..],"implementation_sha256":implementation,
                "source_capture_sha256":bound::CAPTURE,"live_chain_calls":false,
                "public_source_requests":"Only immutable GitHub URLs recorded in primary-sources.json"
            }))?
        ),
    )?;
    let raw = fs::read(Path::new(&args[1]).join(format!("out/ranks101-150-source-review/{}.json", bound::CONTRACT)))?;
    let capture = bound::verify_capture(&raw)?;
    fs::write(output.join("source-capture.json"), raw)?;
    let mut sources = serde_json::Map::new();
    for name in capture["sources"]
        .as_object()
        .context("source set")?
        .keys()
        .filter(|n| n.as_str() != bound::TOKEN)
    {
        let url = bound::primary_url(name)?;
        let content = ureq::get(&url).call()?.into_string()?;
        let path = output.join("primary").join(name);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, &content)?;
        sources.insert(name.clone(), json!({"url":url,"sha256":bound::sha(content.as_bytes()),"content":content}));
    }
    let mut revisions = Vec::new();
    for (pin, _) in bound::TOKEN_REVISIONS {
        let url = bound::token_url(pin);
        let content = ureq::get(&url).call()?.into_string()?;
        fs::write(output.join(format!("token-{pin}.sol")), &content)?;
        revisions.push(json!({"pin":pin,"url":url,"sha256":bound::sha(content.as_bytes()),"exact_match":capture["sources"][bound::TOKEN]["content"]==content,"content":content}));
    }
    let primary = json!({"qualified":false,"oz_pin":bound::OZ_PIN,"token_source_gap":bound::SOURCE_GAP,"sources":sources,"token_revisions":revisions});
    fs::write(output.join("primary-sources.json"), format!("{}\n", serde_json::to_string_pretty(&primary)?))?;
    bound::verify_primary(&capture, &primary)?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    for (name, value) in [
        ("source-review.json", bound::review(&capture)?),
        (
            "layouts.json",
            bound::candidate(&fs::read(root.join("tests/fixtures/bsc-refined450-layouts.json"))?)?,
        ),
    ] {
        fs::write(output.join(name), format!("{}\n", serde_json::to_string_pretty(&value)?))?;
    }
    println!("Prepared one NOT-QUALIFIED BAS membership/fixed-admin candidate in {}", output.display());
    Ok(())
}
