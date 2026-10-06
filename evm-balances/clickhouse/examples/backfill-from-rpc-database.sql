-- Backfill a new evm-balances database (`extended`) with the history of the
-- RPC-era evm-balances v0.3.4 database (`rpc`). Rename both databases.
--
-- Preconditions:
--   * Same server. The schema embedded in this package is byte-identical to
--     the one in evm-clickhouse-balances-v0.3.4; step 0 checks the DDL.
--   * No block is written by both pipelines: the RPC-era sink stopped with its
--     cursor at block S-1 and this package's sink started at block S.
--   * `rpc` stays a separate database. Never copy its `cursors` table, and never
--     create a table without a primary key inside `extended` (the sink refuses
--     to start).
--
-- Partition copies do not fire materialized views. `INSERT ... SELECT` into
-- native_balances or erc20_balances would re-run mv_historical_* and add the
-- copied rows to the OHLC state a second time (`transactions` double-counts).
-- Copying the state tables lets AggregatingMergeTree merge the windows that
-- span block S from both sides.

-- Steps 0 and 1 throw on failure, so a run of the whole file (for example
-- `clickhouse-client --multiquery < this-file`) stops before step 2 copies
-- anything. A partial copy (blocks attached, a balance table refused) would
-- otherwise need manual cleanup.

-- 0. Check the table structures match. The listing shows any difference; the
--    assertion stops the run on one. A database set up from another schema,
--    such as the unmerged v0.3.5 variant, cannot be attached.
SELECT name, r.q = '' AS missing_in_rpc, e.q = '' AS missing_in_extended
FROM (SELECT name, replaceAll(create_table_query, 'rpc.', '') AS q FROM system.tables WHERE database = 'rpc') AS r
FULL OUTER JOIN (SELECT name, replaceAll(create_table_query, 'extended.', '') AS q FROM system.tables WHERE database = 'extended') AS e
USING (name)
WHERE r.q != e.q;

SELECT throwIf(count() > 0, 'rpc and extended table structures differ (listed above): nothing was copied')
FROM (SELECT name, replaceAll(create_table_query, 'rpc.', '') AS q FROM system.tables WHERE database = 'rpc') AS r
FULL OUTER JOIN (SELECT name, replaceAll(create_table_query, 'extended.', '') AS q FROM system.tables WHERE database = 'extended') AS e
USING (name)
WHERE r.q != e.q;

-- 1. Check the boundary before copying: rpc_last < extended_first, and
--    extended already has blocks.
SELECT
    (SELECT max(block_num) FROM rpc.blocks)      AS rpc_last,
    (SELECT min(block_num) FROM extended.blocks) AS extended_first;

SELECT throwIf(
    (SELECT count() FROM extended.blocks) = 0
        OR (SELECT max(block_num) FROM rpc.blocks) >= (SELECT min(block_num) FROM extended.blocks),
    'rpc must end before extended starts, and extended must have blocks: nothing was copied');

-- 2. Copy history.
ALTER TABLE extended.blocks                            ATTACH PARTITION tuple() FROM rpc.blocks;
ALTER TABLE extended.native_balances                   ATTACH PARTITION tuple() FROM rpc.native_balances;
ALTER TABLE extended.erc20_balances                    ATTACH PARTITION tuple() FROM rpc.erc20_balances;
ALTER TABLE extended.historical_native_balances_state  ATTACH PARTITION tuple() FROM rpc.historical_native_balances_state;
ALTER TABLE extended.historical_erc20_balances_state   ATTACH PARTITION tuple() FROM rpc.historical_erc20_balances_state;
