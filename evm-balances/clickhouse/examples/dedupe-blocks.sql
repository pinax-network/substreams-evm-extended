-- Remove duplicate `blocks` rows from an evm-balances database (`extended`),
-- for replays whose `transactions` you accept as approximate instead of
-- repairing them with repair-replayed-range.sql. Rename it. The contract is
-- docs/initialization-and-completeness.md (section 5). Record the replayed
-- ranges and their excess rows (check-replays-and-completeness.sql) where the
-- database is documented: this file leaves the history states as they are.
--
-- `OPTIMIZE TABLE blocks FINAL DEDUPLICATE` is refused, because `blocks` has
-- projections. This file keeps one row of each duplicated block in a helper
-- table, deletes every row of those blocks, and inserts the kept rows back.
-- The helper table goes into a database `repair` (created here, separate from
-- the sink database); drop it afterwards.
--
-- Step 0 throws when two rows of one block differ (another chain or a fork,
-- not a replay), so a run of the whole file stops before anything changes.
-- Step 1 stops when the helper table already exists, which keeps the rows of
-- an interrupted run: if step 2 stops part-way, run step 2 again. Step 3
-- throws if a duplicate remains. Stop the sink first if it may replay again
-- while this runs; rows it writes meanwhile are not affected.
--
-- Tested offline on ClickHouse 25.8 with non-replicated tables. Replicated
-- tables can refuse a mutation whose WHERE reads another table
-- (`allow_nondeterministic_mutations`).

-- 0. Every duplicated block has one hash and one timestamp.
SELECT throwIf(count() > 0, 'a duplicated block has rows that differ: not a replay: nothing was changed')
FROM (SELECT block_num FROM extended.blocks GROUP BY block_num
      HAVING count() > 1 AND uniqExact(block_hash, timestamp) > 1);

-- 1. Keep one row of each duplicated block.
CREATE DATABASE IF NOT EXISTS repair;
CREATE TABLE repair.duplicate_blocks ENGINE = MergeTree ORDER BY block_num AS
SELECT block_num, any(block_hash) AS block_hash, any(timestamp) AS timestamp, count() AS copies
FROM extended.blocks
GROUP BY block_num
HAVING copies > 1;

SELECT count() AS duplicated_blocks, sum(copies - 1) AS rows_removed,
       min(block_num) AS first_block, max(block_num) AS last_block
FROM repair.duplicate_blocks;

-- 2. Replace those blocks' rows with one row each.
ALTER TABLE extended.blocks
    DELETE WHERE block_num IN (SELECT block_num FROM repair.duplicate_blocks)
    SETTINGS mutations_sync = 2;
INSERT INTO extended.blocks (block_num, block_hash, timestamp)
SELECT block_num, block_hash, timestamp FROM repair.duplicate_blocks;

-- 3. Verify.
SELECT throwIf(count() > 0, 'duplicate blocks rows remain')
FROM (SELECT block_num FROM extended.blocks GROUP BY block_num HAVING count() > 1);
