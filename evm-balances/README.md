# evm-balances (legacy `db_out` patch for BSC)

A temporary, non-breaking patch of substreams-evm
[`evm-balances` v0.3.4](https://github.com/pinax-network/substreams-evm/tree/cb8607f59a37aa9daf85547162b8871d12568282/db-evm-balances)
for the existing legacy `substreams-sink-sql` ClickHouse deployment. It keeps
the same `db_out`, the same tables and the same ClickHouse schema. Only the
native input changes:

| `db_out` input | substreams-evm v0.3.4 | This patch |
| --- | --- | --- |
| `native_balances:map_events` | `evm-native-balances-v0.3.4` (`eth_getBalance` on candidate accounts) | [`native/balances`](../native/balances/README.md): RPC-free, Extended blocks, live-qualified on BSC |
| `erc20_balances:map_events` | `erc20-balances-v0.3.4` (RPC) | the same package, byte-identical (`spkg/erc20-balances-v0.3.4.spkg`) |

A [no-RPC variant](#no-rpc-variant) keeps the same `db_out` and also takes
ERC-20 balances from Extended blocks. It covers only configured tokens.

This is the only `db_out` and database-changes module in the workspace. It
exists until the next release, when a solution without `db_out` replaces it
([native sink](../erc20/balances/clickhouse/README.md)). Do not add features
here; keep it 1:1 with upstream.

## Provenance

Upstream is `pinax-network/substreams-evm@cb8607f59a37aa9daf85547162b8871d12568282`,
the commit that built `evm-balances-v0.3.4.spkg` and
`evm-clickhouse-balances-v0.3.4.spkg`. Its sources equal current upstream
`main` (`333e8c8`), apart from paths in the Makefiles and manifests.

| Here | Upstream | Status |
| --- | --- | --- |
| `src/lib.rs`, `src/native_balances.rs`, `src/erc20_balances.rs` | `db-evm-balances/src/` | verbatim |
| `common/src/lib.rs` | `common/src/lib.rs`, lines 11–81 | verbatim subset at the same line numbers (encoding helpers only) |
| `clickhouse/schema.*.sql` (6 files) | `db-evm-balances-clickhouse/` | verbatim |
| `clickhouse/examples/refresh-*.sql` | `db-evm-balances-clickhouse/examples/` | verbatim |
| `Cargo.toml` | `db-evm-balances/Cargo.toml` | same crate name and dependencies (`substreams` 0.7.6, `substreams-database-change` 3.0.0, `prost` 0.13.5 in the lockfile); `common` is the vendored subset; one Clippy lint allowed for the SDK macro |
| `substreams.yaml` | `db-evm-balances/substreams.yaml` | same `db_out`, inputs and `db_out` params; native import replaced; `network: bsc`; the native map's params are explicit |
| `clickhouse/substreams.yaml` | `db-evm-balances-clickhouse/substreams.yaml` | same sink config; `network: bsc` |
| `Makefile`, `clickhouse/Makefile` | same files | same targets with BSC defaults; the Tron target is dropped |

The PostgreSQL variant, the Ethereum ERC-20 `verify-balances.sh` spot check
and the package image are not ported. `clickhouse/examples/backfill-from-rpc-database.sql`
is new (see [cutover](#cutover)).

## Artifacts

Deploy `spkg/evm-clickhouse-balances-v0.3.4-extended.spkg`; it embeds
`spkg/evm-balances-v0.3.4-extended.spkg`. SPKG digests change whenever this
README changes, because `substreams pack` embeds it. Module and WASM hashes
identify the code.

| Module | This patch | Deployed v0.3.4 |
| --- | --- | --- |
| `db_out` | module `dc856c85…`, WASM `555aa847…` | module `f9e5b995…`, WASM `73872799…` |
| `db:native_balances:map_events` | module `5a2a2e0c…`, WASM `48d89d28…` (`spkg/native-balances-v0.1.0.spkg`, the qualified `fbb46fc7…`) | module `c09c9163…` plus `map_balance_changes` (RPC) |
| `db:erc20_balances:*` (4 modules) | `eec800ee…`, `996b7124…`, `7d6f427f…`, `fe4d1f2d…` | identical |
| SQL sink schema | MD5 `024d2130…`, engine `clickhouse` | identical |

`db_out` gets a new module hash because its binary and its native input
changed; its code did not. The reference package is preserved as
`spkg/reference/evm-clickhouse-balances-v0.3.4.spkg` (git blob `b44fe61e…`,
equal to upstream's).

## Verification

`evm-balances-tools` runs both packaged `db_out` WASM binaries in an
interpreter that implements the four Substreams host imports (`output`,
`println`, `register_panic`, `skip_empty_output`). It requires identical
outputs, logs and panics for the same inputs, and checks the package wiring:
`db_out` inputs, sink module and engine, schema bytes, unchanged ERC-20
modules, RPC host imports confined to those modules, and the qualified
native WASM and params.

```sh
cargo test --locked -p evm-balances-tools             # CI: committed artifacts
make -C evm-balances build compare OUTPUT=evm-balances/out/<fresh dir>
# The dated report adds the local qualification recordings and every fixture:
target/release/evm-balances-tools --built-wasm target/wasm32-unknown-unknown/release/db_evm_balances.wasm \
  --native-events native/balances/out/live-2026-09-23/events/saved-122288006.jsonl \
  --native-events native/balances/out/live-2026-09-23/events/live-123548070.jsonl \
  --blocks erc20/balances/tests/fixtures --output evm-balances/out/<fresh dir>
```

[2026-10-02 report](docs/evidence/compare-2026-10-02.json): 16/16 artifact
checks pass, and the locally built `db_out` equals the packaged one. Outputs
are byte-identical on:

| Inputs | Blocks | Native rows | ERC-20 rows |
| --- | --- | --- | --- |
| Committed block 122260950 (native map output plus the saved RPC ERC-20 rows) | 1 | 82 | 18 |
| Synthetic: genesis clocks, Tron encoding, invalid params, missing timestamp, rows each table skips | 14 | 13 | 6 |
| Packaged native output of the saved control blocks 122,288,006–122,289,029 (recording `7f5a38ce…`) | 1,024 | 76,139 | 0 |
| Packaged native output of the live window 123,548,070–123,553,069 (recording `8d0f8743…`) | 4,999 | 670,873 | 0 |
| Committed fixture blocks the native map accepts | 95 | 926 | 0 |

The two recordings are the ones checked against `eth_getBalance` in the
[native qualification](../native/balances/README.md#live-qualification-bsc-2026-09-23).
They carry no clock, so both binaries receive the same synthetic clock. The
359 other committed `.pb` files are transaction traces or trimmed
single-transaction blocks that the native map refuses; the report lists them.

These checks don't run the Substreams engine, the RPC ERC-20 maps or the SQL
sink. The patch has not been streamed or sunk live.

## Deploy with the legacy SQL sink

```sh
export DSN='clickhouse://<user>:<password>@<host>:9000/<database>'
substreams-sink-sql setup "$DSN" spkg/evm-clickhouse-balances-v0.3.4-extended.spkg
substreams-sink-sql run "$DSN" spkg/evm-clickhouse-balances-v0.3.4-extended.spkg \
  -e bsc.substreams.pinax.network:443 <start>: <the current production sink flags>
```

The [no-RPC variant](#no-rpc-variant) deploys the same way with
`spkg/evm-clickhouse-balances-v0.3.4-extended-norpc.spkg`.

`make -C evm-balances/clickhouse setup|dev` repacks first and then runs the
upstream development targets against `DSN`.

## Cutover

Each block must come from exactly one pipeline. Pick the boundary from the
RPC-era sink's cursor: stop it, read its last block `S-1` from the `cursors`
table, and start this package at `S`.

- **Same database, no copy.** Run this package against the existing database
  with `--on-module-hash-mistmatch=warn`. The legacy sink then resumes from
  the cursor at the highest block when the module hash differs
  ([`db/cursor.go`](https://github.com/streamingfast/substreams-sink-sql/blob/69fa9da10ec3cad27c5983babc7095e84e435cb8/db/cursor.go)).
  Tables and history continue in place.
- **New database, then backfill.** Start this package at `S` in a fresh
  database. Then copy the RPC-era history with
  [`examples/backfill-from-rpc-database.sql`](clickhouse/examples/backfill-from-rpc-database.sql).
  It uses partition copies, because `INSERT … SELECT` into a balance table
  re-fires `mv_historical_*` and double-counts `transactions`.
  `AggregatingMergeTree` merges the windows that span `S`.

  Checked in clickhouse-local 25.8.1 with the packaged schema, on synthetic
  rows split at a cutover block:
  - native and ERC-20 latest balances and every OHLC bucket equal a
    single-pipeline baseline, including 35 native and 75 ERC-20 buckets
    spanning the boundary;
  - `INSERT … SELECT` inflates `transactions` instead.

  If the RPC-era database is on another server, load it into same-schema
  staging tables without the views, then attach from those.

## What changes after the cutover

- **Native row set.** Native rows come from every account whose native balance
  persistently changed in the block. That includes internal-call recipients,
  fee recipients and system contracts. The RPC module emitted its candidate
  accounts, including unchanged ones.
  - `native_balances` holds the exact balance for every account changed since `S`.
  - Accounts untouched since `S` keep their RPC-era value.
  - `transactions` in `historical_native_balances` counts blocks with a
    persisted change.
- **Fail-closed.** An unqualified `Block.ver`, a missing Extended block or
  ambiguous producer data makes the native map error, and the sink stops
  rather than writing guessed rows ([rules](../native/balances/README.md#persistence-and-fail-closed-rules)).
  - The start block must be a producer-version-5 block. Saved data shows version 4 up
    to 104,975,334 and version 5 from 119,571,736 at the latest.
  - A new producer version needs qualification before
    `native_balances:map_events` params can list it.
- **ERC-20 rows** are unchanged: same modules, same hashes.
- **BSC only.** The native qualification covers BSC version 5. Other networks
  stay under [#8](https://github.com/pinax-network/substreams-evm-extended/issues/8).

## No-RPC variant

`substreams.norpc.yaml` and `clickhouse/substreams.norpc.yaml` package the
same `db_out` binary with both inputs RPC-free. No module in
`spkg/evm-clickhouse-balances-v0.3.4-extended-norpc.spkg` imports an RPC host
function; `evm-balances-tools` checks every binary's imports.

| `db_out` input | No-RPC variant |
| --- | --- |
| `native_balances:map_events` | the same qualified native map |
| `erc20_balances:map_events` | the storage-layout map that ran on live BSC on 2026-09-23 (`spkg/erc20-balances-v0.1.0.spkg`, `532b571f…`, WASM `005a2d3d…`) |

The ERC-20 map's parameters embed the 425 live-run profiles: module
`c8252ee7…`, layouts in `erc20/balances/tests/fixtures/bsc-live425-layouts.json`
(`f8f0fa8f…`). That is the 431-profile qualified set minus the six excluded
in [#61](https://github.com/pinax-network/substreams-evm-extended/issues/61).
The layouts are embedded rather than passed with `-p`, because a forgotten
`-p` would fall back to the default `[]` and silently write no ERC-20 rows.

```sh
make -C evm-balances pack-norpc && make -C evm-balances/clickhouse pack-norpc
```

### Coverage: most rows, a minority of tokens

ERC-20 rows come only from configured tokens. These are the 425 deployed
profiles' shares of the RPC reference stream's ERC-20 activity, measured by
`evm-balances-tools --rpc-reference` ([report](docs/evidence/norpc-2026-10-02.json)):

| RPC reference window | Rows | (token, holder) pairs | Active tokens |
| --- | --- | --- | --- |
| 122,288,006–122,289,029: the window the profiles were ranked and chosen from | 93.2% (177,658 of 190,651) | 87.2% (61,318 of 70,303) | 25.1% (425 of 1,696) |
| 123,561,000–123,562,023: eight days later ([live run](../erc20/balances/docs/live-package-bsc-2026-09-23.md)) | 69.9% (376,418 of 538,758) | 52.7% (88,534 of 167,959) | 9.5% (324 of 3,428) |

The second window is the out-of-sample figure. BSC's active token set
rotates, so coverage of a fixed list decays, and tokens launched after the
list are absent until qualified. In the backfill plan:
- tokens outside the list keep their RPC-era `erc20_balances` value, which
  goes stale without any marker;
- configured tokens emit a row only when holder storage changed (253,503 of
  the 376,418 configured RPC rows in the live window). Latest values are
  unaffected, but `transactions` in `historical_erc20_balances` counts
  fewer rows than the RPC package did.

### End-to-end replay

`evm-balances-tools --pipeline-blocks` runs the deploy package itself.
The packaged native and ERC-20 WASM maps produce the `db_out` inputs, and
both `db_out` binaries run on them, block by block, as the engine chains
them. [2026-10-02 report](docs/evidence/norpc-2026-10-02.json): 16/16
artifact checks pass, including no RPC import in any binary.

The replay covers the 1,024 Extended blocks 123,561,000–123,562,023, which
were fetched from Firehose for the ERC-20 live run:
- **No refusals.** That's with the 425 profiles; with all 431, the stream
  halts at block 123,561,001.
- **ERC-20:** 253,503 rows across 320 tokens. That's exactly the live run's
  output, all of which equalled `balanceOf` at its block hash. The packaged
  map's output equals the engine's recorded stream (`79ccea19…`) on all
  1,024 blocks.
- **Native:** 168,199 rows. The packaged map equals its host build on every
  block. This window was not checked against `eth_getBalance`.
- **`db_out`:** identical between the deployed upstream binary and the port
  on all 1,024 blocks, giving 1,024 `blocks`, 253,503 `erc20_balances` and
  168,199 `native_balances` row changes.

The committed full block and 95 fixture blocks (4,951 Extended ERC-20 rows)
also give identical `db_out` outcomes. CI replays the committed full block
end to end. Nothing was streamed or sunk live.

```sh
target/release/evm-balances-tools --candidate spkg/evm-clickhouse-balances-v0.3.4-extended-norpc.spkg \
  --erc20-layouts erc20/balances/tests/fixtures/bsc-live425-layouts.json \
  --pipeline-blocks erc20/balances/out/live-2026-09-23/firehose-blocks \
  --engine-erc20-events erc20/balances/out/live-2026-09-23/compare/events.jsonl \
  --rpc-reference erc20/balances/out/ranks201-250-ranking/reference.jsonl \
  --rpc-reference erc20/balances/out/live-2026-09-23/compare/reference.jsonl \
  --blocks erc20/balances/tests/fixtures --output evm-balances/out/<fresh dir>
```

### Halts

The storage map fails closed per block. Any of these on a configured token
refuses the block:
- an unreviewed storage write;
- a runtime or dependency change;
- a changed balance divisor.

Both maps feed one `db_out`, so a refusal also stops native balances. Six
of the 431 qualified profiles broke within about eight days; with all 431,
the stream halts at block 123,561,001. Expect more refusals over time.

Each refusal needs the token reviewed or removed. Either way the map's
parameters change, so the module hashes change too; resume the existing
database with `--on-module-hash-mistmatch=warn`.

