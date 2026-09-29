# ERC20TokenX enumerable candidates

Three separate **NOT-QUALIFIED** candidates select the same captured ERC20TokenX
runtime for ORI, FNA and PHI. They preserve the historical 431-profile baseline
and historical qualified 425-profile cohort. The independently reviewed
[compiled operation proof](erc20tokenx-operation-proof.md) is a prerequisite,
not a replacement for deployment, initial-state or producer qualification.

| Profile | Contract | Evidence boundary |
| --- | --- | --- |
| ORI | `0xda033999bb6165e64db01bd9be14b40f5653092e` | Complete saved source/compiler/runtime/creation capture |
| FNA | `0x08332a515cb2a57884176e887b682e7da2eb114e` | Complete saved source/compiler/runtime/creation capture |
| PHI | `0xa71add46ea4fbf0058b36e6baa39530f8e48b103` | Exact whole-runtime attribution only; individual source/creation/deployment unavailable |

The common runtime is 7,896 bytes, Keccak-256
`0xbaaa46c4d0133a30bd1020c46f86c0ea4ac0609d18317a95fc923efb5b22afdf`.
PHI's separate historical runtime capture is at block 122288067. Equality with
the selected runtime does not fabricate a third source record or prove its
history throughout any replay interval.

## Exact replacement and unchanged validator

Each candidate removes only `other_mapping_words[root8]=3` and adds:

```json
{"root":"0x0000000000000000000000000000000000000000000000000000000000000008","key_types":["bytes32"],"semantics":"oz_3_4_2"}
```

The rule has **no `membership_root`**, including no explicit null. There is no
separate boolean stage, typed boolean path, fixed admin scalar, general admin
permission, array range or standalone index permission. The complete captured
source has no reachable `_setRoleAdmin` callsite. Restoring the width-three entry
and removing the enumerable rule recovers every original profile field.

Balance root 0, allowance root 1, nonce root 6, scalars 2/3/4/5/7/9/10/11/12 and
the runtime guard stay unchanged. None of these profiles has a deployment or
proxy admission rule. Captured ORI/FNA creation remains refused, even with the
expected new runtime. No zero-supply or holder seed is introduced.

The existing `oz_3_4_2` parser and validator are reused unchanged. Its name refers
to independently bound source/write-order semantics, not blanket support for all
contracts using a library version. This selected compiler independently executes
the legacy template; other builds require their own source/runtime comparison.

For role `r`, `R=H(r||8)` holds length, `H(R)+i` addresses an element modulo
2^256, `H(address||(R+1))` holds one-based position and `R+2` is the admin word.
The observed source order is:

| Operation | Ordered stores |
| --- | --- |
| Add absent member | Length increment; append member; set one-based position |
| Remove member | Copy tail to destination; update moved position; clear tail; decrement length; delete removed position |

Tail/sole removal executes the equal self-copy and equal moved-position stores.
Zero sole removal also executes an equal zero tail-clear. Only independently
derived equal stages may be omitted. These are the existing legacy semantics,
distinct from the newer coupled boolean/set templates. Duplicate add and absent
remove execute no stores. No-op evidence cannot establish untouched initial
array/index coherence.

## Structural boundaries

Complete ordered operations remain scoped to one storage account, full role,
root and actual execution frame. Foreign persisted stores and call boundaries,
including reverted children, remain barriers. Reused ordinals, incomplete
operations, aliasing and contradictory observed or omitted stages fail closed.
Coherent initial membership, unique canonical members and unused-tail zeros
remain caller qualification assumptions; there is no persistent role cache.

The legacy Extended-v3 root-call fallback remains available under its existing
conditions. Extended v4/v5 requires positive actual root-call begin ordinals.
Numeric keys/values retain the existing normalization up to 32 bytes; 33-byte
words refuse. No new coupled-mode producer or exact-width policy is imported.
Actual role-write/preimage/equality visibility remains unqualified.

Changing unknown/admin writes refuse. Recognized admin/anchor equality records
with verified role hints also refuse. Unrecognized equal writes without those
hints retain ordinary no-op handling; this work does not claim universal no-op
refusal. A lone equal array-element store without a changing length witness is
also unrecognized and ignored, even with an outer-role hint; it grants no legacy
operation permission. Recognized standalone length/index no-ops still refuse.
Legacy compiler masking of dirty address ABI arguments does not make
dirty address mapping preimages canonical.

The actual malformed maximum-length push wraps. The validator's checked logical
growth refusal is a conservative admission restriction, not a source panic.
Stale indices, dirty tail words and explicit source INVALID controls remain
separate from coherent candidate admission.

## Provenance and controls

ORI/FNA bind complete five-file captures, official solc 0.7.5 with original
optimizer-200 settings, fresh complete compiler output and exact runtime bytes
without substitutions. Four OpenZeppelin files match immutable commit
`8e0296096449d9b1cd7c5631e917330635244c37`; the custom token primary revision is
unresolved. Complete captured creation bytes include the independent 288-byte
arguments. Constructor execution stops at unsupported CHAINID and rolls back;
it proves no initialized role set. PHI preserves its original five-field null
source response and separate runtime/report bindings.

Source-derived projector controls select records by name, exact function
signature and code kind, avoiding getter records that share operation names.
All three addresses exercise grant/revoke/self-renounce, full-width role keys,
zero/sole/moved/tail cases, every optional equality subset and repeated sequences.
The baseline controls show both the old complete-operation refusal and its
fabricated nested-mapping permission; the candidate reverses those outcomes.
Negative controls carry an independently valid balance row and require atomic
whole-projection refusal. Synthetic boundary cases are labeled separately.

The host helper pins all 18 final Phase A fixture/evidence artifacts before
parsing, then repeats their complete source/compiler/runtime/creation and primary
checks. The five original ORI/FNA/PHI cache files must equal their frozen copies.
Preparation reproduces the candidate and source-review fixtures without changing
the immutable baseline. Each artifact and original-cache binding has a mutation
control. The final Phase A report is
`340d3d1b0ed1542a8e2328b05560bddb2d61d4e712411e399c9bcaf5628033a1`;
its 520 calls remain the independent source-execution evidence, not new candidate
replay observations.

Eleven public-projector tests exercise these boundaries, including all equality
subsets for 30 selected coherent source calls at every profile and a nine-step
sequence. Five helper tests cover exact restoration, all-artifact/raw-cache
bindings and length-witness counting, including measured source operations.
An additive replay dispatch control prevents treating the legacy set as a
separate boolean membership mapping. The preserved first projector failure was
a draft expectation that an unrecognized lone zero array equality would refuse;
the corrected test documents the unchanged policy described above. Production
parser, validator, persistence rules and output remain unchanged.

## Saved replay scope

The [replay report](evidence/erc20tokenx-enumerable-candidate-20260929.json) and
[source inventory](evidence/erc20tokenx-enumerable-candidate-20260929-source-inputs.json)
are exact copies from `out/erc20tokenx-enumerable-candidate-20260929/replay-01`.
Report SHA-256: `a074c8ec7d4ecad265e6017b641cd6b09c679ede3f6d315d2213644a2939109e`.
Inventory SHA-256: `9587910a14878def21f97162935121376b7c88e266599a7a80fe518501711b13`.
All 212 source inputs, including the as-run tool and frozen Phase A artifacts,
remained unchanged.

The saved BSC interval **[122288006,122289030)** contains 1,024 Extended v5
blocks. All clocks and parent links match the canonical-bound history, and all
**110,139** full protobuf rows equal both the unchanged current baseline and
historical output. There are 110,138 same-block canonical matches and one
unchanged native-only row; reference-only observations contain 4,012 retained
matches and 66,265 cold observations left unknown.

| Bounded observed scope | ORI | FNA | PHI |
| --- | ---: | ---: | ---: |
| Emitted rows / same-block canonical matches | 41 / 41 | 31 / 31 | 35 / 35 |
| Initialized observed holders | 20 | 13 | 14 |
| Reference-only retained matches | 1 | 0 | 0 |
| Cold reference observations | 24 | 21 | 23 |
| Nonzero cold observations | 19 | 12 | 12 |
| Changed role-length writes / validated legacy operations | 0 / 0 | 0 / 0 | 0 / 0 |

Canonical RPC values never initialize retained state. These counts describe
observations and initialized holders in this interval, not universal token or
global-holder support. PHI's historical single-block runtime attribution does
not independently prove its full deployment/history continuity.

A separate diagnostic counts changed persisted role-length words `H(r||8)`
with exact verified outer preimages. Only after successful projection can each
length witness be labeled a validated legacy operation. It is not a boolean
membership counter or proof of authorization. No role operations occurred in
this replay, so the source-derived structural controls still do not establish
actual producer role-write/preimage/equality visibility.

Release `prepare-02` reproduced both candidate fixtures exactly. On merged main
`b1e50a031a526d72defbd2608736212e0dac728d`, the pinned, locked offline workspace
test run passed **1,146 tests across 95 suites** (65 nonempty). Formatting,
workspace all-target Clippy with warnings denied and the WASM workspace check
passed. Logs are `fmt-check-02.log`, `workspace-tests-01.log`,
`workspace-clippy-01.log` and `wasm-check-01.log` under
`out/erc20tokenx-enumerable-candidate-20260929/`, using its unique `target-isolated`.
Native saved replay and offline gates do not qualify a new WASM package.

There are no live RPC, Firehose/Substreams or sink calls, ABI/protobuf/dependency
changes, VM extensions, historical fixture replacements or production promotion.
Initial coherence, runtime/history continuity, real producer visibility and
replacement package/getter/initialized-holder/final-state qualification remain open.
