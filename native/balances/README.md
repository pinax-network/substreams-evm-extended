# native/balances

A single RPC-free `map_events` reads Firehose Extended blocks and emits
**`evm.balances.v1.Events`** for native account balances. The protobuf is the
shared [`proto/v1/balances.proto`](../../proto/v1/balances.proto); nothing is
added or renumbered. `Balance.contract` is **absent** for native amounts and
`Balance.amount` is the account's balance after the last persisted change in
the block, as an exact decimal `uint256` string.

This package ports the reducer of the historical `evm-balances-storage`
prototype ([source](https://github.com/pinax-network/substreams-evm/blob/311f9005cc8606e9cb1034562b97cf7281f30c99/evm-balances-storage/src/lib.rs))
without its WBNB hardcode, custom protobuf, extra maps, `db_out` or Python
helpers. The [current RPC native module](https://github.com/pinax-network/substreams-evm/blob/970a665e15619de8ad7f686bd89412a1030d46dc/native/balances/src/lib.rs)
discovers candidate addresses and calls `eth_getBalance`; its heuristics are
comparison context, not persistence rules, and are not ported.

## What a row means

Balances are **account state**. Every persisted `BalanceChange` record of the
block is collected by the shared [`evm-persist`](../../common/persist) rules,
sorted by execution ordinal and reduced per account. The row is the final
value; the reducer also retains the value before the account's first change and
the ordinals of its first and last change for host-side checks.

- An account without a persisted change in the block is **not emitted**.
  Absence is no observation; it never initializes an untouched account to zero.
- An account that changes and returns to its starting value **is emitted**
  (a net-zero changing account is different from an individual no-op record,
  which the persistence rules drop).
- Final known zeros are emitted as `"0"`.
- The zero address, precompiles, BSC system contracts and burn-looking
  addresses are ordinary accounts. A credit to `0x…dead` is a balance change,
  not a supply statement.
- An absent `old_value`/`new_value` message **inside an observed record**
  encodes zero under the pinned Ethereum protobuf contract. This does not turn
  a missing record into a zero balance.
- CALLCODE/DELEGATECALL value, value-bearing calls and transfer logs are not
  consulted. Balances are not a sum of inferred transfers, and transfer lists
  do not cover every account-state effect.
- Rows are sorted by account and carry no block hash; the stream's clock and
  cursor identify the block. An empty output block is a block with no
  persisted native change, not a missing block.

## Persistence and fail-closed rules

Which records persist is decided by [`evm-persist`](../../common/persist):
non-reverted frames of succeeded transactions; only `GAS_BUY`, `GAS_REFUND` and
`REWARD_TRANSACTION_FEE` from the root call of a failed or reverted
transaction; non-reverted system calls; every block-level record.

The reducer rejects the whole block when the producer data is ambiguous or
incomplete instead of guessing:

| Condition | Error |
| --- | --- |
| Block is not `DETAILLEVEL_EXTENDED` | `Extended blocks required` |
| `Block.ver` not listed in the parameters | `Extended producer version not qualified for native balances` |
| Missing header, non-32-byte hash/parent/state root, header number mismatch | `invalid block identity` / `header number mismatch` |
| Block number 0 | `genesis block not qualified for native balances` |
| Transaction without status 1–3 or without calls | `incomplete transaction persistence data` |
| Failed/reverted transaction whose root call carries a reason other than the three gas reasons or the reasons known to revert (`TRANSFER`, `TOUCH_ACCOUNT`, `SUICIDE_REFUND`, `SUICIDE_WITHDRAW`, `CALL_BALANCE_OVERRIDE`, `BURN`) | `failed transaction carries a balance-change reason without pinned persistence semantics` |
| Address not 20 bytes | `invalid native account address` |
| Persisted record with ordinal 0 | `persisted native balance change has no execution ordinal` |
| Value longer than 32 bytes | `native balance exceeds uint256` |
| Two records of one account with the same ordinal | `ambiguous native balance execution order` |
| A record's old value differs from the account's previous new value | `discontinuous native balance changes within block` |

The failed-transaction reason guard exists because BNB blob-fee rewards
(`REASON_REWARD_BLOB_FEE`, 17), OP-stack deposit mints (`INCREASE_MINT`, 18)
and reverts (`REVERT`, 19) have no saved fixture inside a failed transaction.
Silently dropping such a record could omit a persisted credit; failing the
block surfaces the case for qualification. See the
[persisted-effect matrix](docs/persisted-effects.md).

## Parameters

`map_events` takes a JSON object. Nothing about the producer or network is
inferred from a block, and the block carries no chain id; the manifest binds
the network and the parameters bind the producer version:

```json
{"producer_versions":[5]}
```

`producer_versions` lists the `Block.ver` values the caller has qualified for
the manifest's network. It must be non-empty; unknown fields are rejected. The
committed manifest uses `network: bsc` and version 5, the only version replayed
against saved RPC controls. Version 4 blocks reduce without projection errors
in the saved-data replay but have no RPC oracle, so they are not enabled by
default. Version 3 is never acceptable for this reducer: its tracer recorded
system-call ordinals on a separate scale and zeroed root-call begin ordinals,
so ordering across scopes cannot be trusted (see the
[persisted-effect matrix](docs/persisted-effects.md)). Reusing this package on
another network requires that network's fixtures and qualification under
[#8](https://github.com/pinax-network/substreams-evm-extended/issues/8).

## Build and test

```sh
cargo test --locked -p native-balances -p native-balances-tools -p evm-persist
make -C native/balances build      # release WASM only; no network
make -C native/balances replay     # offline replay over captured blocks
```

`make pack`, `make run` and any native sink command are live-usage steps and
remain paused until explicitly resumed. No SPKG of this package is committed;
building and qualifying one is tracked with
[#17](https://github.com/pinax-network/substreams-evm-extended/issues/17) and
the package gates in
[#6](https://github.com/pinax-network/substreams-evm-extended/issues/6).

## Offline evidence

Rust regressions (`src/tests.rs`) cover the complete captured BSC block
122260950 against all **82 saved native `eth_getBalance` rows**, the historical
prototype's 82 old/new/ordinal rows, the BSC fee-reset order (transaction
ordinal 3237 credit, block ordinal 3244 reset to zero), the two captured
failed SetCode transactions, net-zero accounts, absent value messages, zero and
burn-looking addresses, `uint256` max, tied/zero ordinals, discontinuity,
reverted frames, failed transactions, reverted system calls, the failed-
transaction reason guard, producer-version and parameter parsing, and the
absent-contract wire encoding.

The host replay tool (`native/balances/tools`) reduces directories of captured
`<height>.pb` blocks, links consecutive block hashes, checks that each
account's `old_amount` equals its previously emitted amount whenever every
intervening block was replayed, and compares against saved oracles. Its
reports over the locally retained BSC captures are in
[`docs/evidence/`](docs/evidence/) and summarized in the
[persisted-effect matrix](docs/persisted-effects.md): 1,439 version-5 blocks,
126,180 rows and 86,564 cross-block continuity checks with zero mismatches,
plus the 82-row same-block RPC oracle. Those captures are the
ERC-20 campaign's block cache, not a native-specific RPC audit: only block
122260950 has a same-block native RPC oracle. The historical 192-block,
37,086-check native audit ([legacy qualification](../../erc20/balances/docs/legacy-qualification.md))
applies to the removed prototype package, not to this one.

## Native sink

The [native ClickHouse sink](../../erc20/balances/clickhouse/README.md) derives
one `Balance` table from `Events.balances`. Its mapping stores optional
`contract` as `String`, so an absent native contract and an empty ERC-20
contract are not distinguishable in SQL. Consume native and ERC-20 packages
into separate databases or tables, or bind rows by package identity, before
treating `contract = ''` as native. The sink's `_blocks_` markers list blocks
with nonempty output only; completeness requires the stream's clock or cursor.
This has not been exercised for this package; it remains paused live work.

## Boundaries

No RPC, no candidate discovery, no transfer events, no supply or burn
interpretation, no wallet labels, no `db_out`, no custom sink, no global holder
enumeration. Initialization of untouched accounts, exact checkpoints, reorg
completeness and the complete-block clock contract are shared under
[#7](https://github.com/pinax-network/substreams-evm-extended/issues/7).
Ethereum, Base, HyperEVM and Arc semantics are separate qualification work
under [#8](https://github.com/pinax-network/substreams-evm-extended/issues/8).
`erc20/balances` is unchanged.
