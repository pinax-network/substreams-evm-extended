# compound-v2/balance-state

A single RPC-free `map_events` reads Firehose Extended blocks and emits
**`evm.balance_state.v1.Events`** ([contract](../../docs/balance-state-contract.md))
for explicitly qualified Compound v2 cToken epochs. It does not emit
`evm.balances.v1`. Three metrics stay distinct and the map carries inputs for
all of them without computing any:

| Metric | What it is | Rows |
| --- | --- | --- |
| cToken `balanceOf` | the stored share count | `HolderBasis` SHARES |
| `exchangeRateStored` conversion | `(cash + totalBorrows - totalReserves) * 1e18 / totalSupply`, or `initialExchangeRateMantissa` when supply is zero | `GlobalState` market words + cash |
| `balanceOfUnderlying` | shares times the exchange rate **after** `accrueInterest`, which needs the rate model, the accrual block and the block clock | the above + rate-model rows |

The consumer evaluates with [`conformance::compound_v2`](../../conformance/src/compound_v2.rs)
at a canonical block clock: `accrue` reproduces the pinned block-delta,
borrow-rate, truncation and ordering, and `underlying_balance` the truncated
conversion. The map never replaces the ERC-20 share amount with an
underlying-equivalent value.

## What is emitted

| Table | Row | Source |
| --- | --- | --- |
| `HolderBasis` (`SHARES`) | `accountTokens[holder]` before the first and after the last write of the block | cToken storage, verified Keccak preimage `(holder, account_tokens)` |
| `GlobalState` `COMPOUND_V2_TOTAL_BORROWS` / `TOTAL_RESERVES` / `TOTAL_SUPPLY` / `BORROW_INDEX` / `ACCRUAL_BLOCK_NUMBER` / `RESERVE_FACTOR_MANTISSA` / `INITIAL_EXCHANGE_RATE_MANTISSA` | the cToken scalar words, scales `1` or `1e18` | cToken storage, configured slots |
| `GlobalState` `COMPOUND_V2_TOTAL_CASH` (`key` = cToken) | CErc20: the low `value_bits` of `underlying.balances[cToken]` from the qualified underlying's mapping (USDC: 255 bits, the blacklist flag lives in bit 255); CEther: persisted native balance changes of the cToken | underlying storage or cToken balance changes |
| `GlobalState` `COMPOUND_V2_IRM_*` | rate-model storage writes for configured slots (JumpRateModelV2 `updateJumpRateModel`), and qualified constants (`blocksPerYear`, WhitePaper immutables) at BOUND / REAFFIRMED | rate-model storage, parameters |
| `ModelEpoch` INVALIDATED | every persisted write, including equal-value and restored ones, to the rate-model pointer on the cToken (`RATE_MODEL_CHANGE`), the delegator implementation pointer or the underlying implementation pointer, each with its own evidence; code change on the cToken, its implementation, the rate model or the underlying; each with evidence word or code hash | persisted writes and code changes |
| `ModelEpoch` + `Dependency` | binding rows at the activation block and on the heartbeat (`basis_carryover = true`: share storage persists across upgrades; `global_carryover = false`: a rate-model replacement starts an epoch whose IRM rows do not carry): implementation (delegators) and interest-rate model as storage pointers on the cToken, underlying declared | parameters |
| `BlockClock` | exactly one per block | header |

Cash is cross-contract state. A direct USDC transfer to cUSDC or an ETH
transfer to cETH changes the exchange rate without any cToken write; the map
records those as `TOTAL_CASH` rows on the block they persist. CErc20 cash is
read only when the parameters bind the underlying's balances-mapping model
with its own `model_id` and `source_pin`; an underlying whose `balanceOf` is
not a plain mapping read (rebasing, fee-on-transfer, cDAI's DSR pot) must not
be configured this way. Rate-model reads are inputs for the consumer's
`accrue`; a change of the rate-model address is an epoch invalidation, not a
silent parameter switch.

## Parameters

The default manifest parameters bind no market and emit only `BlockClock`.
[`tests/fixtures/mainnet-ctoken-epochs.json`](tests/fixtures/mainnet-ctoken-epochs.json)
binds cUSDC (`CErc20`, USDC cash via the FiatToken `balances` mapping,
`IRM_USDC_Updateable` JumpRateModelV2 slots) and cETH (`CEther`, native
cash, `Base0bps_Slope2000bps` WhitePaper constants). Slots follow the pinned
`CTokenInterfaces.sol` declaration order (`interestRateModel` 6 …
`accountTokens` 14, allowances 15, borrow snapshots 16, `underlying` 17) and
`BaseJumpRateModelV2.sol` (multiplier 1, base 2, jump 3, kink 4). These
cToken and rate-model slots are **compiler-verified**: `solc 0.8.10
--storage-layout` of the pinned source is committed under
[`docs/evidence/storage-layouts/compound-v2@a3214f67.json`](../../docs/evidence/storage-layouts/compound-v2@a3214f67.json)
and [`tests/storage_layout.rs`](tests/storage_layout.rs) pins the `CErc20`,
`CEther`, `CErc20Delegator` and `JumpRateModelV2` fixtures to it with a
completeness check. The USDC cash slot is compiler-verified too
(`FiatTokenV2_2` at circlefin/stablecoin-evm v2.2.0: slot 9 is
`balanceAndBlacklistStates`, whose bit 255 is the blacklist flag masked by
`_balanceOf`, hence `value_bits: 255` in the fixture). Not verified: the
deployed runtime code hashes (including which FiatToken version the USDC
proxy points to) and the placeholder `activation_block` values; no Ethereum
Extended blocks are cached locally and live Firehose and RPC use is paused.

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

## Fail-closed rules

| Condition | Result |
| --- | --- |
| Non-Extended block, `Block.ver` not listed (only 4 and 5 may be listed), incomplete transaction data | block fails |
| Persisted cToken write that is not a configured scalar, `accountTokens` entry, pointer or reviewed slot / mapping member (struct words and nested mappings through verified preimages) | `unresolved storage … refusing incomplete balance state` |
| Two changes to one key with equal ordinals, or a change whose old value is not the previous new value | `ambiguous` / `discontinuous`, naming the contract, key and ordinals |
| Cash kind not matching the market kind, unqualified underlying model, overlapping slots, a rate-model parameter bound both as slot and constant, `blocks_per_year` other than the pinned `2102400`, producer versions outside 4 and 5, unknown fields | parameters rejected |

Reverted frames and failed transactions never contribute, per the shared
[`common/persist`](../../common/persist) rules.

## Validation

```sh
cargo test -p compound-v2-balance-state
cargo clippy -p compound-v2-balance-state --all-targets -- -D warnings
cargo check -p compound-v2-balance-state --target wasm32-unknown-unknown
make -C compound-v2/balance-state build
```

Tests are synthetic: share writes and same-block reduction, every market
word, donation-only ERC-20 and native cash changes, rate-model slot writes
and constants, each invalidation with its evidence, delegator binding,
reviewed struct and nested-mapping members, reverts, failed transactions,
ties, discontinuities and every parameter refusal. There is no captured-block
replay for this package yet; see issue
[#14](https://github.com/pinax-network/substreams-evm-extended/issues/14).
