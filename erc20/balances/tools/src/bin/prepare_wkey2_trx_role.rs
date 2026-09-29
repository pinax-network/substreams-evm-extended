#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::ensure;
    use erc20_balances_tools::wkey2_trx_role as bound;
    use serde_json::json;
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: prepare_wkey2_trx_role ORIGINAL_PACKAGE_ROOT FRESH_OUTPUT");
    let out = Path::new(&args[2]);
    fs::create_dir(out)?;
    fs::write(out.join("prepare_wkey2_trx_role.rs"), include_str!("prepare_wkey2_trx_role.rs"))?;
    fs::write(out.join("wkey2_trx_role.rs"), include_str!("../wkey2_trx_role.rs"))?;
    let result = (|| -> anyhow::Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        bound::verify_cache(root, Path::new(&args[1]))?;
        let review = bound::review(root)?;
        let candidate = bound::candidate(&fs::read(root.join("tests/fixtures/bsc-refined450-layouts.json"))?)?;
        for (name, value) in [("layouts.json", candidate), ("source-review.json", review)] {
            fs::write(out.join(name), format!("{}\n", serde_json::to_string_pretty(&value)?))?;
        }
        Ok(())
    })();
    let report = match &result {
        Ok(()) => json!({"status":"prepared","qualified":false}),
        Err(e) => json!({"status":"failed","qualified":false,"error":format!("{e:#}")}),
    };
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    result
}
