# Kgen/Deep plain-role candidates

Issue [#4](https://github.com/pinax-network/substreams-evm-extended/issues/4)
has two separate **NOT-QUALIFIED** OFT candidates. Kgen replaces only legacy
role root 10 width 2 with exact `[bytes32,address]` membership. Deep replaces
its ERC-7201 role width 2 with the same typed shape and adds only three fixed
admin words derived from its initializer. No generic admin, parser, schema,
dependency, deployment seed or coupled enumerable change is included.

The [complete fixture review](../tests/fixtures/oft-role-candidates/README.md)
preserves 55 Kgen, 66 Deep implementation and 12 proxy source files, exact
raw/compiler/metadata/runtime/creation bindings and all original labels.
Fresh immutable source reads match 123 direct upstream entries plus one exact
Kgen vendored interface. Kgen's token whitespace mismatch, Deep's seven custom
primary gaps and nonmatching upstream interface remain explicit.

Saved compiler bytes reconstruct every declared immutable and complete runtime;
independent ABI encoding binds all constructor appends. Deep's proxy constructor
targets an older implementation whose initializer stays opaque. The matching
nonce-1 ProxyAdmin derivation is a reconstruction hypothesis, not deployment
history. The existing later implementation guard and creation refusals remain.

Ordinary role changes do not couple to Kgen's forwarder-array or either token's
long-options bytes. Those metadata writes retain their existing refusals.
Fixed admin and membership permissions constrain locations/key shapes, not
authorization, values or initialization context. Synthetic Extended controls
are separate from actual producer role-operation evidence.

## Saved-window evidence

The [final report](evidence/oft-role-candidates-20260928.json) and
[source inventory](evidence/oft-role-candidates-20260928-source-inputs.json)
cover the complete saved BSC interval **[122288006, 122289030)**. The as-run
runner, native output, per-block comparisons and clock/digest chain remain
under `out/oft-role-candidates-20260928/replay-01/` in `bsc-exclusion-review`.

- All 1,024 blocks and 110,139 emitted rows match the unchanged current
  baseline and historical capture across every Events/Balance protobuf field,
  including optional contract presence. Only row ordering is normalized.
- Canonical checks have 110,138 same-block matches and 4,012 retained
  reference-only matches, with no mismatches. One emitted row has no canonical
  counterpart and is checked against both baselines only.
- The ledger initializes 33,591 observed holders, including 6,197 known zeros
  at the last block. No checkpoint/deployment seeds are used; 66,265 cold
  reference observations remain unknown.

| Result | Kgen | Deep |
| --- | ---: | ---: |
| Emitted rows / same-block canonical matches | 72 / 72 | 17 / 17 |
| Initialized observed holders | 13 | 8 |
| Cold reference observations | 71 | 15 |
| Cold observations with nonzero reference value | 31 | 7 |
| Retained reference-only matches | 0 | 0 |
| Persisted membership writes | 0 | 0 |
| Persisted fixed admin writes | No permission added | 0 |

Canonical values never initialize or repair retained state, including the
38 nonzero cold observations for these profiles. Counts describe initialized
observed holders, not all holders. This window contains no membership/admin
writes; the new permissions have source and synthetic-shape controls, not
actual producer role-operation evidence.

The runner checks the canonical chain, interval, block count, historical
reference digest, boundary hashes and every saved parent link. All 165 source
input hashes stay unchanged during replay. Failure would preserve its completed
prefix without fake empty rows; this run completed the entire window.

Preserved attempts under `out/oft-role-candidates-20260928/` include the initial
missing Rust return (`01-prepare.log`) and the synthetic projector control that
incorrectly selected unrelated checkpoint profiles (`04-projector-tests.log`).
The corrected source preparation is `source-01/`; all seven fixture artifacts
and both as-run helper/CLI hashes match it. Focused helper tests pass seven
groups, projector tests pass ten, and the runner has an additional fixed-admin
counter regression. These checks do not establish deployed role execution.

Final validation on main `5291d2a` plus this patch passed **924 tests across
58 Cargo result suites**, all-target workspace Clippy with warnings denied,
the workspace WASM check, formatting and staged whitespace checks. The patch
adds 18 tests (seven binding, ten projector, one runner). The unique isolated
target and full logs remain under the same output directory. Preparation
source, fixture bytes and replay inputs are frozen; historical reports and
qualification fixtures remain unchanged.

Fresh authorized runtime/dependency checks, actual replacement-package/getter
parity, initialized-holder/final-state checks and producer role-operation
observations remain promotion gates. Public-source and old-initializer history
gaps remain unresolved. Historical qualification stays unchanged and issue 4
remains open. No live RPC, Substreams, Firehose or sink call is performed.
