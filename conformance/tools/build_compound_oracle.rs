//! Fetch the exact Compound sources used by the offline arithmetic oracle.
//! Host-only; never contacts a chain endpoint.
use std::{env, fs, path::Path, process::Command};

const CURRENT: &str = "a3214f67b73310d547e00fc578e8355911c9d376";
const LEGACY: &str = "f385d71983ae5c5799faae9b2dfea43e5cf75262";
const LEGACY_JUMP: &str = "4caf72a1f88335adc9cc06acf6f372241369ed01";

fn run(command: &mut Command) -> String {
    let output = command.output().expect("start external tool");
    assert!(output.status.success(), "{command:?}: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).expect("tool stdout UTF-8")
}

fn item(source: &str, prefix: &str) -> String {
    let start = source.find(prefix).expect("pinned item exists");
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

fn harness(directory: &Path, legacy: bool) {
    let ctoken = fs::read_to_string(directory.join("CToken.sol")).unwrap();
    let exchange = item(&ctoken, "function exchangeRateStoredInternal()");
    let mut accrue = item(&ctoken, "function accrueInterest()");
    let (imports, base, cap, event, wrapper, scalar, locals) =
        if legacy {
            let call = "interestRateModel.getBorrowRate(getCashPrior(), totalBorrows, totalReserves)";
            assert_eq!(accrue.matches(call).count(), 1);
            accrue = accrue.replace(call, "(rateError, boundRate)");
            (
            "import './Exponential.sol'; import './ErrorReporter.sol';", "Exponential, TokenErrorReporter", "5e14",
            "event AccrueInterest(uint a, uint b, uint c);",
            "function exchange() external view returns (MathError, uint) { return exchangeRateStoredInternal(); }",
            "function underlying(uint rate, uint shares) external pure returns (MathError, uint) { return mulScalarTruncate(Exp({mantissa: rate}), shares); }",
            item(&ctoken, "struct AccrueInterestLocalVars"),
        )
        } else {
            let call = "interestRateModel.getBorrowRate(cashPrior, borrowsPrior, reservesPrior)";
            assert_eq!(accrue.matches(call).count(), 1);
            accrue = accrue.replace(call, "boundRate").replace("virtual override public", "public");
            (
                "import './ExponentialNoError.sol';",
                "ExponentialNoError",
                "5e12",
                "event AccrueInterest(uint a, uint b, uint c, uint d); uint constant NO_ERROR = 0;",
                "function exchange() external view returns (uint) { return exchangeRateStoredInternal(); }",
                "function underlying(uint rate, uint shares) external pure returns (uint) { return mul_ScalarTruncate(Exp({mantissa: rate}), shares); }",
                String::new(),
            )
        };
    let text = format!(
        r#"// Source-execution harness. CToken function bodies are extracted verbatim,
// except the external rate call is a controlled input and override modifiers
// are omitted. State slots and dependency/clock stubs are harness-only.
pragma solidity {};
{imports}
contract MarketOracle is {base} {{
uint cash; uint totalBorrows; uint totalReserves; uint totalSupply;
uint borrowIndex; uint accrualBlockNumber; uint reserveFactorMantissa;
uint initialExchangeRateMantissa; uint currentBlock; uint boundRate; uint rateError;
uint constant borrowRateMaxMantissa = {cap};
{event}
function getCashPrior() internal view returns (uint) {{ return cash; }}
function getBlockNumber() internal view returns (uint) {{ return currentBlock; }}
{locals}
{exchange}
{accrue}
{wrapper}
{scalar}
function accrual() external returns (uint, uint, uint, uint, uint) {{
uint result = accrueInterest();
return (result, totalBorrows, totalReserves, borrowIndex, accrualBlockNumber);
}}
}}
"#,
        if legacy { "0.5.8" } else { "0.8.10" }
    );
    fs::write(directory.join("MarketOracle.sol"), text).unwrap();
    let rate = if legacy {
        r#"pragma solidity 0.5.8;
import './WhitePaperInterestRateModel.sol';
contract RateOracle is WhitePaperInterestRateModel {
constructor() WhitePaperInterestRateModel(0, 0) public {}
function utilization(uint cash, uint borrows) external pure returns (IRError, uint) {
(IRError err, Exp memory value) = getUtilizationRate(cash, borrows);
return (err, value.mantissa);
}
}
"#
    } else {
        r#"pragma solidity 0.8.10;
import './BaseJumpRateModelV2.sol';
contract RateOracle is BaseJumpRateModelV2 {
constructor() BaseJumpRateModelV2(0, 0, 0, 1, address(0)) {}
function getBorrowRate(uint cash, uint borrows, uint reserves) external view override returns (uint) {
return getBorrowRateInternal(cash, borrows, reserves);
}
function parameters(uint base, uint multiplier, uint jump, uint kink_) external returns (uint, uint, uint, uint) {
updateJumpRateModelInternal(base, multiplier, jump, kink_);
return (baseRatePerBlock, multiplierPerBlock, jumpMultiplierPerBlock, kink);
}
}
"#
    };
    fs::write(directory.join("RateOracle.sol"), rate).unwrap();
}

fn legacy_jump_harness(directory: &Path) {
    fs::write(
        directory.join("RateOracle.sol"),
        r#"pragma solidity 0.5.17;
import './LegacyJumpRateModelV2.sol';
contract RateOracle is LegacyJumpRateModelV2 {
constructor() LegacyJumpRateModelV2(0, 0, 0, 1, address(0)) public {}
function parameters(uint base, uint multiplier, uint jump, uint kink_) external returns (uint, uint, uint, uint) {
updateJumpRateModelInternal(base, multiplier, jump, kink_);
return (baseRatePerBlock, multiplierPerBlock, jumpMultiplierPerBlock, kink);
}
}
"#,
    )
    .unwrap();
}

fn main() {
    let args: Vec<_> = env::args().collect();
    assert_eq!(
        args.len(),
        5,
        "usage: build-compound-oracle SOLC_0_8_10 SOLC_0_5_8 SOLC_0_5_17 FRESH_OUTPUT_DIRECTORY"
    );
    let output = Path::new(&args[4]);
    assert!(!output.exists(), "preserve earlier evidence; output must be fresh");
    fs::create_dir_all(output).unwrap();
    let mut provenance = String::new();
    for (label, pin, names) in [
        (
            "current",
            CURRENT,
            vec!["CToken.sol", "ExponentialNoError.sol", "BaseJumpRateModelV2.sol", "InterestRateModel.sol"],
        ),
        (
            "legacy",
            LEGACY,
            vec![
                "CToken.sol",
                "Exponential.sol",
                "CarefulMath.sol",
                "WhitePaperInterestRateModel.sol",
                "InterestRateModel.sol",
                "ErrorReporter.sol",
            ],
        ),
        (
            "legacy-jump",
            LEGACY_JUMP,
            vec![
                "LegacyJumpRateModelV2.sol",
                "BaseJumpRateModelV2.sol",
                "LegacyInterestRateModel.sol",
                "SafeMath.sol",
            ],
        ),
    ] {
        let directory = output.join(label);
        fs::create_dir(&directory).unwrap();
        for name in names {
            let url = format!("https://raw.githubusercontent.com/compound-finance/compound-protocol/{pin}/contracts/{name}");
            let path = directory.join(name);
            run(Command::new("curl")
                .args(["--fail", "--silent", "--show-error", "--location", &url, "--output"])
                .arg(&path));
            let digest = run(Command::new("shasum").args(["-a", "256"]).arg(&path));
            provenance.push_str(&format!("{url}\n{digest}"));
        }
        if label == "legacy-jump" {
            legacy_jump_harness(&directory);
        } else {
            harness(&directory, label == "legacy");
        }
        let compiler = fs::canonicalize(
            &args[match label {
                "current" => 1,
                "legacy" => 2,
                _ => 3,
            }],
        )
        .unwrap();
        let version = run(Command::new(&compiler).arg("--version"));
        let expected = match label {
            "current" => "0.8.10+commit.fc410830",
            "legacy" => "0.5.8+commit.23d335f2",
            _ => "0.5.17+commit.d19bba13",
        };
        assert!(version.contains(expected), "wrong compiler: {version}");
        provenance.push_str(&version);
        provenance.push_str(&run(Command::new("shasum").args(["-a", "256"]).arg(&compiler)));
        let mut command = Command::new(&compiler);
        command.current_dir(&directory).args([
            "--bin-runtime",
            "--hashes",
            "--optimize",
            "--evm-version",
            "petersburg",
            "-o",
            "build",
            "RateOracle.sol",
        ]);
        if label != "legacy-jump" {
            command.arg("MarketOracle.sol");
        }
        if label == "current" {
            command.args(["--metadata-hash", "none"]);
        }
        let compiled = run(&mut command);
        provenance.push_str(&compiled);
        for name in [
            "MarketOracle.sol",
            "RateOracle.sol",
            "build/MarketOracle.bin-runtime",
            "build/RateOracle.bin-runtime",
            "build/MarketOracle.signatures",
            "build/RateOracle.signatures",
        ] {
            if label == "legacy-jump" && name.contains("MarketOracle") {
                continue;
            }
            provenance.push_str(&run(Command::new("shasum").args(["-a", "256"]).arg(directory.join(name))));
        }
    }
    fs::write(output.join("provenance.txt"), provenance).unwrap();
    println!("Pinned source capture saved in {}", output.display());
}
