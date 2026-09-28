//! Build bounded Aave / static-aToken source oracles without any chain calls.
use std::{env, fs, path::Path, process::Command};

const CURRENT: &str = "8305565ae342f1773c42cd2e4593f175fe5968a0";
const HALF_UP: &str = "d7a64127dbecb944a73670fbe6fba136329d9e15";
const STATIC: &str = "101f5d977889254ca2d2711b9582b45f832d10a0";
const OZ: &str = "69c8def5f222ff96f2b5beff05dfba996368aa79";
const SOLC_PIN: &str = "16a99b8c26ed33a91796e209ff6797ee7baf2b0d";
const SOLC_SHA: &str = "8c406fa5cab9bd0a175da02c652072f814c3d06205a2fd6d92bc152599a6aabb";
const ROOT: &str = "src/contracts/protocol/libraries";

fn run(command: &mut Command) -> String {
    let result = command.output().expect("start command");
    assert!(result.status.success(), "{command:?}: {}", String::from_utf8_lossy(&result.stderr));
    String::from_utf8(result.stdout).unwrap()
}
fn hash(path: &Path) -> String {
    run(Command::new("shasum").args(["-a", "256"]).arg(path))
}
fn fetch(output: &Path, url: &str, path: &str, provenance: &mut String) {
    let target = output.join(path);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    run(Command::new("curl")
        .args(["--fail", "--silent", "--show-error", "--location", url, "--output"])
        .arg(&target));
    provenance.push_str(&format!("{url}\n{}", hash(&target)));
    fs::write(output.join("provenance.txt"), &*provenance).unwrap();
}
fn item(source: &str, prefix: &str) -> String {
    assert_eq!(source.matches(prefix).count(), 1, "ambiguous item {prefix}");
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
    panic!("unterminated item");
}
fn replace(source: String, from: &str, to: &str, count: usize, provenance: &mut String) -> String {
    assert_eq!(source.matches(from).count(), count, "substitution count: {from}");
    provenance.push_str(&format!("substitute {count}x {from:?} -> {to:?}\n"));
    source.replace(from, to)
}
fn read(output: &Path, path: &str) -> String {
    fs::read_to_string(output.join(path)).unwrap()
}
fn aave_harness(output: &Path, old: bool, provenance: &mut String) {
    let (dir, contract) = if old { ("half-up", "HalfUpOracle") } else { ("current", "AaveOracle") };
    let base = format!("sources/{dir}/{ROOT}");
    let reserve_source = read(output, &format!("{base}/logic/ReserveLogic.sol"));
    let token_source = read(output, &format!("sources/{dir}/src/contracts/protocol/tokenization/AToken.sol"));
    let normalized = item(&reserve_source, "function getNormalizedIncome(");
    let mut getter = item(&token_source, "function balanceOf(");
    getter = replace(getter, "override(IncentivizedERC20, IERC20)", "", 1, provenance);
    getter = replace(getter, "super.balanceOf(user)", "boundShares", 1, provenance);
    getter = replace(
        getter,
        "POOL.getReserveNormalizedIncome(_underlyingAsset)",
        "getNormalizedIncome(boundReserve)",
        1,
        provenance,
    );
    let (extra_imports, extra_using, extra_functions) = if old {
        (String::new(), String::new(), String::new())
    } else {
        let update = item(&reserve_source, "function _updateIndexes(");
        (
            format!("import {{TokenMath}} from './{base}/helpers/TokenMath.sol';\nimport {{SafeCast}} from './sources/oz/contracts/utils/math/SafeCast.sol';"),
            "using TokenMath for uint256; using SafeCast for uint256;".into(),
            format!(
                r#"{update}
    // Construct the liquidity cache from controlled packed storage; all debt
    // fields are zero. This calls _updateIndexes, not the outer updateState.
    function updatedIndex() external returns (uint256) {{
        DataTypes.ReserveCache memory cache;
        cache.currLiquidityIndex = boundReserve.liquidityIndex;
        cache.nextLiquidityIndex = boundReserve.liquidityIndex;
        cache.currLiquidityRate = boundReserve.currentLiquidityRate;
        cache.reserveLastUpdateTimestamp = boundReserve.lastUpdateTimestamp;
        _updateIndexes(boundReserve, cache);
        require(cache.nextLiquidityIndex == boundReserve.liquidityIndex);
        return boundReserve.liquidityIndex;
    }}
    function mulFloor(uint256 a, uint256 b) external pure returns (uint256) {{ return WadRayMath.rayMulFloor(a, b); }}
    function mulCeil(uint256 a, uint256 b) external pure returns (uint256) {{ return WadRayMath.rayMulCeil(a, b); }}
"#
            ),
        )
    };
    let source = format!(
        r#"// SPDX-License-Identifier: BUSL-1.1 AND MIT
// Extracted original Aave getter/index function bodies; see provenance.txt.
pragma solidity 0.8.27;
import {{WadRayMath}} from './{base}/math/WadRayMath.sol';
import {{MathUtils}} from './{base}/math/MathUtils.sol';
import {{DataTypes}} from './{base}/types/DataTypes.sol';
{extra_imports}
contract {contract} {{
    using WadRayMath for uint256;
{extra_using}
    DataTypes.ReserveData internal boundReserve;
    uint256 internal boundShares;
    {normalized}
    {getter}
{extra_functions}
    function normalizedIncome() public view returns (uint256) {{ return getNormalizedIncome(boundReserve); }}
    function mulHalf(uint256 a, uint256 b) external pure returns (uint256) {{ return WadRayMath.rayMul(a, b); }}
    function divHalf(uint256 a, uint256 b) external pure returns (uint256) {{ return WadRayMath.rayDiv(a, b); }}
    function linear(uint256 rate, uint40 last) external view returns (uint256) {{ return MathUtils.calculateLinearInterest(rate, last); }}
}}
"#
    );
    fs::write(output.join(format!("{contract}.sol")), source).unwrap();
}
fn static_harness(output: &Path, provenance: &mut String) {
    let source = read(output, "sources/static/src/StaticATokenLM.sol");
    let mut functions = String::new();
    for name in [
        "rate",
        "convertToShares",
        "convertToAssets",
        "previewDeposit",
        "previewMint",
        "previewWithdraw",
        "previewRedeem",
        "maxRedeem",
        "maxWithdraw",
        "_convertToShares",
        "_convertToAssets",
    ] {
        let mut function = item(&source, &format!("function {name}("));
        if name == "rate" {
            function = replace(
                function,
                "POOL.getReserveNormalizedIncome(_aTokenUnderlying)",
                "getNormalizedIncome(boundReserve)",
                1,
                provenance,
            );
        }
        if name == "maxRedeem" {
            function = replace(function, "POOL.getReserveData(cachedATokenUnderlying)", "boundReserve", 1, provenance);
            function = replace(
                function,
                "ReserveConfiguration.getActive(reserveData.configuration)",
                "boundActive",
                1,
                provenance,
            );
            function = replace(
                function,
                "ReserveConfiguration.getPaused(reserveData.configuration)",
                "boundPaused",
                1,
                provenance,
            );
            function = replace(
                function,
                "IERC20(cachedATokenUnderlying).balanceOf(reserveData.aTokenAddress)",
                "boundLiquidity",
                1,
                provenance,
            );
            function = replace(function, "balanceOf[owner]", "boundShares", 1, provenance);
        }
        functions.push_str(&function);
        functions.push('\n');
    }
    let source = format!(
        r#"// SPDX-License-Identifier: MIT
// Original StaticATokenLM conversion/preview/max function bodies with explicit
// controlled external/storage reads. Aave rate composition is independently pinned.
pragma solidity 0.8.27;
import {{AaveOracle}} from './AaveOracle.sol';
import {{DataTypes}} from './sources/current/{ROOT}/types/DataTypes.sol';
import {{RayMathExplicitRounding, Rounding}} from './sources/static/src/RayMathExplicitRounding.sol';
contract StaticOracle is AaveOracle {{
    using RayMathExplicitRounding for uint256;
    address constant _aTokenUnderlying = address(0);
    uint256 internal boundLiquidity;
    bool internal boundActive;
    bool internal boundPaused;
    {functions}
    function mulDown(uint256 a, uint256 b) external pure returns (uint256) {{ return RayMathExplicitRounding.rayMulRoundDown(a, b); }}
    function mulUp(uint256 a, uint256 b) external pure returns (uint256) {{ return RayMathExplicitRounding.rayMulRoundUp(a, b); }}
    function divDown(uint256 a, uint256 b) external pure returns (uint256) {{ return RayMathExplicitRounding.rayDivRoundDown(a, b); }}
    function divUp(uint256 a, uint256 b) external pure returns (uint256) {{ return RayMathExplicitRounding.rayDivRoundUp(a, b); }}
}}
"#
    );
    fs::write(output.join("StaticOracle.sol"), source).unwrap();
}
fn main() {
    let args: Vec<_> = env::args().collect();
    assert_eq!(args.len(), 3, "usage: build-aave-static-oracles OFFICIAL_SOLC_0_8_27 FRESH_OUTPUT_DIRECTORY");
    let output = Path::new(&args[2]);
    assert!(!output.exists(), "preserve attempts; output must be fresh");
    fs::create_dir_all(output).unwrap();
    let compiler = fs::canonicalize(&args[1]).unwrap();
    let digest = hash(&compiler);
    assert_eq!(digest.split_whitespace().next().unwrap(), SOLC_SHA);
    let version = run(Command::new(&compiler).arg("--version"));
    assert!(version.contains("0.8.27+commit.40a35a09"));
    fs::write(output.join("build_aave_static_oracles.rs"), include_str!("build_aave_static_oracles.rs")).unwrap();
    let mut provenance = format!("Aave current {CURRENT}\nAave v3.4 HalfUp {HALF_UP}\nStaticATokenLM {STATIC}\nSafeCast {OZ}\n{version}{digest}");
    provenance.push_str(&hash(&output.join("build_aave_static_oracles.rs")));
    let manifest_url = format!("https://raw.githubusercontent.com/ethereum/solc-bin/{SOLC_PIN}/macosx-amd64/list.json");
    fetch(output, &manifest_url, "solc-list.json", &mut provenance);
    let manifest = read(output, "solc-list.json");
    let release = "\"path\": \"solc-macosx-amd64-v0.8.27+commit.40a35a09\"";
    assert_eq!(manifest.matches(release).count(), 1);
    let entry = manifest[manifest.find(release).unwrap()..].split('}').next().unwrap();
    assert!(entry.contains(&format!("\"sha256\": \"0x{SOLC_SHA}\"")));
    assert!(entry.contains("\"longVersion\": \"0.8.27+commit.40a35a09\""));
    for (dir, pin) in [("current", CURRENT), ("half-up", HALF_UP)] {
        for file in ["math/WadRayMath.sol", "math/MathUtils.sol", "types/DataTypes.sol", "logic/ReserveLogic.sol"] {
            let path = format!("{ROOT}/{file}");
            fetch(
                output,
                &format!("https://raw.githubusercontent.com/aave-dao/aave-v3-origin/{pin}/{path}"),
                &format!("sources/{dir}/{path}"),
                &mut provenance,
            );
        }
        for path in [
            "src/contracts/protocol/tokenization/AToken.sol",
            "src/contracts/protocol/tokenization/base/IncentivizedERC20.sol",
            "LICENSE",
            "foundry.toml",
            "remappings.txt",
            ".gitmodules",
        ] {
            fetch(
                output,
                &format!("https://raw.githubusercontent.com/aave-dao/aave-v3-origin/{pin}/{path}"),
                &format!("sources/{dir}/{path}"),
                &mut provenance,
            );
        }
        let base = read(output, &format!("sources/{dir}/src/contracts/protocol/tokenization/base/IncentivizedERC20.sol"));
        assert!(base.contains("uint120 balance;"));
        assert!(item(&base, "function balanceOf(").contains("return _userState[account].balance;"));
    }
    // Preserve the actual import resolution, not merely a separately chosen
    // library with a similar interface.
    for (repo, pin, path, dependency_repo, dependency_pin, label) in [
        (
            "aave-dao/aave-v3-origin",
            CURRENT,
            "lib/solidity-utils",
            "aave-dao/solidity-utils",
            "21dafc37b032aafac64fefb1a54d8be2ce137429",
            "aave-solidity-utils",
        ),
        (
            "aave-dao/solidity-utils",
            "21dafc37b032aafac64fefb1a54d8be2ce137429",
            "lib/openzeppelin-contracts-upgradeable",
            "OpenZeppelin/openzeppelin-contracts-upgradeable",
            "fa525310e45f91eb20a6d3baa2644be8e0adba31",
            "utils-oz-upgradeable",
        ),
        (
            "OpenZeppelin/openzeppelin-contracts-upgradeable",
            "fa525310e45f91eb20a6d3baa2644be8e0adba31",
            "lib/openzeppelin-contracts",
            "OpenZeppelin/openzeppelin-contracts",
            OZ,
            "upgradeable-oz",
        ),
    ] {
        let modules = format!("dependency-proof/{label}.gitmodules");
        fetch(
            output,
            &format!("https://raw.githubusercontent.com/{repo}/{pin}/.gitmodules"),
            &modules,
            &mut provenance,
        );
        assert!(read(output, &modules).contains(&format!("https://github.com/{dependency_repo}")));
        let link = format!("dependency-proof/{label}.json");
        fetch(
            output,
            &format!("https://api.github.com/repos/{repo}/contents/{path}?ref={pin}"),
            &link,
            &mut provenance,
        );
        assert!(read(output, &link).contains(&format!("\"sha\": \"{dependency_pin}\"")));
    }
    let path = format!("{ROOT}/helpers/TokenMath.sol");
    fetch(
        output,
        &format!("https://raw.githubusercontent.com/aave-dao/aave-v3-origin/{CURRENT}/{path}"),
        &format!("sources/current/{path}"),
        &mut provenance,
    );
    for path in ["contracts/utils/math/SafeCast.sol", "LICENSE"] {
        fetch(
            output,
            &format!("https://raw.githubusercontent.com/OpenZeppelin/openzeppelin-contracts/{OZ}/{path}"),
            &format!("sources/oz/{path}"),
            &mut provenance,
        );
    }
    for path in [
        "src/StaticATokenLM.sol",
        "src/RayMathExplicitRounding.sol",
        "src/ERC20.sol",
        "LICENSE",
        "foundry.toml",
    ] {
        fetch(
            output,
            &format!("https://raw.githubusercontent.com/bgd-labs/static-a-token-v3/{STATIC}/{path}"),
            &format!("sources/static/{path}"),
            &mut provenance,
        );
    }
    assert!(read(output, "sources/static/src/ERC20.sol").contains("mapping(address => uint256) public balanceOf;"));
    provenance.push_str("Whole libraries/types are unmodified. Harness imports point to their original relative trees. Public wrappers and controlled state declarations are new.\nAave _updateIndexes is complete, but debt cache inputs are zero; no updateState/treasury/debt-path claim.\nStatic configuration booleans/liquidity/holder are supplied facts; no external calls or actual mapping/config decoding.\nOracle uses official 0.8.27 with explicit Paris target; current Aave foundry configuration specifies Shanghai. No deployed compiler/runtime claim.\n");
    aave_harness(output, false, &mut provenance);
    aave_harness(output, true, &mut provenance);
    static_harness(output, &mut provenance);
    let arguments = [
        "--bin-runtime",
        "--hashes",
        "--storage-layout",
        "--optimize",
        "--optimize-runs",
        "200",
        "--evm-version",
        "paris",
        "--metadata-hash",
        "none",
        "-o",
        "build",
        "AaveOracle.sol",
        "HalfUpOracle.sol",
        "StaticOracle.sol",
    ];
    provenance.push_str(&format!("compiler arguments: {}\n", arguments.join(" ")));
    for file in ["AaveOracle.sol", "HalfUpOracle.sol", "StaticOracle.sol"] {
        provenance.push_str(&hash(&output.join(file)));
    }
    fs::write(output.join("provenance.txt"), &provenance).unwrap();
    let result = Command::new(&compiler).current_dir(output).args(arguments).output().unwrap();
    fs::write(output.join("compiler.stdout"), &result.stdout).unwrap();
    fs::write(output.join("compiler.stderr"), &result.stderr).unwrap();
    assert!(result.status.success(), "compiler failed; retained stdout/stderr in {}", output.display());
    for contract in ["AaveOracle", "HalfUpOracle", "StaticOracle"] {
        for suffix in [".bin-runtime", ".signatures", "_storage.json"] {
            provenance.push_str(&hash(&output.join(format!("build/{contract}{suffix}"))));
        }
    }
    fs::write(output.join("provenance.txt"), provenance).unwrap();
    println!("Saved pinned source oracles in {}", output.display());
}
