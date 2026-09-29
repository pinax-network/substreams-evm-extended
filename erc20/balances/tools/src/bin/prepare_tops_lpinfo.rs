#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::ensure;
    use erc20_balances_tools::{tops_lpinfo as p, tops_proof::sha};
    use serde_json::json;
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: prepare_tops_lpinfo ORIGINAL_PACKAGE FRESH_OUTPUT");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let out = Path::new(&args[2]);
    fs::create_dir(out)?;
    let mut snapshots = vec![];
    for (name, relative, source) in [
        ("preparer.rs", "tools/src/bin/prepare_tops_lpinfo.rs", include_str!("prepare_tops_lpinfo.rs")),
        ("binding.rs", "tools/src/tops_lpinfo.rs", include_str!("../tops_lpinfo.rs")),
        ("validator.rs", "src/lpinfo_arrays.rs", include_str!("../../../src/lpinfo_arrays.rs")),
    ] {
        ensure!(fs::read(root.join(relative))? == source.as_bytes(), "stale executable {relative}");
        fs::write(out.join(name), source)?;
        snapshots.push(json!({"file":name,"source":relative,"sha256":sha(source.as_bytes())}));
    }
    let result = (|| -> anyhow::Result<_> {
        p::verify_cache(root, Path::new(&args[1]))?;
        let review = p::review(root)?;
        fs::write(out.join("source-review.json"), serde_json::to_vec_pretty(&review)?)?;
        let layouts = p::candidate(&fs::read(root.join("tests/fixtures/bsc-refined450-layouts.json"))?)?;
        fs::write(out.join("layouts.json"), serde_json::to_vec_pretty(&layouts)?)?;
        Ok(
            json!({"status":"prepared","qualified":false,"network_requests":0,"source_review_sha256":sha(&fs::read(out.join("source-review.json"))?),"layouts_sha256":sha(&fs::read(out.join("layouts.json"))?)}),
        )
    })();
    let (mut report, status) = match result {
        Ok(v) => (v, Ok(())),
        Err(e) => (json!({"status":"failed","qualified":false,"error":format!("{e:#}")}), Err(e)),
    };
    report["as_run_sources"] = json!(snapshots);
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    status
}
