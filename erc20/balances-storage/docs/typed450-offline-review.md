# APD and DSG: saved allowance refusals

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
The partial candidate rejects every role update. Valid role grant, revoke and
renounce operations still need dedicated support; absence from a sampled window
does not establish support. Zero address is a legal set member, and tail
self-swaps can produce unchanged writes that a future validator must retain as
witnesses.

## Evidence and limits

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

Fresh independent controls, actual packaged-WASM/RPC parity and initialized
holder/final-state checks remain required before adding either token to a
qualified cohort. Existing 431-profile fixtures and historical packages are
unchanged. Live Substreams, Firehose and RPC testing remains paused.
