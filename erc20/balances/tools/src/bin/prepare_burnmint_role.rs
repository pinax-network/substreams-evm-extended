#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use erc20_balances_tools::burnmint_role::{self as review, CONTRACT, PIN, PREFIX};
    use serde_json::json;
    use std::{fs, path::Path};
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "usage: prepare_burnmint_role <original package root> <fresh output>");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let output = Path::new(&args[2]);
    ensure!(!output.exists(), "output must be fresh");
    fs::create_dir_all(output)?;
    let raw = fs::read(Path::new(&args[1]).join(format!("out/ranks201-250-source-review/{CONTRACT}.json")))?;
    let capture = review::verify_capture(&raw)?;
    fs::write(output.join("source-capture.json"), &raw)?;
    let mut sources = serde_json::Map::new();
    for name in capture["sources"].as_object().context("source set")?.keys() {
        let url = review::primary_url(name)?;
        let content = ureq::get(&url).call()?.into_string()?;
        let saved = output.join("primary").join(name.strip_prefix(PREFIX).unwrap());
        fs::create_dir_all(saved.parent().unwrap())?;
        fs::write(saved, &content)?;
        sources.insert(name.clone(), json!({"url":url,"sha256":review::sha(content.as_bytes()),"content":content}));
    }
    let primary = json!({"qualified":false,"pin":PIN,"source_pin":format!("smartcontractkit/chainlink@{PIN} contracts/src/v0.8/shared/token/ERC20/BurnMintERC20.sol, contracts/src/v0.8/ccip/interfaces/IGetCCIPAdmin.sol and vendor/openzeppelin-solidity/v4.8.3; saved BSC source/runtime capture for {CONTRACT}"),"sources":sources,"difference":"Only BurnMintERC20's IGetCCIPAdmin import relocates ccip/interfaces to shared/interfaces; that interface is otherwise byte-identical.","captured_source_prefix":PREFIX});
    fs::write(output.join("primary-sources.json"), format!("{}\n", serde_json::to_string_pretty(&primary)?))?;
    review::verify_primary(&capture, &primary)?;
    let baseline = fs::read(root.join("tests/fixtures/bsc-refined450-layouts.json"))?;
    fs::write(
        output.join("layouts.json"),
        format!("{}\n", serde_json::to_string_pretty(&review::candidate(&baseline)?)?),
    )?;
    println!(
        "Prepared one unqualified BurnMint role candidate and 14 pinned primary sources in {}",
        output.display()
    );
    Ok(())
}
