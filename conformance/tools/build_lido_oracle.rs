//! Build the bounded Lido getter oracle from immutable primary sources.
//! Run with rustc; only public source/compiler metadata reads, never chain RPC.
use std::{env, fs, path::Path, process::Command};

const LIDO: &str = "2da0f48f1a2a103a394dcf8760810fe9165697fb";
const ARAGON: &str = "f3ae59b00f73984e562df00129c925339cd069ff";
const SOLC_BIN: &str = "16a99b8c26ed33a91796e209ff6797ee7baf2b0d";
const COMPILER_SHA: &str = "7034c4048bc713d5c14cdd6681953c736e2adbdb9174f8bfbfb6a097109ffaaa";
const COMPILER_NAME: &str = "solc-macosx-amd64-v0.4.24+commit.e67f0147";

fn run(command: &mut Command) -> String {
    let output = command.output().expect("start tool");
    assert!(output.status.success(), "{command:?}: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).expect("UTF-8 stdout")
}
fn sha(path: &Path) -> String {
    run(Command::new("shasum").args(["-a", "256"]).arg(path))
}
fn fetch(url: &str, path: &Path, provenance: &mut String) {
    run(Command::new("curl")
        .args(["--fail", "--silent", "--show-error", "--location", url, "--output"])
        .arg(path));
    provenance.push_str(&format!("{url}\n{}", sha(path)));
}
// The selected pinned functions contain no braces in quoted strings/comments.
// Require one exact prefix, then preserve every byte through its closing brace.
fn function(source: &str, prefix: &str) -> String {
    assert_eq!(source.matches(prefix).count(), 1, "ambiguous pinned function {prefix}");
    let start = source.find(prefix).unwrap();
    let open = start + source[start..].find('{').unwrap();
    let mut depth = 0;
    for (offset, byte) in source.as_bytes()[open..].iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return source[start..=open + offset].into();
        }
    }
    panic!("unterminated source function");
}
fn declaration(source: &str, prefix: &str) -> String {
    assert_eq!(source.matches(prefix).count(), 1, "ambiguous pinned declaration");
    let start = source.find(prefix).unwrap();
    let end = start + source[start..].find(';').unwrap();
    source[start..=end].into()
}

fn main() {
    let args: Vec<_> = env::args().collect();
    assert_eq!(args.len(), 3, "usage: build-lido-oracle SOLC_0_4_24 FRESH_OUTPUT_DIRECTORY");
    let compiler = fs::canonicalize(&args[1]).unwrap();
    let output = Path::new(&args[2]);
    assert!(!output.exists(), "preserve earlier attempts; output must be fresh");
    fs::create_dir_all(output).unwrap();
    let mut provenance = String::new();
    fs::write(output.join("build_lido_oracle.rs"), include_str!("build_lido_oracle.rs")).unwrap();
    provenance.push_str(&sha(&output.join("build_lido_oracle.rs")));
    for (repo, pin, source, name) in [
        ("lidofinance/core", LIDO, "contracts/0.4.24/StETH.sol", "StETH.sol"),
        ("lidofinance/core", LIDO, "contracts/0.4.24/Lido.sol", "Lido.sol"),
        (
            "lidofinance/core",
            LIDO,
            "contracts/0.4.24/utils/UnstructuredStorageExt.sol",
            "UnstructuredStorageExt.sol",
        ),
        ("aragon/aragonOS", ARAGON, "contracts/common/UnstructuredStorage.sol", "UnstructuredStorage.sol"),
        ("aragon/aragonOS", ARAGON, "contracts/lib/math/SafeMath.sol", "SafeMath.sol"),
        ("lidofinance/core", LIDO, "LICENSE", "lido-LICENSE.txt"),
        ("aragon/aragonOS", ARAGON, "LICENSE", "aragon-LICENSE.txt"),
    ] {
        fetch(
            &format!("https://raw.githubusercontent.com/{repo}/{pin}/{source}"),
            &output.join(name),
            &mut provenance,
        );
        fs::write(output.join("provenance.txt"), &provenance).unwrap();
    }
    let manifest = format!("https://raw.githubusercontent.com/ethereum/solc-bin/{SOLC_BIN}/macosx-amd64/list.json");
    fetch(&manifest, &output.join("solc-list.json"), &mut provenance);
    let list = fs::read_to_string(output.join("solc-list.json")).unwrap();
    let start = list.find(&format!("\"path\": \"{COMPILER_NAME}\"")).expect("official compiler manifest entry");
    let end = start + list[start..].find('}').unwrap();
    assert!(list[start..end].contains(&format!("\"sha256\": \"0x{COMPILER_SHA}\"")));
    assert!(sha(&compiler).starts_with(COMPILER_SHA), "compiler differs from official immutable manifest");
    let version = run(Command::new(&compiler).arg("--version"));
    assert!(version.contains("0.4.24+commit.e67f0147"));
    provenance.push_str(&format!(
        "compiler_url=https://raw.githubusercontent.com/ethereum/solc-bin/{SOLC_BIN}/macosx-amd64/{COMPILER_NAME}\n{version}{}",
        sha(&compiler)
    ));

    let steth = fs::read_to_string(output.join("StETH.sol")).unwrap();
    let lido = fs::read_to_string(output.join("Lido.sol")).unwrap();
    let ext = fs::read_to_string(output.join("UnstructuredStorageExt.sol")).unwrap();
    let mut text = String::from(
        "// SPDX-License-Identifier: GPL-3.0\n// Extracted pinned-source getter harness; substitutions are listed in provenance.txt.\npragma solidity 0.4.24;\nimport './UnstructuredStorage.sol';\nimport './SafeMath.sol';\nlibrary PackedOracle {\nusing UnstructuredStorage for bytes32;\n",
    );
    text.push_str(&declaration(&ext, "uint256 constant internal UINT128_LOW_MASK"));
    text.push('\n');
    text.push_str(&function(&ext, "function getLowAndHighUint128("));
    text.push_str("\n}\ncontract LidoOracle {\nusing SafeMath for uint256;\nusing PackedOracle for bytes32;\n");
    text.push_str(&declaration(&steth, "uint256 constant internal UINT128_MAX"));
    text.push('\n');
    for (source, prefix, original, replacement) in [
        (
            &steth,
            "bytes32 internal constant TOTAL_SHARES_POSITION_LOW128",
            "0x6038150aecaa250d524370a0fdcdec13f2690e0723eaf277f41d7cae26b359e6",
            "bytes32(0)",
        ),
        (
            &lido,
            "bytes32 internal constant BUFFERED_ETHER_AND_DEPOSITED_POST_REPORT_POSITION",
            "0x81a11fa1111afa59b50051f60ccf604a39d96acb484dc467ad8eadb4a63f0a5f",
            "bytes32(1)",
        ),
        (
            &lido,
            "bytes32 internal constant CL_VALIDATORS_BALANCE_AND_CL_PENDING_BALANCE_POSITION",
            "0x096e465397f38e659238ccd5d5a2c434ced54a63fd8d694045bfb058ab9d8112",
            "bytes32(2)",
        ),
    ] {
        let constant = declaration(source, prefix);
        assert_eq!(constant.matches(original).count(), 1);
        text.push_str(&constant.replace(original, replacement));
        text.push('\n');
        provenance.push_str(&format!("storage relocation: {original} -> {replacement}\n"));
    }
    text.push_str(&declaration(&lido, "bytes32 internal constant TOTAL_AND_EXTERNAL_SHARES_POSITION"));
    text.push('\n');
    for (source, prefixes) in [
        (
            &steth,
            vec![
                "function balanceOf(",
                "function getSharesByPooledEth(",
                "function getPooledEthByShares(",
                "function getTotalPooledEther(",
            ],
        ),
        (
            &lido,
            vec![
                "function _getInternalEther(",
                "function _getExternalEther(",
                "function _getTotalPooledEther(",
                "function _getShareRateNumerator(",
                "function _getShareRateDenominator(",
                "function _getTotalAndExternalShares(",
                "function _getBufferedEtherAndDepositedPostReport(",
                "function _getClValidatorsBalanceAndClPendingBalance(",
            ],
        ),
    ] {
        for prefix in prefixes {
            text.push_str(&function(source, prefix));
            text.push('\n');
            provenance.push_str(&format!("verbatim function: {prefix}\n"));
        }
    }
    // No mapping is traversed: tests bind this one holder independently.
    text.push_str(
        "function _sharesOf(address) internal view returns (uint256 value) { assembly { value := sload(3) } }\nfunction internalEther() external view returns (uint256) { return _getInternalEther(); }\nfunction internalShares() external view returns (uint256) { return _getShareRateDenominator(); }\nfunction externalEther() external view returns (uint256) { return _getExternalEther(_getInternalEther()); }\n}\n",
    );
    provenance.push_str("Other harness substitutions: PackedOracle contains the verbatim UnstructuredStorageExt getLowAndHighUint128 function and low mask; library name/import plumbing changed only. _sharesOf(address) is a controlled slot3 read, not a real holder mapping. Three public wrappers expose unchanged internal getters. No arithmetic function body edits.\n");
    fs::write(output.join("LidoOracle.sol"), text).unwrap();
    let compiler_args = [
        "--bin-runtime",
        "--hashes",
        "--optimize",
        "--optimize-runs",
        "200",
        "--evm-version",
        "byzantium",
        "-o",
        "build",
        "LidoOracle.sol",
    ];
    provenance.push_str(&format!(
        "compiler arguments (working directory is fresh output): {}\n",
        compiler_args.join(" ")
    ));
    let built = Command::new(&compiler).current_dir(output).args(compiler_args).output().unwrap();
    fs::write(output.join("compiler.stdout"), &built.stdout).unwrap();
    fs::write(output.join("compiler.stderr"), &built.stderr).unwrap();
    fs::write(output.join("provenance.txt"), &provenance).unwrap();
    assert!(built.status.success(), "compiler failed; inspect preserved stdout/stderr");
    for name in ["LidoOracle.sol", "build/LidoOracle.bin-runtime", "build/LidoOracle.signatures"] {
        provenance.push_str(&sha(&output.join(name)));
    }
    fs::write(output.join("provenance.txt"), provenance).unwrap();
    println!("Pinned Lido oracle saved in {}", output.display());
}
