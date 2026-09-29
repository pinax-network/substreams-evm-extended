# TOPS bounded LPInfo candidate

This separate **NOT-QUALIFIED** profile adds bounded metadata admission for
`0xcdf52c0b13c24f32f1d8d4ec6356203a1ef0826a`. The historical 431 and qualified 425
layouts are unchanged. All six issue #61 exclusions remain excluded from the
qualified cohort.

The candidate removes only broad mapping root 32 and adds
`lpinfo_array: "tops_solc_0_8_28_lpinfo_prefix"`. Balance root 5 and every other
historical permission remain unchanged. The parser pins the reviewed scalar,
mapping and address-list fields, exact address and runtime hash, preventing an
additional generic permission from bypassing the selected namespace's bounds.
It does not admit creation or changed runtime code.

## Complete operations and conservative omissions

Root 31 is `mapping(address => LPInfo[])`, with three uint256 words per record;
root 32 is the user's LP credit. Admission requires a canonical root 31 address
preimage, successful persisted effects, positive unique selected ordinals and
one complete operation in one physical call frame. Arrays must contain at most
six records both before and after. Independent or extra root 32 writes refuse.

`createLPInfo(address,uint256)` must match the source's exact head increment,
amount, creation time and checked 100-day expiry stores. Cleanup through
`transfer` or `transferFrom` must match an expired prefix, ordered forward copies,
reverse tail clears and repeated length decrements, then the wrapped credit
addition. Expiry equal to block time counts as expired. Zero or wrapped-zero
expired sums provide no cleanup permission. ABI address padding, calldata,
origin/time context and physical frame barriers are checked; these checks do not
independently establish caller authorization or external contract behavior.

An omitted equality store is accepted only when actual observed stores or an
earlier accepted block-local operation independently fix both values. Omission
never supplies an old zero. All logical stages, including omitted ones, must
respect physical continuity and protected scalar, mapping, balance and admitted
address-list keys. Multiple operations require distinct physical frames. Unknown
selected no-ops cannot disappear through the ordinary changed-write filter.

These conservative rules may reject a valid call whose producer omits an
unanchored equality. Missing non-equal stages are detected in the tested bounded
operation families; the validator does not claim that all possible incomplete
traces are distinguishable from another valid initial state. Unrelated historical
field permissions retain their existing limits. This is not a global router-path
refusal policy or whole-call validation.

## Source and projector evidence

The helper binds 35 immutable raw artifacts from both [initial source proof](tops-operation-proof.md)
and [original-runtime cleanup proof](tops-runtime-cleanup-proof.md), including
complete capture, inputs/outputs, metadata, source maps, official compiler report,
source inventories and transcripts. Report crosslinks and compiled settings bytes
must agree with those artifacts. Nine saved sources contain eight exact
OpenZeppelin dependencies; the custom token source still lacks an independently
recovered exact public maintainer pin. Existing complete runtime/creation and
immutable reconstruction is reused; no new compiler or VM behavior is added.

Projector controls convert 84 bounded original-runtime prefix cases, 21 other
representable successful transfer contexts and three representable original-runtime
append cases to Extended v4/v5 records. Context timestamps outside the signed
protobuf timestamp domain refuse conversion. All 21 source reverts are filtered;
nine explicit host failures cannot become successful chain effects. Synthetic
tests cover omitted/equal stages, aliases, bounds, framing, malformed words,
unknown no-ops, runtime and creation refusals. The original block 123561227,
transaction 63, call 14 fixture passes with its unchanged header/origin and all
23 stores, producing its two exact balance rows. The historical profile still
refuses that fixture. This comparison does not turn the synthetic external source
proof into a full historical transaction execution.

## Saved replay and remaining gates

The Rust replay driver targets the original saved interval
`[123561000,123562024)`. It pins every whole PB to the immutable historical manifest
and compares TOPS separately against the unchanged complete canonical reference.
The ledger starts empty; only emitted balance rows initialize observed holders.
Reports separate same-block matches, retained-only matches and cold observations.
Any first refusal stops processing with a partial report and preserved outputs.

Frozen `source-02` and `replay-01` passed all 1,024 blocks. The
[report](evidence/tops-lpinfo-candidate-20260929.json) and
[source inventory](evidence/tops-lpinfo-candidate-20260929-source-inputs.json)
retain their exact inputs and output hashes. TOPS emitted 42 complete rows (four
zero amounts), all matching the same-block canonical reference, across five
initialized observed holders. Five additional observations matched retained
state. Forty observations remained cold, including 30 nonzero amounts; all 87
canonical observations are accounted for without seeding the ledger.

The saved interval contains one admitted cleanup: five length decrements and
21 metadata stores, including its credit update. It contains no observed append
or equal metadata store. Those paths have source-derived and adversarial tests,
not additional observed-chain coverage. The cohorts are unchanged; this replay
projects TOPS separately and does not rerun or extend the historical cohort's
qualification.

No fresh live calls are made. Actual deployment/current runtime, custom-source
attribution, external dependencies, producer visibility, replacement package,
initialized-holder getters and longer windows remain separate qualification gates.

Reproduction from the repository root, with the pinned Rust toolchain and an
isolated `CARGO_TARGET_DIR`:

```sh
cargo run --locked --offline --release -p erc20-balances-tools --bin prepare_tops_lpinfo -- ORIGINAL_PACKAGE FRESH_PREPARATION
cargo run --locked --offline --release -p erc20-balances-tools --bin replay_tops_lpinfo -- ORIGINAL_PACKAGE FRESH_REPLAY
```

Preserved local attempts live under `out/tops-lpinfo-candidate-20260929/`.
The original missing-clear, timestamp-zero grounding, duplicate-frame and
counter-discovery failures remain there alongside their later passing controls.
Final offline validation passed 1,396 tests across 139 library, binary and
integration suites (92 nonempty), plus 22 empty documentation-test suites, with
zero failures or ignored tests. Workspace/all-target Clippy with warnings denied,
WASM workspace check, pinned formatting and staged whitespace checks passed.
The isolated logs and exact staged manifest are retained in the ignored handoff.
