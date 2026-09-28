# Remaining role-storage boundaries

The MUSD and OLY [configuration correction](role-width-coverage.md) prompted
a wider review of `other_mapping_words`. The review inventories 51 profiles
from the 426-profile baseline, excluding MUSD, OLY and NXT. Complete source
evidence binds 45 profiles across 26 effective runtimes; six profiles retain
their previous bytecode evidence and remain unresolved by this source audit.

The audit reproduces two distinct guard limitations:

- **33 boolean-role profiles:** the recursive two-word rule accepts one word
  past a nested membership boolean. Twelve have no source call to the admin
  setter, 19 set known roles during initialization, one writes during
  construction, and one exposes an owner-authorized arbitrary-role setter.
  Removing the width rule from all of them would reject legitimate admin
  writes in some contracts.
- **Eight enumerable-role profiles:** valid array and offset-one index writes
  currently reject, while a fabricated mapping under the array-length field
  is accepted. These need explicit array and index recognition.

Four reviewed profiles use terminal scalar records and do not need the same
role-specific narrowing. The 344 synthetic checks record exact keys,
preimages, accepted writes and errors. They describe mapper behavior; they
do not establish that fabricated writes are reachable or allege historical
balance mismatches. No RPC calls or configuration changes were made by this
audit.

The audit proposed recognizing an exact sequence of mapping keys and allowed
field offsets. For example, a role admin lives one word after a single
`bytes32` mapping key, while membership is one boolean word after a second,
address-shaped key. A terminal four-word rate record has a different shape.
Each rule must require correct preimages and canonical key widths, without
applying a record's width to every nested leaf. Existing width rules should
be migrated per reviewed profile, preserving legitimate scalar records.

The subsequent [typed mapping-path implementation](typed-mapping-paths.md)
adds that exact-depth configuration to the Rust source, with offline regression
tests. It does not migrate these published profiles or cover enumerable-role
arrays and offset-one index mappings. Profile replacement still needs fresh runtime checks, full native replay, packaged
RPC comparisons and initialized-holder/final-state checks. The current
historical parity evidence remains bounded to its recorded intervals.

The separate [enumerable role-set rule](enumerable-role-sets.md) now adds
opt-in source validation for the reviewed DSG write order. It does not migrate
these eight profiles, support their role-admin mutations automatically, or
establish producer/package qualification. The per-profile review remains
required before using it.

- [Source-bound inventory and findings](evidence/role-shape-audit.json)
- [All synthetic checks](evidence/role-shape-checks.json)
- [Per-profile recommendations and proposed matching rules](evidence/role-shape-recommendations.json)

The original audit and Rust helper files remain under
`out/role-width-audit` and `out/next-candidate-rust-scripts/role-width-audit`.

A subsequent [bounded candidate migration](role-path-candidates.md) replaces
the legacy role widths for Token and CYS in a separate two-profile fixture,
with source bindings, refusal tests and a full saved-window replay. The
published baseline and the other reviewed profiles remain unchanged.

## Candidate progress

The original inventory and its evidence above remain historical. Separate
**NOT-QUALIFIED** fixtures now cover thirteen of its 33 boolean-role profiles;
they do not replace the original 431-profile baseline or qualified 425 cohort.
Each linked report records its complete saved interval, initialized observed
holders, cold gaps, source bindings and regression controls. Synthetic write
shapes do not establish executed authorization or real producer visibility.

| Profiles | Implemented candidate scope | Source limits |
| --- | --- | --- |
| [Token / CYS](role-path-candidates.md) | Exact nested membership paths; unrelated metadata retained. | Recorded source/runtime bindings; replacement qualification remains open. |
| [BurnMintERC20](burnmint-role-candidate.md) | Exact root-5 membership. | Fourteen primary files with one explicitly checked import relocation; saved immutable reconstruction. |
| [Point / Bedrock](point-bedrock-role-candidates.md) | Exact membership at roots 0 / 5; Bedrock freeze fields retained. | Twenty-three captured files; Point's independent token repository remains unresolved. |
| [FHE / B2Token](fhe-b2-role-candidates.md) | Exact membership at roots 5 / 9; both broad FHE permissions removed. | All 51 files match exact primary pins; 15 immutable words are independently recomputed. |
| [BAS](bas-role-candidate.md) | Exact root-6 membership plus only the fixed PAUSER admin word. | Thirteen exact dependencies; six public token revisions do not match the captured token. Constructor binding does not admit creation or attest current admin state. |
| [Tagger](tagger-role-candidate.md) | Exact root-6 membership and arbitrary outer admin at offset 1, preserving the owner-accessible setter. | One flattened captured source with exact bounded CBOR substitutions; independent token and upstream dependency pins remain unresolved. |
| [Artx](artx-role-candidate.md) | Exact root-151 membership; proxy and implementation runtime/pointer guards retained. | Both complete captures preserve 35 input files and 34 exact dependency matches; independent token source remains unresolved. Constructor reconstruction does not initialize holder state. |
| [Kgen / Deep](oft-role-candidates.md) | Exact root-10 / ERC-7201 membership; Deep retains only three fixed initializer admin words. Unrelated metadata and proxy guards remain. | Three complete captures preserve 133 source entries: 123 direct upstream matches and one exact Kgen vendored interface. Kgen's token mismatch, Deep's seven custom-source gaps and nonmatching upstream interface remain explicit. Deep's older proxy initializer remains unqualified. |
| [PTokenV2](ptoken-coupled-role-candidate.md) | Source-selected complete operations couple root-5 membership to the root-6 enumerable set. Independent boolean permission is removed; Extended v4/v5 and actual frame boundaries are required. | The compiled operation proof binds the selected solc 0.8.28/OZ 5.4 template. Exact token-source provenance, initial-set coherence and actual producer visibility remain unqualified. |

The prerequisite [PToken operation proof](ptoken-operation-proof.md) executes its
compiled boolean/set transitions and rollback controls against synthetic state.
That host-only proof remains distinct from the subsequent coupled validator
and candidate listed above. Source execution and synthetic Extended records
do not qualify producer visibility or initial state. The selected write-order
template is not a universal OpenZeppelin enumerable-set rule.

The remaining 20 boolean-role profiles are SecuritiesToken (17), BTRToken
and GMToken (2).
Each needs its own reachability, initialization and dependency review before
candidate migration. This classification concerns the configured role field;
some contracts also contain separate enumerable or dynamic metadata that must
retain its existing handling. The eight enumerable-role profiles, four
terminal-record profiles and six source-unavailable profiles retain the
original audit's separate findings; they are not plain membership migrations.

Fresh runtime/dependency controls, actual replacement-package/getter output,
initialized-holder/final-state checks and relevant role-operation observations
remain promotion gates. No row in this table claims live replacement
qualification or closes issue #4.
