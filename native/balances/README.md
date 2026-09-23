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
Only versions 4 and 5 may be listed (a non-empty subset); version 3, whose
system-call ordinals are broken, is refused at parse time.

## Build and test

```sh
cargo test --locked -p native-balances -p native-balances-tools -p evm-persist
make -C native/balances build      # release WASM only; no network
make -C native/balances replay     # offline replay over captured blocks
```

`make pack` and `make run` read `SUBSTREAMS_API_KEY` from the environment;
live parity reads `RPC_URL`. No SPKG of this package is committed; the
qualified build is identified by its hash below.

## Live qualification (BSC, 2026-09-23)

The packed map (`spkg` sha256 `fbb46fc7…`, wasm `48d89d28…`, default
parameters `{"producer_versions":[5]}`) was streamed with
`--final-blocks-only` from `bsc.substreams.pinax.network` and checked with
`native-balances-tools live-parity`, which binds each block number to its
canonical hash and batches `eth_getBalance(address, {blockHash})`:

- **Saved controls** ([report](docs/evidence/live-parity-bsc-2026-09-23-saved-controls.json)):
  the 1,024 contiguous saved blocks 122,288,006–122,289,029. The packaged
  rows equal the offline Rust replay of the saved `.pb` files row for row
  (76,139 rows, 17,670 accounts, 6,366 known zeros), and every row equals
  `eth_getBalance` at the block hash (76,139/76,139).
- **Live window** ([report](docs/evidence/live-parity-bsc-2026-09-23-live.json)):
  the 5,000 contiguous final blocks 123,548,070–123,553,069, streamed without
  a refused block: 670,873 rows for 102,913 accounts (45,162 known zeros).
  Every row of the last 1,000 blocks and every tenth row of the others was
  checked: 200,344/200,344 equal. Block 123,550,024 has no transaction and
  no output; the miner, fee and system accounts are unchanged across it.
- **Reasons** ([report](docs/evidence/live-reasons-bsc-2026-09-23.json),
  [matrix](docs/persisted-effects.md)): the last 250 blocks fetched from
  Firehose and replayed offline equal the packaged output and carry
  transfers, gas, fee rewards, 37 blob-fee rewards and 500 block-level fee
  resets.

```sh
make -C native/balances pack   # or substreams pack into a fresh out/ dir
substreams run -e bsc.substreams.pinax.network:443 <spkg> map_events -s <start> -t <stop> \
  --final-blocks-only -o jsonl --bytes-encoding hex > events.jsonl
cargo run --locked -p native-balances-tools -- live-parity --events events.jsonl --spkg <spkg> \
  --endpoint bsc.substreams.pinax.network:443 --full-from <a> --full-to <b> --sample-every 10 --output <fresh dir>
```

**Retention from a checkpoint** ([report](docs/evidence/live-retention-checkpoint-bsc-2026-09-23.json)):
`native-balances-tools retention-check` seeds `eth_getBalance` for 14,172
accounts at block 123,548,069 into the `common/retention` ledger, applies the
next 300 packaged blocks and compares every retained value with RPC at four
block hashes: 56,688/56,688 equal, including 1,252 accounts that were never
emitted in the window and kept their checkpoint value.

Parity covers the emitted accounts only: an account without a persisted
change is not emitted and not checked, and an RPC candidate list (such as the
current RPC module's) would also contain unchanged accounts, which are not
state-derived updates. Other networks, producer versions and the Ethereum,
OP and Arc rows of the matrix stay under
[#8](https://github.com/pinax-network/substreams-evm-extended/issues/8).

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
This has not been exercised for this package.

## Boundaries

No RPC, no candidate discovery, no transfer events, no supply or burn
interpretation, no wallet labels, no `db_out`, no custom sink, no global holder
enumeration. Initialization of untouched accounts, exact checkpoints, reorg
completeness and the complete-block clock contract are shared under
[#7](https://github.com/pinax-network/substreams-evm-extended/issues/7).
Ethereum, Base, HyperEVM and Arc semantics are separate qualification work
under [#8](https://github.com/pinax-network/substreams-evm-extended/issues/8).
`erc20/balances` is unchanged.
