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
| **inferred** | derived by reading the pinned source's state-variable declaration order and applying Solidity packing rules by hand; not yet compiled |

## Per contract

### Aave V3 BNB Pool and aTokens (`aave/balance-state`, `erc4626` aave model)

| Slot | Variable | Level | Source |
| --- | --- | --- | --- |
| 52 (`0x34`) on Pool | `_reserves` mapping base | observed | Keccak preimages `(underlying, 0x34)` in saved blocks ([scan](evidence/scans/aave-scan.json)) |
| `keccak(asset, 0x34) + 1` | `liquidityIndex` low 128, `currentLiquidityRate` high 128 | observed | index updates matched `rayMul(linearInterest(rate, ts, now), index)` 6/6 in the replay |
| `keccak(asset, 0x34) + 3` | `lastUpdateTimestamp` bits 128..168 | observed | same oracle |
| `0x34`, `0x35`, `0x36` on aTokens | `_userState`, allowances, `_totalSupply` | observed | writes and preimages in saved blocks; consistent with aave-v3-origin declaration order |
| `0x3608…2bbc` | EIP-1967 implementation | standard | `keccak256("eip1967.proxy.implementation") - 1` |

### Comet cUSDCv3 (`compound-v3/balance-state`)

| Slot | Variable | Level | Source |
| --- | --- | --- | --- |
| 0 | `baseSupplyIndex` 0..64, `baseBorrowIndex` 64..128, tracking indices 128..256 | inferred | `CometStorage.sol` declaration order; re-derived by four reviewers |
| 1 | `totalSupplyBase` 0..104, `totalBorrowBase` 104..208, `lastAccrualTime` 208..248, `pauseFlags` 248..256 | inferred | same |
| 2, 3, 4, 6, 7 | `totalsCollateral`, `isAllowed`, `userNonce`, `userCollateral`, `liquidatorPoints` mappings | inferred | same |
| 5 | `userBasic` mapping (`principal` int104 at 0..104) | inferred | same |
| `0xc98c7730ba19013824f711a9ab74801459b27e6ff7685cb924587c89aeda53ac` | `REENTRANCY_GUARD_FLAG_SLOT` = `keccak256("comet.reentrancy.guard")` | hashed | `CometCore.sol:60`; reviewed by name in the fixture and asserted in a test (was missing before the [review](review-findings-2026-09-21.md)) |

### Compound v2 cTokens and JumpRateModelV2 (`compound-v2/balance-state`)

| Slot | Variable | Level | Source |
| --- | --- | --- | --- |
| 0 | `_notEntered` | inferred | `CTokenInterfaces.sol` `CTokenStorage` |
| 1, 2 | `name`, `symbol` | inferred | |
| 3 | `decimals` (uint8) packed with `admin` | inferred | constants `borrowRateMaxMantissa`, `reserveFactorMaxMantissa` take no slot |
| 4, 5 | `pendingAdmin`, `comptroller` | inferred | |
| 6 | `interestRateModel` | inferred | |
| 7 … 13 | `initialExchangeRateMantissa`, `reserveFactorMantissa`, `accrualBlockNumber`, `borrowIndex`, `totalBorrows`, `totalReserves`, `totalSupply` | inferred | |
| 14, 15, 16 | `accountTokens`, `transferAllowances`, `accountBorrows` (two-word struct) | inferred | |
| 17 | `underlying` (`CErc20Storage`) | inferred | |
| 18 | `implementation` (`CDelegationStorage`, delegators only) | inferred | |
| IRM 0 … 4 | `owner`, `multiplierPerBlock`, `baseRatePerBlock`, `jumpMultiplierPerBlock`, `kink` | inferred | `BaseJumpRateModelV2.sol`; `blocksPerYear` is a constant |
| USDC 9 | FiatToken `balances` | inferred | `FiatTokenV1.sol` declaration order after Ownable/Pausable/Blacklistable; the proxy is FiatTokenProxy (implementation slot `0x7050c9e0f4ca769c69bd3a8ef740bc37934f8e2c036e5a723fd8ee048ed3f8c3`) |

The legacy cUSDC (`CErc20`, 2019) and cETH (`CEther`) are not delegators; whether
their deployed bytecode has exactly this layout is a runtime question for live
qualification.

### Lido stETH (`lido/balance-state`)

| Slot | Variable | Level | Source |
| --- | --- | --- | --- |
| 0 | `shares` mapping | inferred | `StETH.sol` declares `shares` then `allowances`; `Versioned`, `Pausable`, Aragon `AppStorage`/`Initializable`/`ReentrancyGuard`/`ACL` bases use unstructured storage only; `StETHPermit` adds `noncesByAddress` after |
| 1, 2 | `allowances`, `noncesByAddress` | inferred | same |
| `0x6038…59e6` | `lido.StETH.totalAndExternalShares` (total low 128, external high 128) | hashed | test asserts `keccak256(name)` |
| `0x81a1…0a5f` | `lido.Lido.bufferedEtherAndDepositedPostReport` | hashed | |
| `0x096e…8112` | `lido.Lido.clValidatorsBalanceAndClPendingBalance` | hashed | |
| `0x4dd0…64a6` | `lido.Versioned.contractVersion` | hashed | |
| named `other_slot_names` | locator/max ratio, deposited next report, seed deposits, stake limit, EL rewards, deposits reserve (+target), pausable flag, Aragon mutex/kernel/appId/initialization block, EIP-712 position | hashed | names from the pinned files |

### ERC-4626 vaults (`erc4626/balance-state`)

| Slot | Variable | Level | Source |
| --- | --- | --- | --- |
| StaticATokenLM 0 | `Initializable` (`_initialized` uint8 + `_initializing` bool) | inferred | solidity-utils `Initializable.sol` (assumed one slot) |
| 1 … 7 | `name`, `symbol`, `decimals`, `totalSupply`, `balanceOf`, `allowance`, `nonces` | inferred | `src/ERC20.sol` at the pin |
| 8 … 12 | `_aToken`, `_aTokenUnderlying`, `_rewardTokens`, `_startIndex`, `_userRewardsData` | inferred | `StaticATokenLM.sol` |
| SavingsDai 0 … 3 | `totalSupply`, `balanceOf`, `allowance`, `nonces` | inferred | `SavingsDai.sol`; constants and immutables take no slot |
| Pot 0 … 8 | `wards`, `pie`, `Pie`, `dsr`, `chi`, `vat`, `vow`, `rho`, `live` | inferred | `pot.sol` declaration order (fixture uses 3, 4, 7) |
| `0x52c6…ce00` (+0, +1, +2) | ERC-7201 `openzeppelin.storage.ERC20`: `_balances`, `_allowances`, `_totalSupply` | standard | formula `keccak256(abi.encode(uint256(keccak256(id)) - 1)) & ~0xff`, re-derived in `erc4626/balance-state` tests; also a literal constant in `ERC20Upgradeable.sol` v5.0.0 |

## Compiler verification plan (offline, no RPC)

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
