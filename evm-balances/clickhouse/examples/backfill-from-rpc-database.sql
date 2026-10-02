-- Backfill a new evm-balances database (`extended`) with the history of the
-- RPC-era evm-balances v0.3.4 database (`rpc`). Rename both databases.
--
-- Preconditions:
--   * Both databases were created from the same schema.sql (the packaged
--     schema is identical; see evm-balances/README.md) on the same server.
--   * No block is written by both pipelines: the RPC-era sink stopped with its
--     cursor at block S-1 and the extended sink started at block S.
--
-- Partition copies do not fire materialized views. `INSERT ... SELECT` into
-- native_balances or erc20_balances would re-run mv_historical_* and add the
-- copied rows to the OHLC state a second time (`transactions` double-counts).
-- Copying the state tables lets AggregatingMergeTree merge the windows that
-- span block S from both sides.

-- 1. Check the boundary before copying (expect rpc_last < extended_first).
SELECT
    (SELECT max(block_num) FROM rpc.blocks)      AS rpc_last,
    (SELECT min(block_num) FROM extended.blocks) AS extended_first;

-- 2. Copy history.
ALTER TABLE extended.blocks                            ATTACH PARTITION tuple() FROM rpc.blocks;
ALTER TABLE extended.native_balances                   ATTACH PARTITION tuple() FROM rpc.native_balances;
ALTER TABLE extended.erc20_balances                    ATTACH PARTITION tuple() FROM rpc.erc20_balances;
ALTER TABLE extended.historical_native_balances_state  ATTACH PARTITION tuple() FROM rpc.historical_native_balances_state;
ALTER TABLE extended.historical_erc20_balances_state   ATTACH PARTITION tuple() FROM rpc.historical_erc20_balances_state;
