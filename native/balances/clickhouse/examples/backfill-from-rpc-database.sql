-- Backfill a native sink database (`native`) with the native history of the
-- RPC-era evm-balances v0.3.4 database (`rpc`). Rename both databases.
--
-- Preconditions:
--   * Same server; native_balances and historical_native_balances_state come
--     from the same DDL in both (schema.1–3 here are verbatim upstream).
--   * No block is written by both pipelines: the RPC-era sink stopped with its
--     cursor at block S-1 and the native sink started at block S.
--
-- Partition copies do not fire materialized views. `INSERT ... SELECT` into
-- native_balances would re-run mv_historical_native_balances and add the
-- copied rows to the OHLC state a second time (`transactions` double-counts).
-- Copying the state table lets AggregatingMergeTree merge the windows that
-- span block S from both sides.

-- 1. Check the boundary before copying (expect rpc_last < native_first).
SELECT
    (SELECT max(block_num) FROM rpc.blocks)    AS rpc_last,
    (SELECT min(number) FROM native._blocks_)  AS native_first;

-- 2. Copy history.
ALTER TABLE native.native_balances                  ATTACH PARTITION tuple() FROM rpc.native_balances;
ALTER TABLE native.historical_native_balances_state ATTACH PARTITION tuple() FROM rpc.historical_native_balances_state;

-- 3. Optional: RPC-era block records into `_blocks_`, the native path's block
--    table (unprefixed hashes). No view reads it, so a plain insert is safe.
INSERT INTO native._blocks_ (number, hash, timestamp, version, deleted)
SELECT block_num, substring(block_hash, 3), timestamp, 0, false FROM rpc.blocks;
