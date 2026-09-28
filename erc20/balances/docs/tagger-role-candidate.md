# Tagger exact role-path candidate

One separate **NOT-QUALIFIED** candidate advances
[issue #4](https://github.com/pinax-network/substreams-evm-extended/issues/4)
for BSC Tagger `0x208bf3e7da9639f1eaefa2de78c23396b0682025`.
The root-6 width-two rule becomes two exact paths: arbitrary outer admin
`[bytes32]` at offset 1, width 1, and membership `[bytes32,address]` at
offset 0, width 1. Every other profile field, the original 431-profile
baseline and qualified 425 cohort stay unchanged.

The complete flattened source contains a public owner-only admin setter.
Its owner override also admits grant/revoke without membership; ordinary
renounce still requires the caller's own account. A fixed-role or membership-only
rule would omit reachable storage. The
[fixture review](../tests/fixtures/tagger-role-candidate/README.md) records
these source paths and all unchanged metadata.

A dedicated verifier pins the original capture, flattened source, complete
compiler input/output/metadata/layout and both bytecode arrays. It permits
only the recorded 53-byte CBOR replacement in each array, including the
unchanged creation suffix. Original `match` labels are preserved, and no
immutable, link or constructor-argument substitution is allowed. Other
candidate verifiers are unchanged. The single flattened file has no recovered
independent public token commit or independently verified upstream dependency
files; that source gap remains explicit. This is saved compiler reconstruction,
not fresh compilation, executed authorization or deployed-code qualification.

Synthetic Rust controls cover arbitrary admin and membership paths with
simultaneous balances, preserved owner/signer/status/mode/claim/fee/allowance
metadata, adjacent/deeper/malformed paths, persistence filtering and runtime
guards. They establish storage shapes only.

## Saved-window validation

The [final report](evidence/tagger-role-candidate-20260928.json) and
[source inventory](evidence/tagger-role-candidate-20260928-source-inputs.json)
cover the complete BSC interval **[122288006, 122289030)**. The as-run runner,
block digests, combined layout, native output and per-block comparisons remain
in `out/tagger-role-20260928/replay-01/`.

- All 1,024 blocks and 110,139 emitted rows match the unchanged current
  baseline and historical capture across every Events/Balance protobuf field,
  including optional contract presence. Only row ordering is normalized.
- The immutable canonical capture has 110,138 same-block and 4,012 retained
  reference-only matches, with no mismatches. One emitted row has no canonical
  counterpart and is compared against both baselines only.
- The shared ledger initializes 33,591 observed holders, including 6,197
  final known zeros. No checkpoint or deployment seed is supplied;
  66,265 cold reference observations remain unknown.

| Tagger measure | Result in this interval |
| --- | --- |
| Emitted rows / same-block canonical matches | 30 / 30 |
| Initialized observed holders | 6 |
| Retained reference-only matches | 0 |
| Cold reference observations | 38 |
| Of those, nonzero reference values | 14 |
| Persisted outer role-admin writes | 0 |
| Persisted membership writes | 0 |

Canonical values never initialize or repair state. These counts concern only
holders observed in the stated interval, not all Tagger holders. The separate
admin counter requires a verified `role || root6` preimage and its exact outer
word at offset 1; the membership counter requires the inner canonical address
path. Both count changed persisted writes only. No such writes were captured,
so real admin/membership-operation visibility remains unestablished.

The runner binds the canonical capture to the original reference digest,
network, interval and boundary hashes, verifies all 1,024 clocks/parent links,
and requires exact baseline equality after undoing only the two candidate
paths. Its source inventory remained unchanged throughout the successful run.
Earlier candidate modes and BAS's fixed-admin counter are preserved; errors
still retain partial output and a failed report without fabricated blocks.

Final validation on main `d1cab09b` plus this patch passed **881 tests / 51
Cargo result suites**, workspace all-target Clippy with warnings denied,
the workspace WASM check, formatting and whitespace checks including new
fixtures/docs. Ten new tests cover the source/CBOR/candidate binding, synthetic
storage shapes and arbitrary-admin replay counter. The target is isolated
under `out/tagger-role-20260928/`; offline checks do not qualify a live package.

`prepare-01/` preserves the first successful fixture generation; `prepare-02/`
preserves byte-identical fixtures and the final as-run helper/preparer source.
The first focused Clippy log retains two comparison-style warnings; the fixes
passed the second focused run and final integrated gates. The BAS integration
preserved all new-file hashes and retained both README descriptions. All
attempts and the pre-integration recovery patch remain saved.

No RPC, Substreams, Firehose or sink call ran for this candidate.
Fresh runtime, actual replacement-package output/RPC, initialized-holder/
final-state and real admin/membership-write visibility controls remain
separate promotion gates. The independent primary-source gap remains open;
this candidate is not promoted and issue #4's other role cohorts remain open.
