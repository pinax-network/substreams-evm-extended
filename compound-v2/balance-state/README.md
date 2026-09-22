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
at a canonical block clock: `accrue_with` reproduces the block-delta,
borrow-rate, truncation and ordering of the bound cToken revision
(`CTokenRevision::Legacy2019` for the 2019 cUSDC/cETH: borrow-rate cap 5e14
checked before the block delta; `Current` for the 0.8.10 tree: cap 5e12) and
rate model (`RateModel::Jump`, `RateModel::WhitePaper2019`), and
`underlying_balance_with` the truncated conversion. The map never replaces
the ERC-20 share amount with an underlying-equivalent value.

## What is emitted

| Table | Row | Source |
| --- | --- | --- |
| `HolderBasis` (`SHARES`) | `accountTokens[holder]` before the first and after the last write of the block | cToken storage, verified Keccak preimage `(holder, account_tokens)` |
| `GlobalState` `COMPOUND_V2_TOTAL_BORROWS` / `TOTAL_RESERVES` / `TOTAL_SUPPLY` / `BORROW_INDEX` / `ACCRUAL_BLOCK_NUMBER` / `RESERVE_FACTOR_MANTISSA` / `INITIAL_EXCHANGE_RATE_MANTISSA` | the cToken scalar words, scales `1` or `1e18` | cToken storage, configured slots |
| `GlobalState` `COMPOUND_V2_TOTAL_CASH` (`key` = cToken) | CErc20: the low `value_bits` of `underlying.balances[cToken]` from the qualified underlying's mapping (USDC: 255 bits, the blacklist flag lives in bit 255); CEther: persisted native balance changes of the cToken, with no `storage_slot` (a native balance has none) | underlying storage or cToken balance changes |
| `GlobalState` `COMPOUND_V2_IRM_*` | rate-model storage writes for configured slots (jump models' `updateJumpRateModel`), and qualified constants at BOUND / REAFFIRMED at the activation ordinal: `blocksPerYear`, and the 2019 WhitePaper model's per-year `IRM_BASE_RATE_PER_YEAR` / `IRM_MULTIPLIER_PER_YEAR` (set only by its constructor) | rate-model storage, parameters |
| `ModelEpoch` INVALIDATED | every persisted write, including equal-value and restored ones, to the rate-model pointer on the cToken (`RATE_MODEL_CHANGE`), the delegator implementation pointer, the cToken's `underlying` word or the underlying implementation pointer, each with its own evidence; code change on the cToken, its implementation, the rate model, the underlying or its implementation; each with evidence word or code hash. The first such row ends the epoch at its ordinal: later effects of that block are not decoded, so an upgrade's `_becomeImplementation` writes yield the evidence instead of failing the block | persisted writes and code changes |
| `ModelEpoch` + `Dependency` | binding rows at the activation block and on the heartbeat (`basis_carryover = true`: share storage persists across upgrades; `global_carryover = false`: a rate-model replacement starts an epoch whose IRM rows do not carry): implementation (delegators), interest-rate model and underlying as storage pointers on the cToken, the underlying's implementation as a depth-2 pointer under it. `balance_asset` / `balance_decimals` name the underlying (empty for native ether) and `basis_scale` the 1e18 exchange-rate mantissa | parameters |
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
`IRM_USDC_Updateable` LegacyJumpRateModelV2 slots) and cETH (`CEther`, native
cash, `Base0bps_Slope2000bps` 2019 WhitePaper per-year constants).

**The deployed cUSDC and cETH do not run the current compound-protocol tree.**
Their ABI in the pinned `networks/mainnet-abi.json` (`decimals` as `uint256`,
a public `initialExchangeRateMantissa`, a three-field `AccrueInterest`, a
constructor without `admin_`) is the 2019 source, whose `ReentrancyGuard`
puts `_guardCounter` in slot 0 and shifts every word from `admin` up by one.
The fixture follows that layout (`_guardCounter` 0, name 1, symbol 2,
`decimals` 3, admin 4, pendingAdmin 5, comptroller 6, `interestRateModel` 7 …
`totalSupply` 14, `accountTokens` 15, allowances 16, borrow snapshots 17,
`underlying` 18). The 2019 guard counter increments on every `nonReentrant`
call and is reviewed storage. cUSDC's `LegacyJumpRateModelV2` (2020) keeps
`BaseJumpRateModelV2`'s storage (multiplier 1, base 2, jump 3, kink 4) and
arithmetic; cETH's 2019 `WhitePaperInterestRateModel` stores per-year
`multiplier` (0) and `baseRate` (1) and divides the annual rate per call.
These slots are **compiler-verified**:
[`compound-protocol-2019@f385d719.json`](../../docs/evidence/storage-layouts/compound-protocol-2019@f385d719.json)
(the initial public tree, solc 0.5.17 standard-JSON storage layout),
[`compound-protocol-legacy-jump@4caf72a1.json`](../../docs/evidence/storage-layouts/compound-protocol-legacy-jump@4caf72a1.json),
and for current-tree delegators such as cDAI
[`compound-v2@a3214f67.json`](../../docs/evidence/storage-layouts/compound-v2@a3214f67.json)
(solc 0.8.10), all pinned by [`tests/storage_layout.rs`](tests/storage_layout.rs)
with completeness checks. That the deployed bytecode was built from these
trees is **not** verified; it needs the runtime code hashes. The USDC cash
slot is compiler-verified too (`FiatTokenV2_2` at circlefin/stablecoin-evm v2.2.0: slot 9 is
`balanceAndBlacklistStates`, whose bit 255 is the blacklist flag masked by
`_balanceOf`, hence `value_bits: 255` in the fixture). The USDC
implementation the proxy must hold (`underlying.implementation`) is an
**unqualified placeholder**. Not verified: the deployed runtime code hashes
(including which FiatToken version the USDC proxy points to) and the
placeholder `activation_block` values; no Ethereum Extended blocks are
cached locally and live Firehose and RPC use is paused. A parameter set
binds one epoch per market; a successor epoch is a new parameter set.

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
| Cash kind not matching the market kind, unqualified underlying model, ERC-20 cash without `slots.underlying`, an underlying implementation slot without its address, overlapping slots, a rate-model parameter bound both as slot and constant, a constant that is not a canonical uint256 decimal, `blocks_per_year` other than `2102400`, producer versions outside 4 and 5, unknown fields | parameters rejected |

Reverted frames and failed transactions never contribute, per the shared
[`common/persist`](../../common/persist) rules.

## Validation

```sh
cargo test -p compound-v2-balance-state
cargo clippy -p compound-v2-balance-state --all-targets -- -D warnings
cargo check -p compound-v2-balance-state --target wasm32-unknown-unknown
make -C compound-v2/balance-state build
```

Tests are synthetic: share writes and same-block reduction, an accrual-only
block with no holder row, every market word, donation-only ERC-20 and native
cash changes (system-call and block scopes, mid-block activation), rate-model
slot writes and per-year constants, each invalidation with its evidence, the
epoch ending at an in-block upgrade, delegator binding and implementation
code changes, a share write without its preimage, reviewed struct and
nested-mapping members, reverts, failed transactions, ties, discontinuities
and every parameter refusal. `conformance::compound_v2` has exact vectors
computed outside the crate for accrual over one and ten blocks, the kink and
jump branches, both borrow-rate caps and the 2019 WhitePaper rate. There is no captured-block
replay for this package yet; see issue
[#14](https://github.com/pinax-network/substreams-evm-extended/issues/14).
