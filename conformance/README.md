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
| `comet` | `mulFactor`, present and principal values (supply floors, borrow ceils), kinked supply and borrow rates from per-second immutables, stored-index utilization, `accruedInterestIndices`, `balanceOf` (positive principal only) and `borrowBalanceOf` (negative principal only) with int104/uint64/uint104 checks and the uint40 timestamp guard | compound-finance/comet `f766f51583c23acc33b2a7824654ef2029a96804` (`CometWithExtendedAssetList`, `CometCore`, `CometMath`, `CometStorage`) | constructor-scaling vectors; [compiled-source arithmetic oracle](fixtures/comet-oracle/README.md) with explicit inputs; no saved Ethereum blocks |
| `compound_v2` | `exchangeRateStored` (initial rate when supply is zero), `CTokenRevision::{Current, Legacy2019}`, jump and per-year `WhitePaper2019` rate models, block-based `accrueInterest` with checked uint256 operations, truncating `Exp` arithmetic and revision-specific rate guards, stored versus projected underlying balance | compound-finance/compound-protocol: selected cUSDC/cETH `Legacy2019` at `f385d71983ae5c5799faae9b2dfea43e5cf75262`; cUSDC `LegacyJumpRateModelV2` at `4caf72a1f88335adc9cc06acf6f372241369ed01`; separate `Current` model at `a3214f67b73310d547e00fc578e8355911c9d376` | 2,676 bounded calls against [compiled pinned source harnesses](fixtures/compound-v2-oracle/README.md), including legacy errors and modern reverts; compiler-pinned layouts and synthetic vectors; no captured Ethereum getter or deployed runtime/package qualification |
| `lido` | stETH contract version 4: `internalEther / internalShares` share rate with external shares, `getPooledEthByShares` / `getSharesByPooledEth` with the `2^128` argument guards, `getTotalPooledEther`, `TokenRebased` post-report ratio kept distinct from the getter | lidofinance/core `2da0f48f1a2a103a394dcf8760810fe9165697fb` (v4.0.1; `StETH`, `Lido`, `UnstructuredStorageExt`) | synthetic state only |
| `erc4626` | per-implementation conversions: Aave static aToken `rayMulRoundDown/Up` on the reserve normalized income with paused-reserve `maxWithdraw`; Savings DAI `rpow` chi projection with `_divup` and uint256 `rho`; OpenZeppelin v5 virtual-offset floor/ceil using full-precision `mulDiv` | bgd-labs/static-a-token-v3 `101f5d977889254ca2d2711b9582b45f832d10a0`; sky-ecosystem/sdai `665879762f8b5df5d234463f45d1d6a49bd4fbeb` + makerdao/dss `pot.sol`; OpenZeppelin `932fddf69a699a9a80fd2396fd1a2ab91cdda123` | OZ: 1,224 conversion/preview calls against [compiled pinned Solidity](fixtures/oz-v5-oracle.md), including full-precision intermediates and overflow reverts; static aToken: packaged BSC shares, `convertToAssets` and `rate()` controls ([live report](../erc4626/balance-state/docs/evidence/live-parity-bsc-stata-2026-09-23.json)); sDAI: 4,155 comparisons against [compiled pinned source](fixtures/sdai-oracle/README.md) with explicit Pot/holder inputs; no captured Ethereum getter |

Metric names are kept distinct: an ERC-20 `balanceOf` (Aave: observable
aToken amount; Compound v2: share count; Comet: supplied base balance), a
stored conversion (`exchangeRateStored`, stored indices), a projected
conversion (accrual to the evaluation block or timestamp) and debt
(`borrowBalanceOf`, Compound v2 borrow snapshots) are different results, and
none replaces `evm.balances.v1.Balance.amount`.

Explicit numeric zero inputs follow their pinned source branches. A known-zero
Aave index or SavingsDai `chi` is not a missing-state marker; normalization or
projection still runs when the source requires it. SavingsDai floor division
by zero returns an error, while its rounded-up helper returns zero for a zero
numerator after projection succeeds. The retained bridge continues to reject
absent facts, including for a known-zero holder. These
[source-branch regressions](../docs/research/11-Known_zero_reference_inputs.json)
use synthetic explicit values and one initialized observed holder per case
over blocks 10–11, with idle-clock and undo controls. They do not establish
on-chain reachability or new deployment qualification.

SavingsDai's `rho` retains its source uint256 domain in the pure model and
retained adapter. Its public Rust field is BigUint; callers should construct
it as an exact integer. Evaluation timestamps remain u64. A rho above the
clock selects stored chi, while rho above uint256 is refused and absent rho
remains unknown. The synthetic retained regression covers one initialized
holder over blocks 10–11 with provenance and undo. The
[source oracle](fixtures/sdai-oracle/README.md) separately compares legal
uint256 inputs and u64 clocks/exponents; it does not establish parity for
arbitrary-width BigUint inputs or qualify an actual Pot dependency.

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
  layouts, compiled-source arithmetic controls and synthetic vectors do not establish
  deployed runtime equality. Neither Compound v2 nor Comet has a captured
  Ethereum getter oracle or packaged qualification yet. Those controls need
  Ethereum Extended data under
  [#8](https://github.com/pinax-network/substreams-evm-extended/issues/8)
  and explicit live resumption.
- The [OZ oracle](fixtures/oz-v5-oracle.md) executes unmodified inherited
  conversions compiled from the pinned source in a bounded host-only EVM
  interpreter. It checks arithmetic against independent source execution;
  it does not qualify a deployed vault, captured getter or packaged stream.
- The [Comet oracle](fixtures/comet-oracle/README.md) makes 1,515 bounded
  model comparisons against official-solc-compiled pinned source, plus two
  invalid-width ABI controls. It preserves timestamp and both-index execution
  before principal-sign branches. Rate immutables and the holder lookup are
  explicit inputs; constructor, mapping and deployed getter qualification
  remain outside its scope.
- The [static-aToken live report](../erc4626/balance-state/docs/evidence/live-parity-bsc-stata-2026-09-23.json)
  records 162 matching comparisons each for shares, `convertToAssets` and
  `rate()` for 11 observed holders across 2,064 delivered BSC blocks: the activation
  block, selected historical blocks and [114858030, 114860030). These are
  same-block controls for holders with rows, not global-holder coverage.
  `maxWithdraw`, `maxRedeem` and reward accounting were not checked. The
  `maxWithdraw` evaluator additionally requires reserve eligibility and
  underlying liquidity that the current adapter does not extract.
- stETH ([#23](https://github.com/pinax-network/substreams-evm-extended/issues/23))
  still has synthetic arithmetic tests. The sDAI compiled-source controls do
  not qualify deployed epochs; deployed sDAI and OZ epochs remain unqualified.
  The retained-input controls below add offline evaluation
  and undo coverage; remaining independent getters, deployed bindings and
  package qualification stay open under
  [#16](https://github.com/pinax-network/substreams-evm-extended/issues/16).

## Retained-input reference evaluation

[`retained`](src/retained.rs) connects the arithmetic to the host-only
[`ProtocolLedger`](../common/retention/README.md). It exposes distinct metrics
for holder basis, Aave `balanceOf`, static-aToken conversion/rate, sDAI and OZ
conversion, Lido `balanceOf`, the selected 2019 cUSDC/cETH conversions and
USDC Comet supplied-balance inputs. Compound adapters are deliberately limited
to the committed mainnet profiles and their pinned source/layout families;
their deployment/runtime placeholders do not become qualified by this API.
Withdrawal limits, debt evaluation and reward accounting are also outside
this bridge. Lido logs remain inspectable evidence, never a report-time
holder balance calculated from end-of-block shares.

Callers supply an explicit `QualifiedModel`: exact stream and epoch,
dependency set, holder mapping root, global locations, and separate external
runtime qualification. The bridge checks source/rounding/scale and provenance
before applying arithmetic. Runtime attestations bind the full origin header
and code hashes, including when the stream's model row omits hashes. Their
truth remains the caller's independent qualification responsibility; this
library does not fetch code or validate a header against a network. Results
preserve the runtime attestation and consumed facts for review. An empty
holder/global/model binding remains `Unknown`, never inferred zero.

`Evaluation.value` is tagged `MetricValue::Unsigned` or
`MetricValue::SignedPrincipal`, with explicit `MetricUnits`. Raw shares,
scaled basis and signed principal have no inferred display decimals. In
particular, a cToken's `ModelEpoch.balance_decimals` describes its underlying,
not its shares. Asset conversions preserve underlying/native units and
decimals; exchange rates and indices expose their exact scale.

| Selected retained model | Metrics and required inputs |
| --- | --- |
| 2019 cUSDC, `compound-v2/ctoken-2019/exchange-rate-stored`, legacy jump IRM | Raw shares; `CompoundV2ExchangeRateStored` (1e18 mantissa); `CompoundV2StoredUnderlying`; `CompoundV2ProjectedUnderlying` at the evaluation **block number**. Cash is USDC slot-9 mapping state keyed by cUSDC, low 255 bits. The underlying, its depth-2 implementation and the slot-7 IRM pointer require independent binding. |
| 2019 cETH, same cToken model, WhitePaper2019 IRM | The same distinct metrics, with native cash at the cETH address and **no storage slot**. Per-year rate parameters and `blocksPerYear` are explicit runtime-bound constants; they are not interchangeable with jump-model per-block rates. |
| USDC Comet, `comet/base-supply-index` | `CometPrincipal` returns signed int104; `CometStoredSupplyIndex` and `CometProjectedSupplyIndex` expose 1e15 indices; `CometBalanceOf` returns supplied base units at the evaluation **timestamp**. Both supply and borrow indices execute before selecting the principal-sign branch. Negative principal is not an unsigned holder basis or a debt result. |

Exchange-rate/index metrics require no holder. Conversion/projection metrics
require the adapter's complete initialized market input set, including on
zero-holder and equal-clock cases; no absent input is filled with zero.
The cToken bridge uses the 2019 revision and selected IRM, not the modern
delegator/cDAI model. The Comet timestamp guard rejects `timestamp >= 2^40`
before equal-time or principal-sign branches, matching the pinned
[`getNowInternal`](https://github.com/compound-finance/comet/blob/f766f51583c23acc33b2a7824654ef2029a96804/contracts/CometWithExtendedAssetList.sol#L246).
The pure Comet API enforces this guard as well.
An explicitly initialized zero supply index remains a numeric zero; missing
state is detected by retained fact lookup, never by a zero-value sentinel.

Parameter changes create a different stream identity. The old ledger refuses
the next stream; a separately qualified, exactly identified checkpoint starts
a new ledger with explicitly supplied holder/global/model state. This is not
automatic carryover or a multi-epoch production parameter migration.

The WhitePaper2019 rate parameters are constructor-set **storage**, not
immutables encoded in runtime bytecode. Caller qualification must therefore
attest the selected constructor/storage values and parameters digest as well
as source and code hashes. Runtime-code equality by itself cannot prove those
constant values. The projector's declaration is not independent evidence.

The [retained-state tests](tests/retained.rs) exercise unchanged Aave holders
through idle clocks and global-only updates, checkpoints, epoch carryover,
suspension gaps, partial-block refusals, explicit constants, OZ donations,
static-aToken/sDAI idle conversion, exact undo/replacement and bounded history.
They pass actual Lido projector output through retention, including its
`CHANGE`-only report logs and block-local derived total. One test uses the
unmodified captured Aave transaction fixture at BSC block **122288734** to
reproduce the independently recorded Pool index from retained pre-write
inputs and that block's timestamp. Its holder and checkpoint are synthetic;
it is not an independently verified parent snapshot or holder getter check.
All other lifecycle controls are synthetic. These tests establish no new
deployed runtime, SPKG, live reorg, sink or global-holder qualification.

The [Compound retained tests](tests/retained_compound.rs) pass the actual v2
and Comet projectors' output from synthetic Extended blocks through retention
and evaluation. They cover only synthetic blocks **10–13**, the explicitly
initialized cUSDC/cETH holder plus known-zero holder and Comet positive,
negative and zero principal holders. Controls cover donation-only/native cash,
idle clocks, globals-only updates, sign crossing, the zero-supply initial-rate
branch, the 2019 same-block rate cap, both-index failure, uint40 timestamp,
missing/malformed inputs, reverted writes, partial delivery, invalidation,
undo/replacement and separately attested checkpoint rebinding. Runtime hashes,
checkpoints and holder inputs in these tests are synthetic. They are not an
independent Solidity oracle, captured Ethereum replay or live getter/package
qualification. Compound v2 and Comet now have separately recorded bounded
compiled-source arithmetic oracles. Captured Ethereum getters and
deployment/package checks remain separate gates under #14, #15 and #16.

```sh
cargo test --offline --locked -p conformance --lib --tests
```
