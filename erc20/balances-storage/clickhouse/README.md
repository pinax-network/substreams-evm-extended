# Native ClickHouse sink

Substreams CLI **1.22.0** can sink the existing `map_events` output directly.
The canonical `evm.balances.v1.Events` protobuf is unchanged. There is no
`db_out`, custom database writer, schema override, or additional map module.

The CLI derives one `Balance` table from each element of `Events.balances`.
The scalar-free `Events` wrapper does not become a table. With
`--bytes-encoding 0xhex`, `contract` and `address` are hexadecimal strings;
`amount` remains an exact decimal string. The native schema adds block number,
timestamp, version, deletion marker, and a deterministic per-block `_row_id_`.
Its `ReplacingMergeTree` key is `(_block_number_, _row_id_)`, so multiple holders
in the same block remain separate and identical replays deduplicate with `FINAL`.
No `schema.sql` or `sink:` manifest section is required when `map_events` is
passed explicitly.

From this directory, with the package already built:

```sh
export SUBSTREAMS_SINK_DSN='clickhouse://<user>:<password>@127.0.0.1:9000/<new-database>'
export SUBSTREAMS_API_KEY=... # or use the existing authenticated environment
make setup
make run
```

These defaults select the two reviewed bridge450 tokens and finalized blocks
122288006–122288149. `LAYOUTS`, `START`, `STOP`, `ENDPOINT`, `STATE`, and
`SUBSTREAMS` are configurable. Keep the complete state directory: native schema
metadata, cursor, and spool are local durable state. Use a new database and state
directory when changing package, parameters, chain, or bytes encoding. Do not
point this example at existing application tables.

The cursor can point to the last emitted block before the requested stop: the
smoke run stopping at 122288020 stored a cursor for 122288010. A cursor or a
successful bounded run alone is not evidence of every intervening empty block.

Read deduplicated history with:

```sql
SELECT _block_number_, contract, address, amount
FROM Balance FINAL
WHERE NOT _deleted_
ORDER BY _block_number_, _row_id_;
```

For the latest *observed* amount, first deduplicate history and then select by
greatest `_block_number_` for each `(contract, address)`. The native `_version_`
is ingestion time, not blockchain order. Keep zero balances; filtering zeros
before selecting the latest record resurrects an older nonzero value.

The native sink's `_blocks_` table contains nonempty-output block markers, not a
complete clock history or an atomic publication guarantee. ClickHouse writes
are not multi-table transactions. The protobuf has no block hash; bind the
native marker hashes to independently captured canonical clocks when auditing.
This sink also does not initialize balances for holders without an observed
storage write.

The native ClickHouse mapping uses `String` for optional `contract`; it does
not retain absent-versus-empty protobuf presence as SQL NULL. This ERC-20 module
always emits a contract. Native-coin balances with absent contracts need separate
qualification before claiming their SQL representation is lossless.

## Bounded smoke validation

With an existing local server on native port 9000 and HTTP port 8123:

```sh
make smoke
```

The Rust example invokes the native CLI for every write. Its HTTP requests are
read-only. It creates only a fresh `erc20_storage_smoke_*` database, verifies the
generated schema, ingests a short first window, resumes from the same cursor,
and replays the identical full window using a separate state directory. It
checks seven independently captured RPC expectations, including several holders
in one block, zeros, and amounts larger than uint64. Previous row versions must
remain unchanged during resume; after replay, `FINAL` must return exactly the
same seven balances. Failed attempts, schema, rows, logs and cursor hashes stay
in the requested output directory. No database is automatically dropped.

`CH_USER` and `CH_PASSWORD` optionally select local credentials. `SUBSTREAMS`
can point to a checksummed release binary. The smoke output directory must be
fresh; set `SMOKE_OUTPUT` for another attempt. The check is deliberately fixed to
the reviewed 144-block sample; it is not a production streaming command.
Alternate fixture paths must contain the exact reviewed layouts and RPC
expectations; their hashes are checked before starting the CLI or contacting
ClickHouse. Replay versions are compared numerically as Int64 values.

The [captured local result](evidence/report.json) used CLI 1.22.0 and ClickHouse
25.8.1.3064. All seven balances matched, including two zeros and a 22-digit amount.
Resume preserved the earlier row versions. An identical replay produced 14
physical rows and seven deduplicated rows. The [schema](evidence/schema.json),
[first window](evidence/first-rows.json), [resumed rows](evidence/resumed-rows.json),
[replayed rows](evidence/replayed-rows.json), and [block markers](evidence/blocks.json)
are retained as a bounded integration result. This does not claim behavior for
unobserved tokens or production interruption/recovery scenarios.

Upstream references:

- [Substreams 1.22.0](https://github.com/streamingfast/substreams/releases/tag/v1.22.0)
- [Native protobuf mappings and ClickHouse defaults](https://docs.substreams.dev/reference-material/sql/sql/proto-annotations)
- [Relational mappings](https://docs.substreams.dev/how-to-guides/sinks/sql/relational-mappings)
