# Versioned balance-state contract (`evm.balance_state.v1`)

Issue [#12](https://github.com/pinax-network/substreams-evm-extended/issues/12)
asks for a versioned extraction contract for balances whose observable value
depends on shared protocol state or time. `evm.balances.v1.Balance.amount`
means `balanceOf(account)` and is unchanged; scaled units, share counts,
signed principal and underlying-equivalent claims never replace it. This
document fixes the companion schema
[`proto/v1/balance_state.proto`](../proto/v1/balance_state.proto), its
consumption rules and its evidence, using the deployments and getter
formulas recorded in [extraction coverage](extraction-coverage.md).

## Package boundary

- `erc20/balances` and `native/balances` keep their single `map_events` and
  their `evm.balances.v1.Events` output. They gain nothing.
- Each protocol family gets a small companion package with exactly one
  RPC-free `map_events` whose output type is `evm.balance_state.v1.Events`:
  `aave/balance-state` ([#13](https://github.com/pinax-network/substreams-evm-extended/issues/13)),
  `compound-v2/balance-state` ([#14](https://github.com/pinax-network/substreams-evm-extended/issues/14)),
  `compound-v3/balance-state` ([#15](https://github.com/pinax-network/substreams-evm-extended/issues/15)),
  later `lido/balance-state` ([#23](https://github.com/pinax-network/substreams-evm-extended/issues/23)),
  `erc4626/balance-state` ([#24](https://github.com/pinax-network/substreams-evm-extended/issues/24))
  and a chain-scoped alias declaration for Arc. Output is confined to the
  facts a consumer needs to evaluate a balance; no prices, APY, health
  factors, liquidation policy or wallet classification.
- The generated Rust lives in `proto/src/pb/evm.balance_state.v1.rs`, produced
  by `buf generate` with `buf.build/community/neoeinstein-prost:v0.4.0`
  (`file_descriptor_set=false`), the same generator that reproduces the
  committed `evm.balances.v1.rs` byte for byte.

## Tables

Every top-level repeated field of `Events` becomes one table in the native
Substreams ClickHouse sink. Rows are flat: no nested messages, repeated
fields, `oneof` or `optional` inside a row.

| Table | Rows per block | Natural key in the block | Purpose |
| --- | --- | --- | --- |
| `BlockClock` | exactly 1, always | `(chain_id, number)` | Canonical clock: number, hash, parent, timestamp, state root, producer version, package identity, parameters digest, row counts of the other tables |
| `ModelEpoch` | 0..n | `(market, epoch, kind, ordinal)` | Model and runtime epoch of a market: family, `model_id`, source pin, implementation and code hashes, basis kind/scale/rounding/bit range, activation block and ordinal, carry-over flags, invalidation evidence |
| `Dependency` | 0..n | `(market, epoch, role, contract)` | Edges of the evaluation graph (token → pool → rate model → underlying → accumulator) with the binding kind that detects their change |
| `AssetAlias` | 0..n | `(asset, alias_of)` | One balance exposed at two precisions (Arc native USDC and its ERC-20 view) |
| `HolderBasis` | one per written holder word (end of block), or one per write | `(market, holder, ordinal, boundary)` | Holder basis: scaled balance, shares, signed principal, with slot-level provenance |
| `GlobalState` | one per written field | `(market, field, key, ordinal, boundary)` | Shared inputs: indices, rates, accrual clocks, totals, constants, cross-contract cash, report logs |

### Acceptance mapping

| Requirement | Where it is met |
| --- | --- |
| Holder basis with chain, market, holder, type, exact value, unit/scale, epoch, dependency references; negative principal preserved; debt distinct from unsigned balances | `HolderBasis`: `chain_id`, `market`, `holder`, `basis_kind`, `value` (signed decimal), `epoch` → `ModelEpoch.basis_scale`/`balance_rounding`/`balance_asset`/`balance_decimals`, `Dependency` rows. `BASIS_KIND_SIGNED_PRINCIPAL` keeps the sign and is never clamped; `BASIS_KIND_SCALED_DEBT` exists so debt cannot be mislabeled |
| Protocol-specific global state; observed deltas vs verified snapshots vs derived values; no partial delta labeled a snapshot | `GlobalState.field` (typed per family, append-only numeric ranges), `observation` (`OBSERVED_WRITE`, `OBSERVED_LOG`, `QUALIFIED_CONSTANT`, `DERIVED`), one row per field, `""` means not carried |
| Canonical block number/hash/parent/timestamp on every block; finality and undo left to the stream | `BlockClock`, exactly one per delivered block; no finality field; forks handled by `BlockUndoSignal` or `final_blocks_only` |
| Identities and order from transaction/call/log identity and ordinals; block and system scopes without a transaction hash; change boundary vs end-of-block stated | `scope`, `transaction_index`, `transaction_hash`, `call_index`, `log_index`, `ordinal`, `first_ordinal`, `change_count`, `boundary` on every fact row |
| One shared update affects initialized holders without synthetic holder writes | A `GlobalState` row is emitted; no `HolderBasis` row exists without a holder write. Consumers retain state and evaluate at a chosen clock (below) |
| Missing, known zero, unsupported model, invalidated dependency, reverted are distinct; upgrade/dependency invalidation defined; #7 reused | Absence = missing; `"0"` = known zero; `EPOCH_EVENT_KIND_SUSPENDED` = unsupported model; `EPOCH_EVENT_KIND_INVALIDATED` with `reason` and evidence = invalidated dependency; reverted effects never become rows. Checkpoints and rollback stay with [#7](https://github.com/pinax-network/substreams-evm-extended/issues/7) |
| Compatibility, versioning, Rust fixtures; no renumbering, hidden cache, RPC, extra map, `db_out` or custom sink | Rules below; `proto/src/balance_state_tests.rs`; `evm.balances.v1` asserted byte-identical in the same test run |
| Native sink mapping and cross-output identities without promising atomic publication | Section "Sink mapping" below |

## Semantics

**Presence.** `bytes` and `string` become `String` in the sink, so absence is
not representable by field presence. Exact integers are decimal strings where
`""` means "not carried by this row" and `"0"` is an observed zero; every
address that may legitimately be empty is paired with an enum or a scope that
says why (`Scope`, `Observation`, `AliasKind`, `alias_of` empty = native).

**Persistence.** Only persisted effects produce rows, under the shared
[`common/persist`](../common/persist) rules. A persisted write to a bound slot
the model cannot explain, an unexpected code change, an ordinal collision or a
discontinuous word fails the block, exactly as in `erc20/balances` and
`native/balances`. Declared boundaries (`SUSPENDED`, `PARAMETER_REBIND`) are
rows; live surprises are module errors.

**Boundary.** `BOUNDARY_END_OF_BLOCK` rows are the default: the value after
the last persisted write, `previous_value` before the first, `first_ordinal`,
`ordinal` and `change_count` reduced in execution-ordinal order (producer
versions 4 and 5 only; version 3 is refused). `BOUNDARY_CHANGE` rows are
opt-in per write. `BOUNDARY_DECLARATION` rows carry qualified constants.

**Provenance.** Every fact row cites the contract whose storage holds the
word, the 32-byte slot, the raw words before and after, and the decoded bit
range, so a packed slot (Aave `liquidityIndex|currentLiquidityRate`, Comet
`UserBasic`, Lido `totalAndExternalShares`) yields one row per field with the
same slot and words. Cross-contract inputs are visible because
`storage_contract` differs from `market` (USDC for cUSDC cash, the Maker Pot
for sDAI and cDAI, the Aave Pool for a static aToken).

**Epochs.** `ModelEpoch` binds the formula (`model_id`), the pinned source,
the implementation identity, the basis kind, scale, rounding and bit range,
and the activation block and ordinal. `basis_carryover` and
`global_carryover` state whether retained rows remain evaluable across the
boundary (Aave aToken revision 4 → 5: rounding changed, storage did not; Lido
V2 → V3: `shares` unchanged, `totalShares` slot moved). `REAFFIRMED` rows on
a heartbeat let a consumer joining mid-stream recover bindings without a
competing bootstrap. Activation blocks are unknown today
([open questions](extraction-coverage.md#6-open-questions)); until bound, a
market is `SUSPENDED` with `INVALIDATION_REASON_UNQUALIFIED_ERA`.

**Activation position and pointer writes.** Every balance-state package
accepts an optional `activation_ordinal` beside `activation_block`
(default `0`). An effect at `(block, ordinal)` belongs to an epoch only when
`block > activation_block`, or `block == activation_block` and
`ordinal >= activation_ordinal`; earlier effects of the activation block,
including the upgrade write that installs the epoch's implementation, belong
to the previous epoch and are neither decoded nor counted as invalidating the
new one. The BOUND row states `activation_ordinal` and uses it as its
`ordinal`, so a consumer that applies a block's epoch rows in `(ordinal, kind)`
order (as `common/retention` does) processes the previous epoch's
`INVALIDATED` row first and ends the block bound.

A `BINDING_KIND_STORAGE_POINTER` dependency is invalidated by every persisted
write to its slot, including a write back to the same value, and each write is
evidenced by its own `INVALIDATED` row before storage is reduced to end-of-block
values. An excursion X→Z→X inside one block therefore yields two rows that
name Z, rather than one reduced X→X row that would hide the temporary
implementation. Equal-value writes reach the maps through the persistence
rules' `storage_noop` callback; they are consumed only for pointer slots and
remain suppressed as balance effects.

**Evaluation at a selected clock.** To value a holder at block N, a consumer
takes the holder's latest `HolderBasis` at or before N, the latest
`GlobalState` per `(market, field, key)` at or before N under the same epoch,
the `BlockClock` of N for `timestamp` and `number`, and applies the epoch's
formula from the Rust conformance model
([#16](https://github.com/pinax-network/substreams-evm-extended/issues/16)):

| Family | Holder basis | Global fields | Clock input |
| --- | --- | --- | --- |
| Aave V3 aToken | `SCALED_BALANCE` (low 120 bits) | `AAVE_LIQUIDITY_INDEX`, `AAVE_CURRENT_LIQUIDITY_RATE`, `AAVE_LAST_UPDATE_TIMESTAMP` keyed by underlying | `timestamp`; linear interest, half-up `rayMul`, then floor (revision ≥ 4) or half-up (≤ 3) |
| Compound v2 cToken | `SHARES` | `COMPOUND_V2_TOTAL_CASH` (cross-contract), `TOTAL_BORROWS`, `TOTAL_RESERVES`, `TOTAL_SUPPLY`, `BORROW_INDEX`, `ACCRUAL_BLOCK_NUMBER`, `RESERVE_FACTOR_MANTISSA`, `INITIAL_EXCHANGE_RATE_MANTISSA`, `IRM_*` | `number`; stored rate vs projected accrual are separate results |
| Comet | `SIGNED_PRINCIPAL` (int104) | `COMET_BASE_SUPPLY_INDEX`, `BASE_BORROW_INDEX`, `TOTAL_SUPPLY_BASE`, `TOTAL_BORROW_BASE`, `LAST_ACCRUAL_TIME`, rate immutables and scales as `QUALIFIED_CONSTANT` | `timestamp`; `balanceOf` is 0 for negative principal, debt is `borrowBalanceOf` |
| Lido stETH | `SHARES` | `LIDO_TOTAL_SHARES`, `EXTERNAL_SHARES`, internal-ether components, or the `LIDO_REPORT_*` log fields | none between reports; balances are piecewise constant |
| ERC-4626 | `SHARES` | the dependency family's fields (`MAKER_POT_*`, `AAVE_*`) or `ERC4626_*` for virtual-offset vaults | as the dependency requires |

Stored conversion, projected conversion, observable balance and withdrawable
claim are different metrics; the schema carries inputs, the conformance model
names the metric. Debt balances and collateral positions are deferred scopes
and are never reported as supply balances.

## Sink mapping

The native CLI derives one table per top-level repeated message with
`_block_number_`, `_timestamp_`, `_version_`, `_deleted_` and `_row_id_`
columns on a `ReplacingMergeTree` keyed `(_block_number_, _row_id_)`.
`uint64`/`uint32` map to unsigned integers, `bool` to `Bool`, `string` and
`bytes` (`0x` hex) to `String`; the SQL representation of enums must be read
from the generated DDL on the first authorized sink run, and enum numbers are
the stable contract.

- Read history with `FINAL` and `NOT _deleted_`; take the greatest
  `(_block_number_, ordinal)` per natural key; never filter `"0"` values
  before selecting the latest row.
- Because `BlockClock` makes every delivered block nonempty, the sink's
  `_blocks_` markers list every block of this package. ClickHouse writes are
  still not multi-table transactions: compare `BlockClock.*_count` with the
  rows landed per block to detect a partially written block. A cursor is not
  a publication manifest.
- Cross-output identities: `HolderBasis.market` joins `evm.balances.v1`
  `Balance.contract` for the same token; `AssetAlias.asset` joins the ERC-20
  side and `alias_of` (empty) the `native/balances` rows. Keep packages in
  separate databases or tables; the absent-native `contract` is `''` in SQL.
  Summing both sides of an alias double counts one balance.

## Versioning

1. Package `evm.balance_state.v1` is additive to `evm.balances.v1`; no import,
   no shared types, no reinterpretation. The fixtures assert the historical
   `Balance` encodings.
2. New protocol family: new `StateField` values in a new numeric range and,
   if needed, a new top-level message with the next `Events` field number.
   Never renumber or reuse a field.
3. Enums are append-only; `UNSPECIFIED = 0` stays and is never emitted; an
   unknown enum number decodes as a raw integer (prost) and the consumer
   refuses to evaluate the row.
4. Changing a field's meaning is a breaking change → `evm.balance_state.v2`
   beside v1. A getter that changes rounding or inputs is a new epoch
   (`ModelEpoch`), not a schema change. `BlockClock.spec_revision`
   increments when emission rules change without a wire change.
5. `buf lint` passes; `buf breaking --against` the main branch is the CI gate
   to add with the first companion package.

## Fixtures

`proto/src/balance_state_tests.rs` builds, encodes and decodes: an Aave
supply (holder row plus the three reserve fields), a Comet accrual with no
holder rows, a Compound v2 donation-only cash change with `""` versus `"0"`
distinguished on the wire, a Lido rebase as a log observation, an sDAI Pot
drip with its `ModelEpoch` and `Dependency`, an Arc alias, a clock-only
block, int104 and uint256 extremes, unknown enum survival, enum zero values,
disjoint `StateField` ranges, and the unchanged `evm.balances.v1` encodings.
Golden lengths pin the wire size of the Aave and clock-only examples.

## Not covered here

Actual extraction of any family, activation blocks, storage-layout dumps and
live qualification belong to #13–#16, #23, #24 and #8. Live Substreams,
Firehose, RPC and sink usage remains paused.
