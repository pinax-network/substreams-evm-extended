#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::artx_role as bound;
    use serde_json::json;
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 4,
        "usage: prepare_artx_role <original package root> <saved primary proof> <fresh output>"
    );
    let output = Path::new(&args[3]);
    fs::create_dir(output).context("output must be fresh")?;
    let tools = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut implementation = serde_json::Map::new();
    for file in ["src/artx_role.rs", "src/bin/prepare_artx_role.rs"] {
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
            serde_json::to_string_pretty(
                &json!({"qualified":false,"command_arguments":args[1..],"implementation_sha256":implementation,"live_chain_calls":false,"public_source_requests":"Only immutable primary GitHub URLs recorded in primary-sources.json"})
            )?
        ),
    )?;
    let mut captures = Vec::new();
    for p in &bound::CAPTURES {
        let raw = fs::read(Path::new(&args[1]).join(p.cache))?;
        captures.push(bound::verify_capture(&raw, p)?);
        fs::write(output.join(format!("{}.json", p.label)), raw)?;
    }
    let raw = fs::read(&args[2])?;
    let primary = bound::verify_primary_raw(&raw, &captures)?;
    fs::write(output.join("primary-sources.json"), raw)?;
    for s in primary["sources"].as_array().unwrap() {
        let content = ureq::get(s["url"].as_str().unwrap()).call()?.into_string()?;
        let path = output.join("primary").join(s["capture"].as_str().unwrap()).join(s["path"].as_str().unwrap());
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, &content)?;
        ensure!(s["content"] == content, "fresh immutable primary source mismatch");
    }
    let root = tools.parent().unwrap();
    for (name, value) in [
        ("source-review.json", bound::review(&captures)?),
        (
            "layouts.json",
            bound::candidate(&fs::read(root.join("tests/fixtures/bsc-refined450-layouts.json"))?)?,
        ),
    ] {
        fs::write(output.join(name), format!("{}\n", serde_json::to_string_pretty(&value)?))?;
    }
    println!("Prepared one NOT-QUALIFIED Artx proxy membership candidate in {}", output.display());
    Ok(())
}
