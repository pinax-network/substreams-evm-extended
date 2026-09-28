# GM local operation proof inputs

These complete captures and compiler artifacts support a **host-only,
NOT-QUALIFIED Phase A proof**. They are not candidate layout parameters.

The implementation capture contains 20 source bodies. Each independent proxy
capture contains seven: 34 records and 27 unique source/path profiles. All
original metadata, settings, layout, ABI, source IDs, bytecode, source maps,
labels and null fields are preserved. Fifteen implementation dependencies match
an exact Ondo-vendored snapshot; the seven unique proxy dependencies match an
upstream OpenZeppelin pin. Five custom BUSL-1.1 sources have explicit independent
primary gaps. Their contextual public alternatives are retained as nonmatches.

Official solc 0.8.16 uses the unchanged original London/optimizer-200 inputs,
including their different remapping and metadata settings. Only outputSelection
is added. The full raw compiler outputs are pinned, including generated source
maps. Original SPDX headers and compiler license metadata remain embedded;
source-licenses.json records their hashes. LICENSE-proxy-upstream is the exact
upstream MIT text, not a replacement license for the custom or vendor sources.

Exactly one 53-byte CBOR replacement at offset 8,218 (implementation) or 771
(proxy) reconstructs the full saved runtime. No arbitrary suffix normalization,
link or immutable patch is accepted. Every creation match, onchain creation and
creation transformation field remains null. Synthetic compiler creation returns
the compiler runtime; the operation matrix separately compares the entire
compiler and captured runtimes. This is not deployment evidence.

Source-01 preserves the initial creation-CBOR helper refusal. Source-02 is the
first successful full official compilation; final source-03 reproduces all
compiler artifacts with their exact raw-output pins and final as-run source
inventory. `report.json` and `source-inventory.json` match source-03. The
1,567-call final operations-03 proof compares 783 runtime pairs plus one compiler
constructor; its measured scope is linked in
[the focused proof](../../../docs/gm-operation-proof.md). No failed attempt was
rewritten, and no source or metadata text was normalized.
