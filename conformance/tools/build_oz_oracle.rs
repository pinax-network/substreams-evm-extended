//! Regenerate the offline oracle with the pinned Solidity compiler, without
//! contacting a chain. Compile this host tool with the repository toolchain.
use std::{env, fs, path::Path, process::Command};

const PIN: &str = "932fddf69a699a9a80fd2396fd1a2ab91cdda123";
const HARNESS: &str = r#"// SPDX-License-Identifier: MIT
pragma solidity 0.8.20;
import {ERC4626} from "contracts/token/ERC20/extensions/ERC4626.sol";
import {ERC20, IERC20} from "contracts/token/ERC20/ERC20.sol";
// Inputs are supplied as storage by the Rust execution harness. All conversion
// methods are inherited unmodified from the pinned ERC4626 implementation.
contract Oracle is ERC4626 {
    constructor() ERC20("Oracle", "ORACLE") ERC4626(IERC20(address(1))) {}
    function totalAssets() public view override returns (uint256 v) { assembly { v := sload(0) } }
    function totalSupply() public view override(ERC20, IERC20) returns (uint256 v) { assembly { v := sload(1) } }
    function _decimalsOffset() internal view override returns (uint8 v) { assembly { v := sload(2) } }
    function balanceOf(address) public view override(ERC20, IERC20) returns (uint256 v) { assembly { v := sload(3) } }
}
"#;

fn run(command: &mut Command) -> String {
    let output = command.output().expect("start external tool");
    assert!(output.status.success(), "{command:?}: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).expect("tool stdout UTF-8")
}

fn main() {
    let args: Vec<_> = env::args().collect();
    assert_eq!(args.len(), 3, "usage: build-oz-oracle SOLC_0_8_20 FRESH_OUTPUT_DIRECTORY");
    let output = Path::new(&args[2]);
    assert!(!output.exists(), "output directory must be fresh; preserve older evidence");
    let compiler = fs::canonicalize(&args[1]).expect("compiler path");
    let version = run(Command::new(&compiler).arg("--version"));
    assert!(version.contains("0.8.20+commit.a1b79de6"), "wrong compiler: {version}");
    fs::create_dir_all(output).unwrap();
    let output = fs::canonicalize(output).unwrap();
    let sources = output.join("openzeppelin-contracts");
    run(Command::new("git").args(["init", "--quiet"]).arg(&sources));
    run(Command::new("git").arg("-C").arg(&sources).args([
        "fetch",
        "--quiet",
        "--depth=1",
        "https://github.com/OpenZeppelin/openzeppelin-contracts.git",
        PIN,
    ]));
    run(Command::new("git")
        .arg("-C")
        .arg(&sources)
        .args(["checkout", "--quiet", "--detach", "FETCH_HEAD"]));
    let head = run(Command::new("git").arg("-C").arg(&sources).args(["rev-parse", "HEAD"]));
    assert_eq!(head.trim(), PIN);
    fs::write(sources.join("Oracle.sol"), HARNESS).unwrap();
    let build = output.join("build");
    let compiled = run(Command::new(&compiler)
        .current_dir(&sources)
        .args([
            "--bin-runtime",
            "--hashes",
            "--optimize",
            "--evm-version",
            "paris",
            "--metadata-hash",
            "none",
            "--base-path",
            ".",
            "-o",
        ])
        .arg(&build)
        .arg("Oracle.sol"));
    let digests = run(Command::new("shasum").args(["-a", "256"]).arg(&compiler).args([
        sources.join("contracts/token/ERC20/extensions/ERC4626.sol"),
        sources.join("contracts/utils/math/Math.sol"),
        sources.join("Oracle.sol"),
        build.join("Oracle.bin-runtime"),
    ]));
    fs::write(output.join("provenance.txt"), format!("pin: {PIN}\n{version}\n{digests}\n{compiled}")).unwrap();
    println!(
        "Oracle runtime, selectors, pinned checkout and compiler provenance saved in {}",
        output.display()
    );
}
