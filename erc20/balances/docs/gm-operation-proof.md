# GMToken: bounded host operation proof (Phase A)

This **NOT-QUALIFIED host proof** concerns local operations of captured GMToken
implementation `0x578f397ca4661d1db4d9a65065d6b284a1a850fd` and the source bindings
of token proxies `0x9b8e987e6fec8cf1380c4dca7071e2c7853aeea1` and
`0xa9ee28c80f960b889dfbd1902055218cba016f75`. It adds no ingestion rule, candidate,
production dependency, protobuf or live call. It executes implementation bytes
against synthetic account/storage contexts; it does not dispatch the proxies.

## Source and compiler binding

Three complete original captures are retained in the
[fixture directory](../tests/fixtures/gm-operation-proof/README.md). Their 34
source records contain 27 unique source/path profiles. The implementation's 15
MIT dependencies match the exact
[Ondo vendor snapshot](https://github.com/ondoprotocol/usdy/tree/3912ca0698c2992e4db997d0855e62588c44e2c0/contracts/external/openzeppelin/contracts-upgradeable).
This is not asserted to be an exact upstream OZ release. Each proxy's seven MIT
sources match [OpenZeppelin v4.7.3](https://github.com/OpenZeppelin/openzeppelin-contracts/tree/ecd2ca2cd7cac116f7a37d0e474bbb3d7d5e1c4d).

**Five custom BUSL-1.1 source primary revisions remain unresolved.** Their exact
captured bodies are preserved; the same paths in the contextual
[public GM repository](https://github.com/ondoprotocol/rwa-contracts/tree/046c5c58a2d2202e70d7c2c4f13f414f15de47f4/contracts/globalMarkets)
are nonmatches. Neither whitespace normalization nor those replacement bodies
are used for compilation. All source SPDX headers and compiler metadata licenses
are retained separately from the proxy upstream MIT license.

Official compiler `0.8.16+commit.07a7930e` is checked against the immutable
[release manifest](https://github.com/ethereum/solc-bin/blob/16a99b8c26ed33a91796e209ff6797ee7baf2b0d/macosx-amd64/list.json)
using SHA-256 `7d471cb9bae9a7f29c7ebf402f7e16fa8226b17ba9ab68a88ce107114479dc4d`
and Keccak `f3dd3636e7bca007430e091bcacc2dd9b9af72edb838a0e0d77de9610169f7c3`.
The original London, optimizer enabled/200 inputs stay unchanged except for
outputSelection. Implementation settings retain viaIR false, IPFS metadata,
useLiteralContent false and 19 remappings; proxy settings retain IPFS metadata,
empty libraries and 18 remappings, without adding absent settings. Full raw
compiler output, source maps and generated sources are bound.

The saved compiler and captured runtime bytes differ in one declared metadata
suffix. At offset 8,218 of the 8,271-byte implementation, the 53-byte CBOR suffix
replaces IPFS digest `df561304cbde14e11b56159b096eda9b0185e26670a0716b47b4a2f4fb25822d`
with `3e5e4ba8bedc87e31107179160bb56f66f6455be28687114808cd197813a0399`.
Each 824-byte proxy has its corresponding single 53-byte replacement at offset
771. Exact original/replacement bytes and transformation objects must reconstruct
the full captured runtime. No arbitrary metadata stripping or immutable patch
is allowed. Captured implementation Keccak is
`0x85d44c72e84a34f0b4a3bceca35289b10bd0c5510c5a441466a755e903edc36a`;
proxy Keccak is `0x439923c85f956f038ae77871736f789e6d08d22257b6e5fdccfdc83924ecb4d0`.

All original match/runtimeMatch labels remain `match`. **CreationMatch,
onchain creation bytes and creation transformations are null for all three
captures.** There is no saved constructor append or deployment execution binding. The proxy
compiler creation preserves its CBOR at byte 2,186 followed by the exact
`Address: low-level delegate call failed` literal through byte 2,278; that
creation layout is checked independently from the runtime suffix.
The synthetic implementation constructor runs unchanged compiler creation and
must return the complete compiler runtime. It does not return the differing
captured metadata. Both full runtimes are separately exercised on the same local
matrix, with exact trace/state/outcome equality and executed-PC, PUSH-immediate
and CODECOPY exclusion of the changed suffix. This is a bounded path comparison,
not a universal equivalence theorem or a fabricated creation match.

## Local semantics and exclusions

The layout binds membership/admin root 201 and enumerable root 251, balances 51,
allowances 52, supply 53, compliance pointer 301, pause-manager pointer 351 and
name/symbol overrides 401/402. Raw balances return the full stored word. Role
methods use the older void parent override: boolean/log effects are conditional,
then enumerable add/remove is always attempted. Four deliberately incoherent
boolean/index controls therefore differ from coherent admitted operations.
Exact ordering and equality stores require this compiler's traces, not an OZ 5
or BTR assumption.

Local role/getter controls include zeros, no-ops, removals at each array position,
permissions, malformed ABI and states, exact errors, attempted effects and full
rollback. Initialization writes only local strings/pointers/roles; its pointer
setters reject zero but never query those accounts. Repeated initialization uses
the existing explicit self EXTCODESIZE context. Synthetic creation uses installed
self size zero, direct implementation context 8,271, and implementation execution
at either token proxy address 824. The VM and its bounds are unchanged.

Transfer, mint and burn success is excluded: the hook calls external view pause
logic and external **non-view** compliance logic. The latter cannot be replaced
by a static boolean mock. External account queries, GAS and unsupported external
opcodes remain HarnessFailure, not a Solidity refusal or successful compliance
check. Actual beacon/proxy construction/dispatch, gas/fork behavior, reentrancy,
current chain state and deployment qualification are outside the proof.

The measured matrix has **1,567 calls**: one synthetic compiler constructor and
783 compiler/captured runtime pairs, with 1,279 returns, 274 exact reverts and
14 explicit unsupported-path controls. No INVALID is accepted. There are
438,481 recorded opcode steps; all storage/log/hash effects must map to exact
Solidity or generated-source spans. These are local synthetic calls, not holder
or chain coverage.

The coherent role traces preserve boolean store → role log → selected set
stores. Adds update length, append and one-based index. Non-tail removals move
the tail and its index, then clear the tail, decrement length and clear the
removed index; tail removals omit the self-swap. Physical equality stores remain
visible. Deliberately incoherent boolean/index states demonstrate all four
one-sided outcomes, plus exact attempted prefixes before panic/rollback; none
becomes an ingestion permission. Dirty boolean high bits and arbitrary role
admin prestate are separate malformed/source controls, not asserted deployed
facts.

Successful local initialization has 13 stores and four logs; zero compliance
reverts after four stores/no logs, and zero manager after five stores/one log.
Every old/new value, topic/data and store/log interleaving is asserted, followed
by complete rollback on failure. Repeated deployed-context initialization fails
before effects. The maximum dynamic ABI length gives generated-decoder
`Panic(0x41)` before a calldata-tail check, distinct from empty ABI reverts.
Raw balance, supply and allowance getters each perform exactly one expected
storage read in the tested states, including varying client pointer cells.

Seven unsupported controls run per runtime variant: missing self context fails
at EXTCODESIZE PC 1,889; six authorized transfer/mint/burn paths fail at GAS
PC 5,556, immediately before pause STATICCALL. No external call executes.
TransferFrom and burnFrom retain their attempted allowance store/Approval log
before complete rollback. These 14 harness failures are explicitly excluded
from successful source execution and are not treated as Solidity reverts.

No coherent deployed role set, holder seed, producer witness or ingestion
permission follows from synthetic prestate. Dynamic metadata support and a
future coupled-operation validator are separate work. **Issue #4 remains open.**

## Evidence and reproduction

Final [compiler evidence](evidence/gm-operation-proof-20260928-compiler.json)
comes from `out/gm-operation-proof-20260928/source-03`; final
[operation evidence](evidence/gm-operation-proof-20260928.json) comes from
`out/gm-operation-proof-20260928/operations-03`. Both bind the same
[183-file current source inventory](evidence/gm-operation-proof-20260928-source-inputs.json),
SHA-256 `cb3c63c450dcaab117839717bf712836d7e326966f2e42162e86d9f8c4d05248`,
including the four central status documents. The operation report SHA-256 is
`13ba3d4874ccc21310a2c90df3aa7b5ccfe5c9de215ff4a1969c2bc8c5eb38d8`.

The [case inventory](evidence/gm-operation-proof-20260928-cases.json) binds all
1,567 full annotated traces. The
[compact transcripts](evidence/gm-operation-proof-20260928-transcripts.json)
retain operation, rollback and unsupported-control effects and source spans;
full PC/stack traces remain in the local attempt directory. All 6,449 effect
annotations are mapped: 2,655 reads, 529 stores, 137 logs and 3,128 Keccak records.
The final case inventory and transcripts are byte-identical to the earlier
successful operations-02 run. The final source inventory includes the completed
status docs and equivalent Clippy simplification.

Complete compiler output SHA-256 is
`efd8c1ea4803647451d7d7fdddd96614049bc26c35573d4811aac2745eca12bc` for GM and
`f7e663f2aef9c04ce10a3aa7cc681a5874f3a66100c85328d3e8b7fa45cd984f` for each proxy.
The executor revalidates original/augmented inputs, official manifest/version,
license inventory, primary classifications and exact compiler report records;
it does not merely hash arbitrary auxiliary files. Both report copies in the
fixture directory match final source-03.

Preserved attempts include the initial Rust lifetime compile error, source-01's
incorrect assumption that proxy creation CBOR was at its end, operations-01's
empty-revert expectation where the generated decoder gives Panic(0x41), and the
initial Clippy nonminimal-boolean warning. Complete failed reports, attempted
traces and as-run sources stay under the fresh local directories; later attempts
do not rewrite them. Source-02 is the first successful full official compilation;
source-03 pins the complete outputs and freezes final code. No VM changes were
needed.

Reproduction uses the pinned Rust toolchain/lockfile and the verified official
compiler binary. Choose fresh output directory names for every attempt:

```sh
export CARGO_TARGET_DIR=out/gm-operation-proof-20260928/target-isolated
cargo run --locked --offline -p erc20-balances-tools --bin build_gm_proof -- \
  erc20/balances/tests/fixtures/gm-operation-proof/implementation-capture.json \
  erc20/balances/tests/fixtures/gm-operation-proof/proxy-a-capture.json \
  erc20/balances/tests/fixtures/gm-operation-proof/proxy-b-capture.json \
  out/gm-operation-proof-20260928/solc-0.8.16 out/gm-operation-proof-20260928/source-NEW
cargo run --locked --offline -p erc20-balances-tools --bin execute_gm_proof -- \
  out/gm-operation-proof-20260928/source-NEW out/gm-operation-proof-20260928/operations-NEW
cargo test --locked --offline -p erc20-balances-tools \
  --test gm_operation_binding --test gm_operation_cases
```

The preparation tool makes immutable public source/manifest HTTP reads; Cargo's
`--offline` applies to dependency resolution. Operation execution uses saved
inputs only. Neither tool calls a chain endpoint. Full offline workspace gates
remain separate from deployed runtime/package/producer qualification. No
saved-block replay belongs to this host-only phase.
