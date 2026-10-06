-- Detect replayed flushes and unpublished blocks in an evm-balances database
-- (`extended`). Rename it. Read-only. The contract these checks enforce is
-- docs/initialization-and-completeness.md (sections 5 and 6).
--
-- Step 1 needs only the database. Steps 2 and 3 compare it with the rows the
-- stream emitted over a sampled block range, which you load first into
-- `check.expected_rows`: one row per block of the range that has output, with
-- the db_out row counts of that block. Take them from `substreams run` of the
-- same package (its db_out `tableChanges` per table, and the timestamp of the
-- block's `blocks` row), or from an offline replay of the same blocks. Blocks
-- without output may be omitted; they emit no row. Keep `check` a separate
-- database: never create a table inside the sink database.
--
--   CREATE DATABASE IF NOT EXISTS check;
--   CREATE TABLE check.expected_rows (
--       block_num   UInt32,
--       timestamp   DateTime(0, 'UTC'),
--       erc20_rows  UInt64,
--       native_rows UInt64
--   ) ENGINE = MergeTree ORDER BY block_num;
--
-- Only 1-minute windows that the range covers entirely are compared: a window
-- must start after the first listed block's timestamp and end at or before the
-- last listed block's timestamp. Blocks share timestamps (several per second on
-- BSC), so this is the strict reading.
--
-- The last statement throws when any step found something, so a scheduled run
-- exits non-zero. Each finding is listed above it.

-- 1. Duplicate `blocks` rows. `blocks` is a plain MergeTree: a block that has
--    two rows was written twice, by a replayed or retried flush. The converse
--    does not hold: a crash between two tables of a flush replays only the
--    tables that had committed, so balance rows can be duplicated with
--    `blocks` intact (step 2 finds those). More than one hash per block means
--    the rows come from different chains or forks, not from a replay.
SELECT
    count()            AS duplicated_blocks,
    sum(copies - 1)    AS extra_rows,
    min(block_num)     AS first_block,
    max(block_num)     AS last_block,
    countIf(hashes > 1) AS blocks_with_two_hashes
FROM (SELECT block_num, count() AS copies, uniqExact(block_hash) AS hashes
      FROM extended.blocks GROUP BY block_num HAVING copies > 1);

SELECT block_num, count() AS copies, uniqExact(block_hash) AS hashes, min(timestamp) AS timestamp
FROM extended.blocks
GROUP BY block_num
HAVING copies > 1
ORDER BY block_num;

-- 2. `transactions` per 1-minute window against the expected rows. Each row
--    inserted into erc20_balances or native_balances adds 1 to its window, so
--    for a covered window the stored sum equals the emitted rows exactly when
--    every row was stored once. Excess (stored - expected > 0) means replayed
--    rows; a deficit means rows that were never stored.
WITH
    (SELECT min(timestamp) FROM check.expected_rows) AS first_ts,
    (SELECT max(timestamp) FROM check.expected_rows) AS last_ts
SELECT
    (SELECT min(block_num) FROM check.expected_rows) AS first_block,
    (SELECT max(block_num) FROM check.expected_rows) AS last_block,
    (SELECT count() - uniqExact(block_num) FROM check.expected_rows) AS repeated_listed_blocks,
    count()                                   AS covered_windows,
    min(window)                               AS first_window,
    max(window)                               AS last_window,
    sum(expected_erc20)                       AS expected_erc20_rows,
    sum(stored_erc20)                         AS stored_erc20_rows,
    countIf(stored_erc20 != expected_erc20)   AS erc20_windows_differing,
    sum(expected_native)                      AS expected_native_rows,
    sum(stored_native)                        AS stored_native_rows,
    countIf(stored_native != expected_native) AS native_windows_differing
FROM
(
    SELECT window,
           sum(x_erc20) AS expected_erc20, sum(s_erc20) AS stored_erc20,
           sum(x_native) AS expected_native, sum(s_native) AS stored_native
    FROM
    (
        SELECT toDateTime(intDiv(toUInt32(timestamp), 60) * 60, 'UTC') AS window,
               erc20_rows AS x_erc20, toUInt64(0) AS s_erc20, native_rows AS x_native, toUInt64(0) AS s_native
        FROM check.expected_rows
        UNION ALL
        SELECT timestamp, 0, transactions, 0, 0
        FROM extended.historical_erc20_balances_state WHERE interval_min = 1
        UNION ALL
        SELECT timestamp, 0, 0, 0, transactions
        FROM extended.historical_native_balances_state WHERE interval_min = 1
    )
    WHERE window > first_ts AND window + 60 <= last_ts
    GROUP BY window
);

-- The differing windows. A window with stored rows but no expected row in the
-- covered span is listed with expected 0.
WITH
    (SELECT min(timestamp) FROM check.expected_rows) AS first_ts,
    (SELECT max(timestamp) FROM check.expected_rows) AS last_ts
SELECT window, expected_erc20, stored_erc20, expected_native, stored_native
FROM
(
    SELECT window,
           sum(x_erc20) AS expected_erc20, sum(s_erc20) AS stored_erc20,
           sum(x_native) AS expected_native, sum(s_native) AS stored_native
    FROM
    (
        SELECT toDateTime(intDiv(toUInt32(timestamp), 60) * 60, 'UTC') AS window,
               erc20_rows AS x_erc20, toUInt64(0) AS s_erc20, native_rows AS x_native, toUInt64(0) AS s_native
        FROM check.expected_rows
        UNION ALL
        SELECT timestamp, 0, transactions, 0, 0
        FROM extended.historical_erc20_balances_state WHERE interval_min = 1
        UNION ALL
        SELECT timestamp, 0, 0, 0, transactions
        FROM extended.historical_native_balances_state WHERE interval_min = 1
    )
    WHERE window > first_ts AND window + 60 <= last_ts
    GROUP BY window
)
WHERE stored_erc20 != expected_erc20 OR stored_native != expected_native
ORDER BY window;

-- 3. `blocks` over the sampled range: a block with output must have a row, and
--    no other block may have one. A missing row means the block's flush never
--    committed `blocks`; cursor progress does not show it.
SELECT 'missing' AS finding, block_num
FROM check.expected_rows
WHERE erc20_rows + native_rows > 0
  AND block_num NOT IN (SELECT block_num FROM extended.blocks)
UNION ALL
SELECT 'unexpected' AS finding, block_num
FROM extended.blocks
WHERE block_num BETWEEN (SELECT min(block_num) FROM check.expected_rows)
                    AND (SELECT max(block_num) FROM check.expected_rows)
  AND block_num NOT IN (SELECT block_num FROM check.expected_rows WHERE erc20_rows + native_rows > 0)
ORDER BY block_num;

-- 4. Stop on any finding of steps 1 to 3.
WITH
    (SELECT min(timestamp) FROM check.expected_rows) AS first_ts,
    (SELECT max(timestamp) FROM check.expected_rows) AS last_ts,
    (SELECT min(block_num) FROM check.expected_rows) AS first_block,
    (SELECT max(block_num) FROM check.expected_rows) AS last_block
SELECT throwIf(
       (SELECT count() FROM (SELECT block_num FROM extended.blocks GROUP BY block_num HAVING count() > 1)) > 0
    OR (SELECT count() - uniqExact(block_num) FROM check.expected_rows) > 0
    OR (SELECT count() FROM
        (
            SELECT window,
                   sum(x_erc20) AS expected_erc20, sum(s_erc20) AS stored_erc20,
                   sum(x_native) AS expected_native, sum(s_native) AS stored_native
            FROM
            (
                SELECT toDateTime(intDiv(toUInt32(timestamp), 60) * 60, 'UTC') AS window,
                       erc20_rows AS x_erc20, toUInt64(0) AS s_erc20, native_rows AS x_native, toUInt64(0) AS s_native
                FROM check.expected_rows
                UNION ALL
                SELECT timestamp, 0, transactions, 0, 0
                FROM extended.historical_erc20_balances_state WHERE interval_min = 1
                UNION ALL
                SELECT timestamp, 0, 0, 0, transactions
                FROM extended.historical_native_balances_state WHERE interval_min = 1
            )
            WHERE window > first_ts AND window + 60 <= last_ts
            GROUP BY window
        )
        WHERE stored_erc20 != expected_erc20 OR stored_native != expected_native) > 0
    OR (SELECT count() FROM check.expected_rows
        WHERE erc20_rows + native_rows > 0 AND block_num NOT IN (SELECT block_num FROM extended.blocks)) > 0
    OR (SELECT count() FROM extended.blocks
        WHERE block_num BETWEEN first_block AND last_block
          AND block_num NOT IN (SELECT block_num FROM check.expected_rows WHERE erc20_rows + native_rows > 0)) > 0,
    'replayed or unpublished rows found: see the steps above');
