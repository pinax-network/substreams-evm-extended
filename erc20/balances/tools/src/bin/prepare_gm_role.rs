#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::ensure;
    use erc20_balances_tools::gm_role as bound;
    use serde_json::json;
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: prepare_gm_role ORIGINAL_PACKAGE_ROOT FRESH_OUTPUT");
    let out = Path::new(&args[2]);
    fs::create_dir(out)?;
    fs::write(out.join("prepare_gm_role.rs"), include_str!("prepare_gm_role.rs"))?;
    fs::write(out.join("gm_role.rs"), include_str!("../gm_role.rs"))?;
    let result = (|| -> anyhow::Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        bound::verify_cache(root, Path::new(&args[1]))?;
        for (name, value) in [
            ("source-review.json", bound::review(root)?),
            (
                "layouts.json",
                bound::candidate(&fs::read(root.join("tests/fixtures/bsc-refined450-layouts.json"))?)?,
            ),
        ] {
            fs::write(out.join(name), format!("{}\n", serde_json::to_string_pretty(&value)?))?;
        }
        Ok(())
    })();
    let report = match &result {
        Ok(()) => json!({"status":"prepared","qualified":false,"network_requests":0}),
        Err(e) => json!({"status":"failed","qualified":false,"error":format!("{e:#}")}),
    };
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    result
}
