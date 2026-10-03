# Native balances in ClickHouse

`spkg/native-balances-v0.2.0.spkg` loads native balances into the legacy
substreams-evm native tables in two ways. Both paths use the same package and
the same table DDL, so they can run side by side, including in production, and
be compared ([examples/compare-paths.sql](examples/compare-paths.sql)). ERC-20
balances are a separate implementation and are not part of this package.

| Path | Sink | Module | Writes |
| --- | --- | --- | --- |
| **Native sink** | `substreams sink clickhouse` (Substreams CLI, relational mappings) | `map_events` | `Balance`, `_blocks_`. Materialized views feed the legacy tables; a TTL expires `Balance` |
| **Legacy sink** | `substreams-sink-sql` (`DatabaseChanges`) | `db_out` | the legacy tables directly |

```
native sink:  Balance  ──mv_sink_native_balances──▶ native_balances ──mv_historical_native_balances──▶ historical_native_balances_state ──▶ historical_native_balances
              _blocks_ ──mv_sink_blocks───────────▶ blocks
legacy sink:  db_out ──▶ native_balances ──mv_historical_native_balances──▶ historical_native_balances_state ──▶ historical_native_balances
                     ──▶ blocks
```

## Package

| Module | Binary | Identity |
| --- | --- | --- |
| `map_events` | `native_balances.wasm` | the live-qualified BSC map: WASM `48d89d28…`, module `5a2a2e0c…`, params `{"producer_versions":[5]}`. It is unchanged from `spkg/native-balances-v0.1.0.spkg` (`fbb46fc7…`) |
| `db_out` | `native_balances_db_out.wasm` | module `960dbd06…`, params `hex`. A separate binary keeps `map_events` byte-identical |

`db_out` is substreams-evm `evm-balances` v0.3.4's `db_out` (`cb8607f`) with
its ERC-20 input removed:
- `native_balances.rs`, `set_clock`, `update_genesis_clock` and the address
  encoding helpers are verbatim;
- it writes one `native_balances` row per account (key `address`) and a
  `blocks` row when there is at least one.

The package's SQL sink config embeds the four legacy schema files below,
concatenated as upstream's `make schema` does. `make -C native/balances pack`
regenerates `schema.sql`.

| File | Creates | Source | Native sink | Legacy sink |
| --- | --- | --- | --- | --- |
| `schema.0.blocks.sql` | `blocks` | verbatim upstream | ✓ | ✓ (embedded) |
| `schema.1.table.native-balances.sql` | `native_balances` | verbatim upstream: lines 29–51 of `schema.1.table.balances.sql` | ✓ | ✓ |
| `schema.2.mv.historical-native-balances.sql` | `historical_native_balances_state`, `mv_historical_native_balances` | verbatim upstream | ✓ | ✓ |
| `schema.3.view.historical-native-balances.sql` | `historical_native_balances` | verbatim upstream | ✓ | ✓ |
| `schema.4.mv.sink-bridge.sql` | `mv_sink_native_balances`, `mv_sink_blocks` | new | ✓ | |
| `schema.5.ttl.sql` | TTL on `Balance` | new | ✓ | |

Upstream is `pinax-network/substreams-evm@cb8607f` (`db-evm-balances-clickhouse`,
the evm-balances v0.3.4 build). A unit test pins the four copied files' SHA-256.

## Deploy

`SUBSTREAMS_SINK_DSN` (`clickhouse://<user>:<password>@<host>:9000/<database>`)
selects the database; use one database per path. The Makefile never prints it.

```sh
export SUBSTREAMS_API_KEY=...

# Native sink
export SUBSTREAMS_SINK_DSN='clickhouse://…/bsc_native_sink'
make -C native/balances/clickhouse setup TTL_DAYS=7      # sink tables, then schema.0–5
make -C native/balances/clickhouse run START=<block>     # --final-blocks-only --bytes-encoding 0xhex

# Legacy sink
export SUBSTREAMS_SINK_DSN='clickhouse://…/bsc_native_legacy'
make -C native/balances/clickhouse legacy-setup          # creates the database, then substreams-sink-sql setup
make -C native/balances/clickhouse legacy-run START=<block>   # --final-blocks-only --on-module-hash-mistmatch=warn
```

- **Order.** The native `setup` runs the sink's own setup, then applies
  `schema.*.sql` in order. The views must exist before the sink's first
  insert, because a materialized view sees only rows inserted after it exists.
- **Re-running setup is safe.** The CLI sink's tables are `CREATE … IF NOT
  EXISTS`, and its only compatibility check concerns the `_row_id_` column.
  `substreams-sink-sql setup` also re-runs cleanly.
- **The legacy sink needs the database to exist.** `legacy-setup` creates it
  through the server's `default` database, keeping any `?options` of the DSN.
- **Final blocks only.** Both run targets pass `--final-blocks-only`. An undo
  in the native sink writes tombstone rows; the views drop them but cannot
  retract values already copied.
- **Start block.** `START` must be a producer-version-5 block. Any other
  `Block.ver`, a missing Extended block or ambiguous producer data stops the
  stream instead of writing guessed rows
  ([rules](../README.md#persistence-and-fail-closed-rules)).
- **Taking over a database.** The legacy sink can continue an RPC-era
  evm-balances database from its cursor with `--on-module-hash-mistmatch=warn`.
  That database's ERC-20 tables then stop updating.
- **Local state.** The native sink keeps its schema hash, cursor and spool in
  `localdata/`; keep it with the database.
- **Empty password.** A DSN with an empty password (`user:@host`) makes
  `clickhouse client` print a password prompt and continue.

## Base-data TTL (native sink)

`Balance` holds one row per changed account per block, about 48.5 compressed
bytes per row. The sampled BSC windows averaged 134–164 rows per block, which
is about 1.25–1.5 GB a day. Each row is copied into `native_balances` and the
aggregation state when it is inserted, so the base table only serves audits
and rebuilds.

`schema.5.ttl.sql` sets `TTL _block_timestamp_ + INTERVAL {ttl_days} DAY`
(`TTL_DAYS`, default 7, about 9–11 GB). Retention is by block time:
- **Expiry.** Rows expire at background merges, and the views never see
  those deletions.
- **Backfills.** Rows already past the TTL when inserted are discarded at
  insert, though the views still receive them.
- **No TTL on `_blocks_`.** It is small (about 14 MB a day), the bridge reads
  it for block hashes, and a TTL merge between the sink's `_blocks_` and
  `Balance` inserts of an old block would lose that block's hash.
- **Changing retention.** Run `ALTER TABLE Balance MODIFY TTL …` with a new
  interval at any time.

## Tables after the cutover

- **`native_balances`.** `ReplacingMergeTree(block_num) ORDER BY address`:
  the latest observed balance per account. Read it with `FINAL`. Zero balances
  are kept.
- **`historical_native_balances`.** OHLC bars per account at 1 minute to
  1 week. `transactions` counts blocks in which the account's native balance
  persistently changed.
- **Rows.** Rows come from every account whose balance changed in a block:
  transfers, internal calls, gas, fee rewards and system accounts. The RPC-era
  module emitted candidate accounts, including unchanged ones, so counts
  differ from RPC-era history. An account untouched since the cutover keeps its
  RPC-era value.
- **Replays.** A replayed flush, after a crash between insert and cursor
  write, inflates `transactions` for those windows. Latest values and OHLC
  values are unaffected.
- **No chain id.** Neither path stores one. Use one database per network.

## Comparing the two paths

[`examples/compare-paths.sql`](examples/compare-paths.sql) takes both database
names and a block range that both have passed, and counts differences in:
- `blocks`;
- the OHLC bars of windows inside the range, so the databases may be at
  different heads;
- latest balances, which is only meaningful when both stopped at the same
  block.

Run on the two databases of the live-window check, it reports 0 differences,
both for the full range and for a sub-range. One changed balance gives 1 latest
and 2 bar differences.

## Backfill from the RPC-era database

Start a path at block `S` in a fresh database, where the RPC-era sink's last
block is `S-1`. Then copy history with
[`examples/backfill-from-rpc-database.sql`](examples/backfill-from-rpc-database.sql).

It uses partition copies, because `INSERT … SELECT` into `native_balances`
re-fires `mv_historical_native_balances` and double-counts `transactions`.
`AggregatingMergeTree` merges the windows that span `S`.

This was checked in clickhouse-local 25.8.1, with an RPC-era database created
from upstream's full schema and synthetic rows split at a cutover block. After
the copy, latest balances and all 973 aggregate buckets equal a single-pipeline
baseline, including 35 that span the boundary.

## Verification

**`db_out` parity** ([report](evidence/db-out-parity.json)).
`native-balances-tools db-out-parity` runs the deployed
`evm-clickhouse-balances-v0.3.4.spkg` `db_out` WASM with an empty ERC-20
input, and this package's `db_out` WASM, on the same inputs. All 12 package
checks pass, including:
- the qualified `map_events`;
- `db_out` inputs;
- the sink module, engine and verbatim schema;
- no RPC imports;
- the locally built `db_out` equals the packaged one.

Outputs, logs and panic messages are identical on:

| Inputs | Cases | `native_balances` rows |
| --- | --- | --- |
| Committed block 122260950 | 1 | 82 |
| Synthetic: genesis clocks, Tron encoding, invalid params, missing timestamp; 3 identical panics | 14 | 13 |
| Native recording of the saved controls (`7f5a38ce…`, synthetic clocks) | 1,024 | 76,139 |
| Native recording of the live window 123,548,070–123,553,069 (`8d0f8743…`, synthetic clocks) | 4,999 | 670,873 |
| Saved-control blocks 122,288,006–122,289,029 | 1,024 | 76,139 |
| Live-window blocks 123,561,000–123,562,023 | 1,024 | 168,199 |

Fields are compared by name. `substreams-database-change` orders a row's
columns by `HashMap`, so the two binaries serialize the same changes in a
different column order, and the SQL sink maps columns by name. Change order
(ordinals) is identical.

**Real setups** ([native sink](evidence/setup-native-sink-tables.jsonl),
[legacy sink](evidence/setup-legacy-sink-tables.jsonl)). These ran on a local
server with Substreams CLI `be35ad3`, `substreams-sink-sql` `69fa9da` and
ClickHouse 25.8.1.3064:
- both setups created their tables from the same package;
- the five shared legacy objects have identical DDL in both databases;
- re-running either setup succeeded, and the native database kept its views
  and TTL.

**Both paths, offline** (`native-balances-tools clickhouse-bridge`). It
writes the native map's output the way the CLI sink does: `_blocks_` before
`Balance` in 100-block flushes, markers only for blocks with output. The
schema is the captured sink DDL plus the committed `schema.*.sql`, run in
`clickhouse local`. It also loads `db_out`'s rows into a second database
created from the package's legacy schema. It then compares both databases'
`native_balances`, `historical_native_balances` and `blocks` with values
computed in Rust and with each other, forces the TTL merge, and requires the
legacy tables to be unchanged:

| Report | Blocks | Native rows / accounts | Latest | Bars | Both paths | Base rows kept |
| --- | --- | --- | --- | --- | --- | --- |
| [saved controls, TTL 7](evidence/bridge-saved-controls-ttl7.json) | 122,288,006–122,289,029 | 76,139 / 17,670 | 17,670 equal | 164,379 equal | equal | 0 (older than 7 days) |
| [saved controls, TTL 3650](evidence/bridge-saved-controls-ttl3650.json) | same | same | 17,670 equal | 164,379 equal | equal | 76,139 |
| [live window, TTL 7](evidence/bridge-live-window-ttl7.json) | 123,561,000–123,562,023 | 168,199 / 35,092 | 35,092 equal | 350,250 equal | equal | 0 |

The saved-control blocks are the native qualification window, where all 76,139
rows equalled `eth_getBalance` at their block hash.

```sh
cargo run --locked -p native-balances-tools -- db-out-parity --output native/balances/out/<fresh dir>
cargo run --locked -p native-balances-tools -- clickhouse-bridge \
  --blocks <dir of <height>.pb> --ttl-days 7 --output native/balances/out/<fresh dir>
```

These checks write rows themselves rather than running either sink, and do
not stream. A bounded live run of both paths into fresh databases, followed by
`compare-paths.sql`, is the remaining step.
