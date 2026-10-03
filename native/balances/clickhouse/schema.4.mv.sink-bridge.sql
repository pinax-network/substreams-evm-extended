-- Bridge from the native Substreams ClickHouse sink into the legacy native
-- balance tables (schema.1–3, verbatim from substreams-evm evm-balances v0.3.4).
--
--   Balance ──mv_sink_native_balances──▶ native_balances ──mv_historical_native_balances──▶ historical_native_balances_state
--
-- `substreams sink clickhouse` creates `Balance` and `_blocks_` from
-- evm.balances.v1.Events (run `setup` first). No db_out is involved.
--
-- * `_blocks_` is this path's block table (number, unprefixed hash, time); the
--   legacy `blocks` table is not created. `native_balances.block_hash` stays
--   empty here; join `_blocks_` on `block_num = number` for the hash.
-- * Run the sink with --final-blocks-only (the Makefile always does): an undo
--   would write tombstone rows (`_deleted_ = true`) that this view would copy.
-- * Create this view before the sink's first insert: a materialized view sees
--   only rows inserted after it exists.

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_sink_native_balances
TO native_balances
AS
SELECT
    toUInt32(_block_number_)    AS block_num,
    _block_timestamp_           AS timestamp,
    address,
    toUInt256(amount)           AS balance
FROM Balance;
