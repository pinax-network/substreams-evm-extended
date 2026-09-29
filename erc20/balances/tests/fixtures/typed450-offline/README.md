# Unqualified APD / DSG historical regressions

These two complete Extended blocks are byte-for-byte copies of saved captures:
APD at 122288154 and DSG at 122288046. No new network observations were made.
Both candidate layouts remain **unqualified**, outside the published 431-profile
configuration. These historical partial fragments leave DSG role-set writes
unconfigured and guarded. The separate source-bound enumerable opt-in is now
implemented; these saved fragments do not inherit that later configuration.

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

The host [projector/ledger integration tests](../../../tools/tests/typed450_retained.rs)
consume these same unmodified blocks and canonical rows. Cold application makes
only the five emitted holders known and leaves six reference-only observations
unknown, including the two nonzero self-balances. Separately, explicit snapshot
seeds use all eleven canonical values at their actual end-of-block clocks and
apply only a synthetic successor; five of those values are known zeros. The seed
block itself and a wrong parent are refused. A separately labeled derived DSG
allowance-restoration continuation preserves both the cold and seeded holder
sets. These controls do not backfill earlier blocks or establish a captured
continuous initialized interval, final RPC snapshot or global-holder coverage.
