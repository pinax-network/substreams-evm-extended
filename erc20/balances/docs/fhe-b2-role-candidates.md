# FHE and B2Token exact role-path candidates

Two separate **NOT-QUALIFIED** candidates advance
[issue #4](https://github.com/pinax-network/substreams-evm-extended/issues/4):
FHE `0xd55c9fb62e176a8eb6968f32958fefdd0962727e` and B2Token
`0x783c3f003f172c6ac5ac700218a357d2d66ee2a2`. FHE removes root 5 from both
legacy broad-mapping lists; B2 removes the root-9 width-two rule. Each gains
only `[bytes32, address]`, offset 0, width 1. Every other profile field,
the original 431-profile baseline and qualified 425 cohort stay unchanged.

The [fixture review](../tests/fixtures/fhe-b2-role-candidates/README.md)
binds both complete source captures, all 51 exact primary files, compiler
input/output/metadata/layout and complete saved runtimes. A separate verifier
permits only seven FHE and eight B2 32-byte immutable substitutions, with
recomputed EIP-712/ShortString values and B2 cap. It requires original zero compiler
placeholders, exact reference/value/transformation sets and untouched CBOR;
Point/Bedrock's strict no-transformation helper remains unchanged. This is
saved compiler reconstruction, not fresh compilation or live qualification.

Both concrete source trees have no role-admin setter callsite or enumerable
role state. OZ5 labels its inner address/bool mapping `hasRole`. Synthetic
Extended-record tests admit membership shapes while refusing outer admin,
adjacent, malformed or deeper paths and runtime changes. FHE's independent
CCIP admin, B2's pause scalar, nonce/allowance metadata and concurrent balance
writes retain their existing handling. These are shape checks, not executed
role transactions or proof of real producer visibility.

## Saved-window evidence

The [final report](evidence/fhe-b2-role-candidates-20260928.json) and
[source inventory](evidence/fhe-b2-role-candidates-20260928-source-inputs.json)
cover the complete BSC interval **[122288006, 122289030)**. The as-run runner,
block digests, combined candidate layout, native output and per-block
comparisons remain in `out/fhe-b2-role-20260928/replay-01/`.

- All 1,024 blocks and 110,139 emitted rows match the unchanged current
  baseline and historical capture across every Events/Balance protobuf field,
  including optional contract presence. Only row ordering is normalized.
- Canonical capture checks have 110,138 same-block and 4,012 retained
  reference-only matches, with no mismatches. One emitted row has no canonical
  counterpart and is checked against both baselines only.
- The shared ledger initializes 33,591 observed holders, including 6,197
  final known zeros. No checkpoint or deployment seeds are supplied;
  66,265 cold reference observations remain unknown.

| Candidate | Emitted rows / same-block canonical matches | Initialized observed holders | Cold reference observations | Of those, nonzero reference values | Persisted membership writes |
| --- | --- | --- | --- | --- | --- |
| FHE | 18 / 18 | 6 | 18 | 6 | 0 |
| B2Token | 40 / 40 | 12 | 41 | 19 | 0 |

Neither candidate has a retained reference-only match in this window. Cold
observations stay unknown even when the reference value is nonzero. Canonical
values never seed or repair the ledger, and observed-holder counts are not a
global holder enumeration. No persisted membership write was captured for
either candidate; role-operation handling remains supported by synthetic
source-derived shapes only.

The runner binds the canonical capture's chain, interval, block count,
historical reference digest and boundary hashes, then verifies every saved
clock and parent link. FHE's two removed broad declarations are both restored
when checking exact equality to the unchanged baseline. On refusal, partial
output and a failed report are preserved without fabricated empty blocks.
The source inventory remains unchanged throughout the successful replay.

Final validation on main `817e9db` plus this patch passed **860 tests / 47
Cargo result suites**, workspace all-target Clippy with warnings denied,
the workspace WASM check, formatting and staged whitespace checks (including
new fixtures/docs). Eleven added tests cover source/immutable/candidate
bindings and storage shapes. The target is isolated under
`out/fhe-b2-role-20260928/`; these checks do not qualify a live package.

Successful `prepare-01/` retains the original captures, all primary source
files and as-run preparation/helper source. `clippy-01.log` preserves an
initial test-loop style warning; the iterator cleanup passed `clippy-02.log`
and the integrated final gates before replay evidence was accepted. The
main-branch integration preserved every staged file hash and changed no
replay dependency. All attempts remain saved.

No RPC, Substreams, Firehose or sink call ran for this candidate.
Fresh runtime, actual replacement-package output/RPC, initialized-holder/
final-state and real role-operation controls remain separate promotion gates.
The remaining issue #4 cohorts stay open; neither candidate is promoted.
