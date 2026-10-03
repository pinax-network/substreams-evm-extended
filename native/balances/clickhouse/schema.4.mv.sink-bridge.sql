-- Bridge from the native Substreams ClickHouse sink into the legacy native
-- balance tables (schema.0–3, verbatim from substreams-evm evm-balances v0.3.4).
--
--   Balance  ──mv_sink_native_balances──▶ native_balances ──mv_historical_native_balances──▶ historical_native_balances_state
--   _blocks_ ──mv_sink_blocks───────────▶ blocks
--
-- `substreams sink clickhouse` creates `Balance` and `_blocks_` from
-- evm.balances.v1.Events (run `setup` first). No db_out is involved.
--
-- * Run the sink with --final-blocks-only. An undo writes tombstone rows
--   (`_deleted_ = true`) that the filters below drop, but values already
--   copied are not retracted.
-- * Create these views before the sink's first insert: a materialized view
--   sees only rows inserted after it exists.

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_sink_blocks
TO blocks
AS
SELECT
    toUInt32(number)            AS block_num,
    concat('0x', hash)          AS block_hash,
    timestamp
FROM _blocks_
WHERE NOT deleted;

-- Inside a materialized view every reference to `Balance`, including the IN
-- subquery, is the inserted block only, so the `_blocks_` lookup reads just
-- those block numbers through its primary key. The sink inserts `_blocks_`
-- before its data tables in every flush, so the hash is already present.
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_sink_native_balances
TO native_balances
AS
SELECT
    toUInt32(b._block_number_)                       AS block_num,
    if(k.hash = '', '', concat('0x', k.hash))        AS block_hash,
    b._block_timestamp_                              AS timestamp,
    b.address                                        AS address,
    toUInt256(b.amount)                              AS balance
FROM Balance AS b
LEFT JOIN
(
    SELECT number, any(hash) AS hash
    FROM _blocks_
    WHERE NOT deleted AND number IN (SELECT _block_number_ FROM Balance)
    GROUP BY number
) AS k ON k.number = b._block_number_
WHERE NOT b._deleted_ AND b.contract = '';
