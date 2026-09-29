# OG retained inputs: bounded host reference

This separate host adapter retains finite raw OG inputs and calls the unchanged
`og_model` arithmetic. It does not change production layouts, balance protobufs,
other retained adapters, dependencies, the VM or persistence rules. It is not
qualified for deployment or global holder extraction. The relevant remaining
work is tracked in issues #5, #7 and #21.

## Inputs and outcomes

A checkpoint binds chain 56, the model revision, a positive epoch, activation,
block hash and timestamp, registered holders, permitted physical keys and raw
facts with their origins. The five saved checkpoints remain independent. Their
raw word counts are 153, 114, 633, 114 and 114; they concern different holders at
different heights. A key in the finite universe does not supply its value.
Missing input remains `Unknown`; a persisted equality store can establish an
actual value with an observed origin. Expected getter results never populate
facts.

Each registered holder has four separate results: raw basis, hourly reward and
stopping hour, independently requested daily reward, and combined observable
balance. The combined result preserves checked raw-plus-hourly overflow
precedence before using the daily result. Evaluation never advances stored
cursors or replaces raw facts with a calculated amount. Unsupported recursive
pool entry is `ScopeRefusal`; arithmetic failures are `ModelRefusal`. Neither is
a fabricated RPC revert or a gas simulation. The historical recursive responses
that omit error data continue to omit it.

Hourly planning requires at most 168 periods. Daily planning checks the U256
range before conversion/allocation and refuses more than 1,000 periods. Within
that bound it conservatively requires the full planned range, even if the
unchanged model would stop after three positive contribution conditions. Those
conditions are not equivalent to three elapsed days; a contribution that floors
to zero can count. Active-path planning also eagerly gathers pool, cap and
reserve inputs. Missing later inputs can therefore yield `Unknown` before a
source early stop or earlier arithmetic failure. No exact minimal-read or
arbitrary missing-input error-order parity is claimed.

## Runtime and dependency limits

The binding verifies full runtime bytes for OG, its helper, router and both pool
identities. Original OG artifacts contain token/helper/router bytes. A separately
saved YBC pool prestate supplies the byte-identical pool code whose Keccak agrees
with both independently recorded OG pool hashes. Only code is reused; no YBC
address, storage, initialization or new OG code capture is inferred.

Token/helper source lookup reports returned 404. This is historical lookup
evidence, not proof that source can never be found. No compiler reconstruction or
verified Solidity field names are claimed. Selected helper/router PUSH32
operands are checked at real instruction boundaries, separately from stored
pointer facts.

All five runtime identities are sticky guards. Any runtime excursion suspends
the epoch even if restored later. Token scalar 6 and helper scalars 1/2 check
both old and new low-160-bit dependency addresses, including a cold
foreign-to-bound transition. Full words remain raw facts. Because helper source
is unavailable and upper-bit independence is unproved, **every changed helper
pointer word** suspends, including an upper-only round trip. Unchanged bound
words with nonzero upper bits are accepted. The token scalar retains its
separate masked-address policy.

Other changed helper/router words and non-reserve pool words also suspend;
pool scalar 8 is the selected changing reserve input. These conservative guards
are not a global storage-admission policy. Suspended calculated results never
reuse a previous balance, while raw facts continue to be retained and compared.
A reset requires a separately initialized newer epoch at the same boundary,
restored runtime identities, the same registry and universe, and currently
complete required inputs. No old facts, origins or undo history carry over.

## Retention and evidence

Applied blocks require Extended v4/v5, exact parent/hash continuity, nondecreasing
time and fixed producer version after its first binding. The checkpoint may
lack parent/version metadata that was not captured. Every persisted effect is
validated before atomic commit. Same-key ordinals must be positive and strictly
increasing with continuous old/new words. Distinct-key ties and input order are
accepted for end-of-block calculations, without claiming source event order.
Code changes remain sticky regardless of such ordering.

Facts retain their checkpoint or observed origin, block metadata, epoch and
original PB digest. Each evaluation binds the complete retained state and its
clock by a deterministic input digest. Snapshots have strict schema, identity,
size and duplicate-key controls and intentionally omit undo history. Bounded
undo/replacement and portable journals preserve full raw state and provenance.
Resource limits independently bound holders, keys, effects, undo depth, total
retained cells and serialized snapshots/journals.

The original five snapshots and 111 controls yield 391 external observations:
303 values, 40 exact captured Panic(0x11) observations and 48 recursive
observations, including additional recorded gas budgets. The adapter does not
synthesize a revert ABI. Ten internal preview controls remain in the unchanged
pure-model tests; they are separate from the 391 external observations.

Original report/runtime/fixture bytes and publisher prerequisites are pinned.
One historical provenance gap remains explicit: the mismatch report's recorded
producer-source SHA does not equal the later saved Rust inspector copy. Both
are retained; that copy is not represented as the exact as-run producer. The
published original failures, runtime witnesses and sampled SLOAD values are
mechanically linked back to the immutable mismatch report.

## Saved continuation

The offline replay initializes only the earliest actual holder and pool A at
block 122288107, then targets the 922 original PBs
`[122288108,122289030)`. Its replay-only checkpoint joins the unchanged earliest
153 raw facts with exactly four missing pool cursor words from an independently
captured row at that same block, hash and timestamp. All 17 overlapping words
must agree. The resulting 157-fact checkpoint has structured provenance binding
both raw artifacts; all five original checkpoints remain unchanged.

Whole PB hashes must equal the immutable original
manifest before decoding. Independent pool captures compare 21 raw words at
923 boundaries, including initialization. Later captures never backfill the
ledger, and the other four holders are never inserted into this continuation.
There is no continuous independent getter history for the retained nonpool
holder. Model outcome counts and any first suspension must therefore be
reported separately from raw pool-word comparisons.

The replay saves decoded attempts, a raw journal, per-block counts, evaluation
input digests, pool comparisons, snapshots and source inventories. It rejects
stale compiled source or embedded fixtures and checks the inventory again at
completion. Midpoint snapshot continuation, bounded undo/replay and portable
journal replay compare the complete state or ledger, not only final amounts.
The saved-journal regression additionally rejects a 17-block undo atomically,
then undoes the full configured 16-block ring and replays it to identical ledger
state, including history and provenance.

The final continuation matched all 19,383 raw pool words across 923
boundaries. Every one of the 1,844 registered-holder evaluations retained a
known raw basis and suspended hourly, daily and observable results. The first
suspended block is 122288108: pool A's lock at slot 12 changes from 1 to 0 at
ordinal 843, cumulative slots 9/10 change at 861/862, and the lock restores at
868. The saved reason describes the last invalidating effect in that block,
ordinal 868; the complete input also preserves the earlier trigger. These
unreviewed pool writes remain sticky and are not admitted to regain calculated
coverage. Raw-word agreement does not establish continuous getter parity.

The frozen [report](evidence/og-retained-inputs-20260929.json) records 727
persisted selected storage effects, no code effects, and 274 retained writes.
The [source inventory](evidence/og-retained-inputs-20260929-source-inputs.json)
binds all 147 executed-source, embedded-fixture and central-document inputs.
Report SHA-256: `b99ed3784a9eb1d4435bde0029cf2c338a658c9eba8a943929cc24eed1511a9b`.
Inventory SHA-256: `48816566a80b4ba8f94c776183acfca898efa03f168fb2539a8c97fcfcda723a`.
The final run is `out/og-retained-inputs-20260929/replay-02`; preliminary attempts
remain alongside it. These offline results add no live or deployment
qualification.

Final pinned, locked offline validation passed: 1,442 tests across 144 suites
(96 nonempty), with no failures or ignored tests; workspace Clippy for all
targets, the WASM workspace check, formatting and staged whitespace checks also
passed. Independent verification rederived the complete continuation from
original PBs without calling the new adapter and confirmed its raw facts,
provenance, suspension and snapshots. Historical attempts and gate logs remain
under `out/og-retained-inputs-20260929/`.
