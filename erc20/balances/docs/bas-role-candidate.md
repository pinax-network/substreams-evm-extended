# BAS exact membership and fixed-admin candidate

Issue [#4](https://github.com/pinax-network/substreams-evm-extended/issues/4)
has a separate **NOT-QUALIFIED** BAS candidate for
`0x0f0df6cb17ee5e883eddfef9153fc6036bdb4e37`. Legacy role root6 width2
becomes exact `[bytes32,address]` membership, offset0, width1. Exactly one
fixed PAUSER admin scalar is added. All other profile fields and deployment
and runtime guards remain intact. The historical431 fixture and qualified425
cohort are unchanged.

The [complete fixture review](../tests/fixtures/bas-role-candidate/README.md)
records all14 sources, exact capture/compiler/layout/runtime binding, the
sole cap immutable at two sites, and the independently encoded full
constructor append. Original match labels remain `match`. All13 OpenZeppelin
dependencies match pinned primary sources; none of six public BAS token
revisions matches the captured token. That source gap remains explicit.
Saved compiler reconstruction is not fresh Solidity compilation or current
runtime/deployment qualification.

The sole captured `_setRoleAdmin` call is constructor-only and selects
PAUSER_ROLE. The candidate admits only that fixed word, not a generic admin
path. The inherited getter reads ordinary balances; membership/pause/whitelist
state does not enter it. Synthetic Extended-record tests exercise exact
shapes, refusal boundaries and preserved balances. They do not execute role
transactions or qualify actual producer visibility.

## Saved-window evidence

The [final report](evidence/bas-role-candidate-20260928.json) and
[source inventory](evidence/bas-role-candidate-20260928-source-inputs.json)
cover the complete BSC interval **[122288006,122289030)**. The as-run runner,
block digests, native output and per-block comparisons remain under
`out/bas-role-20260928/replay-01/` in the `bsc-exclusion-review` worktree.

- All 1,024 blocks and 110,139 emitted rows match the unchanged current
  baseline and historical capture across every Events/Balance protobuf field,
  including optional contract presence. Only row ordering is normalized.
- Canonical comparisons have 110,138 same-block matches and 4,012 retained
  reference-only matches, with no mismatches. One emitted row has no canonical
  counterpart and is checked only against both baselines.
- The ledger initializes 33,591 observed holders, including 6,197 known zeros
  at the final block. No checkpoint or deployment seeds are used; 66,265 cold
  reference observations remain unknown.

| BAS result | Count |
| --- | ---: |
| Emitted rows / same-block canonical matches | 90 / 90 |
| Initialized observed holders | 15 |
| Cold reference observations | 88 |
| Cold observations with nonzero reference value | 39 |
| Retained reference-only matches | 0 |
| Persisted membership writes | 0 |
| Persisted fixed PAUSER admin writes | 0 |

Canonical values never initialize or repair retained state, including the 39
nonzero cold observations. Holder counts cover the initialized observed set,
not all BAS holders. No membership or fixed-admin writes occur in this window;
those paths are supported only by source-derived synthetic Extended records.
The replay does not establish actual role-operation visibility.

The runner binds chain, interval, block count, the historical report's exact
reference digest, boundary hashes and every saved parent link. Source hashes
are checked before and after replay. Failure preserves the completed prefix
and report without inserting empty rows; this run completed all 1,024 blocks.

Final offline validation on main `57652e8` plus this patch passed **871 tests
across 49 Cargo result suites**, all-target workspace Clippy with warnings
denied, the workspace WASM check, formatting and the staged whitespace check.
Eleven added tests cover six storage-shape groups, four source/candidate
binding groups and the fixed-admin replay counter. The unique target and
complete logs remain under `out/bas-role-20260928/`.

`source-01/` preserves initial preparation; `source-02/` adds as-run Rust helper
and CLI copies/hashes. `09-final-clippy.log` preserves an allocation style
warning; an equivalent canonical string comparison resolves it in final
`source-03/`. All four generated fixtures remain byte-identical across these
preparations. Final gates and replay use the final source; no historical
evidence is overwritten.

No live RPC, Substreams, Firehose or sink operation ran for this candidate.
Fresh runtime/deployment controls, actual replacement-package/RPC parity,
initialized-holder/final-state checks and actual role-operation visibility
remain promotion gates. The token source gap and other legacy profiles remain
outside this bounded offline implementation; issue #4 stays open.
