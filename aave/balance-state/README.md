# aave/balance-state

A single RPC-free `map_events` reads Firehose Extended blocks and emits
**`evm.balance_state.v1.Events`** ([contract](../../docs/balance-state-contract.md))
for explicitly qualified Aave V3 aToken epochs. It does not emit
`evm.balances.v1`; the aToken's observable `balanceOf` is evaluated by the
consumer from the rows below at a canonical block clock, using the exact
model in [`conformance::aave`](../../conformance/src/aave.rs).

## What is emitted

| Table | Row | Source |
| --- | --- | --- |
| `HolderBasis` | scaled balance of a holder whose `_userState[holder]` word was written: low `basis_bits` bits before the first and after the last write of the block | aToken storage, verified Keccak preimage `(holder, user_state_slot)` |
| `GlobalState` `AAVE_LIQUIDITY_INDEX` / `AAVE_CURRENT_LIQUIDITY_RATE` | low and high 128 bits of `ReserveData` word 1 for the bound underlying | Pool storage, `keccak(underlying, reserves_slot) + 1` |
| `GlobalState` `AAVE_LAST_UPDATE_TIMESTAMP` | bits 128..168 of `ReserveData` word 3 | Pool storage, `+ 3` |
| `GlobalState` `AAVE_SCALED_TOTAL_SUPPLY` | the aToken's `_totalSupply` | aToken storage |
| `ModelEpoch` + `Dependency` | binding rows at the activation block and on the heartbeat | parameters |
| `BlockClock` | exactly one per block | header |

`balanceOf(holder)` under aToken revision 4 and later is
`scaled.rayMulFloor(getNormalizedIncome(reserve))`, where
`getNormalizedIncome` returns the stored index when the reserve was updated
in the same second and otherwise
`calculateLinearInterest(currentLiquidityRate, lastUpdateTimestamp).rayMul(liquidityIndex)`
(half-up). Revision 3 and earlier used half-up for the final step too. The
epoch configuration names the rounding era; the map never computes the
balance. An idle block changes the evaluated amount of every initialized
holder without any row; a holder without a row is unknown, not zero.

## Parameters

```json
{"chain_id":56,"producer_versions":[5],"heartbeat_blocks":1000,
 "pool":{"address":"0x6807…e0cB","reserves_slot":"0x…34","implementation_slot":"0x3608…2bbc","implementation":"0x5e2B…3B6d","source_pin":"…"},
 "markets":[{"atoken":"0xa925…f1B1","underlying":"0x55d3…7955","epoch":1,"model_id":"aave-v3/atoken/scaled-floor",
   "source_pin":"aave-dao/aave-v3-origin@8305565a…","implementation_revision":"5","activation_block":122288006,
   "balance_decimals":18,"rounding":"floor","basis_bits":120,
   "user_state_slot":"0x…34","total_supply_slot":"0x…36","other_mapping_slots":["0x…35"],
   "implementation_slot":"0x3608…2bbc","implementation":"0x7e19…4134"}]}
```

The default manifest parameters bind no market and emit only `BlockClock`.
[`tests/fixtures/bsc-aave-v3-epochs.json`](tests/fixtures/bsc-aave-v3-epochs.json)
is the BSC configuration replayed offline: Pool, aBnbUSDT and aBnbUSDC
addresses from `bgd-labs/aave-address-book@4e13aa19…`; `_reserves` at Pool
slot 52 and `_userState` / allowances / `_totalSupply` at aToken slots
0x34 / 0x35 / 0x36 as observed from Keccak preimages in the saved blocks and
consistent with the pinned source declaration order. Runtime code hashes and
the aToken revision activation block are **not** bound offline; the
configured `activation_block` is the start of the saved window, not the
upgrade block.

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
| Code change on the Pool, its implementation, an aToken or its implementation | `ModelEpoch` INVALIDATED (`CODE_CHANGE` for the aToken or its implementation, `DEPENDENCY_CODE_CHANGE` for the Pool side) with the new code hash as evidence; the block's other writes are still decoded |
| Any persisted write to the Pool or aToken implementation pointer slot, including an equal-value write and each step of an in-block excursion that restores the bound implementation | `ModelEpoch` INVALIDATED per write (`DEPENDENCY_POINTER_WRITE` for every market active at that ordinal / `IMPLEMENTATION_POINTER_WRITE`) with the old and new words as evidence; the block's other writes are still decoded |
| aToken write that is not `_userState` (verified preimage), `_totalSupply`, or a reviewed `other_slots` / `other_mapping_slots` entry | `unresolved storage for aToken … refusing incomplete balance state` |
| Two writes to one key with equal ordinals, or a write whose old word differs from the previous new word | `ambiguous` / `discontinuous` |
| Pool writes to `ReserveData` words other than 1 and 3 (configuration, variable-debt index and rate, addresses, treasury accrual, virtual balance) | recognized, not emitted |
| Pool writes outside the bound reserve structs | outside the model, ignored |
| Market whose `activation_block` is in the future | not bound: no rows, no guards |

Reverted frames and failed transactions never produce rows
([`common/persist`](../../common/persist)).

## Offline evidence

- `src/tests.rs` replays three unmodified captured transactions
  ([provenance](tests/fixtures/cases.json)): an aBnbUSDT burn to a known zero,
  a withdrawal, and one holder touching aBnbUSDT and aBnbUSDC with both
  reserves updating; plus synthetic cases for continuity, reverted frames,
  pointer writes, code changes, unrelated Pool writes and parameter guards.
- `conformance::aave` reproduces every captured reserve index update exactly
  from the previous index, rate and clock, and shows that floor instead of
  half-up already diverges for some of them.
- The replay tool (`tools/`) projected 1,433 saved producer-version-5 BSC
  blocks ([report](docs/evidence/replay-bsc-v5.json)): 0 projection errors,
  1,380 clock links, 14 cross-block global continuity checks and 6 stored
  index writes reproduced by the model with 0 mismatches, 4 holder rows for 3
  holders (Aave activity is sparse in the saved window). This is saved-data
  evidence for the Rust map, not packaged output or same-block `balanceOf`
  controls.
- After the 2026-09-21 hardening (INVALIDATED rows instead of failing the
  block on pointer writes and code changes; producer versions restricted to
  4 and 5; contextual `reduce()` diagnostics) the same replay over every
  cached directory ([report](docs/evidence/replay-bsc-v5-rev2.json)) produced
  the identical 4 holder, 22 global and 4 epoch rows: 1,438 blocks, 0
  projection errors, 14/14 continuity checks, 6/6 index-oracle checks,
  1,445 clock links, 0 mismatches.
- After aligning pointer handling with the `STORAGE_POINTER` contract and
  adding `activation_ordinal` (2026-09-22), the same replay produces a
  byte-identical `rows.jsonl` (sha256 `a42b84a6…`, [report](docs/evidence/replay-bsc-v5-rev3.json)):
  no pointer write, equal-value or otherwise, occurs in the saved window.
- With `_nonces` reviewed and the in-block epoch end (2026-09-22, after the
  live run), the replay again produces the byte-identical `rows.jsonl`
  ([report](docs/evidence/replay-bsc-v5-rev4.json)).

```sh
cargo test --locked -p aave-balance-state -p aave-balance-state-tools -p conformance
cargo run --locked -p aave-balance-state-tools -- replay --blocks <dir> --params tests/fixtures/bsc-aave-v3-epochs.json --output out/replay
```

## Live qualification (BSC, 2026-09-22)

The qualified epoch is [`epochs/bsc-aave-v3.json`](epochs/bsc-aave-v3.json):
aBnbUSDT and aBnbUSDC on the Aave V3 BNB Pool, aToken implementation
`0x7e19…4134` (`ATOKEN_REVISION` 5, installed at block 76,571,348) with Pool
implementation `0x5e2B…3B6d` (installed at block 101,087,794 by the write at
ordinal 3346), so the epoch starts at **block 101,087,794, ordinal 3347**,
producer versions 4 and 5 ([binding evidence](docs/evidence/epoch-binding-bsc-2026-09-22.json)).
The layout is **compiler-verified** against aave-v3-origin `8305565a` with
solc 0.8.27 ([layout](../../docs/evidence/storage-layouts/aave-v3-origin@8305565a.json),
[`tests/storage_layout.rs`](tests/storage_layout.rs)): `_userState` 52
(`balance` uint120), `_allowances` 53, `_totalSupply` 54, `_nonces` 58; Pool
`_reserves` 52 with `liquidityIndex`/`currentLiquidityRate` in word 1 and
`lastUpdateTimestamp` at bits 128..168 of word 3.

The packed map of this source (`spkg` sha256 `a6db4088…`, wasm `7611bbf1…`)
was streamed from `bsc.substreams.pinax.network` and every emitted row was
checked with `aave-balance-state-tools live-parity` against RPC getters at
the row's exact block hash ([report](docs/evidence/live-parity-bsc-2026-09-22-rev3.json);
events sha256 `9d1c5978…`, byte-identical to the pre-`rustfmt` build
`baef8568…` of [rev2](docs/evidence/live-parity-bsc-2026-09-22-rev2.json)):
2,060 blocks (the activation block, the 2,000 contiguous blocks
123,449,757–123,451,756, and every earlier block since 123,441,756 with an
aToken log, plus the nine aToken `Approval` blocks of the preceding
400,000 blocks), 8,240
clock fields, 103 holder rows for 28 holders (`scaledBalanceOf` 103/103,
`balanceOf` evaluated by `conformance::aave` 103/103), 281 reserve words and
94 scaled total supplies, all equal; BOUND at ordinal 3347, two heartbeats,
no invalidation; implementation pointers and revisions equal at the first and
last block.

The first live runs found a production defect: a router `permit` writes
`_nonces[owner]` (slot 58), which the parameters did not review, so block
123,068,971 was refused ([failure record](docs/evidence/live-permit-failure-bsc-2026-09-22.json)).
The nonces mapping is now reviewed and pinned by the layout test. An upgrade
now ends the epoch at its pointer write for the rest of the block, so its
`initialize` writes yield the INVALIDATED evidence instead of failing the
block. Earlier reports for superseded packages are kept
([rev2](docs/evidence/live-parity-bsc-2026-09-22-rev2.json),
[rev1](docs/evidence/live-parity-bsc-2026-09-22-rev1.json),
[fixture parameters](docs/evidence/live-parity-bsc-2026-09-22-fixture-params.json)).

```sh
# credentials only in the environment: SUBSTREAMS_API_KEY and RPC_URL
make -C aave/balance-state pack
substreams run -e bsc.substreams.pinax.network:443 <spkg> map_events -s <N> -t +1 \
  -p "map_events=$(jq -c . aave/balance-state/epochs/bsc-aave-v3.json)" -o jsonl --bytes-encoding hex > events.jsonl
cargo run --locked -p aave-balance-state-tools -- live-parity --events events.jsonl \
  --params aave/balance-state/epochs/bsc-aave-v3.json --spkg <spkg> --endpoint bsc.substreams.pinax.network:443 --output <fresh dir>
```

This qualifies the stated blocks and the holders written in them. It does
not initialize or check holders without a row, other markets, variable debt,
other networks, or bytecode equality of the deployments with a build of the
pinned source.

## Boundaries

No RPC, no synthetic holder fan-out, no evaluated balances in the output, no
`evm.balances.v1` rows, no `db_out` or custom sink. Variable-debt tokens,
Ethereum Core markets, legacy revisions and other chains need their own
epochs and fixtures ([#8](https://github.com/pinax-network/substreams-evm-extended/issues/8)).
Initialization and checkpoints are shared under
[#7](https://github.com/pinax-network/substreams-evm-extended/issues/7);
live qualification covers only the epoch and interval stated above.
