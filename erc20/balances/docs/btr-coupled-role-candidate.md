# BTR role-only coupled candidate

This separate **NOT-QUALIFIED, ROLE-ONLY** candidate narrows the selected BTR
proxy `0xfed13d0c40790220fbde712987079eda1ed75c51`. It preserves the immutable
431-profile baseline and historical 425-profile qualification cohort. The
[Phase A compiled operation proof](btr-operation-proof.md) remains an independent
frozen prerequisite. Direct whitelist-set admission is separate work.

## Selected source template and fixed admin

The opt-in identifier `btr_token_solc_0_8_24_oz_4_9_3` means the exact ordered
operation template of the reviewed BTR build, not universal OpenZeppelin 4.9.3
support. It requires membership root 101, enumerable role-set root 151, and one
full bytes32 role key. Later compilers/runtimes/profiles need independent binding
and execution comparison. PToken, Securities and legacy DSG keep their separate
semantics; legacy serialization and explicit-null refusal remain unchanged.

For role `r`, membership is `H(address || H(r || 101))`, set length is
`A=H(r || 151)`, element `i` is `H(A)+i` modulo 2^256, and one-based position is
`H(address || (A+1))`. Exact hash-verified mapping preimages and canonical address
padding are required. The selected compiled coherent operations have these
ordered logical stages:

| Operation | Ordered stages |
| --- | --- |
| Grant absent member | Boolean 0→1; length increment; append element; set position |
| Revoke non-tail | Boolean 1→0; copy tail to removed index; update moved position; clear tail; decrement length; delete removed position |
| Revoke tail/sole | Boolean 1→0; clear tail; decrement length; delete removed position |

Only the source-executed equal zero-address append or zero-tail clear may be
omitted. There are no invented self-swap stages. Source authorization and wrapper
logs are independently covered in Phase A; the projector checks persisted storage
structure, not authorization. This OZ4 extension invokes set add/remove after a
void parent role method. Its four incoherent source successes (boolean-only and
set-only grant/revoke) are deliberately refused as incomplete operations.

The only source admin setter is the initializer's
`_setRoleAdmin(PAUSER_ROLE, PAUSER_ROLE)`. Rust independently derives:

- `PAUSER_ROLE=H("PAUSER_ROLE")`:
  `0x65d7a28e3265b37a6474929f336521b332c1681b933f6cb9f3376673440d862a`.
- `H(PAUSER_ROLE || 101)+1`:
  `0xbfe93621c6aa2dbf737e9056c9b79e4d30ee4f6b28b18be2cb71aac8a7bf258f`.

The frozen initializer transcript independently records zero→PAUSER_ROLE at
that exact word. The prior width-two permission covered this location implicitly.
The candidate removes only that broad root 101 entry, adds the coupled rule, and
appends this exact scalar to `other_slots`. The scalar is mandatory for this mode.
Every persisted write must have new value PAUSER_ROLE, including equal writes
and writes without a preimage. No prior value is fabricated. Other admin words,
wrong new values and adjacent fields remain refused. The fixed scalar remains
protected from array/coherence aliases and remains a store barrier; it is never
a role stage. This does not establish deployed initialization or authorization.

## Complete operations and producer boundary

Both roots are reserved against other permissions and discovered from the union
of verified outer preimages. Boolean-only, set-only and recognized no-op fragments
cannot bypass completeness. Each observed stage belongs once to one unique
operation in the same account, root pair, full role and structural frame.
Foreign persisted stores and execution boundaries, including reverted children,
are barriers. Declared call indices alone do not establish frame identity.

Extended versions **4 and 5** require positive actual root-call begin ordinals;
version 3 is refused for this selected mode. Legacy DSG's existing fallback is
unchanged. Selected-account keys are exactly 32 bytes, while numeric old/new words
retain the shared normalization of words up to 32 bytes. Actual BTR producer visibility
of all required changing stages and preimages remains unqualified.

Initial boolean/array/position coherence and unused-tail zeros are caller
qualification assumptions. Moved-tail membership equal to 1 is a derived
coherence constraint, not a store permission or required invented producer
preimage. All observed and omitted stages receive protected-key/cross-role alias
checks and exact block-local continuity. Logical length growth at uint256 maximum
is conservatively refused: the actual malformed compiled push wraps, so refusal
must not be called a source overflow revert. No persistent role cache is added.

## Inherited whitelist behavior remains partial

The candidate preserves direct whitelist length 555 and broad index 556 permission
exactly. `setWhitelister` is PAUSER-only and separate from grant/revoke/renounce,
setMinter and setBurner. Initialization does not call it. This phase adds no
direct-head whitelist operation template.

Ordinary nonzero array-mutating whitelist operations remain refused because the
array-data stores have no permission. Inherited metadata rules can nevertheless
accept zero-address add or sole removal when their equal zero array stores are
omitted or ignored under the ordinary no-op policy. Tests explicitly preserve
both representations and compare them against the unchanged baseline. A mixed
valid role operation plus nonzero whitelist mutation fails atomically, including
a simultaneous valid balance row. These rules establish neither blanket refusal
nor complete whitelist support.

Future full whitelist support must independently replace **both** scalar 555 and
broad index 556 with a source-bound direct-head operation rule. Saved replay
counters distinguish changed membership, exact PAUSER admin, literal whitelist
length and canonical address-index writes. The whitelist counters do not establish
complete array visibility or count independently validated whitelist operations.

## Source binding and scope

The helper/preparer/replay bind both full captures and fresh compiler outputs,
primary-source artifact, compiler report, operation report and compact transcripts
by frozen Phase A hashes. They recheck all 38 exact dependency bodies, original
source/settings/layout/metadata, exact UUPS immutable substitutions, complete
runtime/creation and proxy constructor ABI binding. The custom token's independent
primary source remains unresolved; captured source is not rewritten or promoted.

All unrelated fields are preserved exactly: balances 201, allowances 202, nonce 303,
pause 404, quota 554, whitelist 555/556, remaining scalars and proxy/runtime/pointer
and creation guards. No independent boolean mapping path is added. Synthetic
implementation execution at a proxy storage address does not execute the actual
proxy dispatcher or qualify deployed initialization, UUPS upgrades or permit.
There is no ABI/protobuf, dependency version, persistence-rule or VM change.

Source-derived projector tests cover coherent grant/revoke/renounce/minter/burner
routes, zero/sole/moved/tail members, sequential transitions and all 40 allowed
equality representations for 34 selected source calls. Adversarial controls cover
one-sided outcomes, missing/corrupt/extra/reordered stages, preimages, dirty words,
role/account/frame/transaction joins, foreign/reverted barriers, producer versions,
ordinals, protected aliases and inferred continuity. Fixed-admin equality, missing
hints, wrong values and 33-byte keys/words are explicit negatives. Runtime/pointer
excursions and creation remain refused. Negative cases include a valid balance
row and require whole-projection refusal.

## Saved replay and validation

The final [replay report](evidence/btr-coupled-role-candidate-20260928.json) and
[source inventory](evidence/btr-coupled-role-candidate-20260928-source-inputs.json)
are exact copies from `out/btr-coupled-validator-20260928/replay-01`.
Report SHA-256: `b6c3f807e2b087a356ad180ed5eb8fb8e66b18dfe9b21bfedaf5ee4966b7bb00`.
Inventory SHA-256: `6dc8b6384993d7376b9f1c80fc6b523d6e733354593c38657b8b043e3f884d49`.
All 201 source inputs remained unchanged, including the as-run replay tool.

The canonical-bound BSC interval **[122288006,122289030)** contains 1,024
Extended v5 blocks. All **110,139** rows match both the unchanged current baseline
and historical protobuf output. The canonical overlap contains 110,138 same-block
matches and one unchanged native-only row, plus 4,012 reference-only retained
matches and 66,265 cold observations left unknown.

| BTR observed scope | Result |
| --- | ---: |
| Emitted rows / same-block canonical matches | 178 / 178 |
| Initialized observed holders | 16 |
| Reference-only retained matches | 0 |
| Cold reference observations | 167 |
| Nonzero cold reference observations | 80 |
| Changing role membership / validated coupled operations | 0 / 0 |
| Fixed PAUSER admin writes | 0 |
| Whitelist length / canonical index writes | 0 / 0 |

This window contains no observed role operations or scoped admin/whitelist writes.
Its parity does not establish real producer visibility for those source-derived
operations. Canonical RPC values never initialize retained state. Counts describe
observations and initialized holders in this interval, not global-holder support.

Release preparation `prepare-02` reproduced both candidate fixture files exactly.
The build uses the pinned toolchain/lockfile and the worktree-local target
`out/btr-coupled-validator-20260928/target-isolated`. On merged main
`65a9183a0e49236ac92c8eb7317b799c9044caef`, formatting, **1,097 tests in 84 suites**
(59 nonempty), workspace all-target Clippy with warnings denied and the WASM
workspace check passed. Logs are `fmt-check-integrated-01.log`,
`workspace-tests-01.log`, `workspace-clippy-01.log` and `wasm-check-01.log` under
that output directory. Offline validation and native saved replay do not qualify
a new WASM package or live stream.

No live RPC, Firehose/Substreams or sink calls are part of this work. Runtime,
proxy/init history, coherent initial state, real producer visibility, replacement
package/getter and final-holder qualification remain open.
