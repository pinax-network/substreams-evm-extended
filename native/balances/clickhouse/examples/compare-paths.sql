-- Compare the two ClickHouse paths of the native package, e.g. in production:
--   {native:Identifier}  native sink (`make setup run`)
--   {legacy:Identifier}  legacy substreams-sink-sql with db_out (`make legacy-setup legacy-run`)
-- over blocks [{from:UInt32}, {to:UInt32}] that both have already passed:
--
--   clickhouse client "$DSN" --param_native=bsc_native_sink --param_legacy=bsc_native_legacy \
--     --param_from=123561000 --param_to=123562023 --queries-file compare-paths.sql
--
-- Every `differences` must be 0. Bars are compared only for windows inside the
-- range, so the two databases may be at different heads.

-- Blocks with output, their hashes and times.
SELECT 'blocks' AS check, count() AS differences
FROM (SELECT block_num, block_hash, timestamp FROM {native:Identifier}.blocks WHERE block_num BETWEEN {from:UInt32} AND {to:UInt32}) AS n
FULL OUTER JOIN (SELECT block_num, block_hash, timestamp FROM {legacy:Identifier}.blocks WHERE block_num BETWEEN {from:UInt32} AND {to:UInt32}) AS l
    USING (block_num)
WHERE n.block_hash != l.block_hash OR n.timestamp != l.timestamp;

-- OHLC bars of every interval whose window lies inside the range.
WITH
    (SELECT min(timestamp) FROM {native:Identifier}.blocks WHERE block_num BETWEEN {from:UInt32} AND {to:UInt32}) AS start_time,
    (SELECT max(timestamp) FROM {native:Identifier}.blocks WHERE block_num BETWEEN {from:UInt32} AND {to:UInt32}) AS end_time
SELECT 'historical_native_balances' AS check, count() AS differences
FROM (
    SELECT * FROM {native:Identifier}.historical_native_balances
    WHERE timestamp >= start_time AND timestamp + toIntervalMinute(interval_min) <= end_time
) AS n
FULL OUTER JOIN (
    SELECT * FROM {legacy:Identifier}.historical_native_balances
    WHERE timestamp >= start_time AND timestamp + toIntervalMinute(interval_min) <= end_time
) AS l USING (interval_min, address, timestamp)
WHERE n.open != l.open OR n.high != l.high OR n.low != l.low OR n.close != l.close
   OR n.transactions != l.transactions OR n.min_block_num != l.min_block_num OR n.max_block_num != l.max_block_num;

-- Latest balances. Meaningful only when both have stopped at the same block
-- (`STOP`/`LEGACY_STOP`), since each keeps the newest row per address.
SELECT 'native_balances (same stop block only)' AS check, count() AS differences
FROM (SELECT address, block_num, block_hash, balance FROM {native:Identifier}.native_balances FINAL) AS n
FULL OUTER JOIN (SELECT address, block_num, block_hash, balance FROM {legacy:Identifier}.native_balances FINAL) AS l
    USING (address)
WHERE n.block_num != l.block_num OR n.block_hash != l.block_hash OR n.balance != l.balance;
