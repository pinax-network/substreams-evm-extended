# Cold-start initialization and holder completeness

Issue [#7](https://github.com/pinax-network/substreams-evm-extended/issues/7).
The maps in this repository emit what a block wrote: `evm.balances.v1`
end-of-block balances for holders whose storage or native balance changed, and
`evm.balance_state.v1` holder basis and market words for the protocol
packages. A consumer that answers "what is the balance of X at block N" adds
**retained state** to those rows. This page specifies where retained state may
come from, which cases must stay distinct, how continuity, gaps and forks are
handled, and how completeness is reported. Nothing here adds an RPC call to a
map. The rules are implemented for host-side consumers in
[`common/retention`](../common/retention/src/lib.rs) (`evm-retention`) and, for
the ERC-20 qualification tools, in the earlier
[`HolderState`](../erc20/balances/tools/src/coverage.rs) ledger.

## 1. Supported initialization origins

| Origin | Meaning | Allowed when | Recorded as |
| --- | --- | --- | --- |
| **Complete history** | Every block since the token or market was created has been applied; every holder that ever received a balance has an emitted row | The stream started at or before the creation block and no gap occurred | `Origin::Observed` for every holder; `first_block` at or before creation |
| **Deployment zero baseline** | EVM-initial storage of a token created inside the applied range is zero for every slot, so holders that cannot precede the creation start at a known `0` | Only after the map validated the pinned first `CREATE` of that exact address (code hash, source binding), never for a pre-existing token | `Origin::DeploymentZero { block, hash }`; the ERC-20 tools distinguish this from an RPC checkpoint (`deployment_holder_baseline_is_distinct_from_an_rpc_checkpoint`) |
| **Verified checkpoint** | An independently produced snapshot at the block immediately before the first applied block, with its evidence reference | Values come from a host-side qualification tool (RPC allowed on the host, `eth_call balanceOf` at the checkpoint hash) or a previously published, hash-bound sink state; the checkpoint block must equal `first_block - 1` and its hash must be the first block's parent | `Origin::Checkpoint { block, hash, evidence }` |

Anything else is **unknown**. In particular:

- An absent write is never a zero. `Lookup::Unknown` is the only answer for a
  holder without an origin, and a comparison counts it as a coverage gap,
  never as a match or a mismatch.
- A checkpoint for a token that was not yet created at the checkpoint block
  is rejected by the ERC-20 tools (`known_checkpoint_amount` returns nothing);
  a deployment baseline for a token that already has state is rejected by
  both ledgers. `common/retention` remembers the first block at which it held
  any state for a contract (a checkpoint, a row, a seed or an epoch row), so
  a token whose entries a `BOUND` without carryover dropped is still not new.
  Holders the creation block itself wrote (a constructor mint) keep their
  observed row; only the other listed holders are seeded with `0`.
- One ledger retains one output (`Domain::Balances` or
  `Domain::BalanceState`). `evm.balances.v1` amounts and
  `evm.balance_state.v1` bases of the same token share a key but are different
  quantities: a `Balances` checkpoint holds unsigned `balanceOf` values, a
  `BalanceState` checkpoint holds the market's basis in the units of its
  epoch (negative only for a signed principal). Values are exact uint256 or
  int256 decimals.
- `seed_checkpoint` accepts an explicit 32-byte hash in addition to the
  height and evidence reference. Every seed must share that identity; the
  first applied block must extend it. A hash written only in an evidence
  filename is insufficient.
- `seed_deployment_zero` accepts the full creation `Clock` immediately after
  that exact block was applied and before the next block. Its retained undo
  journal is required (`undo_depth` must be nonzero). The seeds are journaled
  as creation-block effects, so undoing creation makes those holders unknown
  again. The caller still supplies independently qualified creation evidence;
  accepting a clock alone does not prove contract creation.
- Passive state (Aave index, Comet base indices, cToken exchange-rate inputs,
  stETH `totalShares`/`totalPooledEther`) is initialized the same way. A
  holder basis without the market's global words is not evaluable; the
  balance-state packages emit `REAFFIRMED` bindings and constants on a
  heartbeat so a consumer joining mid-stream sees what it still lacks.

## 2. Distinct cases

| Case | `evm.balances.v1` | `evm.balance_state.v1` | Retained lookup |
| --- | --- | --- | --- |
| Missing / cold | no row | no `HolderBasis` row | `Unknown` |
| Uninitialized token or market | no rows before creation or activation | `SUSPENDED` with `UNQUALIFIED_ERA` until bound | `Unknown` (`Suspended` once the market is declared) |
| Reverted effect | never a row (shared [`common/persist`](../common/persist) rules) | never a row | unchanged entry |
| Known zero | `amount == "0"` | `value == "0"` | `Known("0")` |
| Unsupported token / model | the map fails closed on unknown writes; host tools list the contract as unsupported | `SUSPENDED` | `Unsupported(reason)`; rows for it are refused |
| Invalidated dependency | not expressible; the map fails the block | `INVALIDATED` with `reason` and evidence | `Suspended { epoch, reason }` until a new `BOUND` |
| Carryover across an epoch | n/a | `ModelEpoch.basis_carryover` / `global_carryover` | `BOUND` with `basis_carryover == false` drops the market's retained basis to `Unknown` |

## 3. Retention scenarios that must hold

Covered by [`common/retention/src/tests.rs`](../common/retention/src/tests.rs)
with synthetic rows, and by saved-data replays where noted:

| Scenario | Rule | Evidence |
| --- | --- | --- |
| Empty output blocks | entries unchanged; `updated` stays at the last emitted block; the block still advances the clock | unit test; native replay of 1,439 saved BSC blocks with 86,564 cross-block continuity checks ([evidence](../native/balances/docs/evidence/replay-bsc-v5.json)) |
| Passive / reflection / reward changes | a `GlobalState` row touches no holder entry (it is validated, not retained); a holder's evaluated amount moves without a write, so evaluating it needs the market's global words from the stream, which this ledger does not keep; a holder without a row stays unknown | unit test; Aave index oracle in [`aave/balance-state`](../aave/balance-state/docs/evidence/replay-bsc-v5.json); ERC-20 `captured_pending_rewards_make_a_correct_checkpoint_stale_without_balance_writes` |
| Migrations / upgrades | `INVALIDATED` or `SUSPENDED` suspends lookups for the market and `REAFFIRMED` does not resume it; a later `BOUND` of a strictly newer epoch resumes it and decides carryover. Epoch rows apply in `(ordinal, kind)` order, and rows of the previous epoch in an activation block are applied just before the new `BOUND`, so they are carried over or dropped with the rest of the basis. Rows or epoch rows of a stale or future epoch are refused; the ERC-20 map fails closed instead | unit test; `runtime_qualification_rejects_changed_proxy_target_or_implementation_code` |
| Burn / mint transitions | `Known(v)` → `Known("0")` and `Unknown` → `Known(v)` are transitions between distinct states; `since` records the first known block | unit test |
| Final-state snapshot | `compare` reports matches, known-zero matches, mismatches, unknown, unknown-nonzero, unsupported and suspended separately; status is `bounded_parity`, `coverage_gap`, `mismatch`, or `no_reference` for an empty reference | unit test; typed-path baseline replay below |

## 4. Continuity, gaps, forks and publication

- **Exact identities.** Every applied block carries number, hash and parent
  hash. A block whose number is not `last + 1` is a gap and a block whose
  parent hash differs from the retained hash is a fork; both are refused. The
  maps emit exactly one `BlockClock` per block for this purpose and the
  `evm.balance_state.v1` number, hash and parent hash must match the applied
  block.
- **Atomic application.** Every holder, global and epoch row is validated
  (known enum values, `END_OF_BLOCK` holder rows, a sign only for a signed
  principal, exact decimals, the stream's chain, the market's epoch sequence)
  before changing retained entries, suspensions, epochs, clocks, counters or
  the undo journal. A refused block has no effect and can be retried after its
  cause is resolved. Unsupported contracts are refused by both row APIs and
  by both seeding calls.
- **Undo.** Forks are resolved by `undo(to_number)`, which restores the exact
  prior entries and suspensions from a journal bounded by `undo_depth`. An
  undo beyond the journal fails; the consumer then re-initializes from a
  checkpoint at or before the fork point. Substreams delivers this as
  `BlockUndoSignal`, or the consumer requests final blocks only.
- **Runtime continuity.** Producer versions are explicit parameters of every
  map; a block with an unlisted `Block.ver` fails instead of degrading. The
  replays keep unqualified-version blocks in a separate count and reset their
  continuity run at gaps and refusals.
- **Sink publication.** The native sink consumes the protobuf as emitted. A
  published state is complete for a block only when every row of that block
  and every earlier block since the initialization origin is stored, and the
  stored clock chain is unbroken from the origin to that block. Partial
  publication of a block is not a state: consumers check the clock chain and
  each block's `BlockClock` row counts against the rows stored for it, and
  `apply_state` refuses a block whose counts differ. A balance-state ledger
  also binds the stream identity of its first block (chain, package, package
  version, spec revision, parameters SHA-256) and refuses a block from a
  different stream; a changed package or parameter set starts a new ledger
  from a checkpoint.

## 5. Reporting completeness

A report states, separately and without summing them into one "coverage"
figure:

| Figure | Source | Never implies |
| --- | --- | --- |
| Emitted rows | map output for the interval | holders |
| Initialized observed holders | entries with `Origin::Observed` | the token's holder universe |
| Checkpoint-seeded and deployment-seeded holders | entries by origin | that the checkpoint was complete |
| Known-zero holders | entries with value `"0"` | that other holders are zero |
| Cold unknown lookups / rows | queries or reference rows without an entry | errors |
| Globally enumerated set | only from independent evidence attached with its reference (`attach_enumeration`) at the latest applied block; every attachment is reported with its block and the holders it lists that were not retained at that block, and undoing the block discards it | correctness of values |
| Entries of unsupported contracts | entries retained before `mark_unsupported`, counted as `unsupported_entries` and excluded from every holder count | anything about that contract |

The tested interval and the initialized observed-holder set are part of every
claim. The existing ERC-20 evidence follows this shape: the
[typed-path baseline replay](../erc20/balances/docs/evidence/typed-path-baseline-replay.json)
over BSC blocks [122288006, 122289030) reproduces 110,139 historical rows,
retains 4,012 reference observations that matched without a same-block row,
and leaves 66,265 cold observations unknown. Those unknowns are reported, not
initialized, and not zero. Matching samples do not establish universal token
or global-holder support.

## 6. Live evidence and what remains

**Verified checkpoint, done on BSC (2026-09-23).**
`native-balances-tools retention-check`
([report](../native/balances/docs/evidence/live-retention-checkpoint-bsc-2026-09-23.json))
took `eth_getBalance` for 14,172 accounts at the hash of block 123,548,069
and loaded it through `seed_checkpoint`. The accounts are the 12,920 that the
next 300 blocks emit, plus 1,252 from an earlier window that those blocks
never emit. The tool applied the packaged `native_balances` output of blocks
123,548,070–123,548,369 (41,679 rows) and compared every retained value with
`eth_getBalance` at four block hashes. All 56,688 comparisons were equal, with
no unknown lookup. Accounts without a row kept their checkpoint value, and
the block clock chain extended the checkpoint identity. This covers the
tracked accounts over one window; accounts outside the checkpoint stay
unknown.

Still open:

- Sink-side clock-chain verification against a published state (needs a
  native sink run).
- Reorg handling against a live stream with `BlockUndoSignal`. The BSC
  qualification runs stream final blocks only; `undo` is covered by the
  offline ledger tests.
