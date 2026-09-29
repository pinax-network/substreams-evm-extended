# WKEYDAO and GOT enumerable candidates

Two separate **NOT-QUALIFIED** candidates narrow the historical 431-profile
baseline. This change does not modify the production validator,
parser, persistence rules, output, dependencies or historical qualified cohort.
The final [Phase A proof](wkey-got-operation-proof.md) is bound separately from
the candidate's source-derived projector controls and canonical-bound saved
replay. None of these establishes live qualification.

| Profile | Contract | Removed broad permissions | Exact legacy rule |
| --- | --- | --- | --- |
| WKEYDAO | `0x194b302a4b0a79795fb68e2adf1b8c9ec5ff8d1f` | root 9 width 3 | root 9, `[bytes32]`, `oz_3_4_2` |
| GOT | `0x701add4311e85c1f9c1549319fe2c476bc8a1b8b` | root 8 width 3 **and** root 8 in `other_mapping_slots` | root 8, `[bytes32]`, `oz_3_4_2` |

Each rule omits `membership_root`, including explicit null. No boolean mapping,
admin scalar/path, standalone index permission, array interval or creation rule
is introduced. Restoring every removed value at its original position reproduces
the complete baseline profile. GOT retaining either broad rule is refused.

Both preserve balance root 0, allowance root 1, nonce root 7 and MaxSupply slot 6.
WKEYDAO also preserves domain slot 8 and receiver/ratio slots 10–14; GOT's role
root is 8 and its receiver/ratio slots are 9–13. All other scalars and runtime
guards stay exact. There are no proxy or deployment rules to broaden.

| Profile | Runtime bytes | Keccak-256 |
| --- | ---: | --- |
| WKEYDAO | 7,991 | `0x84d1cbfc7b7c569181930ce930f0dbe6edb8e8df5631b0a066bd0197d109b9f3` |
| GOT | 8,440 | `0x8f10d493bbd10ba2062c25efaa1cfe1035b392f339a485fce3faed0c0768dc9c` |

## Source-selected structural scope

The prerequisite independently compiles each complete captured source
with official solc 0.7.5, optimizer 200 and the original omitted EVM setting
(default Istanbul). Four WKEYDAO and six GOT dependencies match immutable
OpenZeppelin revision `8e0296096449d9b1cd7c5631e917330635244c37`; both custom
AGPL source maintainer revisions remain unestablished. The helper pins all
**25 final Phase A artifacts** before parsing and repeats complete source,
compiler, raw metadata, creation append and primary-body/URL verification. Both
original captures must equal their committed copies. The compiler and operation
reports bind the same 231-input inventory; the operation report's additional
compiler-report artifact is checked by its exact raw digest. The two runtimes
are not interchangeable.

The final prerequisite report SHA-256 is
`0dfc95a2b5c949ec5eab362fb7a638256a86de527b1ce94cb06966426aa86e08`;
compiler report `bd61d288b2d02fc6820c89c7f88e2907c11551fae3271c2bf429634bdcb555c0`;
source inventory `4782430d017a36991bbefe3577cf72380b311025a701a632cb503d8aff7a2ca9`.
Its 1,263 synthetic calls comprise 1,000 returns, 252 reverts, six named INVALID
controls and five unsupported contexts. These are distinct from saved replay
activity and never imply deployed initialization.

For `R=H(role||root)`, length is `R`, array elements are `H(R)+i`, one-based
positions are `H(address||(R+1))`, and the admin word is `R+2`. The existing
legacy template requires length→element→position for append and
destination-copy→moved-position→tail-clear→length-decrement→removed-position
for removal. Tail/sole removal performs equal self-swaps. Only independently
derived equal stages may be omitted; changing stages are mandatory. No-op
grant/removal proves no unseen initial coherence.

The unchanged legacy Extended-v3 root fallback and v4/v5 positive actual frame
boundaries remain distinct from newer coupled templates. Numeric words normalize
up to 32 bytes; 33-byte words refuse. Complete operations stay within one role,
root, account and structural frame; unknown stores and call boundaries remain
barriers. Recognized standalone length/index/admin/anchor no-ops refuse, while
unrecognized equal array writes retain ordinary no-op handling. Checked logical
growth is conservative admission, not a claim that malformed source MAX growth
reverts rather than wrapping.

Twelve public-projector tests select exact target, operation name, signature
and code kind from the raw-pinned final proof. Each runtime's own store PCs are
verified before translating its writes and Keccak witnesses into synthetic
Extended blocks. Tests cover 30 coherent source calls per target with every
optional equality subset, plus nine sequential transitions. They exercise
actual add/removal/renounce,
full-width roles, optional equality subsets, sequential transitions, malformed
source successes, aliases, continuity, dirty words/preimages and atomic refusal
alongside a valid balance write. Cross-target transcript substitution is forbidden.
Both creations remain refused: GOT's synthetic constructor success would not
prove deployed initialization; WKEYDAO stops at unsupported CHAINID and rolls back.
External fee and permit contexts remain outside supported source execution.

Five host binding/counter tests cover full-profile restoration, either GOT
leftover permission, every artifact/raw-cache mutation, measured append/removal/
no-op counters and cross-target root traps. The additive replay dispatch has its
own regression. Synthetic raw-balance zero/MAX and MaxSupply-decrease controls
preserve ordinary storage units and distinct metadata fields, without adding a
cap invariant or getter formula. Negative operations include a valid balance
write and require whole-projection refusal.

## Saved replay and qualification

A dedicated diagnostic counts changed persisted outer length words only, using
the closed map WKEYDAO→root9 and GOT→root8 plus exact verified Keccak preimages.
The other target's root, admin/index/array words, equality, failed transactions,
reverted calls and malformed words cannot supply that count. Only after a
successful complete projection may length witnesses be called validated legacy
operations. This counter confers no permission or authorization claim.

The [replay report](evidence/wkey-got-enumerable-candidate-20260929.json) and
[source inventory](evidence/wkey-got-enumerable-candidate-20260929-source-inputs.json)
are exact copies from `out/wkeydao-got-enumerable-candidate-20260929/replay-01`.
Report SHA-256: `a7cddb299e1663b007b4fba7f0fb00ca3bbe08d253e114046581217c81159f74`.
Inventory SHA-256: `4ca94b61fb7279cf88ab912af8629b8475abc9095327af968b4c389257999475`.
All 227 source inputs remained unchanged, including the as-run runner, proof
sources/tests, complete frozen proof artifacts and candidate controls.

The saved BSC interval **[122288006,122289030)** contains 1,024 Extended v5
blocks. All clocks/parent identities match the canonical-bound history, and all
**110,139** full protobuf rows equal the unchanged current baseline and
historical output. Canonical comparisons contain 110,138 same-block matches,
one unchanged native-only row, 4,012 reference-only retained matches and 66,265
cold observations left unknown.

| Bounded observed scope | WKEYDAO | GOT |
| --- | ---: | ---: |
| Emitted rows / same-block canonical matches | 30 / 30 | 21 / 21 |
| Initialized observed holders | 14 | 10 |
| Reference-only retained matches | 0 | 0 |
| Cold reference observations | 18 | 13 |
| Nonzero cold observations | 12 | 8 |
| Changed role-length writes / validated legacy operations | 0 / 0 | 0 / 0 |

Canonical RPC values never initialize retained state. These counts describe
observations and initialized holders in this interval, not global-holder support.
No source calls were spliced into saved activity. The window contains no observed
role operations, so it does not qualify actual role/preimage/equality visibility.
Release preparation `prepare-02` reproduced both candidate fixture files exactly.
On merged main `63548759dc6e5b8ff230212a9aea7541b797d7ea`, the pinned, locked
offline workspace run passed **1,181 tests across 102 suites** (69 nonempty).
Formatting, workspace all-target Clippy with warnings denied and the WASM
workspace check passed. Logs are `fmt-check-02.log`, `workspace-tests-01.log`,
`workspace-clippy-01.log` and `wasm-check-01.log` under
`out/wkeydao-got-enumerable-candidate-20260929/`, using its unique `target-isolated`.
Native saved replay and offline checks do not qualify a new WASM package.

Initial set coherence/uniqueness/unused-tail zeros, runtime/history continuity,
actual producer framing/preimage/equality visibility and replacement package/
getter/initialized-holder/final-state qualification remain open. No live RPC,
Firehose/Substreams or sink calls are part of this work.
