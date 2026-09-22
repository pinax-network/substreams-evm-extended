# Storage-slot provenance and the compiler verification plan

Every balance-state package takes its storage slots as caller-qualified
parameters. This page records, per bound contract, where each committed slot
came from and how strongly it is verified, and gives the plan for turning
"inferred from declaration order" into "compiler-verified" without any RPC.

Verification levels used below:

| Level | Meaning |
| --- | --- |
| **observed** | the slot was seen written in saved Extended blocks with a Keccak preimage or a decode that matched an independent oracle |
| **hashed** | the slot is `keccak256(name)` and a Rust test asserts the hex against the pinned name |
| **standard** | a published standard constant (EIP-1967, ERC-7201) recomputed or read from the pinned source |
| **compiler-verified** | `solc --storage-layout` of the pinned source with the exact compiler; the raw layout is committed under [`evidence/storage-layouts/`](evidence/storage-layouts/README.md) and a Rust test pins the fixture to it, including a completeness check that every compiled slot is decoded or reviewed |
| **ast-derived** | solc 0.4.24 has no `--storage-layout`; the layout is derived from the compact AST over the linearized inheritance chain and the `bytes32` constants are read from the AST (Lido) |
| **inferred** | derived by reading the pinned source's state-variable declaration order and applying Solidity packing rules by hand; not yet compiled |

## Per contract

### Aave V3 BNB Pool and aTokens (`aave/balance-state`, `erc4626` aave model)

Compiled on 2026-09-22 from aave-v3-origin `8305565a` with its pinned
submodules and solc 0.8.27 ([layout](evidence/storage-layouts/aave-v3-origin@8305565a.json));
test `aave/balance-state/tests/storage_layout.rs`. The deployed BSC
implementations (`0x5e2B…3B6d`, `0x7e19…4134`) were also checked live with
same-block getters (`aave/balance-state/docs/evidence/live-parity-bsc-2026-09-22-rev2.json`);
bytecode equality with a build of the pinned source is not established.

| Slot | Variable | Level | Source |
| --- | --- | --- | --- |
| 52 (`0x34`) on Pool | `_reserves` mapping base | compiler-verified | `PoolInstance`; also observed from Keccak preimages ([scan](evidence/scans/aave-scan.json)) |
| `keccak(asset, 0x34) + 1` | `liquidityIndex` bits 0..128, `currentLiquidityRate` bits 128..256 | compiler-verified | `ReserveData` word 1; index updates matched the conformance oracle 6/6 in the replay and `getReserveData` live |
| `keccak(asset, 0x34) + 3` | `lastUpdateTimestamp` (uint40) bits 128..168 | compiler-verified | `ReserveData` word 3, offset 16 |
| `0x34`, `0x35`, `0x36`, `0x3a` on aTokens | `_userState` (`balance` uint120 at bits 0..120), `_allowances`, `_totalSupply`, `_nonces` | compiler-verified | `ATokenInstance`; `_nonces` is written by `permit` and was found unreviewed by the live run (block 123,068,971) |
| `0x3608…2bbc` | EIP-1967 implementation | standard | `keccak256("eip1967.proxy.implementation") - 1` |

### Comet cUSDCv3 (`compound-v3/balance-state`)

| Slot | Variable | Level | Source |
| --- | --- | --- | --- |
| 0 | `baseSupplyIndex` 0..64, `baseBorrowIndex` 64..128, tracking indices 128..256 | compiler-verified | `solc 0.8.15` ([layout](evidence/storage-layouts/comet@f766f515.json)); test `compound-v3/balance-state/tests/storage_layout.rs` |
| 1 | `totalSupplyBase` 0..104, `totalBorrowBase` 104..208, `lastAccrualTime` 208..248, `pauseFlags` 248..256 | compiler-verified | byte offsets 0, 13, 26, 31 in the compiled layout |
| 2, 3, 4, 6, 7 | `totalsCollateral`, `isAllowed`, `userNonce`, `userCollateral`, `liquidatorPoints` mappings | compiler-verified | complete: no other regular slot exists |
| 5 | `userBasic` mapping (`principal` int104 at 0..104) | compiler-verified | `UserBasic` member `principal` slot 0 offset 0 `t_int104`, struct size 32 |
| `0xc98c7730ba19013824f711a9ab74801459b27e6ff7685cb924587c89aeda53ac` | `REENTRANCY_GUARD_FLAG_SLOT` = `keccak256("comet.reentrancy.guard")` | hashed | `CometCore.sol:60`; reviewed by name in the fixture and asserted in a test (was missing before the [review](review-findings-2026-09-21.md)) |

### Compound v2 cTokens and rate models (`compound-v2/balance-state`)

The deployed cUSDC and cETH run the **2019** compound-protocol source, not the
pinned `^0.8.10` tree: their ABI in the pinned `networks/mainnet-abi.json`
(`decimals` as `uint256`, a public `initialExchangeRateMantissa`, a
three-field `AccrueInterest`, no `admin_` constructor argument) matches the
initial public tree `f385d719`. That was found by an independent review on
2026-09-22; the fixture previously used the 0.8.10 slots below, one slot too
low from `admin` on.

| Slot (2019, cUSDC / cETH) | Variable | Level | Source |
| --- | --- | --- | --- |
| 0 | `_guardCounter` (`ReentrancyGuard`, incremented on every `nonReentrant` call) | compiler-verified | `solc 0.5.17` standard-JSON ([layout](evidence/storage-layouts/compound-protocol-2019@f385d719.json)); test `compound-v2/balance-state/tests/storage_layout.rs` |
| 1, 2 | `name`, `symbol` | compiler-verified | |
| 3 | `decimals` (uint256) | compiler-verified | |
| 4, 5, 6 | `admin`, `pendingAdmin`, `comptroller` | compiler-verified | |
| 7 | `interestRateModel` | compiler-verified | |
| 8 … 14 | `initialExchangeRateMantissa`, `reserveFactorMantissa`, `accrualBlockNumber`, `borrowIndex`, `totalBorrows`, `totalReserves`, `totalSupply` | compiler-verified | |
| 15, 16, 17 | `accountTokens`, `transferAllowances`, `accountBorrows` (two-word struct) | compiler-verified | |
| 18 | `underlying` (`CErc20` only) | compiler-verified | |
| cETH IRM 0, 1 | 2019 `WhitePaperInterestRateModel` `multiplier`, `baseRate` (per year, constructor-only) | compiler-verified | same layout file; carried as qualified constants |
| cUSDC IRM 0 … 4 | `LegacyJumpRateModelV2` `owner`, `multiplierPerBlock`, `baseRatePerBlock`, `jumpMultiplierPerBlock`, `kink` | compiler-verified | `solc 0.5.17` at `4caf72a1` ([layout](evidence/storage-layouts/compound-protocol-legacy-jump@4caf72a1.json)) |
| USDC 9 | FiatToken `balanceAndBlacklistStates` (balance in bits 0..255, blacklist flag in bit 255) | compiler-verified | `solc 0.6.12` on circlefin/stablecoin-evm v2.2.0 ([layout](evidence/storage-layouts/fiat-token@v2.2.0.json)); `FiatTokenV2_2._balanceOf` masks bit 255, so the cash row decodes `value_bits = 255`; the proxy is FiatTokenProxy (implementation slot `0x7050c9e0f4ca769c69bd3a8ef740bc37934f8e2c036e5a723fd8ee048ed3f8c3`); the mainnet implementation version is live-gated |

Current-tree delegators (cDAI and later markets) use the 0.8.10 layout of
`a3214f67` ([layout](evidence/storage-layouts/compound-v2@a3214f67.json)):
`_notEntered` 0, name 1, symbol 2, `decimals` (uint8) packed with `admin` in
3, pendingAdmin 4, comptroller 5, `interestRateModel` 6 … `totalSupply` 13,
`accountTokens` 14, allowances 15, borrow snapshots 16, `underlying` 17,
`implementation` 18; `JumpRateModelV2` has the same five words as the legacy
jump model. The layout test checks a delegator built from that file.

Whether the deployed bytecode was built from these trees is a runtime
question for live qualification (code-hash binding).

### Lido stETH (`lido/balance-state`)

| Slot | Variable | Level | Source |
| --- | --- | --- | --- |
| 0 | `shares` mapping | ast-derived | solc 0.4.24 AST over the 22-contract linearized chain ([layout](evidence/storage-layouts/lido-core@2da0f48f.json)): only `StETH` and `StETHPermit` declare regular state; test `lido/balance-state/tests/storage_layout.rs` |
| 1, 2 | `allowances`, `noncesByAddress` | ast-derived | same |
| `0x6038…59e6` | `lido.StETH.totalAndExternalShares` (total low 128, external high 128) | hashed | test asserts `keccak256(name)` |
| `0x81a1…0a5f` | `lido.Lido.bufferedEtherAndDepositedPostReport` | hashed | |
| `0x096e…8112` | `lido.Lido.clValidatorsBalanceAndClPendingBalance` | hashed | |
| `0x4dd0…64a6` | `lido.Versioned.contractVersion` | hashed | |
| named `other_slot_names` | locator/max ratio, deposited next report, seed deposits, stake limit, EL rewards, deposits reserve (+target), pausable flag, Aragon mutex/kernel/appId/initialization block, EIP-712 position | hashed + ast-derived | every `*_POSITION` constant of the compiled chain (16) is configured or reviewed (test) |

### ERC-4626 vaults (`erc4626/balance-state`)

| Slot | Variable | Level | Source |
| --- | --- | --- | --- |
| StaticATokenLM 0 | `Initializable` (`_initialized` uint8 + `_initializing` bool) | compiler-verified | `solc 0.8.20` ([layout](evidence/storage-layouts/static-a-token-v3@101f5d97.json)); test `erc4626/balance-state/tests/storage_layout.rs` |
| 1 … 7 | `name`, `symbol`, `decimals`, `totalSupply`, `balanceOf`, `allowance`, `nonces` | compiler-verified | complete with the rows below |
| 8 … 12 | `_aToken`, `_aTokenUnderlying`, `_rewardTokens`, `_startIndex`, `_userRewardsData` | compiler-verified | |
| SavingsDai 0 … 3 | `totalSupply`, `balanceOf`, `allowance`, `nonces` | compiler-verified | `solc 0.8.17` ([layout](evidence/storage-layouts/sdai@66587976.json)) |
| Pot 0 … 8 | `wards`, `pie`, `Pie`, `dsr`, `chi`, `vat`, `vow`, `rho`, `live` | compiler-verified | `solc 0.6.12` ([layout](evidence/storage-layouts/dss-pot@fa4f6630.json)); dss master at capture, mainnet MCD_POT bytecode not bound |
| `0x52c6…ce00` (+0, +1, +2) | ERC-7201 `openzeppelin.storage.ERC20`: `_balances`, `_allowances`, `_totalSupply` | standard + compiled constant | formula re-derived in the erc4626 tests and equal to `ERC20StorageLocation` in the pinned `ERC20Upgradeable.sol` ([evidence](evidence/storage-layouts/openzeppelin-upgradeable@v5.0.0.json)); the compiled harness has no regular storage. All five members are covered: `_name` (+3) and `_symbol` (+4) are reviewed, and the separate `openzeppelin.storage.ERC4626` word (`_asset`, `_underlyingDecimals`) is bound as an invalidating pointer |

## Compiler verification plan (offline, no RPC) — executed on 2026-09-21

All seven contracts below were compiled (or, for Lido, AST-derived) with the
exact pinned compilers; the layouts live under `evidence/storage-layouts/` and
each package has a `tests/storage_layout.rs` that fails when a fixture slot
diverges from the compiled layout or a compiled slot is neither decoded nor
reviewed. The USDC FiatToken family followed on the same day. The steps are
kept for re-runs at a new pin.

`solc` is installed on the original machine (`forge` is not). For each contract:

1. Clone the pinned repository into a scratch directory and check out the
   pinned commit; initialize submodules where the repo uses them
   (`static-a-token-v3`: `lib/aave-v3-core`, `lib/solidity-utils`).
2. Pick the compiler from the pragma / `foundry.toml` / hardhat config and, if
   the installed `solc` does not satisfy it, download the release binary from
   `https://binaries.soliditylang.org/` (record version and sha256). Layout
   rules are stable across 0.5.13+, but the pragma must be satisfied to compile.
3. Run `solc --storage-layout <file> <remappings> --base-path . --include-path
   lib --include-path node_modules` on the concrete contract
   (`CometWithExtendedAssetList`, `CErc20Delegator`/`CErc20Immutable`/`CEther`
   and `JumpRateModelV2`, `StaticATokenLM`, `SavingsDai`, `Pot`, and a
   one-line harness that inherits `ERC4626Upgradeable` because abstract
   contracts print no layout). Save the JSON under
   `docs/evidence/storage-layouts/<contract>@<commit>.json`.
4. Lido is `solc 0.4.24`, which has no `--storage-layout`. Use
   `--ast-compact-json` on `Lido.sol` with the `@aragon/os@4.4.0` and
   `openzeppelin-solidity@2.0.0` packages available, walk the linearized base
   contracts and count regular (non-constant) state variables to derive
   `shares` = 0; document that method next to the artifact.
5. Add a Rust test per package (host-only, under `tests/`) that loads the
   committed layout JSON and asserts every fixture slot and bit range against
   it, so a fixture edit that diverges from the compiler fails CI.
6. Update the "Level" column above and each package README from "inferred" to
   "compiler-verified", keeping the runtime code hash and activation block as
   live-gated items.

What compilation cannot settle: whether the deployed bytecode at the fixture
addresses was built from that source (code-hash binding) and the block at which
each epoch became active. Those remain for live resumption.
