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
| `aave` | `rayMul` (half-up), `rayMulFloor`, `rayMulCeil`, `rayDiv`, `calculateLinearInterest`, `getNormalizedIncome`, `balanceOf` per rounding era (aToken revision ≤3 half-up, ≥4 floor), next liquidity index | aave-dao/aave-v3-origin `8305565ae342f1773c42cd2e4593f175fe5968a0` (`WadRayMath`, `MathUtils`, `ReserveLogic`, `TokenMath`) | four reserve index updates captured from the Aave V3 BNB Pool are reproduced exactly from the previous index, rate and clock; the [replay tool](../aave/balance-state/tools) re-derives every stored index write in 1,433 saved blocks |
| `comet` | `mulFactor`, present and principal values (supply floors, borrow ceils), kinked supply and borrow rates from per-second immutables, stored-index utilization, `accruedInterestIndices`, `balanceOf` (positive principal only) and `borrowBalanceOf` (negative principal only) with int104/uint64/uint104 checks | compound-finance/comet `f766f51583c23acc33b2a7824654ef2029a96804` (`CometWithExtendedAssetList`, `CometCore`, `CometMath`, `CometStorage`) | constructor scaling of the mainnet cUSDCv3 genesis rates; synthetic state only, no saved Ethereum blocks |
| `compound_v2` | `exchangeRateStored` (initial rate when supply is zero), `JumpRateModelV2` scaling, utilization, borrow and supply rates, block-based `accrueInterest` with truncating `Exp` arithmetic and the absurd-rate guard, stored versus projected underlying balance | compound-finance/compound-protocol `a3214f67b73310d547e00fc578e8355911c9d376` (`CToken`, `CTokenInterfaces`, `ExponentialNoError`, `BaseJumpRateModelV2`) | IRM_USDC_Updateable deployment parameters; synthetic state only |
| `lido` | stETH contract version 4: `internalEther / internalShares` share rate with external shares, `getPooledEthByShares` / `getSharesByPooledEth` with the `2^128` argument guards, `getTotalPooledEther`, `TokenRebased` post-report ratio kept distinct from the getter | lidofinance/core `2da0f48f1a2a103a394dcf8760810fe9165697fb` (v4.0.1; `StETH`, `Lido`, `UnstructuredStorageExt`) | synthetic state only |
| `erc4626` | per-implementation conversions: Aave static aToken `rayMulRoundDown/Up` on the reserve normalized income with paused-reserve `maxWithdraw`; Savings DAI `rpow` chi projection with `_divup`; OpenZeppelin v5 virtual-offset floor/ceil using full-precision `mulDiv` | bgd-labs/static-a-token-v3 `101f5d977889254ca2d2711b9582b45f832d10a0`; sky-ecosystem/sdai `665879762f8b5df5d234463f45d1d6a49bd4fbeb` + makerdao/dss `pot.sol`; OpenZeppelin `932fddf69a699a9a80fd2396fd1a2ab91cdda123` | OZ: 1,224 conversion/preview calls against compiled pinned Solidity, including full-precision intermediates and overflow reverts; static aToken and sDAI: synthetic state only |

Metric names are kept distinct: an ERC-20 `balanceOf` (Aave: observable
aToken amount; Compound v2: share count; Comet: supplied base balance), a
stored conversion (`exchangeRateStored`, stored indices), a projected
conversion (accrual to the evaluation block or timestamp) and debt
(`borrowBalanceOf`, Compound v2 borrow snapshots) are different results, and
none replaces `evm.balances.v1.Balance.amount`.

## What is and is not established

- The Aave arithmetic is checked against values the contract itself computed
  in captured blocks, so the linear-interest and half-up rounding chain is
  independently confirmed for the saved BSC window.
- Comet and Compound v2 follow the pinned source line by line but have no
  captured Ethereum oracle yet; their tests are synthetic and repeat the
  implementation's own arithmetic. Captured getter fixtures and same-block
  controls wait for Ethereum Extended data under
  [#8](https://github.com/pinax-network/substreams-evm-extended/issues/8)
  and explicit live resumption.
- The [OZ oracle](fixtures/oz-v5-oracle.md) executes unmodified inherited
  conversions compiled from the pinned source in a bounded host-only EVM
  interpreter. It checks arithmetic against independent source execution;
  it does not qualify a deployed vault, captured getter or packaged stream.
- stETH ([#23](https://github.com/pinax-network/substreams-evm-extended/issues/23)),
  static aToken and sDAI models still have synthetic arithmetic tests. The
  static-aToken `maxWithdraw` evaluator additionally requires reserve
  eligibility and underlying liquidity that the current adapter does not
  extract; its presence here does not make that metric self-contained.

```sh
cargo test --locked -p conformance
```
