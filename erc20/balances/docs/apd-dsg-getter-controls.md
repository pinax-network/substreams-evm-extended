# APD / DSG offline getter and metadata controls

This host proof executes the exact saved APD and DSG runtimes to check direct
`balanceOf(address)` against independently selected root 0 storage words. It also
checks each admitted metadata group through its actual getter and feeds measured
metadata operations into the native projector. It adds no production rule,
layout, dependency, schema, VM opcode or runtime qualification.

APD is `0x001208f7f53f78db2b32e1c68198d3e8f320aa23`, with saved solc
0.8.24 settings, seven immutable replacements and exact CBOR substitution.
DSG is `0x3a090ac70c4f453838c34490e3b1cf925c03fc71`, with saved solc
0.7.5 optimizer runs 200 and exact CBOR substitution. Complete capture hashes,
all source/compiler fields, runtime/creation reconstruction and both original
boundary hex files are verified. There is no fresh compiler execution or network
request. Exact independent primary revisions for the flattened token sources
remain unestablished; original source notices and historical match labels remain.

The frozen `operations-04` run passed 2,409 actual runtime calls: 2,378 returns
and 31 exact expected reverts. It recorded 409,882 opcode steps and 5,412 effects,
all resolved to the saved Solidity source. The 120 native projector controls
comprise 41 raw metadata controls and 79 measured operation controls, with 137
mixed balance/getter comparisons. All 1,270 workspace tests passed across 118
suites (78 nonempty), with no failed or ignored tests. Formatting, all-target
Clippy and the WASM workspace check passed in the isolated target directory.

Committed copies are byte-identical to the frozen run:

- [Report](evidence/apd-dsg-getter-controls-20260929.json), SHA-256
  `78e9775f23b7672566931894f76a93ad16867764f5a894fe16d8f5f9b6a74c8c`.
- [Source inventory](evidence/apd-dsg-getter-controls-20260929-source-inputs.json),
  259 files, SHA-256 `c13f8bf12749ec68d6fb0b684697e02ac17d853526593a8235eab7b5c3c63213`.
- [Full case inventory](evidence/apd-dsg-getter-controls-20260929-cases.json),
  [operation transcripts](evidence/apd-dsg-getter-controls-20260929-transcripts.json)
  and [projector controls](evidence/apd-dsg-getter-controls-20260929-projector.json).

Every complete raw and annotated case remains under
`out/apd-dsg-getter-controls-20260929/operations-04/`. Earlier executions remain
preserved. Log 01 records an initial type-inference compile error; log 04 records
a test delimiter typo; log 06 records two Clippy style findings, subsequently
fixed. Runs 02 and 03 are explicitly preliminary: later controls clear the DSG
unused array tail in coherent perturbations and add direct getter postconditions
and exact log/store ordering. They are not substituted for the frozen evidence.

The bounded matrix covers:

- Four holders per token (one observed holder, a second address, token self and
  zero address), two callers, and selected balance words 0, 1, 123 and uint256 max.
  Getters must return the exact raw word, read only that balance leaf and leave
  every storage cell and log unchanged.
- Allowance, nonce, supply, pair and fee scalars separately and together, plus
  DSG totalBurnt and canonical role states. Actual metadata getters must witness
  each perturbed cell. Raw nonce/ratio max values are storage-domain controls;
  they are not claims that permit or a bounded setter can produce those values.
- APD full-width roles, zero members, grant/revoke no-ops, renounce confirmation,
  authorization failures, approval and its zero-party errors, pair and exact tax
  setter limits. Source-derived expectations check ordered stores, full log topics
  and data, all final cells and rollback.
- DSG's eleven pinned historical synthetic role operations, re-executed on the
  exact runtime with the same ordered stores and PCs. Fresh controls cover
  approval max/reset/replacement, actual allowance getters, ratio types 0/1/255,
  limits 0/100000/100001, pair updates and unauthorized/zero-party refusals.
- Metadata-only Extended frames emit no balances. Measured operations with both
  complete and omitted equal-value stores, and separate raw metadata frames with
  one synthetic balance observation, emit exactly one row matching a separately
  recorded runtime getter. Synthetic frames do not establish producer visibility.
- Excluded strings, tax/fee receiver words, DSG decimals and role admin fields
  remain refused, including change/restoration with no balance row. Captured
  contexts with synthetic runtime X→Y→X changes refuse; reverted changes are
  ignored. Missing mapping preimages and incomplete DSG set operations refuse.

The tool saves every raw execution before annotation and assertions, preserving
failed attempts and callback errors. It records exact code SHA, prestate, calldata,
caller, PC/stack trace, reads, stores, logs, Keccak witnesses and full committed
state. Saved compiler maps lack generated bodies; unresolved spans are explicitly
reported if encountered, never substituted with new AST/Yul text. The compact
transcript omits getter traces, whose complete raw and annotated files remain in
the full case inventory.

Run from the repository root with a fresh output directory:

```sh
CARGO_TARGET_DIR=out/apd-dsg-getter-controls-20260929/target-isolated \
  cargo run --locked --offline -p erc20-balances-tools \
  --bin execute_apd_dsg_controls -- out/apd-dsg-getter-controls-20260929/NEW_ATTEMPT
```

The sixteen focused tests include capture/immutable/CBOR mutations, wrong getter
code, corrupted effects, missing logs, callback failure and token-specific refusal
boundaries. PR #93 already checks cold versus explicitly seeded retained values,
checkpoint clocks and synthetic continuation; those controls are not duplicated.
Neither this synthetic matrix nor two saved runtime boundary observations proves
continuous runtime history, live getter behavior, all holders or parity of a newly
packaged WASM module. Permit signatures, tax-transfer callbacks, constructors and
new RPC/stream/holder qualification remain outside this proof. Issue #3 retains
those qualification gates.
