# BurnMint exact role-path candidate

Issue [#4](https://github.com/pinax-network/substreams-evm-extended/issues/4)
has one additional, separate offline candidate for BurnMintERC20
`0xac23b90a79504865d52b49b327328411a23d4db2`. It replaces only the legacy
root-5 width-two rule with `[bytes32, address]`, offset 0, width 1.
The published 431-profile fixture and qualified 425 cohort remain unchanged;
the candidate is not promoted or live-qualified.

The [fixture review](../tests/fixtures/burnmint-role-candidate/README.md)
retains the complete original source capture at SHA-256
`8c2bf80a6aef279bc19fbb13885c1067247b9fd8b2d6cf3a0e0645b7bd4fd489`.
Rust checks bind chain 56, the exact address, all 14 source files and metadata
hashes, layout, immutable patches and the saved runtime
`0x688f1e2193eea752e7eaa2acc9607ede95429383234670d6d84c0ee3336eb3d0`.
Primary source is pinned to
`smartcontractkit/chainlink@e6287bd5ec0cb86925584b050b6aed0fa9b1b2e8`.
Thirteen files match byte for byte; BurnMint differs only in the documented
`IGetCCIPAdmin` import relocation. The verifier permits exactly that single
replacement and preserves both source texts. This reconstructs cached
compiler output; it does not claim a fresh Solidity compilation.

The inherited `balanceOf` reads the root-0 balance mapping. Complete source
review identifies membership writes through grant, revoke and renounce,
with no `_setRoleAdmin` callsite and no enumerable role structure. The
candidate refuses the outer role record, its admin field, adjacent membership
words, deeper paths, wrong roots/key order, malformed address padding and
missing or corrupt preimages. Source-derived Extended-record tests also
cover membership grants/revokes/no-ops, failed/reverted calls, concurrent
balance writes, malformed storage widths and runtime changes. These tests
exercise key shapes and persistence rules, not executed role transactions
or boolean-value validation.

## Saved-window evidence

The [report](evidence/burnmint-role-candidate-20260928.json) and
[source inventory](evidence/burnmint-role-candidate-20260928-source-inputs.json)
record the complete BSC interval **[122288006, 122289030)**.
As-run source, blocks/digests, native output, combined candidate layout and
per-block comparisons remain in `out/burnmint-role-20260928/replay-02/`.
The replay checks the canonical capture's chain, interval, block count,
historical reference digest and boundary hashes before processing any block.
Every saved clock and parent link is then checked individually.

- All 1,024 blocks pass candidate/current-baseline and historical full
  protobuf parity, including all Events/Balance fields and optional-contract
  presence. Only row ordering is normalized.
- All 110,139 emitted rows match the historical/current baseline. Canonical
  RPC capture comparisons include 110,138 same-block matches and 4,012 values
  retained from prior native emissions, with no mismatches. One emitted row
  has no canonical capture counterpart and is checked only against the two
  baselines.
- The shared ledger initializes 33,591 observed holders, including 6,197
  known zeros at the final block. It uses no checkpoints or deployment seeds
  and leaves 66,265 cold reference observations unknown.
- BurnMint contributes 38 emitted rows, all matching the canonical capture,
  and 11 initialized observed holders. Its 34 cold reference observations
  remain unknown; all 34 saved reference values are zero. It has no retained
  reference-only matches or persisted role-membership writes in this window.

Thus the saved replay establishes unchanged balance behavior for the tested
interval and observed-holder set. Role-shape handling is checked only by
source-derived synthetic tests; actual role-write visibility remains
unestablished. These counts are not a global holder enumeration. Canonical
values never seed or repair retained state. A refusal preserves partial output and a
failed report without inserting empty rows for missing blocks.

Initial preparation outputs, the first test failure caused by missing
code-change hashes in its synthetic fixture, the first Clippy failures and
the earlier successful replay remain under `out/burnmint-role-20260928/`.
The final evidence uses the corrected fixture and final source inventory.

No RPC, Substreams, Firehose or sink operation ran. Fresh authorized runtime
controls, real packaged output/RPC parity, initialized-holder/final-state
checks and actual role-operation visibility remain promotion gates. The
other legacy profiles and unresolved source cases also remain outside this
one-profile candidate; issue #4 stays open.
