# Mai source/compiler proof fixtures

These files retain the complete original chain-56 Mai capture and exact fresh
solc 0.8.9 output. Fourteen OpenZeppelin source bodies match immutable revision
`8c49ad74eae76ee389d038780d407cf90b4ae1de`; every original notice and the MIT
license are preserved. The custom UNLICENSED Mai source has no independently
established maintainer revision in the reviewed material.

The only runtime transformation is the source-derived immutable cap `10^29` at
offset 1767 (AST declaration 1258). The original no-argument creation bytes are
unchanged. Both raw metadata strings match exactly. Complete compiler output
pins bind ASTs, runtime/creation maps and their generated Yul sources.

See [the focused proof document](../../../docs/mai-operation-proof.md) for hashes,
state schemas, measured constructor/operation scope and explicit limits. These
are host-proof inputs; no production layout candidate or runtime qualification
is implied. Final `source-02` / `operations-03` passed 1,632 calls against the
same source inventory: 1,427 returns and 205 exact expected reverts, with no
INVALID or unsupported outcome. All 1,216 workspace tests, formatting, Clippy
and the WASM check passed. The focused document links the copied reports,
inventories and transcripts; preliminary attempts remain preserved.
