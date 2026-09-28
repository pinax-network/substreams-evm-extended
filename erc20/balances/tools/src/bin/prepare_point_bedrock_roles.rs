#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::point_bedrock_roles as bound;
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: prepare_point_bedrock_roles <original package root> <fresh output>");
    let output = Path::new(&args[2]);
    fs::create_dir(output).context("output must be fresh; parent must exist")?;
    let mut captures = Vec::new();
    for p in &bound::PROFILES {
        let raw = fs::read(Path::new(&args[1]).join(format!("out/ranks251-300-source-review/{}.json", p.contract)))?;
        captures.push(bound::verify_capture(&raw, p)?);
        fs::write(output.join(format!("{}.json", p.name)), raw)?;
    }
    let mut primary_profiles = Vec::new();
    for (capture, p) in captures.iter().zip(&bound::PROFILES) {
        let mut sources = serde_json::Map::new();
        for name in capture["sources"].as_object().context("source set")?.keys() {
            if let Some(url) = bound::primary_url(p, name)? {
                let content = ureq::get(&url).call()?.into_string()?;
                let path = output.join("primary").join(p.name).join(name);
                fs::create_dir_all(path.parent().unwrap())?;
                fs::write(path, &content)?;
                sources.insert(
                    name.clone(),
                    serde_json::json!({"url":url,"sha256":bound::sha(content.as_bytes()),"content":content}),
                );
            }
        }
        primary_profiles.push(serde_json::json!({"contract":p.contract,"sources":sources}));
    }
    let primary = serde_json::json!({"qualified":false,"point_token_source_gap":bound::POINT_SOURCE_GAP,"profiles":primary_profiles});
    fs::write(output.join("primary-sources.json"), format!("{}\n", serde_json::to_string_pretty(&primary)?))?;
    bound::verify_primary(&captures, &primary)?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    for (name, value) in [
        ("source-review.json", bound::review(&captures)?),
        (
            "layouts.json",
            bound::candidate(&fs::read(root.join("tests/fixtures/bsc-refined450-layouts.json"))?)?,
        ),
    ] {
        fs::write(output.join(name), format!("{}\n", serde_json::to_string_pretty(&value)?))?;
    }
    println!("Prepared two NOT-QUALIFIED candidates in {}", output.display());
    Ok(())
}
