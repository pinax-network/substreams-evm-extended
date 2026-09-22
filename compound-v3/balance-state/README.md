# compound-v3/balance-state

A single RPC-free `map_events` reads Firehose Extended blocks and emits
**`evm.balance_state.v1.Events`** ([contract](../../docs/balance-state-contract.md))
for explicitly qualified Compound III (Comet) market epochs. It does not
emit `evm.balances.v1`; the market's observable `balanceOf` / `borrowBalanceOf`
is evaluated by the consumer from the rows below at a canonical block clock,
using the exact model in [`conformance::comet`](../../conformance/src/comet.rs).

## What is emitted

| Table | Row | Source |
| --- | --- | --- |
| `HolderBasis` (`SIGNED_PRINCIPAL`) | the signed int104 `UserBasic.principal` (low 104 bits, two's complement) before the first and after the last write of the block, for **every** written `userBasic` word (a write that only moves the tracking fields yields a row with `value == previous_value`) | Comet storage, verified Keccak preimage `(account, user_basic_slot)` |
| `GlobalState` `COMET_BASE_SUPPLY_INDEX` / `COMET_BASE_BORROW_INDEX` | bits 0..64 and 64..128 of the indices word, one row each per written word | Comet storage, `indices_slot` |
| `GlobalState` `COMET_TOTAL_SUPPLY_BASE` / `COMET_TOTAL_BORROW_BASE` / `COMET_LAST_ACCRUAL_TIME` / `COMET_PAUSE_FLAGS` | bits 0..104, 104..208, 208..248 and 248..256 of the totals word, one row each per written word | Comet storage, `totals_slot` |
| `GlobalState` kinks, rate slopes, rate bases, scales | `QUALIFIED_CONSTANT` / `DECLARATION` rows of the bound implementation's immutables | parameters, cross-checked against the pinned constants |
| `ModelEpoch` INVALIDATED | every persisted write to the implementation pointer, including an equal-value write and each step of an in-block excursion (`IMPLEMENTATION_POINTER_WRITE`); code change on the Comet or its implementation (`CODE_CHANGE`); each with evidence; the block's other writes are still decoded | persisted writes and code changes |
| `ModelEpoch` + `Dependency` | binding rows at the activation block and on the heartbeat (`basis_carryover = true`, `global_carryover = false`: storage persists across an upgrade, rate immutables do not) | parameters |
| `BlockClock` | exactly one per block | header |

One persisted write to a packed word yields one row per decoded field, as the
contract requires; consumers that want change-only semantics compare `value`
with `previous_value`. Tracking indices (bits 128..256 of the indices word) and
the non-principal fields of the user word (bits 104..256: `baseTrackingIndex`,
`baseTrackingAccrued`, `assetsIn`, `_reserved`) are reward and collateral
bookkeeping and are not decoded.

`balanceOf(account)` is `principal > 0 ? principal * accruedSupplyIndex / 1e15 : 0`
and `borrowBalanceOf` is `principal < 0 ? -principal * accruedBorrowIndex / 1e15 : 0`
(the negation of int104 min reverts), where the accrued index projects the
stored index to the evaluation timestamp with the per-second rate derived
from utilization and the implementation's immutable kink / slope / base
constants. Comet rates are immutables of the implementation, so every
governance rate change deploys a new implementation and starts a new epoch.
An idle block changes the evaluated balance of every account without any
row; an account without a row is unknown, not zero.

## Parameters

The default manifest parameters bind no market and emit only `BlockClock`.
[`tests/fixtures/mainnet-cusdcv3-epoch.json`](tests/fixtures/mainnet-cusdcv3-epoch.json)
is the Ethereum cUSDCv3 configuration the synthetic tests use: Comet
`0xc3d688B6…cdc3`, USDC base, immutables from the pinned
`deployments/mainnet/usdc/configuration.json` rate parameters scaled to
per-second factors, storage slots `0` (indices), `1` (totals) and `5`
(`userBasic`) with the other mappings `2, 3, 4, 6, 7`, and the reviewed
unstructured slot `comet.reentrancy.guard` (`CometCore.sol:60`,
`keccak256` of the label = `0xc98c7730…53ac`, written `0→1→0` by every
guarded call: `supply`, `withdraw`, `transfer`, `buyCollateral`, `absorb`).
`producer_versions` must be a subset of `[4, 5]`; `base_index_scale`,
`factor_scale` and `base_scale` must equal the pinned `1e15`, `1e18` and
`10^base_decimals`.

The slot numbers and bit ranges are **compiler-verified**: `solc 0.8.15
--storage-layout` of the pinned source is committed under
[`docs/evidence/storage-layouts/comet@f766f515.json`](../../docs/evidence/storage-layouts/comet@f766f515.json)
and [`tests/storage_layout.rs`](tests/storage_layout.rs) pins every fixture slot
and bit range to it and checks that no compiled slot is left unreviewed
([provenance](../../docs/storage-layout-provenance.md)). The implementation
address, its code hash and the activation block are **not verified**: no Ethereum Extended blocks are cached locally and live Firehose
and RPC use is paused. The fixture uses placeholder `implementation` and
`activation_block` values for that reason; a real epoch must replace them
after qualification.

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
| Non-Extended block, `Block.ver` not listed, incomplete transaction data, malformed identity or timestamp | block fails |
| Persisted Comet write that is not the indices word, the totals word, the pointer, a `userBasic` entry or a reviewed slot / mapping (`other_slots`, `other_slot_names`, `other_mapping_slots`, chained through verified preimages) | `unresolved storage for Comet 0x… at key 0x…` |
| Two writes to one key with equal ordinals, or a write whose old value is not the previous new value | `ambiguous` / `discontinuous`, naming the contract, key and ordinals |
| Overlapping slots (including a named slot equal to a configured one), non-decimal or inconsistent constants, `base_decimals > 18`, unknown parameter fields, producer versions outside 4 and 5, duplicate markets | parameters rejected |

Reverted frames and failed transactions never contribute, per the shared
[`common/persist`](../../common/persist) rules; no-op writes are not persisted.

## Validation

```sh
cargo test -p compound-v3-balance-state
cargo clippy -p compound-v3-balance-state --all-targets -- -D warnings
cargo check -p compound-v3-balance-state --target wasm32-unknown-unknown
make -C compound-v3/balance-state build
```

The tests are synthetic: the guard slot and scales against the pinned
labels; positive, zero, negative and int104-extreme principals, sign
crossings, tracking-only writes and first-time holders; same-block repeated
writes with provenance; every decoded field of both market words; a routine
supply in a delegatecall frame with the reentrancy guard and a system-call
write; activation, heartbeat and carryover flags; pointer and code
invalidations versus the binding write; reviewed mapping chains; unresolved
writes; reverted frames, FAILED and REVERTED transactions; every
`validate_block` refusal; pre-activation blocks; two markets in one block;
determinism under input permutation; and every parameter refusal. There is
no captured-block replay for this package yet; see issue
[#15](https://github.com/pinax-network/substreams-evm-extended/issues/15).
