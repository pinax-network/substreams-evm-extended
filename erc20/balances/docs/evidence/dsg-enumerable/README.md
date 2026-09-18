# DSG enumerable role-set investigation

> Historical research snapshot, preceding the opt-in
> [enumerable role-set implementation](../../enumerable-role-sets.md).
> Runtime/producer and token qualification remain separate requirements; the
> synthetic cases and proof data below are unchanged.

This is saved-runtime research for unfinished support, not a production
validator or token qualification. No chain requests were made. The production
mapper still rejects DSG role-set mutations.

The saved Solidity 0.7.5 runtime (optimizer runs 200) has Keccak
`0x60cdb82077b195e34bf239223f78f452a414001e509cac48230a865895d86884`.
Its source SHA-256 is
`8eb6fe9ff8ab4d163c95db324a27e5a2ab8604a96cc49e67db7101bc8ce71dcc`.
The compiler output matches executable runtime bytes before the reviewed CBOR
suffix. The stored source map binds the instruction addresses to source lines.

| Operation | Ordered fields | SSTORE instruction addresses |
| --- | --- | --- |
| Append | length, new element, member index | 6494, 6510, 6529 |
| Remove | destination, moved member index, tail clear, length, removed index | 7257, 7277, 7308, 7310, 7335 |

The [annotated instruction windows](ordering-windows.json) establish this order.
The [ordering proof](ordering-proof.json) records the source/runtime bindings
and independent final-state comparisons. A bounded local Rust diagnostic
interpreter exercised 11 synthetic calls, yielding 39 stores, including nine
unchanged writes. It is not a validated full EVM and does not model gas or fork
costs. The [compact cases](runtime-cases.json) explicitly identify synthetic
prestates and preserve ordered writes and final words.

An executed unchanged SSTORE does not prove the Firehose producer exports that
record or establish its ordinal policy. The 1,024 saved APD/DSG blocks contain
no recorded unchanged storage writes for these tokens and do not establish
role-mutation visibility. The [omission review](optional-noop-review.md)
describes required checks for optional equal-value witnesses, structural call
boundaries, complete operations and event-level acceptance.

The incomplete validator and proposed tests remain local under repository-root
`out/typed450-enumerable-implementation/`; neither is compiled into the workspace.
Full interpreter traces, disassembly, helper source and saved compiler inputs
remain under `out/typed450-enumerable-runtime/` and
`out/typed450-candidates/`. Their hashes are preserved in the evidence here.
Implementation, adversarial tests, producer validation and fresh packaged
parity are follow-up work. Live testing remains paused until explicitly resumed.
