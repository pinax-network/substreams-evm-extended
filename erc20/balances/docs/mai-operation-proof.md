# Mai: bounded host operation proof

This host-only Phase A binds the complete saved Mai capture for
`0x35803e77c3163fed8a942536c1c8e0d5bf90f906` on chain 56 and executes its actual
compiled code in synthetic local state. It adds no ingestion template,
validator, candidate, schema, dependency or VM opcode. No RPC, Firehose,
Substreams or sink is called.

## Source and compiler binding

The [fixtures](../tests/fixtures/mai-operation-proof/README.md) retain the whole
original capture, all 15 source bodies, compiler inputs/outputs, raw metadata,
ABI, storage layout, source IDs, ASTs and generated Yul. Fourteen dependencies
match [OpenZeppelin 4.7.0 at its exact commit](https://github.com/OpenZeppelin/openzeppelin-contracts/tree/8c49ad74eae76ee389d038780d407cf90b4ae1de)
byte for byte. The original MIT notices and license are retained. The custom
`contracts/Mai.sol` has an UNLICENSED notice and no independently established
maintainer revision in the reviewed material; exact compilation does not remove
that provenance gap.

| Binding | Exact value |
| --- | --- |
| Whole original capture SHA-256 | `e5fb99260765d58a5fecdd4d2eccd61c84e6f0032821fc75402f00e93535386b` |
| Custom source SHA-256 | `fee53b965e3b7929b2f863e2a833b4b68bfb3783a5593cfab2722d4028a4948b` |
| Official compiler | `0.8.9+commit.e5eed63a`, macOS amd64 |
| Official compiler SHA-256 | `d619d4f5d8fd988bc63262407e749e905ccc8d8ab1ccf0280da1d12b918894ce` |
| Complete fresh compiler output SHA-256 | `18a24ae3680b1b25d1b47c0d0c9a9b9d53dbd5a90c6bd6c003e6a0f5a5a20764` |
| Creation object | 12,772 bytes; no arguments or transformations |
| Captured runtime | 11,296 bytes |
| Runtime Keccak | `1134938644214395aba790bf7d50e13027c30b74b4bd09dcbe167c0ccc2cc9e5` |

The binary is tied to the version, SHA-256, Keccak and release association in
[the immutable official compiler manifest](https://github.com/ethereum/solc-bin/blob/16a99b8c26ed33a91796e209ff6797ee7baf2b0d/macosx-amd64/list.json).
Original settings disable optimization, retain runs 200 and empty libraries,
and omit the EVM version. Only outputSelection is extended. Complete fresh
runtime/creation objects, source maps and raw metadata match the captured
compiler artifact before the declared immutable replacement.

The sole runtime replacement is the 32-byte cap at offset 1767, from immutable
AST declaration 1258 (`ERC20Capped._cap`). The custom constructor literal
`1000_0000_0000e18` independently gives `10^29`; fresh AST literal 2249, source
span `541:17:14`, its exact source bytes and compiler rational value are checked.
The compiler placeholder must be all zeroes. Patching only this word must produce
the entire captured runtime. There is no metadata stripping, generalized
substitution, mock dependency or external value oracle.

## Measured state and operations

Membership is at root 0 and the separate enumerable set at root 1. Balances are
at root 2, allowances at root 3, total supply at 4, name at 5 and symbol at 6.
The cap is immutable. These declarations are checked against both captured and
fresh compiler layouts. Creation sets Matrix DAO / MAI and grants
DEFAULT_ADMIN_ROLE and MINTER_ROLE to the original captured deployer. It has no
admin setter callsite.

The exact constructor executes locally without a VM extension and returns the
fully patched runtime. Its ten ordered stores and two RoleGranted logs match an
independent expected state. No supply is minted. This synthetic construction
proves neither historical deployed state nor qualified initial set coherence.

The role matrix checks exact ordered boolean/array/index stores, full logs and
interleaving, source PCs, observed preimages, reads, whole committed state and
rollback. It covers empty/nonempty, zero members, duplicates, absent removals,
conditional middle versus tail removal, moved zero, sole members, sequential
state, full-width role/admin identities, last-admin loss, unauthorized calls,
wrong renunciation and raw getters. Source-implied physical zero-to-zero stores
remain visible.

This older AccessControlEnumerable calls the `void` superclass grant/revoke and
then unconditionally calls its set helper. Four separately measured incoherent
states therefore differ from newer implementations: a grant can update only the
boolean or only the set; a revoke can likewise update only either side. Such
source behavior is not permission to admit incomplete producer operations.
Qualified initial boolean/set equivalence remains a separate prerequisite.

Malformed controls keep their distinct outcomes: packed boolean writes preserve
unrelated upper bytes; dirty array entries can move a full bytes32 key while an
address getter masks it; a maximum-length push wraps length and one-based index.
Empty length with a nonzero position and a position beyond length revert with
exact panic data after a boolean/log prefix and restore all state. These
inconsistent synthetic states are not coherent-set admission examples. No
existing PToken, Securities, BTR or legacy DSG template is relabeled for Mai.

Token controls independently follow Mai and its exact dependencies. Mint checks
MINTER_ROLE, checked supply addition and the cap before the zero-recipient guard;
it emits the ordinary zero-address Transfer sender. Burn reduces supply without
reducing the immutable cap. Finite allowance spending writes and logs Approval
before transfer/burn; infinite allowance omits those effects. Self transfers,
zero amounts, zero addresses, cap boundaries, recipient overflow, supply
underflow and allowance arithmetic preserve exact failure order and rollback,
including attempted earlier writes/logs. ABI tests cover truncated and dirty
arguments across role, token and getter entrypoints, unknown/empty selectors,
bytes4 padding and accepted trailing calldata.

## Evidence and remaining gates

Final `source-02` compilation and `operations-03` passed **1,632 calls**:
1,427 returns and 205 exact expected reverts, with no INVALID or unsupported
outcomes. Both runs use source inventory SHA-256
`23ff1571cf7808ac328c6ce494ab64dbc35351fda6f84921ff7435004c4234e5`.
The complete evidence contains 1,632 raw and 1,632 source-annotated executions,
524 compact transcripts, 445,655 opcode records and 4,334 mapped effects.

Committed copies are the [operation report](evidence/mai-operation-proof-20260929.json),
[compiler report](evidence/mai-operation-proof-20260929-compiler.json),
[source inventory](evidence/mai-operation-proof-20260929-source-inputs.json),
[case inventory](evidence/mai-operation-proof-20260929-cases.json) and
[compact transcripts](evidence/mai-operation-proof-20260929-transcripts.json).
The operation report SHA-256 is
`dc63c4701919dc758e1484d5d24d9b6912d870c0836077613f1b1d4c3714a926`;
the compiler report is
`cab29d18fdf543edcc779dcc8ce773b945ffa044a97da1bc541cc04e3d8104a6`.

All 1,216 workspace tests passed in 110 suites (73 nonempty, zero ignored),
including eight new binding tests, nine operation tests and one checker-
robustness unit test. Formatting, all-target Clippy with warnings denied and the
WASM workspace check passed. The checks ran on the reviewed PR #99 source tree;
the subsequent fast-forward to its tree-identical merge preserved all inputs.

Preliminary `source-01`, `operations-01` and `operations-02` remain under
`out/mai-operation-proof-20260929/`. A filtered early test command is preserved
separately from the complete focused run. The initial Clippy attempt found one
needless borrow in artifact-path handling; that equivalent style correction and
the successful rerun remain recorded. Final logs and packaging records are
retained under the same output root.

Both tools require fresh output directories and preserve as-run source snapshots.
The executor saves every raw attempt before assertions and source annotation.
Malformed-input and corrupted store/log/exit tests exercise useful failure
reports and output-reuse refusal. Runtime and original creation use separate
source-map contexts, including their separately bound generated Yul.

```sh
CARGO_TARGET_DIR=out/mai-operation-proof-20260929/target-isolated \
  cargo run --locked --offline -p erc20-balances-tools --bin build_mai_proof -- \
  erc20/balances/tests/fixtures/mai-operation-proof /path/to/official-solc-0.8.9 NEW_SOURCE
CARGO_TARGET_DIR=out/mai-operation-proof-20260929/target-isolated \
  cargo run --locked --offline -p erc20-balances-tools --bin execute_mai_proof -- \
  NEW_SOURCE NEW_OPERATIONS
```

The builder fetches only immutable public dependency/license/compiler-manifest
files. The executor makes no network requests. Any later candidate requires
separate production review of an explicitly selected coupled rule at roots 0/1,
removal of both broad membership/set permissions, adversarial projector tests
and saved canonical replay. All other profile metadata must remain intact.
Custom-source provenance, actual runtime/dependency continuity, initial
coherence, real producer framing/preimages/equality visibility and separately
authorized package/getter/holder checks remain unresolved. This is the eighth
historical enumerable profile, not an addition to the 33 boolean-profile cohort.
