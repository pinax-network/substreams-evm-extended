-- Backfill the native sink database (`native`) with the native history of the
-- RPC-era evm-balances v0.3.4 database (`rpc`). Rename both databases.
--
-- Preconditions:
--   * Same server; the legacy tables in both come from the same DDL
--     (schema.0–3 here are verbatim upstream).
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
    (SELECT min(block_num) FROM native.blocks) AS native_first;

-- 2. Copy history.
ALTER TABLE native.blocks                           ATTACH PARTITION tuple() FROM rpc.blocks;
ALTER TABLE native.native_balances                  ATTACH PARTITION tuple() FROM rpc.native_balances;
ALTER TABLE native.historical_native_balances_state ATTACH PARTITION tuple() FROM rpc.historical_native_balances_state;
