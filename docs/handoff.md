# Session handoff (2026-09-21): roadmap state, evidence, and how to continue

The subsequent [offline issue-audit remediation](audit-remediation-2026-09-21.md)
tracks retention correctness, executed layout tests, ERC-4626 arithmetic and
asset bindings, Aragon upgrade guards, execution receipt validation and a
bounded legacy-profile candidate migration. Consult that record for the
follow-up implementation and validation; the historical evidence below keeps
its original scope.

This page is the entry point for anyone picking up the roadmap in
[#21](https://github.com/pinax-network/substreams-evm-extended/issues/21). It
records what was built between 2026-09-18 and 2026-09-21, what is verified and
what is only inferred, the open review findings, where every artifact lives,
and the ordered next steps. Procedural know-how is in [`../skills/`](../skills/README.md).

## 1. Read this first: the constraints

- **Live use on BSC resumed on 2026-09-22.** The owner supplied the Pinax
  BSC endpoints (`bsc.substreams.pinax.network:443`,
  `bsc.firehose.pinax.network:443` and a key-bearing RPC URL). Keep the key
  and the RPC URL in environment variables only (`SUBSTREAMS_API_KEY`,
  `RPC_URL`); never write them to the repository, evidence or logs, keep
  live ranges modest and always pass a stop block to `substreams run`. Other
  networks (#8) have no endpoints yet, and the `dex/pool-state` live hold in
  `AGENTS.md` still needs its own explicit reauthorization. The first live
  qualification is `aave/balance-state` (#13, see its README); everything
  else below was done offline and says so.
- **Production boundary** ([`../AGENTS.md`](../AGENTS.md)): one RPC-free
  `map_events` per package; `evm.balances.v1` field numbers frozen; protocol
  packages emit the companion `evm.balance_state.v1`; no `db_out`, no custom
  sink; unknown writes and unreviewed runtime/dependency changes fail closed;
  all tooling in Rust and excluded from the WASM path; never print
  access-bearing endpoint URLs or secrets.
- **Two agents work in this repository.** Branches prefixed `codex/` (PRs #9,
  #10, #40 `dex/pool-state`) come from another assistant. Coordinate through
  the GitHub issues; do not rewrite their evidence or packages.

## 2. What exists now

| Package (crate) | Output | Issue | Evidence | Slot provenance |
| --- | --- | --- | --- | --- |
| `erc20/balances` | `evm.balances.v1` | pre-existing production module; #2–#6, #22 | RPC-qualified historical evidence under `erc20/balances/docs`; typed-path baseline replay (1,024 blocks, 110,139 rows, 4,012 retained matches, 66,265 cold unknowns) | RPC-qualified layouts |
| `native/balances` | `evm.balances.v1` (`contract` absent) | #17 open (BSC live-qualified; other networks #8) | **live 2026-09-23**: 1,024 saved control blocks equal to the offline replay and `eth_getBalance` (76,139/76,139); 5,000 live blocks, 200,344/200,344 same-block `eth_getBalance` ([evidence](../native/balances/docs/evidence)); saved replay 1,439 blocks | n/a |
| `erc20/events` | `erc20.events.v1` | #19 closed | BSC single-tx fixtures | n/a |
| `evm/executions` | `evm.executions.v1` | #18 closed; producer semantics under #8 | saved-block replay: 1,509 BSC v4/v5 blocks, 116,951 txs, 1,075,108 receipt logs matched, 2,093,149 writes in storage context, 0 errors, determinism checked ([evidence](../evm/executions/docs/evidence/replay-bsc-v4-v5.json)); live: 250 final v5 blocks, 0 errors ([evidence](../evm/executions/docs/evidence/replay-bsc-live-2026-09-23.json)) | n/a |
| `aave/actions` | `aave.actions.v1` | #20 | **live 2026-09-23**: 459/459 Pool receipt logs equal persisted rows field by field over 2,397 blocks, all six kinds ([evidence](../aave/actions/docs/evidence/live-parity-bsc-2026-09-23.json)); compiled v3.7.0 event ABI with V3.0/V2 comparison | n/a |
| `aave/balance-state` | `evm.balance_state.v1` | #13 | **live-qualified on BSC** for epoch 101,087,794/3347 over 2,060 blocks: packaged output, same-block `scaledBalanceOf`/`balanceOf`/reserve parity, 0 mismatches ([evidence](../aave/balance-state/docs/evidence/live-parity-bsc-2026-09-22-rev3.json)); saved-block replay 1,433 blocks | **compiler-verified** (aave-v3-origin `8305565a`, solc 0.8.27): aToken 52/53/54/58, Pool `_reserves` 52 |
| `compound-v2/balance-state` | `evm.balance_state.v1` | #14 open | synthetic tests only | **compiler-verified**: deployed cUSDC/cETH against the 2019 tree (`solc 0.5.17`, `_guardCounter` at slot 0), cUSDC IRM `LegacyJumpRateModelV2`, delegators against `solc 0.8.10`; USDC FiatToken 0.6.12; `tests/storage_layout.rs` |
| `compound-v3/balance-state` | `evm.balance_state.v1` | #15 open | synthetic tests only; review findings fixed (§4) | **compiler-verified** (`solc 0.8.15`, `tests/storage_layout.rs`) |
| `lido/balance-state` | `evm.balance_state.v1` | #23 open | synthetic tests; named slots asserted `== keccak256(name)` | **ast-derived** (`solc 0.4.24` AST; all 16 position constants configured or reviewed) |
| `erc4626/balance-state` | `evm.balance_state.v1` | #24 | **live 2026-09-23 (BSC static aToken)**: 162/162 shares, `convertToAssets` and `rate()` via conformance, 297 reserve words, 63 supplies over 2,064 blocks ([evidence](../erc4626/balance-state/docs/evidence/live-parity-bsc-stata-2026-09-23.json)); sDAI and OZ epochs are Ethereum placeholders (#8) | **compiler-verified** (StaticATokenLM 0.8.20, SavingsDai 0.8.17, Pot 0.6.12, OZ constants) |
| `common/persist` | library | – | shared copy of `erc20/balances` persistence rules; fixtures reused | – |
| `common/retention` | host library | #7 open | seven scenario tests ([spec](initialization-and-completeness.md)) | – |
| `conformance` | host library | #16 open | Aave has a captured-block index oracle; Comet, Compound v2, Lido, ERC-4626 are source-line models with synthetic tests | – |
| `dex/pool-state` | `dex` protos | (other agent, PR #40) | see its README | – |

Merged this pass: PRs #25–#39 (this agent) and #40 (other agent); later
#46 (audit remediation), #47 (pointer contract) and the `evm/executions`
regression PR. Closed issues: #11, #12, #18, #19, #22. Open with offline
progress recorded in a comment: #7, #13, #14, #15, #16, #17, #20, #23, #24. Not started because they need
RPC or SPKG qualification: #2, #3, #4, #5, #6, #8.

## 3. Where the artifacts live

| What | Where |
| --- | --- |
| Roadmap requirements and open questions | [`extraction-coverage.md`](extraction-coverage.md), [`follow-up.md`](follow-up.md) |
| Balance-state contract and its rejected alternatives | [`balance-state-contract.md`](balance-state-contract.md), [`design/balance-state-alternatives/`](design/balance-state-alternatives/README.md) |
| Initialization / completeness spec | [`initialization-and-completeness.md`](initialization-and-completeness.md) |
| Pinned-source research notes (JSON) | [`research/`](research/README.md) |
| Saved-block scans | [`evidence/scans/`](evidence/scans/README.md) |
| Storage-slot provenance and the solc verification plan | [`storage-layout-provenance.md`](storage-layout-provenance.md) |
| Open review findings | [`review-findings-2026-09-21.md`](review-findings-2026-09-21.md) |
| Ad-hoc block scanners (trim, probe, aave, stata) | [`../tools/scratch-scan/`](../tools/scratch-scan/README.md) |
| Per-package replay tools | `native/balances/tools`, `aave/balance-state/tools`, `erc20/balances/tools` |
| Skills (procedures) | [`../skills/`](../skills/README.md) |

### Locally cached Extended blocks (not in git; on the original machine only)

41 directories, 6,093 `<height>.pb` files, all BSC (chain id 56), all
`Block.ver` 5 except the 71 v4 blocks in the vote/voting/hlbp/star directories.
The main contiguous window is `erc20/balances/out/top50-full-holder-blocks`
= [122288006, 122289030), 1,024 blocks, complemented by
`top50-holder-extension512` [122288458, 122288970) and `-extension60`
[122288970, 122289030). Other directories are samples of the same window
(`ranks*-sample-blocks`, `next50-sample-blocks`, `computed/clone/deployment/
fallback-holder-blocks`) or single blocks for specific tokens (`4stock-*`
120607788+, `hlbp-active-blocks` 104727168+, `shareholder-removal-*`
122264480+, `star-mint-blocks` 58597371+, `vote-history-blocks`). If the
machine is gone, these must be recaptured with the `erc20/balances/tools`
capture command once live use resumes.

## 4. Hardening review status

On 2026-09-21 a review workflow (20 finders = 5 crates x 4 dimensions, 3
adversarial verifiers per finding, 6 solc layout extractors) was started
against PRs #33–#38. The subagent spend limit stopped it after 4 of 110 agents;
only the four `compound-v3` finders returned. Their 28 unverified findings are
in [`review-findings-2026-09-21.md`](review-findings-2026-09-21.md). The
dimensions were: pinned-source conformance; fail-closed and persisted-effect
semantics; balance-state contract conformance; test adequacy versus the issue
acceptance list.

Must-fix, in order (the first was confirmed by four independent reviewers and
re-derived by the maintainer from `CometCore.sol:60` and `keccak256` of the label):

1. **Done for `compound-v3` (PR "compound-v3 hardening").** The guard slot is
   reviewed by name (`other_slot_names: ["comet.reentrancy.guard"]`), asserted
   equal to `0xc98c7730…53ac` in a test, and a routine supply in a
   delegatecall frame with the `0→1→0` guard writes is a test case.
2. Row semantics for packed words: the proto says one row per decoded field
   of a written slot. **Done everywhere** (`compound-v3` market words and
   holder rows; `lido` packed halves; `erc4626` Aave reserve fields).
3. `producer_versions` restricted to 4 and 5: **done in every map crate**
   (`compound-v3`, `compound-v2`, `lido`, `erc4626`, `aave/balance-state`,
   `native/balances`), each with a parse test.
4. INVALIDATED instead of failing the block on pointer writes and code
   changes: **done in every balance-state package.** The first version for
   `compound-v3` and `aave` treated a write onto the bound implementation as
   the binding, which violated the proto's `STORAGE_POINTER` rule and would
   have concealed an in-block excursion X→Z→X. All five packages now
   invalidate on every persisted pointer write, including equal-value and
   restored writes, with per-write evidence, and honour `activation_ordinal`
   so an epoch can be bound at its own upgrade block (see "Activation position
   and pointer writes" in `balance-state-contract.md`). The Aave saved-block
   replay reproduces the committed rows byte for byte after the change
   (`aave/balance-state/docs/evidence/replay-bsc-v5-rev3.json`).
5. Constant cross-checks and carryover flags: **done for `compound-v3`**
   (`base_index_scale == 1e15`, `factor_scale == 1e18`, `base_scale ==
   10^decimals`, `global_carryover = false`) and **`compound-v2`**
   (`blocks_per_year == 2102400`, `global_carryover = false`); `erc4626` has
   no pinned constant to check beyond the ERC-7201 namespace slot, which is
   now re-derived in a test.
6. `conformance::comet`: **done** (int104-min negation refused; exact rates
   on both sides of the kink and at the market's utilization; exact indices
   after 3600 s and one year; uint64 overflow arms).
7. Tests listed in the findings: **done everywhere.** `compound-v3` has 14
   tests; the shared subset (producer versions, delegatecall frame shape,
   FAILED and REVERTED transactions, pre-activation blocks, contextual
   `reduce()` diagnostics, determinism under permutation) and then the full
   `validate_block` refusal table, same-block provenance and multi-market
   attribution were mirrored into `compound-v2`, `lido`, `erc4626` and
   `aave`.
8. Re-run the review for `compound-v2`, `lido`, `erc4626`, `common/retention`.
   **Done 2026-09-22: `common/retention` (12 findings), `erc4626` (10, two
   high), `lido` (7) and `compound-v2` (10, one high: the deployed cUSDC and
   cETH run the 2019 layout) reviewed independently; all fixed (see the
   findings doc).**
   Earlier state: blocked twice. A second workflow (16 finders,
   no verifiers) was launched on 2026-09-21 and every one of its 16 agents
   died on the account spend limit, so those four crates have never been
   reviewed by an independent reader. Their fixes so far came from applying
   the `compound-v3` findings by analogy, not from evidence about them. When
   credit is available, re-run
   `workflows/scripts/review-remaining-balance-state-crates-wf_2c8ed0bb-fb7.js`
   (run id `wf_2c8ed0bb-fb7`, nothing cached) or review by hand with the four
   dimensions above. Treat this as the highest-value open item: the one crate
   that *was* reviewed turned out to have a defect that would have failed
   every real block.

### Mixed-provenance pull request (merged)

[PR #46](https://github.com/pinax-network/substreams-evm-extended/pull/46)
combined the offline issue-audit remediation (authored by a concurrent session
in this same working tree; see
[`audit-remediation-2026-09-21.md`](audit-remediation-2026-09-21.md)) with the
sibling-crate refusal tests. It was reviewed and merged on 2026-09-22. It
changed the `common/retention` host API: `seed_checkpoint` and
`seed_deployment_zero` now require explicit identities.

## 5. Facts learned that are not written anywhere else

- USDC (FiatToken V2.2) stores the blacklist flag in bit 255 of the same
  mapping word as the balance (`balanceAndBlacklistStates`, slot 9) and
  `_balanceOf` masks it; any "raw balances slot" model of USDC must decode
  255 bits, which the compound-v2 cash model does via `value_bits`.
- The BSC producer (versions 4 and 5) records **no** equal-value storage
  change: none of 2,093,149 storage changes in the saved data is equal-valued
  (`evm/executions/docs/evidence/replay-bsc-v4-v5.json`). An earlier version of
  this page claimed the opposite; that was wrong. `common/persist` would route
  such a record to `storage_noop` anyway. A reentrancy flag that goes 0→1→0
  within one frame is two *changing* writes that reduce to `old == new`; it is
  still a write and must be a reviewed slot.
- The same producer **does** record a code change whose old and new code are
  identical: a SetCode authorization re-delegating an account to its current
  target (86 cases in the saved data). `evm/executions` keeps the row as
  `DELEGATION_SET` with `persisted = false` (it compares the hashes).
  `common/persist` does not compare them and passes the change on, so a
  balance-state package would invalidate a watched contract's epoch on it:
  fail-closed, like an equal-value pointer write. (A watched protocol contract
  has code and cannot be a SetCode authority, so this is not expected.)
- Storage context: a `DELEGATE` or `CALLCODE` frame writes its `caller`'s
  storage, every other frame its own `address`; this held for all 2,093,149
  saved writes.
- Firehose facts observed in saved blocks: a contract-creation transaction
  has `to` equal to the created address; logs of reverted frames keep their
  receipt `index` (with `block_index` 0) and are absent from the receipt;
  the root call is not necessarily call index 0 (use depth 0 / minimum
  index); BSC fee resets appear as `REASON_REWARD_TRANSACTION_FEE`; reason 17
  is `REWARD_BLOB_FEE` (from the upstream proto), 18/19 are OP-stack, 20 Monad.
- Lido v4 computes the share rate as `internalEther / internalShares`
  (`totalShares - externalShares`), not `totalPooledEther / totalShares`; the
  two differ in integer truncation.
- Aave V3 BSC: Pool `_reserves` mapping is slot 52; `ReserveData` word +1 is
  `liquidityIndex` (low 128) | `currentLiquidityRate` (high 128) and word +3
  holds `lastUpdateTimestamp` at bits 128..168; aToken `_userState` is slot
  0x34 (scaled balance low 120 bits, `additionalData` high 128), allowances
  0x35, `_totalSupply` 0x36; EIP-1967 implementation slot
  `0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc`.
- The captured Aave index oracle `rayMul(linearInterest(prev rate, prev ts,
  now), prev index)` matched every stored index update in the replay.
- zsh does not word-split unquoted `$VAR`; use `${=VAR}` in scripts.
- `gh pr checks` output is tab-separated; the repository has no auto-merge;
  CI runs `clippy --all-targets -D warnings` (rustdoc lints included) and a
  `--locked` WASM check, so new crates need a committed `Cargo.lock` update and
  `#[cfg(target_arch = "wasm32")] mod handler`.

## 6. Next steps, in order

1. **Done:** the `compound-v3`-only tests (validate_block table, same-block
   provenance, multi-market attribution) are mirrored in every sibling.
2. **Done:** the independent review of `compound-v2`, `lido`, `erc4626` and
   `common/retention` (§4 item 8), PRs #49–#52.
   Follow-ups it exposed, not yet done: (a) every package refuses a second
   epoch for the same market, so `basis_carryover` cannot be exercised inside
   one parameter set, and the retention ledger binds the parameter hash;
   supporting successor epochs means accepting several `(market, epoch)`
   entries with increasing activation in every package; (b) only
   `compound-v2` stops decoding after an in-block invalidation; the siblings
   still fail a block whose upgrade writes unknown storage after the pointer
   write; (c) `aave/balance-state` was not part of this review.
3. **Done 2026-09-21:** solc storage layouts for every pinned contract are
   committed under `docs/evidence/storage-layouts/` with a `tests/storage_layout.rs`
   per package ([provenance](storage-layout-provenance.md)). The USDC FiatToken
   family was compiled the same day (v2.2.0): slot 9 is
   `balanceAndBlacklistStates`, so the compound-v2 cash row now decodes only
   the low 255 bits (`value_bits`). No inferred slot remains.
4. Live on BSC: **done for `aave/balance-state`** (runtime epoch bound
   from the upgrade writes, compiled layout, 2,060 packaged-output blocks
   with same-block getter parity; it also found an unreviewed `_nonces`
   write that halted a real block). Next with the same method
   (`aave-balance-state-tools live-parity` is the template): **done for
   `native/balances`** (#17, `native-balances-tools live-parity`, batched
   `eth_getBalance`) and **`aave/actions`** (#20, `aave-actions-tools`,
   receipt-log parity from the compiled ABI) and the **BSC static aToken**
   (#24, `erc4626-balance-state-tools`) and the **`evm/executions` producer
   checks** (250 live final blocks, `evm-executions-tools replay`, 0 errors);
   next the RPC-gated ERC-20 issues #2–#6. Ethereum and the
   other networks (#8) wait for endpoints.
