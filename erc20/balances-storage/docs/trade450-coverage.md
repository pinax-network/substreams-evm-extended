# GIGGLE, LABUBU and ARKIE balance layouts

These three candidates from the original BSC RPC balance ranking are reviewed
on **122288006–122289029**, inclusive. Token labels identify the sampled
contracts. The explicit layouts use one RPC-free `map_events` and the shared
`evm.balances.v1.Events` output; the production default remains empty.

| Rank | Token | Contract | Getter |
| ---: | --- | --- | --- |
| 404 | GIGGLE | `0x20d6015660b3fe52e6690a889b5c51f69902ce0e` | Raw uint256 mapping at root 0 |
| 405 | LABUBU | `0x3494dfe19b721dac6c5c8d7470c8f89548177777` | Raw uint256 mapping at root 0 |
| 433 | ARKIE | `0x6cd3025f851f36e74937d4e652f6ae9d4b2e88cf` | Raw uint256 mapping at root 0 |

## Getter and storage review

All three complete inherited `balanceOf` bodies return `_balances[account]`
directly, without an overriding getter, external call, caller branch, clock,
conversion or fallback. Their custom transfer logic changes stored balances and
metadata; it does not derive the returned balance. The
[source evidence](evidence/trade450-source-review.json) retains complete source
files, compiler declarations, storage types and bytecode. The compiler-produced
runtime is reconstructed using only declared, nonoverlapping immutable and CBOR
suffix replacements, and bound to fresh parent, sampled and final code reads.
This is a review of returned compiler artifacts, not a local recompilation claim.

GIGGLE's router/pair immutables, bot flags, fee exclusions, launch state, limits,
fees and transfer-delay bookkeeping affect transfers. Its two trading flags
share slot 13. ARKIE's immutable deployer and mutable supervisor, fee, burn and
monitoring permissions likewise affect writes. Independent getter calls verify
the two GIGGLE immutable addresses and ARKIE's deployer against their bytecode
replacement values; these are not external balance-getter dependencies.

LABUBU's price history is a **fixed six-element array of three uint256 words**,
explicitly covering slots 8–25. It is not a dynamic array or a recursively
accepted record width. Slot 26 packs two uint8 heads and a bool. Slot 34 packs
an address, a bool at byte 20 and a uint64 at byte 21. The original survey's
refusals at blocks 122288793 and 122288855 came from **slot 6**, the inherited
reentrancy guard's 1→2→1 writes. Source declarations and captured original
transactions establish that this field is independent of the balance getter.

Allowances use root 1 in each profile. Other accepted mapping roots come from
explicit address-indexed source declarations. Unknown fixed words and mapping
roots still reject; long string payloads are not admitted by accepting their
name/symbol head words. This cohort does not use a generic multiword-record
exception.

## Independent controls and replay

Fresh canonical hash-pinned controls cover **60 raw balances** (0, 1, 123 and
uint256 MAX for five addresses per token, including null), **158 ordinary or
fixed-array metadata controls**, and **six mixed packed controls**. A further
**150 field getter calls** verify the actual returned fields, including array
indices and packed offsets. All **374 override request/response pairs**, three
immutable calls and nine invalid-call refusals are bound to the exact recorded
requests. Null raw changes match RPC while public Events omit the null address.
See the [request evidence](evidence/trade450-rpc-requests.json).

The strict native scan passes every complete Extended block. The first
packaged audit stopped after 505 checked blocks and 33 balance checks because
of an RPC transport failure, with zero mismatches; that
[initial failure](evidence/trade450-initial-rpc-failure.json) is preserved.
The successful fresh run uses `out/trade450-rpc-verified`.

Final replay metrics are recorded in the [summary](evidence/trade450-summary.json),
[packaged RPC](evidence/trade450-rpc.json),
[native holder](evidence/trade450-holder-coverage.json) and
[actual-WASM retained-state](evidence/trade450-wasm-holders.json) reports.

| Token | Emitted RPC checks | Reference observations | Initialized holders |
| --- | ---: | ---: | ---: |
| GIGGLE | 15 | 30 | 11 |
| LABUBU | 18 | 30 | 14 |
| ARKIE | 18 | 27 | 10 |
| Total | 51 | 87 | 35 |

Parent initialization uses 70 independent balance/storage reads. The actual
packaged events drive retained state; reference amounts never repair it.
There are six emitted zeros and 36 observations matched from retained state
without a new event. Final RPC checks match all 35 initialized holders, including
nine zero balances. Complete canonical clocks cover the 1,004 empty-output
blocks. All completed gates have zero mismatches, and balance processing makes
zero RPC calls.

Three first-active Rust fixtures and two additional original LABUBU refusal
fixtures retain every token-writing transaction from their complete source
blocks, with nine and eight independent RPC amounts respectively. Nine Rust
regressions cover captured independent RPC amounts, raw/null
values, metadata isolation and restoration without holder writes, all 18 array
words, mixed packed offsets, and unknown mapping/fixed/string-payload rejection.
The original slot-6 refusals remain reproducible when that explicit reviewed
word is removed.
All nine focused tests pass. Rust qualification, capture, retained-holder and
export helpers are archived locally under
`out/next-candidate-rust-scripts/trade450-followup/`.

Coverage remains bounded to the initialized observed holders and this window.
The cold replay has 36 unknown observations, including 26 nonzero values; these
require a checkpoint or sufficient earlier history and are never assumed zero.
There is no global-holder enumeration, new combined checkpoint or guarantee for
every future administrative path. Reproduce with Rust `audit-rpc` and
`holder-coverage`, the fixture layouts and this same range, using fresh output
directories.
