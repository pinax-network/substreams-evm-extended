//! Build a bounded SavingsDai arithmetic oracle from pinned source. Only
//! static GitHub source/compiler metadata is downloaded; no chain calls.
use std::{env, fs, path::Path, process::Command};

const PIN: &str = "665879762f8b5df5d234463f45d1d6a49bd4fbeb";
const SOLC_PIN: &str = "16a99b8c26ed33a91796e209ff6797ee7baf2b0d";
const SOLC_SHA: &str = "e40eef83c24d4c42b47f461b01748a6ca89f1e09e778995b71debfa0de99e12a";

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
    assert_eq!(source.matches(prefix).count(), 1, "ambiguous source item {prefix}");
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
    panic!("unterminated source item");
}
fn substitute(function: String, from: &str, to: &str, expected: usize) -> String {
    assert_eq!(function.matches(from).count(), expected, "unexpected substitution count: {from}");
    function.replace(from, to)
}
fn main() {
    let args: Vec<_> = env::args().collect();
    assert_eq!(args.len(), 3, "usage: build-sdai-oracle OFFICIAL_MACOS_SOLC_0_8_17 FRESH_OUTPUT_DIRECTORY");
    let output = Path::new(&args[2]);
    assert!(!output.exists(), "preserve earlier attempts; output must be fresh");
    fs::create_dir_all(output).unwrap();
    let compiler = fs::canonicalize(&args[1]).unwrap();
    let version = run(Command::new(&compiler).arg("--version"));
    assert!(version.contains("0.8.17+commit.8df45f5f"));
    let compiler_digest = digest(&compiler);
    assert_eq!(
        compiler_digest.split_whitespace().next().unwrap(),
        SOLC_SHA,
        "require the pinned official binary"
    );
    let manifest_url = format!("https://raw.githubusercontent.com/ethereum/solc-bin/{SOLC_PIN}/macosx-amd64/list.json");
    fetch(&manifest_url, &output.join("solc-list.json"));
    let manifest = fs::read_to_string(output.join("solc-list.json")).unwrap();
    let release = "\"path\": \"solc-macosx-amd64-v0.8.17+commit.8df45f5f\"";
    assert_eq!(manifest.matches(release).count(), 1);
    let entry = manifest[manifest.find(release).unwrap()..].split('}').next().unwrap();
    assert!(entry.contains(&format!("\"sha256\": \"0x{SOLC_SHA}\"")));
    assert!(entry.contains("\"longVersion\": \"0.8.17+commit.8df45f5f\""));
    fs::write(output.join("build_sdai_oracle.rs"), include_str!("build_sdai_oracle.rs")).unwrap();
    let mut provenance = format!("pin: {PIN}\n{version}{compiler_digest}\ncompiler manifest: {manifest_url}\n");
    provenance.push_str(&digest(&output.join("solc-list.json")));
    provenance.push_str(&digest(&output.join("build_sdai_oracle.rs")));
    for (remote, local) in [("LICENSE", "LICENSE"), ("src/SavingsDai.sol", "SavingsDai.sol")] {
        let url = format!("https://raw.githubusercontent.com/sky-ecosystem/sdai/{PIN}/{remote}");
        fetch(&url, &output.join(local));
        provenance.push_str(&format!("{url}\n{}", digest(&output.join(local))));
    }
    let source = fs::read_to_string(output.join("SavingsDai.sol")).unwrap();
    let header = source.split("pragma solidity").next().unwrap();
    assert!(header.contains("SPDX-License-Identifier: AGPL-3.0-or-later"));
    let constant = "uint256 private constant RAY = 10 ** 27;";
    assert_eq!(source.matches(constant).count(), 1);
    let mut functions = String::new();
    for name in [
        "_rpow",
        "_divup",
        "convertToAssets",
        "convertToShares",
        "previewDeposit",
        "previewMint",
        "previewWithdraw",
        "previewRedeem",
        "maxWithdraw",
    ] {
        let mut function = item(&source, &format!("function {name}("));
        if ["convertToAssets", "convertToShares", "previewMint", "previewWithdraw"].contains(&name) {
            function = substitute(function, "pot.rho()", "boundRho", 1);
            function = substitute(function, "pot.dsr()", "boundDsr", 1);
            function = substitute(function, "pot.chi()", "boundChi", 2);
        }
        if name == "maxWithdraw" {
            function = substitute(function, "balanceOf[owner]", "boundShares", 1);
        }
        functions.push_str(&function);
        functions.push('\n');
    }
    let harness = format!(
        r#"{header}
pragma solidity 0.8.17;
// Controlled input substitutions only: Pot scalar calls and one holder
// mapping read. Source arithmetic, assembly and branch order stay intact.
contract Oracle {{
    {constant}
    uint256 boundChi;
    uint256 boundRho;
    uint256 boundDsr;
    uint256 boundShares;
{functions}
    function rpow(uint256 x, uint256 n) external pure returns (uint256) {{
        return _rpow(x, n);
    }}
    function divup(uint256 x, uint256 y) external pure returns (uint256) {{
        return _divup(x, y);
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
    println!("Pinned SavingsDai source oracle saved in {}", output.display());
}
