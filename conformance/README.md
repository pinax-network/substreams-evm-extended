# conformance

Host-only exact integer reference models for balances whose observable value
depends on shared protocol state and a block clock
([#16](https://github.com/pinax-network/substreams-evm-extended/issues/16)).
Each model is a pure function of the inputs the balance-state packages
extract ([contract](../docs/balance-state-contract.md)); missing input is an
explicit `Unknown`, never zero, and every rounding step follows the pinned
source of the epoch. Nothing here enters the WASM ingestion path.

| Module | Metrics | Pinned source | Oracles |
| --- | --- | --- | --- |
| `aave` | `rayMul` (half-up), `rayMulFloor`, `rayMulCeil`, `rayDiv`, `calculateLinearInterest`, `getNormalizedIncome`, `balanceOf` per rounding era (aToken revision ≤3 half-up, ≥4 floor), next liquidity index | aave-dao/aave-v3-origin `8305565ae342f1773c42cd2e4593f175fe5968a0` (`WadRayMath`, `MathUtils`, `ReserveLogic`, `TokenMath`) | captured BNB Pool reserve-index updates ([saved replay](../aave/balance-state/docs/evidence/replay-bsc-v5.json)); revision-5 packaged output and same-block-hash `balanceOf` / `scaledBalanceOf` controls ([live report](../aave/balance-state/docs/evidence/live-parity-bsc-2026-09-22-rev3.json)) |
| `comet` | `mulFactor`, present and principal values (supply floors, borrow ceils), kinked supply and borrow rates from per-second immutables, stored-index utilization, `accruedInterestIndices`, `balanceOf` (positive principal only) and `borrowBalanceOf` (negative principal only) with int104/uint64/uint104 checks | compound-finance/comet `f766f51583c23acc33b2a7824654ef2029a96804` (`CometWithExtendedAssetList`, `CometCore`, `CometMath`, `CometStorage`) | constructor scaling of the mainnet cUSDCv3 genesis rates; synthetic state only, no saved Ethereum blocks |
| `compound_v2` | `exchangeRateStored` (initial rate when supply is zero), `CTokenRevision::{Current, Legacy2019}`, jump and per-year `WhitePaper2019` rate models, block-based `accrueInterest` with truncating `Exp` arithmetic and revision-specific rate guards, stored versus projected underlying balance | compound-finance/compound-protocol: selected cUSDC/cETH `Legacy2019` at `f385d71983ae5c5799faae9b2dfea43e5cf75262`; cUSDC `LegacyJumpRateModelV2` at `4caf72a1f88335adc9cc06acf6f372241369ed01`; separate `Current` model at `a3214f67b73310d547e00fc578e8355911c9d376` | compiler-pinned layouts and synthetic arithmetic vectors computed outside the crate ([scope](../compound-v2/balance-state/README.md)); no captured Ethereum getter or deployed runtime/package qualification |
| `lido` | stETH contract version 4: `internalEther / internalShares` share rate with external shares, `getPooledEthByShares` / `getSharesByPooledEth` with the `2^128` argument guards, `getTotalPooledEther`, `TokenRebased` post-report ratio kept distinct from the getter | lidofinance/core `2da0f48f1a2a103a394dcf8760810fe9165697fb` (v4.0.1; `StETH`, `Lido`, `UnstructuredStorageExt`) | synthetic state only |
| `erc4626` | per-implementation conversions: Aave static aToken `rayMulRoundDown/Up` on the reserve normalized income with paused-reserve `maxWithdraw`; Savings DAI `rpow` chi projection with `_divup`; OpenZeppelin v5 virtual-offset floor/ceil using full-precision `mulDiv` | bgd-labs/static-a-token-v3 `101f5d977889254ca2d2711b9582b45f832d10a0`; sky-ecosystem/sdai `665879762f8b5df5d234463f45d1d6a49bd4fbeb` + makerdao/dss `pot.sol`; OpenZeppelin `932fddf69a699a9a80fd2396fd1a2ab91cdda123` | OZ: 1,224 conversion/preview calls against [compiled pinned Solidity](fixtures/oz-v5-oracle.md), including full-precision intermediates and overflow reverts; static aToken: packaged BSC shares, `convertToAssets` and `rate()` controls ([live report](../erc4626/balance-state/docs/evidence/live-parity-bsc-stata-2026-09-23.json)); sDAI: synthetic state only |

Metric names are kept distinct: an ERC-20 `balanceOf` (Aave: observable
aToken amount; Compound v2: share count; Comet: supplied base balance), a
stored conversion (`exchangeRateStored`, stored indices), a projected
conversion (accrual to the evaluation block or timestamp) and debt
(`borrowBalanceOf`, Compound v2 borrow snapshots) are different results, and
none replaces `evm.balances.v1.Balance.amount`.

## What is and is not established

- The Aave saved-block oracle checks reserve-index arithmetic. The later
  [revision-5 live report](../aave/balance-state/docs/evidence/live-parity-bsc-2026-09-22-rev3.json)
  records 103 matching `balanceOf` and 103 `scaledBalanceOf` comparisons for
  28 observed holders across 2,060 delivered BSC blocks: the activation
  block, selected historical blocks and the contiguous interval
  [123449757, 123451757). This confirms the selected floor-rounding epoch
  for those observations; it does not initialize holders without a row.
- Compound v2 selects the 2019 cUSDC/cETH rate cap of `5e14`, checked before
  the block delta, separately from the `Current` cap of `5e12`. cUSDC uses
  the legacy jump model; cETH uses `WhitePaper2019` per-year rates. Compiler
  layouts and independently computed synthetic vectors do not establish
  deployed runtime equality. Neither Compound v2 nor Comet has a captured
  Ethereum getter oracle or packaged qualification yet. Those controls need
  Ethereum Extended data under
  [#8](https://github.com/pinax-network/substreams-evm-extended/issues/8)
  and explicit live resumption.
- The [OZ oracle](fixtures/oz-v5-oracle.md) executes unmodified inherited
  conversions compiled from the pinned source in a bounded host-only EVM
  interpreter. It checks arithmetic against independent source execution;
  it does not qualify a deployed vault, captured getter or packaged stream.
- The [static-aToken live report](../erc4626/balance-state/docs/evidence/live-parity-bsc-stata-2026-09-23.json)
  records 162 matching comparisons each for shares, `convertToAssets` and
  `rate()` for 11 observed holders across 2,064 delivered BSC blocks: the activation
  block, selected historical blocks and [114858030, 114860030). These are
  same-block controls for holders with rows, not global-holder coverage.
  `maxWithdraw`, `maxRedeem` and reward accounting were not checked. The
  `maxWithdraw` evaluator additionally requires reserve eligibility and
  underlying liquidity that the current adapter does not extract.
- stETH ([#23](https://github.com/pinax-network/substreams-evm-extended/issues/23))
  and sDAI still have synthetic arithmetic tests. Deployed sDAI and OZ epochs
  remain unqualified. Initialized holder/global-state evaluation with
  canonical clocks and fork undo remains follow-up work under
  [#16](https://github.com/pinax-network/substreams-evm-extended/issues/16).

```sh
cargo test --locked -p conformance
```
