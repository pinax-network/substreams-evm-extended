# Point and Bedrock exact role-path candidates

Issue [#4](https://github.com/pinax-network/substreams-evm-extended/issues/4)
has two separate **NOT-QUALIFIED** candidates for Point/MCoin
`0x826923122a8521be36358bdc53d3b4362b6f46e5` and Bedrock/BR
`0xff7d6a96ae471bbcd7713af9cb1feeb16cf56b41`. Only their legacy role roots
(0 and 5) change to `[bytes32, address]`, offset 0, width 1. All remaining
fields, the 431-profile baseline and qualified 425 cohort stay unchanged.
The Token/CYS and BurnMint replay modes remain available independently.

The [complete fixture review](../tests/fixtures/point-bedrock-role-candidates/README.md)
records both full capture hashes, all 23 source files, compiler
input/output/metadata/layout checks and exact untransformed saved runtimes.
Bedrock's token and dependencies have immutable public source pins; Point's
10 OpenZeppelin dependencies also match pinned upstream bytes, but Point's
token repository remains unresolved. Saved compiler reconstruction does not
constitute a fresh compilation or current deployment qualification.

The reviewed concrete sources have no role-admin setter callsite. Constructor
membership grants and public grant/revoke/renounce use the exact inner mapping;
no enumerable structure is involved. Synthetic Extended-record tests preserve
balance writes and Bedrock's separate freeze settings, while refusing outer
admin writes, adjacent words, malformed paths/preimages/word widths and
runtime changes. They do not execute role transactions or establish actual
producer visibility of role writes.

## Saved-window evidence

The [final report](evidence/point-bedrock-role-candidates-20260928.json) and
[source inventory](evidence/point-bedrock-role-candidates-20260928-source-inputs.json)
cover the complete BSC interval **[122288006, 122289030)**. As-run runner
source, block digests, native output, combined candidate layouts and per-block
comparisons remain under `out/point-bedrock-roles-20260928/replay-02/`.

- All 1,024 blocks and 110,139 emitted rows match the unchanged current
  baseline and historical capture across every Events/Balance protobuf field,
  including optional contract presence. Only row ordering is normalized.
- Canonical capture comparisons have 110,138 same-block matches and 4,012
  retained reference-only matches, with no mismatches. One emitted row lacks
  a canonical counterpart and is checked only against both baselines.
- The shared ledger initializes 33,591 observed holders, including 6,197 known
  zeros at the final block. There are no checkpoint or deployment seeds;
  66,265 cold reference observations remain unknown.

| Candidate | Emitted rows / same-block canonical matches | Initialized observed holders | Cold reference observations | Of those, nonzero reference values | Persisted membership writes |
| --- | --- | --- | --- | --- | --- |
| Point / MCoin | 28 / 28 | 11 | 18 | 13 | 0 |
| Bedrock / BR | 30 / 30 | 18 | 19 | 11 | 0 |

Neither candidate has a retained reference-only match in this window. Cold
observations remain unknown even when their canonical value is nonzero; no
canonical value initializes or repairs the ledger. Holder counts describe
the initialized observed set, not all holders. The replay contains no
persisted membership writes for either candidate, so role-operation handling
is supported by synthetic source-derived shapes only.

The runner binds the canonical capture's chain, interval, block count,
historical reference digest and boundary hashes, then verifies every saved
clock and parent link. It compares candidates against an unchanged 431-profile
baseline, preserves all partial output on refusal, and never inserts an empty
row to bridge a missing block. Mapper/host dependency/fixture hashes must stay
unchanged throughout replay; the as-run source is retained.

`prepare-01.log` preserves an initial verifier mismatch (`match` versus the
captures' `exact_match` label); `prepare-02/` contains the successful complete
capture and pinned public source preparation. `clippy-01.log` preserves the
initial style warning. The successful earlier `replay-01/` is also preserved;
final evidence uses `replay-02/` after the equivalent string-comparison cleanup.

Final offline validation on main `8a32bfa` plus this patch passed 833 tests
across 45 Cargo result suites, workspace all-target Clippy with warnings
denied, the workspace WASM check and formatting. Eight added tests cover
source/candidate bindings and role shapes. Logs use the fresh isolated target
under `out/point-bedrock-roles-20260928/`; these checks do not qualify a live
package or runtime.

No RPC, Substreams, Firehose or sink operation ran for this candidate.
Fresh runtime controls, actual replacement-package output/RPC comparison,
initialized-holder/final-state validation and actual role-operation visibility
remain separate promotion gates. Point's token primary-source gap and all
other legacy profile cohorts remain explicit; issue #4 stays open.
