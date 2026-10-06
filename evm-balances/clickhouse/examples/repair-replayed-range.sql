-- Repair a replayed range in an evm-balances database (`extended`) from a
-- re-stream of that range into a scratch database (`staging`). Rename both.
-- The contract is docs/initialization-and-completeness.md (section 5); find
-- the range with check-replays-and-completeness.sql.
--
-- What a replay does: a flush writes its tables one by one, then the cursor.
-- A crash or a retried flush re-inserts rows that were already stored, so the
-- same rows (same block, same values) are stored twice. `blocks` gets
-- duplicate rows and `transactions` counts the rows again; FINAL balances,
-- open, close, high, low and the window block bounds do not change. This file
-- removes the duplicate `blocks` rows and subtracts the replayed rows from
-- `transactions`. It changes nothing else, and it refuses any range where
-- `extended` and `staging` differ by more than repeated rows.
--
-- Preconditions:
--   * The sink that writes `extended` is stopped for the whole run. Restart it
--     afterwards; it resumes from its cursor.
--   * `staging` is a fresh database set up from the same package
--     (`substreams-sink-sql setup`), loaded by one run of the same package and
--     sink, without a restart, over a block range that starts at least one
--     whole minute before the first replayed block and ends at least one whole
--     minute after the last one. Only 1-minute windows that `staging` covers
--     entirely are repaired. A replay inside `staging` would be taken for the
--     truth, hence one uninterrupted run.
--   * Choosing the range: duplicated `blocks` rows show replayed blocks. A
--     crash between two tables of a flush can replay balance rows with
--     `blocks` intact; then the sink log shows the cursor it resumed from, C,
--     and the replayed flush is C+1 up to C plus the flush interval.
--   * The range lies after the cutover block S: windows before S hold RPC-era
--     rows that the package cannot reproduce. For those, and for any range
--     you do not re-stream, the alternative is to accept `transactions` as
--     approximate and record the range (the contract, section 5).
--
-- The helper tables go into `staging` (prefix `repair_`). Steps 0 to 2 read
-- `extended` and throw on any failed check, so a run of the whole file (for
-- example `clickhouse-client --multiquery < this-file`) stops before step 3
-- changes anything. Step 1 also stops when `repair_` tables already exist,
-- which keeps the corrected rows of an interrupted run: if step 3 stops
-- part-way, run step 3 again (each DELETE and INSERT pair is safe to repeat).
-- Step 4 throws if the repair did not take. Drop `staging` afterwards.
--
-- Tested offline on ClickHouse 25.8 with non-replicated tables. On replicated
-- tables, mutations whose WHERE reads another table need
-- `allow_nondeterministic_mutations = 1`.

-- 0. Checks before anything is written.
--    Same tables (apart from the cursor and this file's helpers), the same
--    db_out module hash in both cursors tables, and the same blocks.
SELECT throwIf(count() > 0, 'extended and staging table structures differ: nothing was changed')
FROM (SELECT name, replaceAll(create_table_query, 'extended.', '') AS q FROM system.tables
      WHERE database = 'extended' AND name NOT LIKE 'repair\\_%') AS e
FULL OUTER JOIN (SELECT name, replaceAll(create_table_query, 'staging.', '') AS q FROM system.tables
      WHERE database = 'staging' AND name NOT LIKE 'repair\\_%') AS s
USING (name)
WHERE e.q != s.q;

SELECT throwIf(
       (SELECT count() FROM staging.cursors FINAL) != 1
    OR (SELECT any(id) FROM staging.cursors FINAL) NOT IN (SELECT id FROM extended.cursors FINAL),
    'staging was not loaded by the db_out module of extended: nothing was changed');

SELECT throwIf(
       (SELECT count() FROM staging.blocks) = 0
    OR (SELECT count() - uniqExact(block_num) FROM staging.blocks) > 0,
    'staging is empty or has duplicate blocks rows: reload it in one run: nothing was changed');

WITH (SELECT min(block_num) FROM staging.blocks) AS p,
     (SELECT max(block_num) FROM staging.blocks) AS q
SELECT throwIf(count() > 0, 'extended and staging hold different blocks in the staging range: nothing was changed')
FROM
(
    (SELECT DISTINCT block_num, block_hash, timestamp FROM extended.blocks WHERE block_num BETWEEN p AND q
     EXCEPT SELECT block_num, block_hash, timestamp FROM staging.blocks)
    UNION ALL
    (SELECT block_num, block_hash, timestamp FROM staging.blocks
     EXCEPT SELECT DISTINCT block_num, block_hash, timestamp FROM extended.blocks WHERE block_num BETWEEN p AND q)
);

-- Covered 1-minute windows start after the first staged block's timestamp and
-- end at or before the last one's. Every duplicated block in the range must
-- fall into one.
WITH (SELECT min(block_num) FROM staging.blocks) AS p,
     (SELECT max(block_num) FROM staging.blocks) AS q,
     (SELECT min(timestamp) FROM staging.blocks) AS first_ts,
     (SELECT max(timestamp) FROM staging.blocks) AS last_ts
SELECT throwIf(
       countIf(toDateTime(intDiv(toUInt32(timestamp), 60) * 60, 'UTC') <= first_ts
            OR toDateTime(intDiv(toUInt32(timestamp), 60) * 60, 'UTC') + 60 > last_ts) > 0,
    'duplicated blocks lie in a minute that staging does not cover entirely: stage a wider range: nothing was changed')
FROM (SELECT block_num, any(timestamp) AS timestamp FROM extended.blocks
      WHERE block_num BETWEEN p AND q GROUP BY block_num HAVING count() > 1);

-- 1. Compare every covered minute per key, and build the corrections.
CREATE TABLE staging.repair_minutes_erc20 ENGINE = MergeTree ORDER BY (timestamp, contract, address) AS
WITH (SELECT min(timestamp) FROM staging.blocks) AS first_ts,
     (SELECT max(timestamp) FROM staging.blocks) AS last_ts
SELECT timestamp, contract, address,
       e.present AS in_extended, s.present AS in_staging,
       e.transactions AS extended_transactions, s.transactions AS staging_transactions,
       (e.min_block_num, e.max_block_num, e.open, e.high, e.low, e.close)
           = (s.min_block_num, s.max_block_num, s.open, s.high, s.low, s.close) AS same_bar
FROM
(
    SELECT timestamp, contract, address, 1 AS present, sum(transactions) AS transactions,
           min(min_block_num) AS min_block_num, max(max_block_num) AS max_block_num,
           argMinMerge(open) AS open, max(high) AS high, min(low) AS low, argMaxMerge(close) AS close
    FROM extended.historical_erc20_balances_state
    WHERE interval_min = 1 AND timestamp > first_ts AND timestamp + 60 <= last_ts
    GROUP BY timestamp, contract, address
) AS e
FULL OUTER JOIN
(
    SELECT timestamp, contract, address, 1 AS present, sum(transactions) AS transactions,
           min(min_block_num) AS min_block_num, max(max_block_num) AS max_block_num,
           argMinMerge(open) AS open, max(high) AS high, min(low) AS low, argMaxMerge(close) AS close
    FROM staging.historical_erc20_balances_state
    WHERE interval_min = 1 AND timestamp > first_ts AND timestamp + 60 <= last_ts
    GROUP BY timestamp, contract, address
) AS s
USING (timestamp, contract, address);

CREATE TABLE staging.repair_minutes_native ENGINE = MergeTree ORDER BY (timestamp, address) AS
WITH (SELECT min(timestamp) FROM staging.blocks) AS first_ts,
     (SELECT max(timestamp) FROM staging.blocks) AS last_ts
SELECT timestamp, address,
       e.present AS in_extended, s.present AS in_staging,
       e.transactions AS extended_transactions, s.transactions AS staging_transactions,
       (e.min_block_num, e.max_block_num, e.open, e.high, e.low, e.close)
           = (s.min_block_num, s.max_block_num, s.open, s.high, s.low, s.close) AS same_bar
FROM
(
    SELECT timestamp, address, 1 AS present, sum(transactions) AS transactions,
           min(min_block_num) AS min_block_num, max(max_block_num) AS max_block_num,
           argMinMerge(open) AS open, max(high) AS high, min(low) AS low, argMaxMerge(close) AS close
    FROM extended.historical_native_balances_state
    WHERE interval_min = 1 AND timestamp > first_ts AND timestamp + 60 <= last_ts
    GROUP BY timestamp, address
) AS e
FULL OUTER JOIN
(
    SELECT timestamp, address, 1 AS present, sum(transactions) AS transactions,
           min(min_block_num) AS min_block_num, max(max_block_num) AS max_block_num,
           argMinMerge(open) AS open, max(high) AS high, min(low) AS low, argMaxMerge(close) AS close
    FROM staging.historical_native_balances_state
    WHERE interval_min = 1 AND timestamp > first_ts AND timestamp + 60 <= last_ts
    GROUP BY timestamp, address
) AS s
USING (timestamp, address);

-- 2. Refuse anything but repeated rows: a key in one database only, a bar
--    that differs, or fewer rows in `extended` than in `staging` is not a
--    replay.
SELECT throwIf(
       (SELECT count() FROM staging.repair_minutes_erc20
        WHERE NOT in_extended OR NOT in_staging OR NOT same_bar OR extended_transactions < staging_transactions) > 0
    OR (SELECT count() FROM staging.repair_minutes_native
        WHERE NOT in_extended OR NOT in_staging OR NOT same_bar OR extended_transactions < staging_transactions) > 0,
    'extended differs from staging by more than repeated rows in a covered minute: nothing was changed');

-- The corrections: every window of every interval that contains a covered
-- minute with excess rows gets one merged row whose `transactions` drops by
-- that excess; its other columns are merged unchanged.
CREATE TABLE staging.repair_fix_erc20 AS extended.historical_erc20_balances_state
ENGINE = MergeTree ORDER BY (interval_min, address, contract, timestamp);

INSERT INTO staging.repair_fix_erc20
    (timestamp, interval_min, min_block_num, max_block_num, contract, address, open, high, low, close, transactions)
SELECT st.timestamp, st.interval_min, min(st.min_block_num), max(st.max_block_num), st.contract, st.address,
       argMinMergeState(st.open), max(st.high), min(st.low), argMaxMergeState(st.close),
       sum(st.transactions) - any(x.excess)
FROM extended.historical_erc20_balances_state AS st
INNER JOIN
(
    SELECT interval_min,
           toDateTime(intDiv(toUInt32(timestamp), interval_min * 60) * interval_min * 60, 'UTC') AS timestamp,
           contract, address, sum(extended_transactions - staging_transactions) AS excess
    FROM staging.repair_minutes_erc20
    ARRAY JOIN [1, 5, 10, 30, 60, 240, 1440, 10080] AS interval_min
    WHERE extended_transactions > staging_transactions
    GROUP BY interval_min, timestamp, contract, address
) AS x
ON st.interval_min = x.interval_min AND st.timestamp = x.timestamp
   AND st.contract = x.contract AND st.address = x.address
GROUP BY st.interval_min, st.timestamp, st.contract, st.address;

CREATE TABLE staging.repair_fix_native AS extended.historical_native_balances_state
ENGINE = MergeTree ORDER BY (interval_min, address, timestamp);

INSERT INTO staging.repair_fix_native
    (timestamp, interval_min, min_block_num, max_block_num, address, open, high, low, close, transactions)
SELECT st.timestamp, st.interval_min, min(st.min_block_num), max(st.max_block_num), st.address,
       argMinMergeState(st.open), max(st.high), min(st.low), argMaxMergeState(st.close),
       sum(st.transactions) - any(x.excess)
FROM extended.historical_native_balances_state AS st
INNER JOIN
(
    SELECT interval_min,
           toDateTime(intDiv(toUInt32(timestamp), interval_min * 60) * interval_min * 60, 'UTC') AS timestamp,
           address, sum(extended_transactions - staging_transactions) AS excess
    FROM staging.repair_minutes_native
    ARRAY JOIN [1, 5, 10, 30, 60, 240, 1440, 10080] AS interval_min
    WHERE extended_transactions > staging_transactions
    GROUP BY interval_min, timestamp, address
) AS x
ON st.interval_min = x.interval_min AND st.timestamp = x.timestamp AND st.address = x.address
GROUP BY st.interval_min, st.timestamp, st.address;

-- One row for each duplicated block of the range, taken from `staging`.
CREATE TABLE staging.repair_blocks ENGINE = MergeTree ORDER BY block_num AS
SELECT block_num, block_hash, timestamp
FROM staging.blocks
WHERE block_num IN (SELECT block_num FROM extended.blocks
                    WHERE block_num BETWEEN (SELECT min(block_num) FROM staging.blocks)
                                        AND (SELECT max(block_num) FROM staging.blocks)
                    GROUP BY block_num HAVING count() > 1);

-- What will change (for the record).
SELECT
    (SELECT min(block_num) FROM staging.blocks) AS staged_from,
    (SELECT max(block_num) FROM staging.blocks) AS staged_to,
    (SELECT count() FROM staging.repair_blocks) AS duplicated_blocks,
    (SELECT sum(extended_transactions - staging_transactions) FROM staging.repair_minutes_erc20) AS erc20_rows_replayed,
    (SELECT sum(extended_transactions - staging_transactions) FROM staging.repair_minutes_native) AS native_rows_replayed,
    (SELECT count() FROM staging.repair_fix_erc20) AS erc20_windows_corrected,
    (SELECT count() FROM staging.repair_fix_native) AS native_windows_corrected;

-- 3. Replace. Each DELETE removes every row of the listed blocks or windows
--    (all parts); the INSERT that follows writes one row for each.
ALTER TABLE extended.blocks
    DELETE WHERE block_num IN (SELECT block_num FROM staging.repair_blocks)
    SETTINGS mutations_sync = 2;
INSERT INTO extended.blocks (block_num, block_hash, timestamp)
SELECT block_num, block_hash, timestamp FROM staging.repair_blocks;

ALTER TABLE extended.historical_erc20_balances_state
    DELETE WHERE (interval_min, timestamp, contract, address)
                 IN (SELECT interval_min, timestamp, contract, address FROM staging.repair_fix_erc20)
    SETTINGS mutations_sync = 2;
INSERT INTO extended.historical_erc20_balances_state
    (timestamp, interval_min, min_block_num, max_block_num, contract, address, open, high, low, close, transactions)
SELECT timestamp, interval_min, min_block_num, max_block_num, contract, address, open, high, low, close, transactions
FROM staging.repair_fix_erc20;

ALTER TABLE extended.historical_native_balances_state
    DELETE WHERE (interval_min, timestamp, address)
                 IN (SELECT interval_min, timestamp, address FROM staging.repair_fix_native)
    SETTINGS mutations_sync = 2;
INSERT INTO extended.historical_native_balances_state
    (timestamp, interval_min, min_block_num, max_block_num, address, open, high, low, close, transactions)
SELECT timestamp, interval_min, min_block_num, max_block_num, address, open, high, low, close, transactions
FROM staging.repair_fix_native;

-- 4. Verify: no duplicate block in the range, and every covered minute of
--    both history states equals `staging`, key by key.
WITH (SELECT min(block_num) FROM staging.blocks) AS p,
     (SELECT max(block_num) FROM staging.blocks) AS q,
     (SELECT min(timestamp) FROM staging.blocks) AS first_ts,
     (SELECT max(timestamp) FROM staging.blocks) AS last_ts
SELECT throwIf(
       (SELECT count() FROM (SELECT block_num FROM extended.blocks WHERE block_num BETWEEN p AND q
                             GROUP BY block_num HAVING count() > 1)) > 0
    OR (SELECT count() FROM
        (
            SELECT timestamp, contract, address, min_block_num, max_block_num, open, high, low, close, transactions
            FROM extended.historical_erc20_balances
            WHERE interval_min = 1 AND timestamp > first_ts AND timestamp + 60 <= last_ts
            EXCEPT
            SELECT timestamp, contract, address, min_block_num, max_block_num, open, high, low, close, transactions
            FROM staging.historical_erc20_balances
            WHERE interval_min = 1 AND timestamp > first_ts AND timestamp + 60 <= last_ts
        )) > 0
    OR (SELECT count() FROM
        (
            SELECT timestamp, contract, address, min_block_num, max_block_num, open, high, low, close, transactions
            FROM staging.historical_erc20_balances
            WHERE interval_min = 1 AND timestamp > first_ts AND timestamp + 60 <= last_ts
            EXCEPT
            SELECT timestamp, contract, address, min_block_num, max_block_num, open, high, low, close, transactions
            FROM extended.historical_erc20_balances
            WHERE interval_min = 1 AND timestamp > first_ts AND timestamp + 60 <= last_ts
        )) > 0
    OR (SELECT count() FROM
        (
            SELECT timestamp, address, min_block_num, max_block_num, open, high, low, close, transactions
            FROM extended.historical_native_balances
            WHERE interval_min = 1 AND timestamp > first_ts AND timestamp + 60 <= last_ts
            EXCEPT
            SELECT timestamp, address, min_block_num, max_block_num, open, high, low, close, transactions
            FROM staging.historical_native_balances
            WHERE interval_min = 1 AND timestamp > first_ts AND timestamp + 60 <= last_ts
        )) > 0
    OR (SELECT count() FROM
        (
            SELECT timestamp, address, min_block_num, max_block_num, open, high, low, close, transactions
            FROM staging.historical_native_balances
            WHERE interval_min = 1 AND timestamp > first_ts AND timestamp + 60 <= last_ts
            EXCEPT
            SELECT timestamp, address, min_block_num, max_block_num, open, high, low, close, transactions
            FROM extended.historical_native_balances
            WHERE interval_min = 1 AND timestamp > first_ts AND timestamp + 60 <= last_ts
        )) > 0,
    'the repair did not take: compare extended with staging');
