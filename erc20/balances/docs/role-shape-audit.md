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
**NOT-QUALIFIED** fixtures now cover all 33 of its boolean-role profiles and
three of its eight enumerable-role profiles;
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
| [SecuritiesToken (17)](securities-coupled-role-candidate.md) | Exact namespaced membership/set operations for the selected solc 0.8.24/OZ 5.3 build; only the fixed ISSUER admin scalar may be written to zero. All other profile fields and guards are retained. | Eight explicit primary-source gaps, absent deployment/creation evidence and unqualified initializer/client/proxy execution. Saved replay does not establish role-operation visibility or coherent initial state. |
| [GMToken (2)](gm-coupled-role-candidate.md) | Exact selected solc 0.8.16/Ondo-vendored membership201/set251 operations; no role-admin permission, with all beacon/runtime/balance and finite long-name words retained. | Five custom primary-source gaps, exact compiler/captured metadata substitutions and absent on-chain creation binding. Actual initial coherence, producer role visibility and proxy/external-client execution remain unqualified. |
| [BTRToken](btr-coupled-role-candidate.md) | Exact selected solc 0.8.24/OZ 4.9.3 membership101/set151 operations plus the fixed PAUSER self-admin scalar. All unrelated fields remain unchanged; direct whitelist length/index permissions remain partial. | Custom token primary-source gap, initial coherence, deployed initialization/proxy history and actual producer/package/getter/holder qualification remain open. Full whitelist operation admission is separate work. |
| [ERC20TokenX: ORI / FNA / PHI](erc20tokenx-enumerable-candidate.md) | Remove only the broad root-8 width-three permission and select the unchanged `oz_3_4_2` complete-operation rule. No separate membership root or admin permission; all unrelated fields remain unchanged. | Exact shared compiled runtime; complete ORI/FNA source and creation bindings, historical PHI runtime-only attribution and one custom primary-source gap. Creation remains refused, and coherent initial state, producer visibility and replacement qualification remain open. |

The prerequisite [PToken operation proof](ptoken-operation-proof.md) executes its
compiled boolean/set transitions and rollback controls against synthetic state.
That host-only proof remains distinct from the subsequent coupled validator
and candidate listed above. Source execution and synthetic Extended records
do not qualify producer visibility or initial state. The selected write-order
template is not a universal OpenZeppelin enumerable-set rule.

The [BTR host operation proof](btr-operation-proof.md) binds the selected
OZ 4.9.3 implementation and OZ 5.2 proxy sources and distinguishes coupled role
transitions from the separate whitelist set. Its subsequent role-only candidate
above removes broad membership permission and checks the exact PAUSER admin
value even for equal writes or absent preimages. Nonzero whitelist array
mutations remain refused, while zero-member add/sole-remove can pass under
inherited equality handling. The custom token's independent primary source,
actual initialization and complete direct-whitelist admission remain unresolved.
The separate [SecuritiesToken host proof](securities-operation-proof.md) binds
the selected OZ 5.3 namespaced role/set behavior and raw ERC-20 getters.
Twenty-three of 31 captured sources have exact independent matches; three
differing BEP bodies and five unresolved token/client sources retain explicit
gaps. Constructor execution uses synthetic state because saved on-chain
creation and deployment evidence are absent. Successful external-client,
initializer, transfer/UI and proxy/beacon paths are outside this proof. It
adds no candidate by itself. The separate [coupled candidate](securities-coupled-role-candidate.md)
now covers the seventeen profiles with their exact source-selected operation
rule and fixed zero-only ISSUER admin word, preserving proxy/beacon/runtime
guards and unrelated metadata. Initial coherence, actual producer visibility
and replacement qualification remain open.
The separate [GMToken host proof](gm-operation-proof.md) binds three complete
captures and the selected local role, initializer and raw-getter behavior.
Fifteen exact Ondo-vendored dependencies and seven unique upstream proxy
dependencies remain distinct; five custom primary-source gaps remain. Compiler
and captured runtimes have exact bounded metadata substitutions, with separate
execution matrices and no on-chain creation binding. Successful external
compliance/pause calls and beacon dispatch remain excluded. The proof itself
adds no ingestion permission; the separate two-profile coupled candidate above
retains its exact source bindings and qualification limits.
Each candidate needs its own reachability, initialization and dependency review
before promotion. This classification concerns the configured role field;
some contracts also contain separate enumerable or dynamic metadata that must
retain its existing handling. Five enumerable-role profiles across five other
runtime groups still need their own source-selected migration. Four legitimate
terminal-record profiles and six source-unavailable profiles retain the original
audit's separate findings; they are not plain membership migrations.

The separate [ERC20TokenX host proof](erc20tokenx-operation-proof.md) measures
the exact shared ORI/FNA/PHI runtime used by the three-profile enumerable
candidate. Complete ORI/FNA captures and constructor bytes remain separate from
PHI's historical runtime-only attribution. Four dependencies match exact primary
sources; the custom token's independent source pin is unresolved. The old
combined role layout has no separate membership boolean or reachable admin
setter. Measured self-swaps, equal stores, authorization, raw getters and failure
boundaries do not initialize the deployed set or qualify producer visibility.
The proof itself adds no candidate. The subsequent three-profile fixture above
reuses the existing validator without changing production code. Its projector
controls distinguish complete admitted operations from fabricated nested paths,
incomplete operations and conservative malformed-state refusals. Recognized
standalone no-ops refuse; unrecognized equal array words retain ordinary no-op
handling. Three of eight enumerable profiles now have separate candidates.

The separate [WKEYDAO/GOT host proof](wkey-got-operation-proof.md) binds two
different complete runtimes and their root-9/root-8 combined role schemas.
Independent compilation and 1,263 synthetic calls measure ordered roles,
authorization, getters, cap reduction on burn and source-specific fee arithmetic.
WKEYDAO construction stops at unsupported CHAINID with full rollback; GOT's
synthetic constructor returns its exact runtime after the expected stores and
logs. Neither proves deployed initial coherence or adds creation permission.
Four shared and two additional GOT dependencies match the immutable upstream
pin; both custom token primary revisions remain unresolved. No candidate,
production validator or VM change accompanies this proof, so the remaining
five-profile migration count is unchanged. A later GOT candidate must remove
both broad root-8 permissions; WKEYDAO's selected role root is 9.

TOPS's [separate host proof](tops-operation-proof.md) concerns a three-word
struct array and wrapping LP credits under issue #61, outside this role cohort.
Original append/getter execution and the unchanged-body cleanup harness do not
make it an enumerable role set. Any future ingestion rule needs its own complete
operation and producer review; no TOPS candidate is added by that host proof.

Fresh runtime/dependency controls, actual replacement-package/getter output,
initialized-holder/final-state checks and relevant role-operation observations
remain promotion gates. No row in this table claims live replacement
qualification or closes issue #4.
