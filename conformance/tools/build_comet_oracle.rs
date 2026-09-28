//! Build the bounded, host-only Comet source oracle. Static pinned GitHub
//! downloads only; never contacts a chain endpoint. Preserve every attempt.
use std::{env, fs, path::Path, process::Command};

const PIN: &str = "f766f51583c23acc33b2a7824654ef2029a96804";
const SOLC_PIN: &str = "16a99b8c26ed33a91796e209ff6797ee7baf2b0d";
const SOLC_SHA: &str = "00656dc73224e4c0702940df10310bdc024b60f4a7598e774d305bc3b94f7d79";

fn run(command: &mut Command) -> String {
    let output = command.output().expect("start external tool");
    assert!(output.status.success(), "{command:?}: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).expect("tool stdout UTF-8")
}

fn digest(path: &Path) -> String {
    run(Command::new("shasum").args(["-a", "256"]).arg(path))
}

fn fetch(url: &str, path: &Path) {
    run(Command::new("curl")
        .args(["--fail", "--silent", "--show-error", "--location", url, "--output"])
        .arg(path));
}

fn item(source: &str, prefix: &str) -> String {
    assert_eq!(source.matches(prefix).count(), 1, "ambiguous pinned item {prefix}");
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
    panic!("unterminated pinned item");
}

fn main() {
    let args: Vec<_> = env::args().collect();
    assert_eq!(args.len(), 3, "usage: build-comet-oracle OFFICIAL_MACOS_SOLC_0_8_15 FRESH_OUTPUT_DIRECTORY");
    let output = Path::new(&args[2]);
    assert!(!output.exists(), "output must be fresh; preserve earlier evidence");
    fs::create_dir_all(output).unwrap();
    let compiler = fs::canonicalize(&args[1]).unwrap();
    let version = run(Command::new(&compiler).arg("--version"));
    assert!(version.contains("0.8.15+commit.e14f2714"), "wrong compiler");
    let compiler_digest = digest(&compiler);
    assert_eq!(
        compiler_digest.split_whitespace().next().unwrap(),
        SOLC_SHA,
        "use the pinned official compiler binary"
    );
    let manifest_url = format!("https://raw.githubusercontent.com/ethereum/solc-bin/{SOLC_PIN}/macosx-amd64/list.json");
    fetch(&manifest_url, &output.join("solc-list.json"));
    let manifest = fs::read_to_string(output.join("solc-list.json")).unwrap();
    let release = "\"path\": \"solc-macosx-amd64-v0.8.15+commit.e14f2714\"";
    assert_eq!(manifest.matches(release).count(), 1);
    let entry = manifest[manifest.find(release).unwrap()..].split('}').next().unwrap();
    assert!(entry.contains(&format!("\"sha256\": \"0x{SOLC_SHA}\"")));
    assert!(entry.contains("\"longVersion\": \"0.8.15+commit.e14f2714\""));
    fs::write(output.join("build_comet_oracle.rs"), include_str!("build_comet_oracle.rs")).unwrap();
    let mut provenance = format!("pin: {PIN}\n{version}{compiler_digest}\ncompiler manifest: {manifest_url}\n");
    provenance.push_str(&digest(&output.join("solc-list.json")));
    provenance.push_str(&digest(&output.join("build_comet_oracle.rs")));
    let license_url = format!("https://raw.githubusercontent.com/compound-finance/comet/{PIN}/LICENSE");
    fetch(&license_url, &output.join("LICENSE"));
    provenance.push_str(&format!("{license_url}\n{}", digest(&output.join("LICENSE"))));
    for name in [
        "CometWithExtendedAssetList.sol",
        "CometMainInterface.sol",
        "CometCore.sol",
        "CometMath.sol",
        "CometStorage.sol",
        "CometConfiguration.sol",
    ] {
        let url = format!("https://raw.githubusercontent.com/compound-finance/comet/{PIN}/contracts/{name}");
        fetch(&url, &output.join(name));
        provenance.push_str(&format!("{url}\n{}", digest(&output.join(name))));
    }
    let main = fs::read_to_string(output.join("CometWithExtendedAssetList.sol")).unwrap();
    let interface = fs::read_to_string(output.join("CometMainInterface.sol")).unwrap();
    assert_eq!(interface.matches("error TimestampTooLarge();").count(), 1);
    let mut functions = String::new();
    for prefix in [
        "function getNowInternal()",
        "function accruedInterestIndices(",
        "function getSupplyRate(",
        "function getBorrowRate(",
        "function getUtilization()",
        "function mulFactor(",
        "function balanceOf(",
        "function borrowBalanceOf(",
    ] {
        let original = item(&main, prefix);
        let mut function = original.replace("override ", "");
        if prefix == "function balanceOf(" || prefix == "function borrowBalanceOf(" {
            assert_eq!(function.matches("userBasic[account].principal").count(), 1);
            function = function.replace("userBasic[account].principal", "boundPrincipal");
        }
        functions.push_str(&function);
        functions.push('\n');
    }
    let harness = format!(
        r#"// SPDX-License-Identifier: BUSL-1.1
pragma solidity 0.8.15;
import './CometCore.sol';
// Pinned function bodies below: only override modifiers and the two holder
// mapping reads are substituted. Rate immutables are explicit input slots.
// Original Core, Storage, Math and Configuration are inherited unchanged.
contract Oracle is CometCore {{
    uint supplyKink;
    uint supplyPerSecondInterestRateSlopeLow;
    uint supplyPerSecondInterestRateSlopeHigh;
    uint supplyPerSecondInterestRateBase;
    uint borrowKink;
    uint borrowPerSecondInterestRateSlopeLow;
    uint borrowPerSecondInterestRateSlopeHigh;
    uint borrowPerSecondInterestRateBase;
    int104 boundPrincipal;
    error TimestampTooLarge();
{functions}
    function indices() external view returns (uint64, uint64) {{
        return accruedInterestIndices(getNowInternal() - lastAccrualTime);
    }}
    function presentSupply(uint64 index, uint104 principal) external pure returns (uint) {{
        return presentValueSupply(index, principal);
    }}
    function presentBorrow(uint64 index, uint104 principal) external pure returns (uint) {{
        return presentValueBorrow(index, principal);
    }}
    function principalSupply(uint64 index, uint present) external pure returns (uint104) {{
        return principalValueSupply(index, present);
    }}
    function principalBorrow(uint64 index, uint present) external pure returns (uint104) {{
        return principalValueBorrow(index, present);
    }}
}}
"#
    );
    fs::write(output.join("Oracle.sol"), harness).unwrap();
    let arguments = [
        "--bin-runtime",
        "--hashes",
        "--storage-layout",
        "--optimize",
        "--evm-version",
        "petersburg",
        "--metadata-hash",
        "none",
        "-o",
        "build",
        "Oracle.sol",
    ];
    provenance.push_str(&format!("compiler arguments: {}\n", arguments.join(" ")));
    provenance.push_str(&run(Command::new(&compiler).current_dir(output).args(arguments)));
    for name in ["Oracle.sol", "build/Oracle.bin-runtime", "build/Oracle.signatures", "build/Oracle_storage.json"] {
        provenance.push_str(&digest(&output.join(name)));
    }
    fs::write(output.join("provenance.txt"), provenance).unwrap();
    println!("Pinned source oracle saved in {}", output.display());
}
