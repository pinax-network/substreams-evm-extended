#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::tagger_role as bound;
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: prepare_tagger_role <original package root> <fresh output>");
    let output = Path::new(&args[2]);
    fs::create_dir(output).context("fresh output; parent must exist")?;
    fs::write(output.join("prepare_tagger_role.rs"), include_str!("prepare_tagger_role.rs"))?;
    fs::write(output.join("tagger_role.rs"), include_str!("../tagger_role.rs"))?;
    let raw = fs::read(Path::new(&args[1]).join(bound::CACHE))?;
    let capture = bound::verify_capture(&raw)?;
    fs::write(output.join("TaggerToken.json"), raw)?;
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
    println!("Prepared one NOT-QUALIFIED Tagger candidate in {}", output.display());
    Ok(())
}
