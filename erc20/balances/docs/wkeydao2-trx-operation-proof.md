# wkeyDAO2 and TRX: bounded host operation proof (Phase A)

This **NOT-QUALIFIED host proof** concerns wkeyDAO2 at
`0xe0a281deff5c9d8d67af09d39340e134ac81b82e` and TRX at
`0xce7de646e7208a4ef112cb6ed5038fa6cc6b12e3`. It adds no ingestion
candidate, production rule, dependency, protobuf or VM instruction. These are
separate contracts from the WKEYDAO/GOT proof and from issue #61's swkeyDAO2.

## Complete source and compiler bindings

The [fixtures](../tests/fixtures/wkey2-trx-operation-proof/README.md) preserve
both original captures byte for byte, including source, ABI, metadata, settings,
compiler output, storage layout, source maps, recorded deployment and full code.
wkeyDAO2's capture SHA-256 is
`1afd2a842f97acb1eb03dca09e802d7dcec1e06eef97533dee2c2b2ebf180df0`;
TRX's is `e0260fa176be3c3e945de5d82af3c07ab9fec026705dc868b42881d474560ffc`.
All original `match`, `runtimeMatch` and `creationMatch` labels remain `match`.

wkeyDAO2 includes five source files. Four complete dependencies match
[OpenZeppelin 3.4.2](https://github.com/OpenZeppelin/openzeppelin-contracts/tree/8e0296096449d9b1cd7c5631e917330635244c37)
byte for byte; the custom AGPL source's independent primary revision remains
unestablished. TRX contains one original flattened CRLF file. Its complete
AccessControl and EnumerableSet declarations match that upstream pin after
CRLF-to-LF normalization, used only for those comparisons. Compilation and
source maps retain every original CRLF byte. Neither those two matches nor the
upstream MIT license establishes the full flattened token's origin or license.
All source notices and the pinned upstream license are retained explicitly.

Official solc binaries are bound to the immutable
[release manifest](https://github.com/ethereum/solc-bin/blob/16a99b8c26ed33a91796e209ff6797ee7baf2b0d/macosx-amd64/list.json):

| Target | Compiler | Binary SHA-256 | Full fresh output SHA-256 |
| --- | --- | --- | --- |
| wkeyDAO2 | 0.7.5+commit.eb77ed08 | `1c100ce86a3167fd4c194290aafec0d3d94fe86c7a1aa0837c1346cc93d8b6ce` | `58f88b6f0a6765147a48d19553a6b69172782eb3355845b741a2f4340df51f3c` |
| TRX | 0.6.6+commit.6c089d02 | `45c7f956197ce08b69f793ea610cf1ee65e12b6a518d6160cc28c8eeff41517c` | `6d9938caf954382e95f098db0d4d85c6d0733b455079d1d264271e367e2e7bd8` |

The original inputs change only by outputSelection. Both retain enabled
optimizer runs 200. wkeyDAO2 retains explicit Istanbul, viaIR false, IPFS
metadata/useLiteralContent false and all three remappings. TRX retains its
omitted EVM and metadata options (the defaults are Istanbul/IPFS) and explicit
empty libraries. No setting is silently added or removed. Fresh output reproduces
full saved compiler code, source IDs/maps, layout, ABI and raw metadata strings.

The solc 0.6.6 output has no generated-source text. Four TRX effect instructions
carry compiler-generated file ID `-1`: creation SLOAD/SSTORE PCs 1,372/1,379 and
runtime PCs 6,169/6,176, with exact spans `27:10:-1` and `45:23:-1`.
The executor permits only those target/code-kind/opcode/span combinations and
labels them separately, without inventing a Solidity path or text. Their complete
compiler-output hash and actual traces remain bound. Other effects require an
exact Solidity source span; every other unmapped effect is refused. The first
operation attempt preserved the constructor trace and failed at this annotation
boundary before any exception was added.

## Exact captured-code transformations

Each runtime and creation has one exact 53-byte CBOR replacement; links and
immutables are empty. Complete byte equality is required after that replacement
and the independently encoded constructor append. The reason for the different
captured IPFS digests remains unestablished; no arbitrary metadata stripping is
allowed.

| Target | Runtime bytes / CBOR offset | Creation bytes / CBOR offset | Bytes after creation CBOR | Append |
| --- | --- | --- | --- | --- |
| wkeyDAO2 | 8,495 / 8,442 | 9,849 / 9,764 | 32 | 96 bytes: two addresses and uint256 |
| TRX | 7,088 / 7,035 | 8,893 / 8,761 | 79 | 192 bytes: canonical `TRON`, `TRX` strings |

wkeyDAO2's IPFS digest changes from
`49612744558ae66a570abe76a38301db5fa1768b4b77dd4cab6d9d484e199307`
to `e45ef6c0177a324ac6c9f3625bfff38021a357370a1c6d68be0e97e47d34119e`.
TRX changes from
`b34311cc03e8db20ef7f48f64aa38b801390ef87549f794d9f731c48b424e297`
to `998943ee624b50801f7940b0c241ecc938347d36617745f8aa0427a614055cb6`.
The remaining bytes, including the compiler version and trailing creation
constants, must stay identical. Captured runtime Keccaks are respectively
`0x9054d0efc5311cd08c2d5c1204bc9f600f43e301ea5fe6446b4d581c44d6e6a3`
and `0x84f4834aef7376b01cc68f967cd357e4bfbe13ba2b775757ed1f8e28b67be82c`.

The executor runs the same local matrix separately against compiler and captured
programs. It requires exact calldata/context/trace/state/outcome equality after
removing only the two fields identifying executed code. Every executed PC,
PUSH immediate and CODECOPY range must avoid the changed CBOR span. This is
bounded path evidence, not a universal equivalence claim or deployment proof.

## Local scope and remaining limits

Both targets have a combined array/index/admin role layout: wkeyDAO2 root 8,
TRX root 6. Legacy removal unconditionally performs the tail self-swap, including
physical equality and zero stores. Malformed-state outcomes must be measured
under each compiler, including explicitly named source INVALID controls; modern
panic codes or newer OZ operation templates cannot be substituted.

wkeyDAO2 has decimals 9, no maximum-supply cap, pair/fee/buy receiver cells
9/10/11 and ratios 12/13. TRX has decimals 6, the permission string at 7,
initialization flag at 8, domain at 9 and nonce mapping at 10. TRX's synthetic
self-address dispatch reads the last 20 calldata bytes as the sender; ordinary
callers cannot replace their identity with that suffix. These local controls do
not execute its external meta-transaction path or prove a valid signature.

Both constructors stop at unsupported CHAINID in the unchanged VM. Their
complete attempted prefixes and rollback are evidence; neither completes local
initialization. TRX grants its fixed predicate role before that boundary and has
no default-admin constructor grant. Arbitrary role-admin prestates in local tests
are therefore explicit synthetic controls, not invented deployed permissions.
External fee callbacks, signature/precompile calls, external self-dispatch,
gas/fork accounting and live state are outside this proof. Unsupported VM paths
remain HarnessFailure and never become Solidity success or refusal.

The final `operations-03` run passed **2,460 calls**, comprising 1,230
compiler/captured pairs with exact full execution equality:

| Target, per variant | Calls | Return | Source revert | Explicit source INVALID | Explicit harness boundary |
| --- | --- | --- | --- | --- | --- |
| wkeyDAO2 | 638 | 505 | 126 | 3 | 4 |
| TRX | 592 | 468 | 118 | 3 | 3 |

The run records 420,956 opcode steps and 10,300 effects: 10,216 exact Solidity
annotations and 84 occurrences of the four pinned compiler-generated
instructions. All four constructors stop at CHAINID with exact rollback.
wkeyDAO2 attempts four stores and no logs. TRX attempts eight stores and one
RoleGranted log; its zero-to-zero initialization-flag write remains visible.
Successful role adds have three stores, including physical zero equality;
removals have five stores, including unconditional tail self-swaps. Duplicate
and absent operations have no stores/logs. Three named malformed index/length
cases per target/variant stop at source INVALID; maximum-length push instead
wraps under legacy unchecked arithmetic. These controls do not admit malformed
sets into production.

Token controls retain source-specific fee arithmetic, aliasing, receiver logs
before stores, buy-disabled attempted prefixes and rollback, public `burnFrom`
allowance effects, and TRX's direct/self-suffix permissions. Unsupported external
paths remain named harness boundaries, including the fee receiver EXTCODESIZE,
default TIMESTAMP, CHAINID and signature-call GAS instructions. No callback,
signature recovery or external self-call is emulated.

All eight binding and nine operation tests passed. Full pinned/locked offline
validation passed **1,198 tests across 106 suites (71 nonempty)** with zero
failures or ignored tests, workspace Clippy for all targets, formatting and the
WASM workspace check. These gates are separate from chain qualification.

## Frozen evidence and preserved attempts

Final [compiler evidence](evidence/wkeydao2-trx-operation-proof-20260929-compiler.json)
comes from `out/wkeydao2-trx-operation-proof-20260929/source-03`; final
[operation evidence](evidence/wkeydao2-trx-operation-proof-20260929.json) comes
from `operations-03` in that same output root. Both bind the identical current
[source inventory](evidence/wkeydao2-trx-operation-proof-20260929-source-inputs.json),
SHA-256 `829cc8b9d643071169bf2c4e3c5c71a22c314334b4eb46f4b25aa14d7a9bc977`.
The operation report SHA-256 is
`e31c76e8a17f75531a2af286a6c04418b663ecd75cd4bd75a628d743169764ef`;
the compiler report SHA-256 is
`842b71fd927ccbd8332f437d14003e0a27e9a540553e6038292792f40e83af56`.

The [case inventory](evidence/wkeydao2-trx-operation-proof-20260929-cases.json)
binds every raw attempt and full annotated PC/stack trace. The
[compact transcripts](evidence/wkeydao2-trx-operation-proof-20260929-transcripts.json)
retain operation, malformed-state, rollback and excluded-path effects. Full
traces stay in the local attempt directories. Final case inventory/transcripts
are byte-identical to the successful preliminary operations-02 outputs.
All 4,048 reads, 940 stores, 324 logs and 4,988 Keccak records are accounted for;
the 84 compiler-generated annotations remain distinct from Solidity spans.

Source-01 retains the initial successful compiler attempt with provisional
matrix/executor sources; source-02 is the preliminary complete regeneration.
Operations-01 retains all 1,277 raw attempts through its annotation failure at
the TRX constructor. Operations-02 explicitly reports different compiler and
executor source inventories after the strict generated-span handling was added.
The initial Clippy `find(...).is_none()` warning is preserved; the equivalent
`!contains(...)` expression passes final checks. Integration patches, integrity
checks and both recovery stashes remain available. No failed attempt or original
capture was overwritten.

No saved-block replay belongs to this host-only phase. Issue #4 remains open.

## Reproduction

Use fresh output directories and the pinned Rust toolchain/lockfile:

```sh
export CARGO_TARGET_DIR=out/wkeydao2-trx-operation-proof-20260929/target-isolated
cargo run --locked --offline -p erc20-balances-tools --bin build_wkey2_trx_proof -- \
  erc20/balances/tests/fixtures/wkey2-trx-operation-proof \
  out/wkeydao2-trx-operation-proof-20260929/source-NEW
cargo run --locked --offline -p erc20-balances-tools --bin execute_wkey2_trx_proof -- \
  out/wkeydao2-trx-operation-proof-20260929/source-NEW \
  out/wkeydao2-trx-operation-proof-20260929/operations-NEW
cargo test --locked --offline -p erc20-balances-tools \
  --test wkey2_trx_operation_binding --test wkey2_trx_operation_cases
```

Preparation fetches immutable public compiler/source artifacts. Cargo's offline
flag governs dependency resolution; execution itself reads only saved inputs.
Neither tool calls a chain endpoint.
