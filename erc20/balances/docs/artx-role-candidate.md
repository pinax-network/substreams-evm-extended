# Artx exact proxy membership candidate

Issue [#4](https://github.com/pinax-network/substreams-evm-extended/issues/4)
has a separate **NOT-QUALIFIED** Artx candidate for proxy
`0x8105743e8a19c915a604d7d9e7aa3a060a4c2c32`. It changes only legacy root 151
width 2 to exact `[bytes32,address]` membership, offset 0, width 1. No admin path,
scalar permission or parser extension is added. Balances, allowances,
initializer flags, owner/counters, proxy pointer and both runtime/deployment
guards stay unchanged; inherited gaps remain unsupported.

The [complete fixture review](../tests/fixtures/artx-role-candidate/README.md)
binds both raw captures, all 35 input sources and exact compiler output sets.
The proxy preserves 14 inputs but exactly 8 selected source IDs/metadata;
the implementation has 21 aligned sources. All 34 dependency files match exact
primary pins; the BUSL-1.1 token's public primary source remains unresolved.
No source normalization or inferred token pin is used.

The helper reconstructs the full 170-byte proxy runtime, all 10,348 bytes of
implementation runtime with three independently bound UUPS self immutables,
and both complete creation objects. Its independent 480-byte proxy constructor
encoding preserves the exact Ultiland/ARTX zero-supply/empty-array payload,
without inferring initialized holders, current pointer/owner or qualified
creation. Saved compiler reconstruction is not fresh compilation.

Complete source review finds no role-admin call beyond the inherited unused
declaration. The getter reads the balance mapping at slot 51. Synthetic Extended
records exercise permitted membership and refused surrounding shapes, not
transaction authorization or actual producer role-operation visibility.

## Saved-window evidence

The [final report](evidence/artx-role-candidate-20260928.json) and
[source inventory](evidence/artx-role-candidate-20260928-source-inputs.json)
cover the complete BSC interval **[122288006, 122289030)**. The as-run runner,
block digests, native output and per-block comparisons remain under
`out/artx-role-20260928/replay-01/` in the `bsc-exclusion-review` worktree.

- All 1,024 blocks and 110,139 emitted rows match the unchanged current
  baseline and historical capture across every Events/Balance protobuf field,
  including optional contract presence. Only row ordering is normalized.
- Canonical comparisons have 110,138 same-block matches and 4,012 retained
  reference-only matches, with no mismatches. One emitted row has no canonical
  counterpart and is checked against both baselines only.
- The ledger initializes 33,591 observed holders, including 6,197 known zeros
  at the final block. No checkpoint or deployment seeds are used; 66,265 cold
  reference observations remain unknown.

| Artx result | Count |
| --- | ---: |
| Emitted rows / same-block canonical matches | 66 / 66 |
| Initialized observed holders | 11 |
| Cold reference observations | 36 |
| Cold observations with nonzero reference value | 33 |
| Retained reference-only matches | 0 |
| Persisted membership writes | 0 |

Canonical values never initialize or repair retained state, including the
33 nonzero cold observations. Holder counts cover the initialized observed
set, not every Artx holder. This window contains no membership writes;
the new path is covered by source review and synthetic Extended records,
not actual producer role-operation evidence.

The runner binds the canonical chain, interval, block count, historical
reference digest, boundary hashes and every saved parent link. All 154 source
input hashes stay unchanged during replay. Failure preserves its completed
prefix and report without inserting empty rows; this run completed all blocks.

Final offline validation on main `79ee1dd` plus this patch passed **892 tests
across 53 Cargo result suites**, all-target workspace Clippy with warnings
denied, the workspace WASM check, formatting and staged whitespace checks.
Eleven new tests cover five storage-shape groups and six source/candidate
binding groups. The unique target and complete logs remain under
`out/artx-role-20260928/`.

`01-prepare.log` and `initial-compile-artx_role.rs` preserve the initial Rust
reference-comparison compile error. The corrected preparation is retained in
`source-01/`, including exact as-run helper/CLI source and hashes, both complete
captures, original primary proof and all 34 freshly fetched primary files.
The binding helper, preparation CLI and five generated fixture files remain
byte-identical to that preparation. The additive runner was validated after
integrating Tagger. Historical evidence remains unchanged.

Fresh authorized runtime/dependency controls, actual replacement-package/RPC
parity, initialized-holder/final-state checks and actual role-operation
observations remain promotion gates. The token-source gap is unresolved.
No live RPC, Substreams, Firehose or sink operation is part of this candidate;
issue 4 remains open.
