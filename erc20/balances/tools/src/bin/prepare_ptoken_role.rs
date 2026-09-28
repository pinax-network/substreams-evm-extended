#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::ensure;
    use erc20_balances_tools::ptoken_role as bound;
    use serde_json::json;
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: prepare_ptoken_role ORIGINAL_PACKAGE_ROOT FRESH_OUTPUT");
    let out = Path::new(&args[2]);
    fs::create_dir(out)?;
    fs::write(out.join("prepare_ptoken_role.rs"), include_str!("prepare_ptoken_role.rs"))?;
    fs::write(out.join("ptoken_role.rs"), include_str!("../ptoken_role.rs"))?;
    let result = (|| -> anyhow::Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let raw = fs::read(Path::new(&args[1]).join(bound::CACHE))?;
        ensure!(
            raw == fs::read(root.join(bound::PROOF_FIXTURE).join("capture.json"))?,
            "original cache matches frozen Phase A capture"
        );
        let review = bound::review(
            &raw,
            &fs::read(root.join(bound::PROOF_FIXTURE).join("compiler-output.json"))?,
            &fs::read(root.join("docs/evidence/ptoken-operation-proof-20260928-transcripts.json"))?,
            &fs::read(root.join("docs/evidence/ptoken-operation-proof-20260928.json"))?,
            &fs::read(root.join(bound::PROOF_FIXTURE).join("primary-sources.json"))?,
        )?;
        for (name, v) in [
            ("source-review.json", review),
            (
                "layouts.json",
                bound::candidate(&fs::read(root.join("tests/fixtures/bsc-refined450-layouts.json"))?)?,
            ),
        ] {
            fs::write(out.join(name), format!("{}\n", serde_json::to_string_pretty(&v)?))?;
        }
        Ok(())
    })();
    let status = match &result {
        Ok(()) => json!({"status":"prepared","qualified":false}),
        Err(e) => json!({"status":"failed","error":format!("{e:#}"),"qualified":false}),
    };
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&status)?)?;
    result
}
