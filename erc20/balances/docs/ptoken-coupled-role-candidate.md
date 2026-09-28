# PTokenV2 coupled role candidate

This separate **NOT-QUALIFIED** candidate replaces one broad root-5 role rule
with a complete coupled operation permission for the selected PTokenV2 runtime.
The historical 431-profile baseline and 425-profile qualification cohort remain
unchanged. It uses the [Phase A compiled operation proof](ptoken-operation-proof.md);
that proof is preserved as historical evidence.

## Exact template and configuration

The opt-in semantics identifier is `ptoken_v2_solc_0_8_28_oz_5_4_0`. It denotes the
reviewed compiler's exact ordered storage template, not universal support for
OpenZeppelin 5.4. Any later runtime, compiler or profile needs its own independent
binding and execution comparison. This mode requires set root6, membership root5,
and one complete `bytes32` role key. Explicit `membership_root:null` is invalid.
Legacy `oz_3_4_2` documents serialize exactly as before and reject any supplied
membership-root field.

For role `r`, boolean membership is `H(address || H(r || 5))`. The set length is
`A=H(r || 6)`, array element `i` is `H(A)+i` modulo 2^256, and one-based member
position is `H(address || (A+1))`. Roles are arbitrary bytes32; member addresses
must have zero high 96 bits in exact 64-byte mapping preimages.

| Operation | Required ordered stages |
| --- | --- |
| Add absent member | boolean0→1; length increment; append member; set one-based position |
| Remove non-tail | boolean1→0; copy tail into removed index; update tail position; clear tail; decrement length; delete removed position |
| Remove tail/sole | boolean1→0; clear tail; decrement length; delete removed position |

Only the actual equal zero-address append or zero-tail clear may be omitted.
The boolean, length and position changes always remain required. DSG's executed
self-swap stages are not part of this selected PToken template. The initial
boolean/position/array coherence and unused-tail-zero invariant are caller
qualification assumptions, not reconstructed history.

Both roots are reserved against other metadata rules and discovered from the
union of verified outer preimages. A missing counterpart can identify a fragment
but cannot grant permission. Every recognized boolean, set, index or equality
record must be consumed once in a unique complete operation for the same storage
account, role, root pair and structural frame. Neither isolated membership nor
isolated set writes are independently permitted.

Logical stages have explicit kinds. All observed and omitted element stages
receive the same protected-root, known-balance, namespace and cross-role alias
checks. Moved-tail membership equal to1 is a derived coherence constraint, never
a write, optional stage, event permission or invented producer preimage. Its key
is protected against element aliases. Observed/inferred values are journaled
only within the block and must remain continuous.

Physical storage arithmetic wraps modulo 2^256. Logical length growth is checked
as a conservative admission restriction. The Phase A maximum-length push wraps
in the actual runtime; refusal must not be described as a Solidity overflow
revert. Clear, role-admin changes, adjacent fields and malformed operations have
no permission.

## Producer decision

The selected template structurally accepts Extended versions **4 and5 only**,
with positive actual root-call begin ordinals and strict frame containment.
Version3 is explicitly refused for this mode; legacy DSG keeps its existing
version3 transaction-begin fallback. All persisted foreign SSTOREs and execution
frame boundaries, including reverted children, remain ordering barriers.
Declared call indices are not frame identity.

Storage keys for the selected account must be exactly32 bytes. Numeric old/new
words retain the shared at-most32-byte normalization and reject oversized values.
Supplied changing stages and exact hash-verified mapping preimages are mandatory.
Only the source-proven equal stages described above may be omitted. This tests
structural compatibility with those producer representations. It does **not**
establish that real PToken role operations expose all needed preimages or records
in a particular Firehose producer. Constant-folded roots cannot be replaced with
invented witnesses. Ordinary unidentified no-op policy remains unchanged; an
unidentified noop cannot repair a missing changing stage.

## Source and candidate binding

The source-bound preparer rechecks the original complete capture, frozen fresh
compiler output and entire Phase A operation evidence. Capture SHA-256 is
`14d5dcc50d8dd1b04fc6283e0d1fafad66fd7fe0bfafdff224a9ba6eda203f9c`; the runtime is
`0x60f53552d1ab18923098e47b0d7952ae7a4a48d8bece60b20d3d41734832c15c`.
Twenty dependencies have exact OpenZeppelin source pins. The independent public
repository for the token source remains unresolved.

The candidate removes only `other_mapping_words[root5]=2` and adds the coupled
rule. It retains balance root0, allowance root1, scalars2/3/4/7, runtime binding,
and all other baseline fields. No independent typed membership path is added.
It is a separate fixture, not a production baseline promotion.

Projector tests translate frozen compiled traces into explicitly synthetic
Extended records and cover equality subsets, missing/corrupt/extra stages,
boolean/set-only discovery, zero/tail/moved members, sequential operations,
role/account/frame splicing, barriers, dirty preimages/words, protected aliases,
logical overflow and rollback. Negative cases include a valid balance write and
require the complete projection to fail. The old DSG suite remains in scope.

## Saved replay and validation

The [fresh report](evidence/ptoken-coupled-role-candidate-20260928.json) covers all
1,024 saved BSC blocks in **[122288006,122289030)**. Every block is Extended version 5
and has the same canonical-bound clock and parent chain as the immutable capture.
The candidate and unchanged current baseline produce identical full protobuf
outputs, also equal to the historical outputs: 110,139 rows. Across all configured
profiles there are 110,138 same-block canonical matches, one native-only row and
4,012 matches retained from earlier native emissions. The 66,265 cold reference
observations remain unknown. Canonical RPC values never initialize state.

For PToken alone, all **100 emitted rows match saved same-block RPC values across
51 initialized observed holders**. There are zero retained reference-only matches
and 100 cold observations, all with reference value zero; those cold holders still
remain unknown. There are **zero observed changing membership writes and zero
validated coupled operations** in this window. These counts do not establish real
role-operation visibility. The operation counter is derived only after successful
coupled projection, where each complete operation requires one changing boolean.

The [frozen source inventory](evidence/ptoken-coupled-role-candidate-20260928-source-inputs.json)
binds 171 source/input files. Its SHA-256 is
`75c1e7785a3dc6d09d0ff152a8916f84d62ba68157e2f477d7493edd4830a766`; report SHA-256 is
`3ee78fc936687ca13cac347d94c37d9ca92817dcf6ff523c8f3f72c4766d7c00`.
The saved preparer attempt02 and replay attempt01 preserve their as-run sources;
earlier compile/fixture attempts remain under the ignored output directory.

On integrated main `dba534f79b25f0d135058c1cc8bdc6884aa5651f`, pinned Rust 1.88
locked offline validation passes: formatting, **948 tests across 61 suites**,
workspace all-target Clippy with warnings denied and the workspace WASM check.
The target is isolated at `out/ptoken-coupled-validator-20260928/target-isolated`.

Runtime/package, producer role-write visibility, initial-state and final-holder
qualification remain separate gates. No live RPC, stream, sink, new ABI/protobuf,
dependency version or persistence-rule changes are part of this offline work.
