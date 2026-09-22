# erc4626/balance-state

A single RPC-free `map_events` reads Firehose Extended blocks and emits
**`evm.balance_state.v1.Events`** ([contract](../../docs/balance-state-contract.md))
for explicitly qualified ERC-4626 vault epochs. It does not emit
`evm.balances.v1`. Three metrics stay distinct:

| Metric | What it is | Rows |
| --- | --- | --- |
| vault `balanceOf` | the share count, the ERC-20 amount | `HolderBasis` SHARES |
| `convertToAssets` / `previewRedeem` | the implementation's conversion of shares to the asset, from bound dependency state and the block clock | `GlobalState` dependency rows |
| `maxWithdraw` / actual redemption | limits, pauses, liquidity and fees on top of the conversion | not emitted; some inputs are cross-contract state the consumer must bind separately |

EIP-4626 fixes interface semantics only: `convertTo*` round down and may be
inexact, `preview*` include fees, `max*` include limits. A standard interface
implies neither a storage layout nor a `totalAssets` implementation, so each
vault binds to one model and one implementation, and the consumer evaluates
with [`conformance::erc4626`](../../conformance/src/erc4626.rs). A historical
Deposit/Withdraw ratio is never a conversion.

## Models and their dependency rows

| `model` | Conversion (pinned) | Dependency rows carried |
| --- | --- | --- |
| `aave-static-atoken-lm` | `rayMulRoundDown(shares, POOL.getReserveNormalizedIncome(asset))` (bgd-labs/static-a-token-v3 `101f5d97…`); `previewMint` rounds up; `maxRedeem` is 0 while the reserve is inactive or paused | Aave Pool `ReserveData` words of the asset: `AAVE_LIQUIDITY_INDEX`, `AAVE_CURRENT_LIQUIDITY_RATE`, `AAVE_LAST_UPDATE_TIMESTAMP` (`key` = asset), one row per decoded field of every written word; dependencies POOL, WRAPPED_ASSET (aToken), UNDERLYING, Pool implementation pointer |
| `maker-savings-dai` | `shares × chi′ / RAY`, `chi′ = rpow(dsr, now − rho) × chi / RAY` when `now > rho` (sky-ecosystem/sdai `66587976…`, makerdao/dss `pot.sol`); `previewWithdraw` rounds up | Maker Pot `MAKER_POT_DSR`, `MAKER_POT_CHI`, `MAKER_POT_RHO`; dependency RATE_ACCUMULATOR (Pot), UNDERLYING |
| `oz-virtual-offset` | `shares × (totalAssets + 1) / (totalSupply + 10^offset)` floor (OpenZeppelin v5.0.0 `ERC4626.sol`); `totalAssets = asset.balanceOf(vault)` in the base | `ERC4626_TOTAL_ASSETS` from the asset's balances mapping entry of the vault (`key` = vault), `ERC4626_DECIMALS_OFFSET` as a qualified constant; dependency UNDERLYING |

Every model also carries the vault's `totalSupply` as `ERC4626_TOTAL_SUPPLY`.
Total-assets changes without share transfers (yield, loss, donations, Pot
drips, Aave index updates) reach the consumer through those rows; no holder
row is fabricated. A vault whose `totalAssets` depends on a strategy or an
oracle that is not one of these three models is unsupported until a model is
added; it is not approximated by a generic ratio.

## What else is emitted

| Table | Row |
| --- | --- |
| `ModelEpoch` INVALIDATED | vault implementation pointer write (`IMPLEMENTATION_POINTER_WRITE`), Pool implementation pointer write (`DEPENDENCY_POINTER_WRITE`), code change on the vault or its implementation (`CODE_CHANGE`), on the asset, Pool, Pool implementation, aToken or Pot (`DEPENDENCY_CODE_CHANGE`), each with evidence |
| `ModelEpoch` + `Dependency` | binding rows at the activation block and on the heartbeat; the Pool implementation is a depth-2 pointer under the Pool |
| `BlockClock` | exactly one per block |

## Parameters

The default manifest parameters bind no vault and emit only `BlockClock`.

- [`tests/fixtures/bsc-stata-usdt-epoch.json`](tests/fixtures/bsc-stata-usdt-epoch.json)
  binds the legacy Aave static aToken for BNB USDT `0x0471…3da6` to the BNB
  Pool `0x6807…e0cB` (`_reserves` slot 52, EIP-1967 implementation pointer) and
  aBnbUSDT. Vault slots follow the pinned declaration order
  (`Initializable` 0; `ERC20` name 1, symbol 2, decimals 3, totalSupply 4,
  balanceOf 5, allowance 6, nonces 7; `_aToken` 8, `_aTokenUnderlying` 9,
  `_rewardTokens` 10, `_startIndex` 11, `_userRewardsData` 12). The vault
  implementation address is a placeholder.
- [`tests/fixtures/mainnet-sdai-and-oz-epochs.json`](tests/fixtures/mainnet-sdai-and-oz-epochs.json)
  binds Savings DAI `0x83F2…BEeA` (`totalSupply` 0, `balanceOf` 1, `allowance`
  2, `nonces` 3; no proxy) to `MCD_POT` `0x197E…7cf7` (`dsr` 3, `chi` 4, `rho`
  7), and a placeholder OpenZeppelin vault using the ERC-7201 `ERC20Storage`
  namespace (`keccak256(abi.encode(uint256(keccak256("openzeppelin.storage.ERC20")) - 1)) & ~0xff`,
  re-derived in a test) with a 12-decimal offset.

All slots are **compiler-verified** against the pinned sources
(`StaticATokenLM` with `solc 0.8.20`, `SavingsDai` 0.8.17, `Pot` 0.6.12, and
the OpenZeppelin ERC-7201 constants of v5.0.0) under
[`docs/evidence/storage-layouts/`](../../docs/evidence/storage-layouts/README.md),
with [`tests/storage_layout.rs`](tests/storage_layout.rs) pinning every fixture
slot and checking completeness. Not verified: deployed runtime code hashes
and activation blocks (placeholders); the 6,093 locally cached BSC Extended
blocks contain no call, write, log or code change for any Aave BNB static
aToken, no Ethereum blocks are cached, and live Firehose and RPC use is paused.

## Fail-closed rules

| Condition | Result |
| --- | --- |
| Non-Extended block, `Block.ver` not listed (only 4 and 5 may be listed), incomplete transaction data | block fails |
| Persisted vault write that is not `totalSupply`, a `balances` entry, the pointer or a reviewed slot / mapping member | `unresolved storage … refusing incomplete balance state` |
| Two writes to one key with equal ordinals, or a write whose old value is not the previous new value | `ambiguous` / `discontinuous`, naming the contract, key and ordinals |
| Model and dependency block mismatch, more or fewer than one dependency block, pointer slot without address, overlapping slots, unknown fields | parameters rejected |

Dependency contracts' other storage (other Pool reserves, Pot `Pie`, other
asset holders) is that contract's own state and is not carried. Reverted
frames and failed transactions never contribute, per the shared
[`common/persist`](../../common/persist) rules.

## Validation

```sh
cargo test -p erc4626-balance-state
cargo clippy -p erc4626-balance-state --all-targets -- -D warnings
cargo check -p erc4626-balance-state --target wasm32-unknown-unknown
make -C erc4626/balance-state build
```

Tests are synthetic: shares and total supply for all three models, Aave
reserve words with 128/40-bit extraction and the other-reserve filter, Pot
drips and rate changes, donation-only total-assets changes, offset constant,
every invalidation, binding rows with depth, reverts, ties, discontinuities
and parameter refusals. `conformance::erc4626` tests cover floor/ceil
rounding, zero supply with and without offset, `rpow` half-up rounding and
overflow, paused-reserve `maxWithdraw`, and uint256 overflow. See issue
[#24](https://github.com/pinax-network/substreams-evm-extended/issues/24).
