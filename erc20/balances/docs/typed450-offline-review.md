# APD and DSG: saved allowance review and replay

This review advances two of the 19 unresolved candidates from the first 450
BSC tokens. **Neither token is newly qualified.** It uses previously captured
source, compiler output, runtime bytes, full Extended blocks and canonical RPC
output from blocks **122288006–122289030, stop exclusive**. No new chain request
was made.

| Candidate | Balance getter | Historical strict failures | Actual rejected field |
| --- | --- | ---: | --- |
| APD, rank 401, `0x001208f7f53f78db2b32e1c68198d3e8f320aa23` | Direct `_balances[account]`, root 0 | 5 blocks | `mapping(address => mapping(address => uint256))` allowance at root 1 |
| DSG, rank 418, `0x3a090ac70c4f453838c34490e3b1cf925c03fc71` | Direct `_balances[account]`, root 0 | 6 blocks | The same two-address allowance shape at root 1 |

All 11 recorded refusals were unconfigured allowance writes. They were not
wrong emitted amounts: the prior survey's 20 APD and 30 DSG sampled RPC value
checks had no mismatches, but strict processing still failed. Five DSG cases
contain an allowance increase followed by restoration in a successful
transaction. Those are two persisted changes, even though their endpoint values
are equal. A guard cannot discard the sequence merely because its net change
is zero.

The [typed mapping-path rule](typed-mapping-paths.md) can describe exactly these
two address keys, plus each token's single-address permit nonce. APD also has
boolean role membership at root 6, expressible with `["bytes32", "address"]`.
Its outer admin field is written only during construction and remains guarded
in the postdeployment candidate. Constructor-only strings and fee receivers are
also excluded. Independent balance/metadata controls remain pending.

DSG's root 8 is different: per-role storage contains an EnumerableSet array at
offset 0 and an address-to-index mapping anchored at offset 1. Its admin field
is at offset 2 and has no setter callsite. Typed terminal offsets do not model
that intermediate mapping offset or the correlated array/index updates.
The original partial candidate leaves every role storage shape unconfigured.
At that review, valid role grant, revoke and renounce operations still needed
dedicated support; absence from a sampled window does not establish support.
Zero address is a legal set member, and tail self-swaps can produce unchanged
writes requiring explicit handling.

The separate opt-in [enumerable rule](enumerable-role-sets.md) now implements
source-bound operation checks. The original candidates and evidence retain
their recorded configuration; no profile inherits producer or package
qualification from the new implementation.

## Evidence and limits

A subsequent native Rust replay covered **all 1,024 cached blocks** in the same
interval, using the two proposed fragments and the saved canonical RPC output.
Every cached block hash, parent link and saved canonical-bound clock matched.
The [durable replay evidence](evidence/typed450-candidate-replay.json) records
the exact input, runner, output and report hashes.

| Candidate | Emitted balances matching saved RPC | Emitted zeros | Reference-only observations, all cold unknown | Blocks with no RPC rows |
| --- | ---: | ---: | ---: | ---: |
| APD | 10 | 0 | 20: 15 zero and 5 nonzero | 1,019 |
| DSG | 15 | 5 | 13: 6 zero and 7 nonzero | 1,018 |

Both candidates had zero projection errors, amount mismatches, retained-holder
matches and native-only rows. Neither emitted a balance on a block without
captured RPC rows. Comparisons preserve all `Balance` fields, including the
contract's presence. No checkpoint or captured RPC value was used to initialize
native holder state. The 33 cold unknown observations remain unresolved; they
are observations, not a count of distinct holders. These results establish
neither full row parity nor holder completeness.

The replay used source commit
`2e35027634fe79fc5bc709dd5ee224e02eab5da9`. Its 18 production/build input hashes
were unchanged from the pre-build snapshot through completion. It observed
25 persisted APD storage changes and 41 DSG changes, with no storage no-op
records for either token. This window therefore supplies no producer-visibility
evidence for unchanged role-set writes. DSG role root 8 remained unconfigured.

This was a **saved canonical comparison, not new qualification**: zero network
requests, fresh RPC controls, Substreams/Firehose runs or sink commands. It did
not test a newly built WASM package or any later source revision. The full local
run remains under `erc20/balances/out/typed450-candidate-replay/`;
the earlier 431-profile replay and all historical inputs remain unchanged.

The offline source review checks saved compiler-input/source equality and
source hashes, reconstructs each complete runtime with only its declared
immutable and CBOR replacements, and compares saved parent/final bytes. It does
not rerun a compiler or make a fresh runtime observation. APD has seven
immutable replacements; DSG has none. Boundary equality alone does not prove
uninterrupted runtime identity inside the interval.

The [two regression captures](../tests/fixtures/typed450-offline/) retain complete,
byte-identical blocks: APD's first refusal at 122288154 and DSG's first restored
allowance at 122288046. Their canonical expected rows and manifest bind the
original block, reference stream, RPC package, source and runtime. Canonical
RPC-only holders are recorded separately from holders with persisted balance
changes; they are not initialized by assuming a missing storage write is zero.
The two blocks contain five emitted balances and six reference-only rows. Two
of those reference-only rows are nonzero token-owned balances, illustrating why
absence of a write cannot supply an initial holder balance.

The full local review, source copies, Rust verifiers and pending control plan
are retained under repository-root `out/typed450-candidates/`. Earlier compact
capture attempts remain there as superseded, unvalidated artifacts. Tests use
the full blocks instead.

## Replay with the enumerable rule enabled

A separate replay at source commit
`3eae9c839e06fec260e8717845547f21ccafa355` enabled DSG's root-8
`enumerable_address_sets` rule with `bytes32` keys and `oz_3_4_2` semantics.
APD's proposed fragment was unchanged. All 19 production/build inputs remained
identical from before compilation through completion. The
[candidate evidence](evidence/enumerable-role-candidate-replay.json) binds the
new configuration, source and saved inputs.

The earlier replay at `d5ec0b9` is preserved separately. Both full replays were
repeated after adding a guard for observed or inferred metadata slots that
alias a holder balance discovered without a hash preimage.

All 1,024 blocks completed without projection errors. The same 10 APD and 15 DSG
balances matched saved canonical RPC output, with zero mismatches or native-only
rows. The same 33 cold unknown observations remain. This window contains no
role mutations or persisted no-op records for these candidates; enabling the
rule here is a compatibility check, not a producer-visibility test.

The independent [431-profile baseline replay](evidence/enumerable-role-baseline-replay.json)
also covered all 1,024 blocks. Its 110,139 balances, including 15,259 zeros,
matched the full historical protobuf output exactly after normalizing row order.
Of these, 110,138 matched captured RPC in the same block. The one historical
native-only row had no same-block RPC observation and is not counted as a match.
There were also 4,012 reference-only observations matching independently retained
native state and 66,265 cold unknown observations. No profile was changed or
promoted, and this remains native Rust evidence rather than packaged WASM parity.

Fresh independent controls, actual packaged-WASM/RPC parity and initialized
holder/final-state checks remain required before adding either token to a
qualified cohort. Existing 431-profile fixtures and historical packages are
unchanged. Live Substreams, Firehose and RPC testing remains paused.

## Captured projector-to-ledger controls

The host [integration tests](../tools/tests/typed450_retained.rs) apply the actual
projector's output to the shared balance ledger using the two original full
blocks above. Block and canonical-row hashes are rechecked. These are separate
from the historical 1,024-block replay and do not alter its cold-holder counts.

| Original snapshot | Cold known holders | Cold reference-only observations | Explicit snapshot seeds | Seeded zeros |
| --- | ---: | ---: | ---: | ---: |
| APD, 122288154 | 2 | 4, including one nonzero | 6 | 3 |
| DSG, 122288046 | 3, including one zero | 2, including one nonzero | 5 | 2 |

A synthetic quiet successor retains the five cold-observed values and cannot
turn any of the six missing values into zero. In a separate mode, all eleven
canonical values are explicitly supplied as checkpoints **at their actual saved
end-of-block hash and height**. The ledger refuses to apply the snapshot's own
block again or a successor with the wrong parent. A correctly linked synthetic
successor preserves the eleven values and their checkpoint provenance, while a
never-observed holder stays unknown. Nothing is seeded before its observation.

A derived DSG continuation retains only its two original successful allowance
writes, preserving their call/preimage context, and assigns a clearly synthetic
successor identity. Their old/new values restore the starting allowance. The
actual projector emits no balance rows, so both the three cold-observed holders
and the separate five-holder checkpoint remain unchanged. DSG's token-owned
20,390,672,839 balance stays unknown in cold mode and known only when explicitly
seeded from the canonical snapshot. APD's token-owned 1,203,955,079,705,976 value
similarly belongs to its snapshot rather than an inferred cold zero.

This validates captured-output retention and exact checkpoint boundaries. It is
not a new bound-bytecode getter/metadata proof, captured successor replay, fresh
final-state RPC snapshot, package/WASM qualification or global holder enumeration.
Those independent controls and the paused live gates remain open under issue #3.
