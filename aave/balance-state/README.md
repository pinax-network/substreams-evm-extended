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

## Fail-closed rules

| Condition | Result |
| --- | --- |
| Non-Extended block, unlisted `Block.ver`, incomplete transaction data | block fails |
| Code change on the Pool, an aToken or either implementation | `code changed; requalify the epoch` |
| Write to the Pool or aToken implementation pointer slot | `implementation pointer changed; requalify the epoch` |
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

```sh
cargo test --locked -p aave-balance-state -p aave-balance-state-tools -p conformance
cargo run --locked -p aave-balance-state-tools -- replay --blocks <dir> --params tests/fixtures/bsc-aave-v3-epochs.json --output out/replay
```

## Boundaries

No RPC, no synthetic holder fan-out, no evaluated balances in the output, no
`evm.balances.v1` rows, no `db_out` or custom sink. Variable-debt tokens,
Ethereum Core markets, legacy revisions and other chains need their own
epochs and fixtures ([#8](https://github.com/pinax-network/substreams-evm-extended/issues/8)).
Initialization and checkpoints are shared under
[#7](https://github.com/pinax-network/substreams-evm-extended/issues/7);
packaging and live qualification stay paused.
