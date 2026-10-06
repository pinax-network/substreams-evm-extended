-- Compare the FINAL balances of a new evm-balances database (`extended`) with
-- the RPC-era evm-balances database (`rpc`) of the same network, over a block
-- interval that both sinks have processed. Rename both databases and pass the
-- interval as parameters:
--
--   clickhouse-client --param_from=<first block> --param_to=<last block> \
--       --multiquery < compare-final-with-rpc-database.sql
--
-- `from` is the first block `extended` processed itself: its start block, or
-- the cutover block S after a backfill (rows copied from `rpc` are identical by
-- construction and must not count). `to` is at most the block of both sinks'
-- cursors. Read-only; the last statement throws when native balances differ.
--
-- Definition (#7; #124 B2 runs it daily during the shadow period):
--   * A pair is (contract, holder) for ERC-20 and an account for native. Its
--     FINAL row in a database is its latest row: the latest block that
--     database's package emitted it at.
--   * Reference pairs: pairs whose FINAL row in `rpc` lies in [from, to]. For
--     ERC-20 these are RPC-era candidate holders whose balanceOf succeeded,
--     not a holder census. A pair whose FINAL row in `extended` is later than
--     its `rpc` one leaves the scope: whether `extended` reproduced the `rpc`
--     row cannot be read from FINAL (counted as `newer_in_extended`).
--   * Same-block pairs: reference pairs whose FINAL rows in both databases are
--     at the same block. Only their balances are compared:
--       exact = same balance, wrong = different balance.
--   * Native must be exact: wrong = 0.
--   * FINAL is filtered only by `block_num >= from`, which keeps a pair's
--     latest row or drops the pair, whether ClickHouse applies it before or
--     after FINAL. The `to` bound is applied to the FINAL rows afterwards.
--   * ERC-20: recall = exact / reference pairs, precision = exact / same-block
--     pairs. Report both with the interval and the reference pairs and tokens.
--   * The other reference pairs are reported, not compared: no `extended` row
--     in [from, to] (`absent_in_extended`), or an older one with the same or a
--     different balance (`older_in_extended_*`). An older row with a different
--     balance is a stale suspect: `extended` holds a value that the RPC era
--     has since replaced.

-- ERC-20.
SELECT
    {from:UInt32} AS from_block,
    {to:UInt32}   AS to_block,
    countIf(in_scope)                                        AS reference_pairs,
    uniqExactIf(contract, in_scope)                          AS reference_tokens,
    countIf(same_block)                                      AS same_block_pairs,
    countIf(same_block AND r_balance = e_balance)            AS exact,
    countIf(same_block AND r_balance != e_balance)           AS wrong,
    round(exact / reference_pairs, 6)                        AS recall,
    round(exact / same_block_pairs, 6)                       AS precision,
    countIf(in_scope AND NOT e_present)                      AS absent_in_extended,
    countIf(in_scope AND e_present AND e_block < r_block AND r_balance = e_balance)  AS older_in_extended_same_balance,
    countIf(in_scope AND e_present AND e_block < r_block AND r_balance != e_balance) AS older_in_extended_stale_suspect,
    countIf(r_in_interval AND e_present AND e_block > r_block)                       AS newer_in_extended,
    countIf(e_present AND e_block <= {to:UInt32} AND NOT r_present)                  AS only_in_extended
FROM
(
    SELECT
        contract, address,
        r.present AS r_present, r.block_num AS r_block, r.balance AS r_balance,
        e.present AS e_present, e.block_num AS e_block, e.balance AS e_balance,
        r_present AND r_block <= {to:UInt32} AS r_in_interval,
        r_in_interval AND NOT (e_present AND e_block > r_block) AS in_scope,
        in_scope AND e_present AND e_block = r_block AS same_block
    FROM
        (SELECT contract, address, block_num, balance, 1 AS present
         FROM rpc.erc20_balances FINAL WHERE block_num >= {from:UInt32}) AS r
    FULL OUTER JOIN
        (SELECT contract, address, block_num, balance, 1 AS present
         FROM extended.erc20_balances FINAL WHERE block_num >= {from:UInt32}) AS e
    USING (contract, address)
)
SETTINGS join_use_nulls = 0;

-- Native.
SELECT
    {from:UInt32} AS from_block,
    {to:UInt32}   AS to_block,
    countIf(in_scope)                                        AS reference_pairs,
    countIf(same_block)                                      AS same_block_pairs,
    countIf(same_block AND r_balance = e_balance)            AS exact,
    countIf(same_block AND r_balance != e_balance)           AS wrong,
    round(exact / reference_pairs, 6)                        AS recall,
    countIf(in_scope AND NOT e_present)                      AS absent_in_extended,
    countIf(in_scope AND e_present AND e_block < r_block AND r_balance = e_balance)  AS older_in_extended_same_balance,
    countIf(in_scope AND e_present AND e_block < r_block AND r_balance != e_balance) AS older_in_extended_stale_suspect,
    countIf(r_in_interval AND e_present AND e_block > r_block)                       AS newer_in_extended,
    countIf(e_present AND e_block <= {to:UInt32} AND NOT r_present)                  AS only_in_extended
FROM
(
    SELECT
        address,
        r.present AS r_present, r.block_num AS r_block, r.balance AS r_balance,
        e.present AS e_present, e.block_num AS e_block, e.balance AS e_balance,
        r_present AND r_block <= {to:UInt32} AS r_in_interval,
        r_in_interval AND NOT (e_present AND e_block > r_block) AS in_scope,
        in_scope AND e_present AND e_block = r_block AS same_block
    FROM
        (SELECT address, block_num, balance, 1 AS present
         FROM rpc.native_balances FINAL WHERE block_num >= {from:UInt32}) AS r
    FULL OUTER JOIN
        (SELECT address, block_num, balance, 1 AS present
         FROM extended.native_balances FINAL WHERE block_num >= {from:UInt32}) AS e
    USING (address)
)
SETTINGS join_use_nulls = 0;

-- The native same-block pairs that differ; this must return no rows.
SELECT address, r.block_num AS block_num, r.balance AS rpc_balance, e.balance AS extended_balance
FROM (SELECT address, block_num, balance FROM rpc.native_balances FINAL
      WHERE block_num >= {from:UInt32}) AS r
INNER JOIN (SELECT address, block_num, balance FROM extended.native_balances FINAL
            WHERE block_num >= {from:UInt32}) AS e
USING (address)
WHERE r.block_num = e.block_num AND r.block_num <= {to:UInt32} AND r.balance != e.balance
ORDER BY block_num, address;

SELECT throwIf(count() > 0, 'native balances differ at the same block: see the rows above')
FROM (SELECT address, block_num, balance FROM rpc.native_balances FINAL
      WHERE block_num >= {from:UInt32}) AS r
INNER JOIN (SELECT address, block_num, balance FROM extended.native_balances FINAL
            WHERE block_num >= {from:UInt32}) AS e
USING (address)
WHERE r.block_num = e.block_num AND r.block_num <= {to:UInt32} AND r.balance != e.balance;
