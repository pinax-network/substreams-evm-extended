#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use erc20_balances_tools::bnbtiger_cookie_candidates as c;
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    anyhow::ensure!(args.len() == 2, "usage: prepare_bnbtiger_cookie_candidates <fresh output>");
    let out = Path::new(&args[1]);
    anyhow::ensure!(!out.exists(), "fresh output required");
    fs::create_dir_all(out)?;
    let mut report = serde_json::json!({"status":"incomplete","qualified":false});
    let result = (|| -> anyhow::Result<()> {
        let inputs = c::snapshot(out)?;
        let review = c::review(Path::new("."))?;
        let baseline = fs::read("erc20/balances/tests/fixtures/bsc-refined450-layouts.json")?;
        let candidates = c::candidates(&baseline)?;
        c::save(&out.join("layouts.json"), &candidates)?;
        for (i, label) in ["bnbtiger", "cookie"].iter().enumerate() {
            c::save(&out.join(format!("{label}-NOT-QUALIFIED.json")), &serde_json::json!([candidates[i]]))?;
        }
        c::save(&out.join("combined433.json"), &c::combined(&baseline, &candidates)?)?;
        c::save(&out.join("source-review.json"), &review)?;
        anyhow::ensure!(c::inventory()? == inputs, "source drift");
        report = serde_json::json!({"status":"passed","qualified":false,"source_inventory_sha256":c::sha(&fs::read(out.join("source-inputs.json"))?),"source_review_sha256":c::sha(&fs::read(out.join("source-review.json"))?),"limits":c::LIMITS});
        Ok(())
    })();
    if let Err(e) = &result {
        report["status"] = serde_json::json!("failed");
        report["error"] = serde_json::json!(format!("{e:#}"));
    }
    c::save(&out.join("report.json"), &report)?;
    result
}
