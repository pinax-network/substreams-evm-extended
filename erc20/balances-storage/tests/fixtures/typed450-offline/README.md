# Unqualified APD / DSG historical regressions

These two complete Extended blocks are byte-for-byte copies of saved captures:
APD at 122288154 and DSG at 122288046. No new network observations were made.
Both candidate layouts remain **unqualified**, outside the published 431-profile
configuration. DSG role-set writes remain unsupported and guarded.

The `*-expected.json` files contain **11 rows from the original immutable canonical
RPC stream**, independently of the storage mapper. Five holders have persisted
non-noop root0 balance writes and matching storage outputs; the other six rows
remain explicit reference-only gaps in `cases.json`. Two gaps have nonzero
balances. They are not inferred zeros or initialized retained-state results.

The tests derive the emitted holder set directly from successful, non-reverted
root0 writes. Removing the exact two-address allowance path reproduces the original
strict error. DSG's two allowance writes restore the original value and must still
be checked, including when isolated from balance writes.

`provenance.json` binds the cases, source proof, candidate spec and original
historical manifest. `historical-manifest.json` is retained verbatim from the
out-only two-case capture bundle; its `../proof.json` source-proof reference maps
to the copied `proof.json` here. Original absolute cache paths remain provenance.
Its relative expected-row files are copied alongside it. The two complete
all-token ranking reports remain under repository-root
`out/typed450-candidates/regression-two/`, with their exact filenames and hashes
retained in the historical manifest; the tests do not need duplicate copies of
those large inventories. The full source
and compiler response inputs remain in `out/typed450-candidates/inputs`; their
content hashes and complete saved runtime reconstruction records are preserved
in `proof.json`.

No fresh raw-word/metadata `eth_call` controls, complete 1024-block qualification,
packaged live RPC comparison or initialized-holder WASM audit is established by
these two tests. All live qualification gates remain pending.
