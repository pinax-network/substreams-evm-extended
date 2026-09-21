# Session handoff (2026-09-21): roadmap state, evidence, and how to continue

This page is the entry point for anyone picking up the roadmap in
[#21](https://github.com/pinax-network/substreams-evm-extended/issues/21). It
records what was built between 2026-09-18 and 2026-09-21, what is verified and
what is only inferred, the open review findings, where every artifact lives,
and the ordered next steps. Procedural know-how is in [`../skills/`](../skills/README.md).

## 1. Read this first: the constraints

- **Live use is paused.** Every roadmap issue states that live Substreams,
  Firehose, RPC and native sink usage stays paused until explicitly resumed.
  Everything below was done offline: Rust tests, replays over the locally
  cached BSC Extended blocks, and pinned-source research through raw GitHub
  fetches and official documentation. Nothing here qualifies a package for
  production; each README and issue comment says which claims are verified
  from saved blocks and which are declaration-order inferences.
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
| `native/balances` | `evm.balances.v1` (`contract` absent) | #17 open | saved-block replay: 1,439 v5 blocks, 126,180 rows, 86,564 continuity checks, 0 mismatches; 1,510 with v4 ([evidence](../native/balances/docs/evidence)) | n/a |
| `erc20/events` | `erc20.events.v1` | #19 closed | BSC single-tx fixtures | n/a |
| `evm/executions` | `evm.executions.v1` | #18 open | BSC single-tx fixtures | n/a |
| `aave/actions` | `aave.actions.v1` | #20 open | BSC single-tx fixtures (borrow, supply) | n/a |
| `aave/balance-state` | `evm.balance_state.v1` | #13 open | saved-block replay: 1,433 BSC blocks, index oracle 6/6, 0 errors ([evidence](../aave/balance-state/docs/evidence/replay-bsc-v5.json)) | **observed** from Keccak preimages in saved blocks (Pool `_reserves` 52; aToken 0x34/0x35/0x36) |
| `compound-v2/balance-state` | `evm.balance_state.v1` | #14 open | synthetic tests only | inferred from pinned declaration order |
| `compound-v3/balance-state` | `evm.balance_state.v1` | #15 open | synthetic tests only; **open high finding** (§4) | inferred; reviewers re-derived slots 0/1/5 and packing from `CometStorage.sol` |
| `lido/balance-state` | `evm.balance_state.v1` | #23 open | synthetic tests; named slots asserted `== keccak256(name)` | unstructured slots verified by hashing; `shares` slot 0 inferred |
| `erc4626/balance-state` | `evm.balance_state.v1` | #24 open | synthetic tests; no static-aToken activity in saved blocks ([scan](evidence/scans/stata-scan.json)) | inferred (StaticATokenLM, SavingsDai, Pot, ERC-7201 namespace) |
| `common/persist` | library | – | shared copy of `erc20/balances` persistence rules; fixtures reused | – |
| `common/retention` | host library | #7 open | seven scenario tests ([spec](initialization-and-completeness.md)) | – |
| `conformance` | host library | #16 open | Aave has a captured-block index oracle; Comet, Compound v2, Lido, ERC-4626 are source-line models with synthetic tests | – |
| `dex/pool-state` | `dex` protos | (other agent, PR #40) | see its README | – |

Merged this pass: PRs #25–#39 (this agent) and #40 (other agent). Closed
issues: #11, #12, #19, #22. Open with offline progress recorded in a comment:
#7, #13, #14, #15, #16, #17, #18, #20, #23, #24. Not started because they need
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

Must-fix, in order (the first is confirmed by four independent reviewers):

1. `compound-v3`: review `keccak256("comet.reentrancy.guard")` =
   `0xc98c7730ba19013824f711a9ab74801459b27e6ff7685cb924587c89aeda53ac`
   (`CometCore.sol:60`) as a scalar slot, derived in code from the pinned
   label the way `lido` handles named slots, and add a test with the real
   transaction shape (guard 0→1→0 in the same frame as a `userBasic` write,
   delegatecall frame whose `Call.address` is the implementation).
2. Row semantics for packed words: the proto says one row per decoded field
   of a written slot. `compound-v3` filters totals fields but not index
   fields; `lido` and `erc4626` filter unchanged halves; `compound-v3` skips
   holder rows whose principal is unchanged. Decide once, apply to all four
   crates, update READMEs and tests.
3. `producer_versions` must be restricted to 4 and 5 in every crate (the
   contract refuses version 3).
4. `compound-v3` (and `aave/balance-state`) fail the block on an
   implementation-pointer write or code change; `compound-v2`, `lido`,
   `erc4626` emit `INVALIDATED` rows and keep decoding. Make them consistent
   (INVALIDATED is what `common/retention` consumes).
5. Cross-check echoed constants (`base_index_scale == 1e15`, `factor_scale ==
   1e18`, `base_scale == 10^decimals`) and make carryover flags explicit.
6. `conformance::comet::borrow_balance_of` must refuse `principal == -2^103`
   (checked negation reverts in the pinned source); pin exact above-kink
   rates and long-elapsed index values; exercise the uint64 overflow arms.
7. Add the missing tests listed in the findings (failed/reverted
   transactions, same-block repeats with provenance, `validate_block`
   refusals, pre-activation blocks, multi-market blocks, determinism).
8. Re-run the review for `compound-v2`, `lido`, `erc4626`, `common/retention`.

## 5. Facts learned that are not written anywhere else

- `common/persist` drops storage writes whose old and new values are equal
  (Firehose records them; the rules do not persist them). A reentrancy flag
  that goes 0→1→0 within one frame is two persisted writes that reduce to
  `old == new`; it is still a write and must be a reviewed slot.
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

1. Fix the `compound-v3` findings (§4 items 1–7) in one PR; mirror the
   cross-crate items in the siblings; comment on #15, #14, #23, #24.
2. Finish the review for the other four crates (workflow resume or by hand).
3. Run the solc storage-layout verification in
   [`storage-layout-provenance.md`](storage-layout-provenance.md) and commit
   the layouts as evidence with tests that pin fixture slots to them.
4. When live use is explicitly resumed: capture Ethereum Extended blocks for
   the bound contracts, verify slots from preimages, bind runtime code hashes
   and activation blocks, run same-block-hash getter parity with the
   `conformance` models, then the RPC-gated issues #2–#6 and #8.
