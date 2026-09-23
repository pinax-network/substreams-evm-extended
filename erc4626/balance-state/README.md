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
| `aave-static-atoken-lm` | `rayMulRoundDown(shares, POOL.getReserveNormalizedIncome(asset))` (bgd-labs/static-a-token-v3 `101f5d97…`); `previewMint` rounds up; `maxRedeem` is 0 while the reserve is inactive or paused | Aave Pool `ReserveData` words of the asset: `AAVE_LIQUIDITY_INDEX`, `AAVE_CURRENT_LIQUIDITY_RATE`, `AAVE_LAST_UPDATE_TIMESTAMP` (`key` = asset), one row per decoded field of every written word; dependencies POOL, WRAPPED_ASSET (aToken) and UNDERLYING as `STORAGE_POINTER` edges on the vault's `_aToken` (8) and `_aTokenUnderlying` (9), Pool implementation pointer |
| `maker-savings-dai` | `shares × chi′ / RAY`, `chi′ = rpow(dsr, now − rho) × chi / RAY` when `now > rho` (sky-ecosystem/sdai `66587976…`, makerdao/dss `pot.sol`); `previewWithdraw` rounds up | Maker Pot `MAKER_POT_DSR`, `MAKER_POT_CHI`, `MAKER_POT_RHO`; dependency RATE_ACCUMULATOR (Pot), UNDERLYING |
| `oz-virtual-offset` | `shares × (totalAssets + 1) / (totalSupply + 10^offset)` floor using fullprecision `Math.mulDiv` (openzeppelin-contracts-upgradeable v5.0.0 `625fb3c2…` `ERC4626Upgradeable.sol`); `totalAssets = asset.balanceOf(vault)` in the base | `ERC4626_TOTAL_ASSETS` from the source-bound asset balance decoder (`key` = vault), `ERC4626_DECIMALS_OFFSET` as a qualified constant; dependency UNDERLYING as a `STORAGE_POINTER` edge on the `ERC4626Storage` word (`_asset` in bits 0–160, `_underlyingDecimals` above) and the asset's bound implementation |

`ModelEpoch` states the evaluated balance, not the share token:
`balance_asset` and `balance_decimals` are the underlying asset and its
decimals, `basis_scale` is RAY for the static aToken and sDAI (`shares × rate /
RAY`) and `""` for the OpenZeppelin share ratio, and `implementation_revision`
is the declared revision (`STATIC__ATOKEN_LM_REVISION` = 2 for the static
aToken).

Every model also carries the vault's `totalSupply` as `ERC4626_TOTAL_SUPPLY`.
Total-assets changes without share transfers (yield, loss, donations, Pot
drips, Aave index updates) reach the consumer through those rows; no holder
row is fabricated. A vault whose `totalAssets` depends on a strategy or an
oracle that is not one of these three models is unsupported until a model is
added; it is not approximated by a generic ratio.

## What else is emitted

| Table | Row |
| --- | --- |
| `ModelEpoch` INVALIDATED (ERC-4626 asset rebinding) | a persisted write to the vault's ERC-7201 `openzeppelin.storage.ERC4626` word, which holds `_asset` and `_underlyingDecimals`: the model would convert into a different asset, so the epoch is invalidated with the raw words as evidence rather than silently re-bound |
| `ModelEpoch` INVALIDATED | vault implementation pointer write (`IMPLEMENTATION_POINTER_WRITE`), Pool or OZ asset implementation pointer write or a write to the static aToken's `_aToken` / `_aTokenUnderlying` (`DEPENDENCY_POINTER_WRITE`), code change on the vault or its implementation (`CODE_CHANGE`), on the asset, OZ asset implementation, Pool, Pool implementation, aToken or Pot (`DEPENDENCY_CODE_CHANGE`), each with evidence |
| `ModelEpoch` + `Dependency` | binding rows at the activation block and on the heartbeat; Pool and OZ asset implementations are depth-2 pointers under their respective dependencies |
| `BlockClock` | exactly one per block |

## Parameters

The default manifest parameters bind no vault and emit only `BlockClock`.

- [`tests/fixtures/bsc-stata-usdt-epoch.json`](tests/fixtures/bsc-stata-usdt-epoch.json)
  binds the legacy Aave static aToken for BNB USDT `0x0471…3da6` to the BNB
  Pool `0x6807…e0cB` (`_reserves` slot 52, EIP-1967 implementation pointer) and
  aBnbUSDT. Vault slots follow the pinned declaration order
  (`Initializable` 0; `ERC20` name 1, symbol 2, decimals 3, totalSupply 4,
  balanceOf 5, allowance 6, nonces 7; `_aToken` 8, `_aTokenUnderlying` 9,
  `_rewardTokens` 10, `_startIndex` 11, `_userRewardsData` 12). `_aToken` and
  `_aTokenUnderlying` are pointers (`aave.atoken_slot`, `aave.underlying_slot`)
  because they select the reserve `rate()` reads. `name`, `symbol` and
  `_rewardTokens` are `other_dynamic_slots`: their data at `keccak256(slot) + i`
  is reviewed, so the permissionless `refreshRewardTokens()` push is accepted.
  The Pool implementation pointer is required: an Aave V3 Pool is always a
  proxy. The vault implementation address is a placeholder.
- [`tests/fixtures/mainnet-sdai-and-oz-epochs.json`](tests/fixtures/mainnet-sdai-and-oz-epochs.json)
  binds Savings DAI `0x83F2…BEeA` (`totalSupply` 0, `balanceOf` 1, `allowance`
  2, `nonces` 3; no proxy) to `MCD_POT` `0x197E…7cf7` (`dsr` 3, `chi` 4, `rho`
  7), and a placeholder OpenZeppelin vault using the ERC-7201 `ERC20Storage`
  namespace (`keccak256(abi.encode(uint256(keccak256("openzeppelin.storage.ERC20")) - 1)) & ~0xff`,
  re-derived in a test) with a 12-decimal offset. All five `ERC20Storage`
  members are accounted for (`_balances`, `_allowances` and `_totalSupply`
  decoded; `_name` and `_symbol` reviewed, with their long-string data areas,
  because `__ERC20_init_unchained` writes them), the
  `openzeppelin.storage.Initializable` word (`0xf0c5…6a00`) that every
  `initializer` and `reinitializer` writes is reviewed, and the
  `openzeppelin.storage.ERC4626` word is bound so that
  `__ERC4626_init_unchained` invalidates instead of failing the block.

The OZ dependency block requires `asset_balance_model` and `asset_source_pin`.
Supported source-qualified decoders are `uint256` and
`fiat-token-v2_2-low255`. The latter masks USDC's blacklist bit at bit 255 and
preserves the full raw words as evidence. Its fixture pins Circle
`405efc10…`, slot 9, and the ZeppelinOS implementation slot
`keccak256("org.zeppelinos.proxy.implementation")`; the implementation address
`0x0000000000000000000000000000000000000022` is an **unqualified placeholder**.
`asset_implementation_slot` and `asset_implementation` must be supplied together
for a proxy. A direct asset omits both and states `asset_not_proxy: true`;
omitting the pointer without that statement is refused, and the FiatToken
model always needs its pointer. Every persisted write to this bound
pointer, a vault implementation pointer or an Aave Pool implementation pointer
invalidates, including same-value writes; change-and-restore retains both
intermediate transitions with their own provenance. Pointer writes undergo
the same ordinal and continuity validation as balance writes. Ordinary
balance noops remain suppressed. Shared Pool writes invalidate every bound
vault that depends on that Pool, while a direct vault pointer affects only
that vault.
Asset source pins are carried in both dependency rows. Source details and
limits are recorded in [the research note](../../docs/research/09-ERC4626-OZ-arithmetic-and-USDC-dependency.json).

All slots are **compiler-verified** against the pinned sources
(`StaticATokenLM` with `solc 0.8.20`, `SavingsDai` 0.8.17, `Pot` 0.6.12, and
the OpenZeppelin ERC-7201 constants of v5.0.0) under
[`docs/evidence/storage-layouts/`](../../docs/evidence/storage-layouts/README.md),
with [`tests/storage_layout.rs`](tests/storage_layout.rs) pinning every fixture
slot and checking completeness. Not verified: deployed runtime code hashes
and activation blocks (placeholders); the 6,093 locally cached BSC Extended
blocks contain no call, write, log or code change for any Aave BNB static
aToken, no Ethereum blocks are cached, and live Firehose and RPC use is paused.

`activation_ordinal` (optional, default `0`) is the first execution ordinal of
`activation_block` at which the epoch applies. Effects earlier in that block,
such as the upgrade write that installs this epoch's implementation, belong to
the previous epoch: they are neither decoded under this epoch nor treated as
invalidating it. The BOUND row carries the activation ordinal as its `ordinal`,
so a consumer applying rows in ordinal order sees the previous epoch's
invalidation before this epoch's binding.

Every persisted write to a storage-pointer slot invalidates the epoch with its
own evidence row, including a write back to the same value and each step of an
excursion that restores the pointer within the block, as the
`BINDING_KIND_STORAGE_POINTER` contract in `proto/v1/balance_state.proto`
requires. Reducing an excursion X→Z→X to its end points would otherwise hide a
temporary implementation that ran inside the block.

## Live qualification (BSC static aToken, 2026-09-23)

[`epochs/bsc-stata-usdt.json`](epochs/bsc-stata-usdt.json) binds the legacy
static aToken for BNB USDT `0x0471…3da6` to its real implementation
`0x1d69…c553` (`STATIC__ATOKEN_LM_REVISION` 2, constant since block
37,302,378 over 33 samples), and starts the epoch at the Pool implementation
install that `rate()` depends on, **block 101,087,794, ordinal 3347**
([binding](docs/evidence/epoch-binding-bsc-stata-2026-09-23.json)). The
packed map (`spkg` sha256 `cb9dc6b2…`, wasm `b7d1055e…`) was streamed from
`bsc.substreams.pinax.network` over 2,064 blocks (the activation block, the
2,000 contiguous blocks 114,858,030–114,860,029 and every block with a vault
log between 114,857,790 and 123,557,790; the vault had none in the last
400,000 blocks) and checked with `erc4626-balance-state-tools` at each
row's block hash ([report](docs/evidence/live-parity-bsc-stata-2026-09-23.json)):
162 share rows for 11 holders equal `balanceOf`, and the conversion evaluated
by `conformance::erc4626::StataTokenLm` from the same-block reserve words
equals the vault's own `convertToAssets` (162/162) and `rate()` (162/162);
297 reserve words equal `getReserveData`, 63 total supplies equal
`totalSupply`, 4,128 clock fields equal the headers; BOUND at ordinal 3347,
two heartbeats, no invalidation; vault and Pool pointers and the revision
equal at the first and last block. `maxWithdraw`/`maxRedeem` limits and
reward accounting were not checked. The sDAI and OpenZeppelin epochs remain
placeholder Ethereum fixtures (#8).

## Fail-closed rules

| Condition | Result |
| --- | --- |
| Non-Extended block, `Block.ver` not listed (only 4 and 5 may be listed), incomplete transaction data | block fails |
| Persisted vault write that is not `totalSupply`, a `balances` entry, a pointer, a reviewed slot / mapping member, or a reviewed dynamic area element proven by the verified preimage of `keccak256(slot)` (offset below 2³²) | `unresolved storage … refusing incomplete balance state` |
| Two writes to one key with equal ordinals, or a write whose old value is not the previous new value | `ambiguous` / `discontinuous`, naming the contract, key and ordinals |
| Model and dependency block mismatch, more or fewer than one dependency block, missing asset decoder/source pin, missing Pool pointer or static aToken asset pointers, an OZ asset without its pointer or `asset_not_proxy`, pointer slot without address, a dynamic area whose root is not reviewed, overlapping slots, unknown fields, OZ decimals inconsistent with its offset or virtual shares exceeding uint256 | parameters rejected |

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
and parameter refusals, plus equal-value/restored vault and Pool pointer writes,
shared-Pool attribution and deterministic evidence, reward-token array pushes,
two-level allowance keys, a reverted child frame and a complete OpenZeppelin
initializing block (Initializable word, long name, namespace words). `conformance::erc4626` tests cover floor/ceil
rounding, zero supply with and without offset, `rpow` half-up rounding and
overflow, paused-reserve `maxWithdraw`, and uint256 overflow. OZ regression
tests execute 1,224 conversion/preview calls against compiled pinned Solidity
using an offline Rust interpreter; see [oracle provenance and regeneration](../../conformance/fixtures/oz-v5-oracle.md).
This covers fullprecision results, checked additions/offsets, and ceil overflow.
It does not supply missing withdrawal-limit/liquidity inputs or qualify any
deployed runtime or newly built package. See issue
[#24](https://github.com/pinax-network/substreams-evm-extended/issues/24).
