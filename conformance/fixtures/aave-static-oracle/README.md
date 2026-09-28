# Aave and static-aToken source-execution oracles

These host-only tests execute compiled pinned Solidity to check the existing
Aave and StaticATokenLM arithmetic models. They establish source arithmetic
parity for explicit inputs, not deployed runtime equality, source reachability,
an actual Pool dependency, or a newly built package.

## Pinned sources and compiler

| Input | Immutable pin and retained source |
| --- | --- |
| Current Aave Floor getter, reserve projection/update, arithmetic | [aave-dao/aave-v3-origin@8305565ae342f1773c42cd2e4593f175fe5968a0](https://github.com/aave-dao/aave-v3-origin/tree/8305565ae342f1773c42cd2e4593f175fe5968a0); [sources/current](sources/current) |
| Existing v3.4.0 HalfUp getter and its own normalization/math | [aave-dao/aave-v3-origin@d7a64127dbecb944a73670fbe6fba136329d9e15](https://github.com/aave-dao/aave-v3-origin/tree/d7a64127dbecb944a73670fbe6fba136329d9e15); [sources/half-up](sources/half-up) |
| StaticATokenLM conversions, previews, limits, explicit rounding | [bgd-labs/static-a-token-v3@101f5d977889254ca2d2711b9582b45f832d10a0](https://github.com/bgd-labs/static-a-token-v3/tree/101f5d977889254ca2d2711b9582b45f832d10a0); [sources/static](sources/static) |
| Current Aave SafeCast dependency | [OpenZeppelin/openzeppelin-contracts@69c8def5f222ff96f2b5beff05dfba996368aa79](https://github.com/OpenZeppelin/openzeppelin-contracts/tree/69c8def5f222ff96f2b5beff05dfba996368aa79); [sources/oz](sources/oz) |

The generator preserves and compiles whole, unmodified `WadRayMath`,
`MathUtils`, `TokenMath`, `SafeCast`, `RayMathExplicitRounding` and both
`DataTypes` libraries. Original source files, SPDX notices and repository
licenses remain alongside the harnesses. Both pinned `IncentivizedERC20`
sources show a **uint120** holder basis; the static ERC20 mapping is uint256.

The actual SafeCast import resolution is preserved in
[dependency-proof](dependency-proof) and the current Aave remappings:
`aave-v3-origin` → `solidity-utils@21dafc37b032aafac64fefb1a54d8be2ce137429`
→ `openzeppelin-contracts-upgradeable@fa525310e45f91eb20a6d3baa2644be8e0adba31`
→ `openzeppelin-contracts@69c8def5f222ff96f2b5beff05dfba996368aa79`.
The generator checks each pinned Gitlink and repository URL.

The compiler is the official macOS amd64 **0.8.27+commit.40a35a09** release,
SHA-256 `8c406fa5cab9bd0a175da02c652072f814c3d06205a2fd6d92bc152599a6aabb`,
verified against the immutable
[solc-bin manifest](https://raw.githubusercontent.com/ethereum/solc-bin/16a99b8c26ed33a91796e209ff6797ee7baf2b0d/macosx-amd64/list.json).
Compiler arguments are recorded exactly:

```text
--bin-runtime --hashes --storage-layout --optimize --optimize-runs 200
--evm-version paris --metadata-hash none -o build
AaveOracle.sol HalfUpOracle.sol StaticOracle.sol
```

The Paris target is an explicit oracle override of current Aave's Shanghai
setting. It avoids PUSH0 in the bounded interpreter without changing the
selected arithmetic semantics. This is not a claim about any deployed
compiler or bytecode. Tests require no compiler or network access.

## Harness substitutions and limits

[AaveOracle.sol](AaveOracle.sol) contains unchanged `getNormalizedIncome`
and complete `_updateIndexes` function bodies. A new wrapper constructs a
liquidity cache from the controlled original `ReserveData` struct and returns
the resulting stored index, asserting cache/storage agreement. All debt cache
inputs are zero. The debt branch remains in source but is not qualified;
the outer `updateState` timestamp shortcut, treasury updates and reserve
transitions are not executed.

[HalfUpOracle.sol](HalfUpOracle.sol) uses the separately pinned v3.4
normalization and math. In each extracted `AToken.balanceOf`, only
`super.balanceOf(user)` and the external Pool normalized-income call become
the controlled holder word and the corresponding extracted reserve getter.
The inheritance-specific `override` annotation is removed. This tests that
specific v3.4 HalfUp getter, not every historical revision.

[StaticOracle.sol](StaticOracle.sol) extracts eleven complete functions:
`rate`, both conversions, four previews, `maxRedeem`, `maxWithdraw` and the
two internal conversion helpers. Only dependency reads change: the Pool
rate call uses the current Aave harness, Pool reserve data uses its bound
struct, configuration predicates use supplied active/paused booleans,
underlying liquidity uses a supplied word, and the holder mapping uses its
supplied word. The underlying address constant is zero because it no longer
selects an external dependency. Branches, helper calls and arithmetic remain
unchanged. Unused original locals/parameters intentionally remain and explain
the retained compiler warnings.

This composition is a controlled current-Aave dependency for the static
wrapper. It does not prove that any deployed static token calls that Pool
revision. No external call, mapping lookup, constructor, supply cap, reward,
deposit or withdrawal state transition is executed. Each VM invocation has
isolated storage; test writes never reach a chain or persist between calls.

## Compared domain and results

The tests count **5,103 EVM executions**: 2,234 Aave and 2,869 static-aToken.
Seven malformed packed/holder/liquidity controls fail before execution and
are excluded. Selector and compiler-layout checks also execute no bytecode.

| Coverage | Executions |
| --- | ---: |
| Aave stored-width counterexample, including wider normalized projection | 2 |
| Both pins' half-up helpers and current floor/ceil helpers, dust/ties/intermediate overflow | 636 |
| Both pins' linear interest, future clocks and checked product | 70 |
| 168 legal packed reserves: both getters/eras, zero/max120 holders, liquidity update | 1,512 |
| Getter half-ray ties and explicit v3.4 max120 holder boundary | 14 |
| Static pre-fix overflow/underflow order controls | 3 |
| Static rounding helpers, zero shortcuts and intermediate bounds | 424 |
| Six static getters across 75 reserve/clock combinations and five amounts | 2,250 |
| Static active/paused limits, zero holder/liquidity, normalization failures | 192 |

Reserve index/rate inputs fit uint128 and stored timestamps fit uint40.
Both aToken holder inputs fit uint120. Static holder/liquidity/amount and
pure-helper operands fit uint256; evaluation clocks are u64. These are
arithmetic domains, not claims that every tested state is reachable on-chain.
The arbitrary-width BigUint API outside these domains is not qualified.
Missing facts remain host `Unknown`; no EVM input is fabricated to simulate
absence.

With `N=2^128`, index `N-1`, rate `RAY` and one elapsed year, the normalized
income getter returns `2N-2`, while `_updateIndexes` reverts with
`SafeCastOverflowedUintDowncast(128, 2N-2)`. The host now applies the cast only
to the stored update. Immediately-below/at-width boundaries, zero rate and
same-second behavior are covered separately.

Static division evaluates the checked numerator first. `MAX / 0` in either
ray-division helper fails product overflow; `rayDivRoundUp(0,0)` fails checked
subtraction; `rayDivRoundDown(0,0)` and either helper at `(1,0)` reach division
by zero. Multiplication helpers retain their source's early zero return.
Inactive/paused `maxRedeem` returns zero before rate/liquidity evaluation,
but `maxWithdraw` still converts that zero; active known-zero holders or
liquidity still execute conversion before the minimum.

Successful ABI words are compared exactly. Failures require their source
payload: empty assembly REVERT for Aave ray guards, Panic(0x11) for checked
overflow/underflow, Panic(0x12) for division, or the full SafeCast custom error
including width/value. INVALID, unsupported opcodes and resource failures
are hard test failures and cannot satisfy an expected empty revert.

## Evidence and reproduction

[provenance.txt](provenance.txt) records every immutable source URL/hash,
Gitlink proof, license, exact compiler/version/arguments, as-run Rust generator
hash, each substitution and final harness/runtime/selector/layout hashes.
The final build is `out/aave-static-oracles-20260928/source-04`; earlier builds,
pre-fix controls and preliminary holder-width correction remain preserved in
that parent output directory. Original source is committed here so the
harness can be inspected without downloading it.

```sh
rustc --edition=2021 conformance/tools/build_aave_static_oracles.rs -o /tmp/build-aave-static-oracles
/tmp/build-aave-static-oracles /absolute/path/to/official-solc-0.8.27 out/aave-static-FRESH
cargo test --offline --locked -p conformance --lib source_oracle
```

The generator fetches only pinned public source/compiler metadata, refuses an
existing output directory and preserves compiler diagnostics. Historical BSC
live reports retain their original intervals/packages; these controls do not
extend them. Runtime/dependency bindings, checkpoints, new packaged output and
unobserved holders remain separate qualification gates under issue #16.
