#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::tops_runtime_proof as p;
    use serde_json::json;
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 2, "usage: prepare_tops_runtime_proof FRESH_OUTPUT");
    let out = Path::new(&args[1]);
    fs::create_dir(out).context("fresh output required")?;
    let result = (|| -> anyhow::Result<_> {
        let inventory = p::snapshot_sources(out)?;
        for source in [erc20_balances_tools::tops_proof::FIXTURE, p::FIXTURE] {
            for e in fs::read_dir(source)? {
                let path = e?.path();
                if path.is_file() && path.file_name().is_none_or(|s| s != "README.md") {
                    let dest = out.join(path.file_name().unwrap());
                    if dest.exists() {
                        ensure!(fs::read(&dest)? == fs::read(&path)?, "conflicting artifact");
                    } else {
                        fs::copy(path, dest)?;
                    }
                }
            }
        }
        p::verify_inputs(out)?;
        Ok(
            json!({"status":"passed","qualified":false,"chain_calls":0,"scope":p::LIMITS,"args":args,"source_inventory_sha256":inventory,"historical_compiler_report_sha256":p::HISTORICAL_REPORT_SHA,"historical_compiler_inventory_sha256":p::HISTORICAL_INVENTORY_SHA,"compiler_run":"historical pinned original artifact reused; this stage verifies and snapshots the fresh executor sources only","artifacts":p::artifacts(out)?,"specifications":p::specs(out)?}),
        )
    })();
    let (report, status) = match result {
        Ok(v) => (v, Ok(())),
        Err(e) => (json!({"status":"failed","qualified":false,"error":format!("{e:#}")}), Err(e)),
    };
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    status
}
