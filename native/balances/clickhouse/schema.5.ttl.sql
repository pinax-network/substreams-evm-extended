-- Base data retention: 7 days. Each `Balance` row is copied into
-- native_balances and the aggregation state when it is inserted, so the sink's
-- own table only serves audits and rebuilds. Rows expire 7 days after their
-- block time and are removed by background TTL merges (soon after insert for
-- old blocks, as in a backfill); materialized views never see these deletions.
--
-- `_blocks_` keeps no TTL: it is the block table, one small row per block with output.
ALTER TABLE Balance MODIFY TTL _block_timestamp_ + INTERVAL 7 DAY;
