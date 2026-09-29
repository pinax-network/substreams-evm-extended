# Two complete captured legacy runtimes

These fixtures preserve the exact WKEYDAO and GOT source responses and separate
fresh solc 0.7.5 outputs. The top-level `*-capture.json` files are the original
independently pinned captures; the per-target copies come from the source builder
and must be identical. Original source notices, compiler metadata and all CBOR
bytes are retained. There are no runtime substitutions.

The four shared OpenZeppelin dependencies and GOT's additional ECDSA/IERC20
sources exactly match commit `8e0296096449d9b1cd7c5631e917330635244c37`.
`primary-sources.json` records each URL, body, digest and classification. The
custom AGPL sources remain explicitly unestablished as independent maintainer
pins. The MIT dependency license and immutable official compiler manifest are
copied without changes.

See [the focused proof document](../../../docs/wkey-got-operation-proof.md) for
schemas, exact runtime/compiler hashes and boundaries. These are host proof
inputs, not ingestion layout candidates. Final `source-02` compilation and
`operations-02` passed 1,263 calls against the same source inventory: 1,000
returns, 252 expected reverts, six named source INVALID controls and five
explicit unsupported boundaries. Earlier runs remain preserved. The focused
document links the copied reports, inventories and transcripts. All 1,163
workspace tests, formatting, Clippy and the WASM check passed; no live
qualification is implied.
