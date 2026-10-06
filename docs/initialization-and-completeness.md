# evm-balances cutover contract: initialization and completeness

Issue [#7](https://github.com/pinax-network/substreams-evm-extended/issues/7).
What an `evm-balances` database contains after the cutover from the RPC-era
sink, which values can be stale, what a replayed flush does, and when a
database is complete, with the checks and the repair that enforce it. The
host-ledger specification this page replaces is kept in
[history](history/initialization-and-completeness-host-ledger-2026-09-19-to-2026-10-06.md).

**Scope.** [`evm-balances`](../evm-balances/README.md) v0.6.0 (`db_out`
`2710d961…`, native `72453949…`, erc20 `d8a9db86…`, schema `ed1c3bff…`, the
upstream v0.3.4 tables), written by pinax-network/substreams-sink-sql v4.13.0,
commit `933a187`. None of these is live-qualified
([#124](https://github.com/pinax-network/substreams-evm-extended/issues/124)).
Unless marked live, every number below was measured offline on captured BSC
Extended blocks: window a is 122,288,006–122,289,029 and window b is 123,561,000–123,562,023
(1,024 blocks each). The methods are in the pull request that added this page.

## 1. One database per network

- No table stores a chain id and blocks carry none. `db_out` has no params and
  no `initialBlock`, so its module hash, which keys the sink cursor, is the
  same on every network. Write each network to its own database, named as
  [#117](https://github.com/pinax-network/substreams-evm-extended/issues/117)
  sets out (`<network>:evm-balances@<version>`).
- Before a sink starts or resumes, compare the hash of the first block it will
  process with `eth_getBlockByNumber` on that network's RPC, and do not start
  on a mismatch (#117). A wrong endpoint writes another chain's balances, and
  neither the module nor the tables can detect it.
- `cursors` holds one row per `db_out` module hash. A package whose `db_out`
  hash changes starts from its own start block; the default
  `--on-module-hash-mistmatch=error` stops a sink whose stored hash differs.
  Never copy `cursors` from another database.

## 2. Sink and flags

- **Sink.** v4.13.0, commit `933a187`, as the
  [README](../evm-balances/README.md#build-and-deploy) pins it.
- **`--final-blocks-only` is required.** The ClickHouse sink logs an UNDO and
  keeps the rows, so a reversible block would stay stored after a fork. With
  final blocks only no UNDO is sent, and the earlier "live `BlockUndoSignal`"
  item of this issue does not apply.
- **Flush interval: any.** Every block flushes on its own at interval 1 from
  the first block after a start, and at any interval ClickHouse receives one
  row per key and block, so the history states see every row. Windows a and b
  give the same FINAL keys, view bars and 1-minute `transactions` at flush
  interval 1 and 1000 (PR #142).
- **Raw rows.** ClickHouse's default `optimize_on_insert` can collapse a key
  repeated within one INSERT in the raw `ReplacingMergeTree` rows; the history
  states and FINAL are unaffected. Read balances with FINAL.

## 3. What the tables hold

| Table | A row means | No row means |
| --- | --- | --- |
| `native_balances` | the account's persisted balance changed in that block; the value is its end-of-block balance | no change in that block |
| `erc20_balances` | the inference proved the pair's end-of-block value in that block | unknown: the pair may have changed |
| `blocks` | the block wrote at least one balance row | no output, or not published (section 6 tells which) |
| `historical_*_balances_state` | the rows inserted in that window | no row inserted in that window |

- Absence is never a zero. A known zero is a stored row whose balance is 0.
- Unchanged holders are never filled or carried (owner, 2026-10-05).
- A pair's FINAL value is its latest stored row: a row emitted from the start
  block S on, or the RPC-era row that the backfill copied.
- Native absence as "no change" has live evidence for one interval and
  account set only (live, 2026-09-23): 14,172 BSC accounts seeded with `eth_getBalance` at
  123,548,069 and the package output of 123,548,070–123,548,369 applied gave
  56,688/56,688 values equal to `eth_getBalance` at four block hashes
  ([#58](https://github.com/pinax-network/substreams-evm-extended/pull/58),
  [report](../native/balances/docs/evidence/live-retention-checkpoint-bsc-2026-09-23.json)).
- A block that native refuses stops the stream: there is one `db_out`, so no
  ERC-20 row of that block or later is written either.

## 4. Stale pairs

- **ERC-20.** A pair keeps its last stored value, RPC-era or emitted, until the
  inference emits it again. Its balance can move without an inferred row for
  computed balances (reflection, rebasing, reward, scaled and constant-default
  tokens, and divisor tokens, where one write moves every holder), for
  event-only tokens without a persisted balance write, and for any contract
  whose rows a block dropped on doubt
  ([Limits](../erc20/balances/README.md#limits)).
- **Measured within an interval.** Against the rows the RPC reference package
  emitted, the pair's final value differed for 15 of 167,959 pairs in window b
  and 5 of 70,303 pairs in window a; scope: the pairs the reference emitted in
  that window. The FINAL comparison of section 8 reproduces both counts
  (9 same-block wrong + 6 stale suspects; 3 + 2). After the cutover the
  interval is unbounded: a stale pair stays stale until its next inferred row.
- **Native** emits every persisted balance change, so it does not go stale by
  design; it stops on a block it cannot resolve instead.
- **Row counts.** The RPC era re-emitted unchanged candidates, so expect fewer
  rows and lower `transactions` after the cutover.
- **Policy** (detection, deny list, Token API presentation) belongs to
  [#123](https://github.com/pinax-network/substreams-evm-extended/issues/123).

## 5. Replays

How the sink stores a flush (v4.13.0):

- Each table is one INSERT, in the order the tables first got rows after the
  start: `erc20_balances`, `native_balances`, then `blocks` when the first
  block has all three. The cursor follows.
- The cursor is an asynchronous insert that the sink does not wait for
  (`clickhouse.WithStdAsync(false)` in `db/flush.go`). A cursor row that
  ClickHouse rejects or loses is not reported to the sink.
- A failed INSERT fails the flush attempt. The sink makes up to 3 attempts,
  5 s apart (`db/db.go`), each re-inserting every table, including those that
  an earlier attempt had stored. A restart resumes after the stored cursor.

So rows are stored more than once when a flush's tables are stored and its cursor is
not (a restart re-sends every such flush, possibly several), or when a table
fails after earlier tables of the same flush were stored (the retry or the
restart re-inserts them). The effects:

- **`blocks`** gets one more row per replayed block, but only when `blocks`
  itself was re-inserted: a duplicate proves a replay, its absence does not
  exclude one.
- **`transactions`** grows by exactly the replayed rows, in every window of
  every interval that contains them.
- **FINAL balances, open, close, high, low** and the window block bounds do not
  change: the replayed rows are identical.

Measured on window a with the sink's own flush code (rows: ERC-20 / native,
equal in all 8 intervals):

| Replay | Replayed blocks | Extra `blocks` rows | Extra `transactions` |
| --- | --- | --- | --- |
| interval 1; the cursor row of block 122,288,600 refused, process stopped, restarted | 122,288,592–122,288,600 (the refused row failed its asynchronous batch of 9) | 9 | 786 / 643 |
| interval 100; the cursor row of the flush ending at 122,288,505 refused, process stopped, restarted | 122,288,406–122,288,505 | 100 | 11,801 / 7,759 |
| interval 100; that flush's `blocks` INSERT failed, one attempt, restarted | 122,288,406–122,288,505 | 0 | 11,801 / 7,759 |
| interval 100; that INSERT failed once, the sink's retry succeeded | 122,288,406–122,288,505 | 0 | 11,801 / 7,759 |

The same replay on window b (blocks 123,561,400–123,561,499) gave 100 extra
`blocks` rows and 33,780 / 16,959 extra rows. In every case FINAL and every bar
apart from `transactions` equal an un-replayed load.

- **Detect** with
  [`check-replays-and-completeness.sql`](../evm-balances/clickhouse/examples/check-replays-and-completeness.sql):
  duplicate `blocks` rows, and `transactions` per 1-minute window against the
  rows the stream emitted. It found exactly the cases above.
- **Repair** with
  [`repair-replayed-range.sql`](../evm-balances/clickhouse/examples/repair-replayed-range.sql):
  stop the sink, re-stream a range from at least one minute before to one
  minute after the replay into a staging database, and the file subtracts the replayed
  rows from `transactions` and keeps one `blocks` row per block. It refuses
  any difference other than repeated rows. After it, all five databases above
  equal an un-replayed load: `blocks` rows with their copies, FINAL, and every
  bar of all 8 intervals.
- **Or accept** `transactions` as approximate for the range: record the range
  and its excess rows, and remove the duplicate `blocks` rows with
  [`dedupe-blocks.sql`](../evm-balances/clickhouse/examples/dedupe-blocks.sql).
  Windows before S hold RPC-era rows that the package cannot reproduce, so
  replays there can only be accepted.
- The RPC-era database has the same exposure: it uses the same sink and
  schema, and sinks older than v4.13.0 also made its history depend on the
  flush cadence.

## 6. Completeness

A database is **complete over a block interval** that the sink has passed when:

1. every block of the interval that has output has exactly one `blocks` row,
   and no other block of the interval has one; and
2. in every 1-minute window inside the interval, the summed `transactions` of
   each history state equals the rows the stream emitted for it.

Cursor progress does not show either. `blocks` alone cannot show a gap,
because a block without output has no row (live: 1 of 5,000 BSC blocks had
none, #7). Check both with
[`check-replays-and-completeness.sql`](../evm-balances/clickhouse/examples/check-replays-and-completeness.sql),
given the per-block row counts of a sampled interval from `substreams run` of
the same package or from an offline replay. Offline at flush interval 1 without
a replay, the stored rows equal the emitted rows in every covered window:
102,845 ERC-20 and 69,863 native rows in the 7 whole minutes of window a, and
306,693 and 152,484 in those of window b.

## 7. Initialization

**Backfill from the RPC-era database**, where a network has one. Stop the
RPC-era sink at S−1, set up a fresh database, start at S, then run
[`backfill-from-rpc-database.sql`](../evm-balances/clickhouse/examples/backfill-from-rpc-database.sql)
([procedure](../evm-balances/README.md#cutover-and-backfill)). Its DDL and
boundary checks throw before anything is copied. Partition copies do not fire
the materialized views, so `transactions` is not counted twice, and windows
that span S merge both sides. Offline at S = 122,288,518 the result equals a
single load of window a: FINAL keys 34,458 / 17,670 and view bars
313,003 / 164,379, and a v0.3.5-schema database is refused before any copy
(PR #142, after PR #115). Open: Token API configuration attributes the
production databases to evm-balances v0.3.3, while this package embeds the
v0.3.4 schema; step 0 must pass on the real database.

**Networks without an RPC-era database** (Arc, Robinhood, Ink, Soneium, Zora
and Linea;
[#8](https://github.com/pinax-network/substreams-evm-extended/issues/8)).
Starting at S leaves every holder unchanged since S unknown, and native refuses
block 0, whose genesis allocations are not traced balance changes. The cold
start is an **open owner decision**, recorded per network in #8:

- (a) start at the first Extended block, which is feasible for Arc, whose
  mainnet started on 2026-09-16;
- (b) a host-side checkpoint at S−1 for a stated holder list, read at the hash
  of S−1 (how it enters the tables is part of the decision);
- (c) accept incomplete holder sets and say so.

## 8. FINAL comparison with the RPC-era database

During the shadow period (#124 B2) the new database is compared daily with the
RPC-era database of the same network, by
[`compare-final-with-rpc-database.sql`](../evm-balances/clickhouse/examples/compare-final-with-rpc-database.sql)
over an interval [`from`, `to`] that both sinks have processed:

- **Reference pairs**: pairs whose FINAL row in the RPC-era database lies in
  the interval. A pair whose new FINAL row is later leaves the scope.
- **Compared**: only reference pairs that both databases last updated at the
  same block. Native must be exact; the file throws on a native difference.
- **ERC-20**: recall is exact pairs over reference pairs, precision is exact
  pairs over compared pairs. Report both with the interval and the reference
  pairs and tokens. For ERC-20 the reference pairs are RPC-era candidate
  holders whose `balanceOf` succeeded, not a holder census.
- Reference pairs that are not compared are counted apart: no new row in the
  interval, or an older new row with the same balance or with a different one
  (a stale suspect).

Offline, against the rows the RPC reference package emitted, the shadow
database started mid-window gave a recall of 54.76% and a precision of 99.990%
over 122,288,518–122,289,029 (35,228 reference pairs, 1,185 tokens), and
53.95% and 99.989% over 123,561,512–123,562,023 (103,930 pairs, 2,509 tokens).
These are pair-level figures on latest rows, not the row recall of the erc20
README.

## 9. Reporting

Report these separately, each with its interval and holder scope, and never
summed into one coverage figure:

- emitted rows;
- holders whose FINAL row comes from the backfill (block below S) and those
  whose FINAL row was emitted from S on;
- known zeros (FINAL balance 0);
- stale-suspect pairs (section 8).

## 10. Open

- **Owner decisions:** the cold start per network (#8); the stale-pair policy
  (#123); S, the production flags and the shadow length (#124 B1).
- **Live, each step with the owner's OK, during #124 B2:** step 0 of the
  backfill on the production database; duplicate `blocks` rows; completeness
  over a sampled interval; the daily FINAL comparison; and an estimate of stale
  ERC-20 pairs from a stated sample of pairs whose last row predates S,
  compared with `balanceOf` at a recent block hash.
- **#117:** the endpoint identity check in the `evm-balances` README.
