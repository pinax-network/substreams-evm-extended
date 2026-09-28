# PTokenV2: bounded host operation proof (Phase A)

This is a **NOT-QUALIFIED source execution proof**, not a layout candidate or an
extension of the production enumerable validator. It executes the selected
PTokenV2 creation/runtime bytes for BSC address
`0xc63961bf9a7d6bb5844524851d04fbd76dbe915b` against explicitly constructed synthetic
accounts. No chain calls, saved-block replay, package promotion, protobuf changes,
or production ingestion changes are part of this proof.

## Exact source and compiler binding

The complete original capture has SHA-256
`14d5dcc50d8dd1b04fc6283e0d1fafad66fd7fe0bfafdff224a9ba6eda203f9c`.
Its original `match`, `runtimeMatch`, and `creationMatch` labels remain `match`.
The 21 complete sources bind to input, metadata, source IDs and saved compiler
output. `src/v2/PToken.sol:PTokenV2` has source SHA-256
`cb4a65707a1304bc98e870bb7b7fa72f757e4a249948415ef63a92f1eeec1f1e`.
The twenty dependency files independently match
[OpenZeppelin v5.4.0 commit c64a1ed](https://github.com/OpenZeppelin/openzeppelin-contracts/tree/c64a1edb67b6e3f4a15cca8909c9482ad33a02b0).
**An exact independent public token repository revision remains unresolved.**
Dependency pins do not establish the token's primary repository.

Fresh standard-JSON compilation uses official solc `0.8.28+commit.7893614a`,
whose macOS binary SHA-256 is
`81515b0e53deaa266d549545ccaac0a5a96e6d4e8201c77f673b2c710976d9ea`,
bound to the immutable
[official release manifest](https://github.com/ethereum/solc-bin/blob/16a99b8c26ed33a91796e209ff6797ee7baf2b0d/macosx-amd64/list.json).
All original sources, six remappings and settings are preserved: Cancun,
optimizer enabled with 10,000 runs, no via-IR, no libraries, and metadata
`appendCBOR=false`, `bytecodeHash=none`, `useLiteralContent=false`. The only input
addition is `outputSelection` for ABI, metadata, documentation, layout and EVM
artifacts. Removing it restores the captured input exactly. Both raw metadata
strings are retained; their parsed JSON objects are identical, although
serialization ordering differs.

The complete **6,065-byte runtime** equals the saved runtime without substitutions,
links, immutables, metadata stripping or CBOR rewriting. Its Keccak-256 is
`0x60f53552d1ab18923098e47b0d7952ae7a4a48d8bece60b20d3d41734832c15c`.
The complete **7,220-byte creation code** is also identical. Independently encoded
256-byte arguments (`ALPHEA Point Token`, `APT`, and the captured admin/pauser)
reproduce all **7,476 captured creation bytes**. Executing this creation code with
synthetic empty storage returns the exact runtime and expected initial roles,
name/symbol storage and zero supply. This is a constructor control, not evidence
of deployment admission or current chain state.

The complete fresh compiler output is additionally pinned by SHA-256
`c85b99ca52a3ef59a6aeeefe10afd2c7c95ab852754293ab16b1489e199da891`.
It binds generated Yul, source maps and all source IDs. Source locations use
instruction-indexed maps across all original and generated files; unmapped and
source-ID `-1` spans remain explicitly identified.

## Executed operation controls

The matrix contains **382 constructor/ABI calls** against synthetic accounts,
including getter checks after mutations. It covers empty/nonempty grants;
duplicate grants; first, middle, tail and sole removals; zero-address members in
every position; absent revokes; renounce success/no-op/bad confirmation;
unauthorized calls; arbitrary/default/known roles; sequential add/remove/add;
full-array getters; array bounds; malformed address/calldata and unknown selector
refusals. It also executes mint, distributor claim, burn, approval, pause/unpause,
and their relevant refusals. Exact balances, total supply, all remaining storage
cells, log topics/data and getter results are checked independently. Role cases
seed both holder balance and total supply at 123.

For coherent role sets, selected compiler order is:

| Operation | Ordered storage and log effects |
| --- | --- |
| Add | Root-5 boolean `0→1`; `RoleGranted`; root-6 length increment; append array element; record one-based position |
| Remove non-tail | Root-5 boolean `1→0`; `RoleRevoked`; copy last element into removed position; update moved member's position; clear old tail; decrement length; delete removed position |
| Remove tail/sole | Root-5 boolean `1→0`; `RoleRevoked`; clear tail; decrement length; delete position; **no self-swap stores** |
| Duplicate grant / absent revoke or renounce | No storage writes or logs |

Zero-address append and tail clear execute equal `0→0` array stores. They remain
in the ordered SSTORE transcript. This OZ 5.4 behavior is distinct from DSG's
existing OZ 3.4.2 executed self-swap semantics; the DSG rule is unchanged.
Actual Keccak inputs prove the root-5 membership mapping, root-6 set mapping,
position mapping and array base used during each changed operation.

Two controls are explicitly **outside the coherent-state assumption**. A
maximum-length array push wraps, producing a non-coherent set result; the proof
does not reinterpret this as an admitted operation. A nonzero boolean/position
with an empty array causes `Panic(0x11)` after the boolean clear and role log;
neither storage nor logs commit. A valid-account `transferFrom` similarly
executes an allowance decrement before `RestrictedTransfer` and restores it.
These controls retain the complete attempted prefixes and exact revert payloads.

## Executor and artifact limits

The Rust executor is host-only. It implements only bounded bytecode operations
needed by this proof and direct controls. Storage uses full 256-bit sparse keys;
caller/address, calldata, ordered reads/stores, log topics/data, actual Keccak
preimages, and PC/stack traces are explicit. It has no external calls,
transaction gas/refund accounting, fork-cost qualification or chain environment.
Unlisted storage is **synthetic account zero**, not an inference about a cold
on-chain holder.

Limits are 100,000 instructions, 1 MiB memory, 1,024 stack words and 8 MiB aggregate
copied hash/log/return witness payloads per call. Trace and record counts are
also bounded by the instruction limit. Zero-length memory operations ignore
their offsets; MCOPY uses overlapping-copy semantics. RETURN, REVERT, INVALID
and harness failure remain distinct. Only RETURN commits state/logs; failures
retain attempted transcripts. Unsupported opcodes, invalid jumps, malformed
stack accesses and resource exhaustion cannot count as Solidity agreement.

Every attempt uses a fresh directory. The tools preserve as-run Rust sources,
local host dependency source hashes, raw compiler input/output, official compiler
manifest and license, all case traces, effect source annotations and file hashes.
Raw case records are saved before annotations or expectations can fail. Failed
attempts remain available under `out/ptoken-coupled-role-20260928/`, including the
initial wrong assumption that pop decremented length before clearing the tail
and the disproved maximum-length revert assumption.

Committed fixtures and compact effect transcripts allow offline Rust regression
checks. Full PC/stack traces remain in the recorded local artifacts. The tests
also mutate the capture/compiler/source-map/generated-source bindings and a
membership SSTORE to show that malformed evidence or broken execution cannot
pass.

## Remaining gate before any ingestion support

The initial equivalence of boolean membership, one-based position and array
contents, plus zero unused tail cells, is an explicit qualification assumption.
The operation proof does not establish it for a real account. `_clear` is not
reachable through this selected token and is not supported or claimed.

A future separately reviewed rule must link root-5 membership with the complete
root-6 operation for the same role/member and call frame. An independent typed
membership allowance would incorrectly admit a boolean-only transition. Any
new rule needs its own malformed/incomplete/interleaved/preimage/persistence
controls, unchanged DSG regressions, candidate configuration and canonical-bound
saved replay. Fresh package/runtime/holder and producer visibility checks remain
separate. **Phase A neither changes the 431-profile baseline nor narrows this
profile, and does not close issue #4.**

## Recorded artifacts and reproduction

The final [compiler report](evidence/ptoken-operation-proof-20260928-compiler.json)
is from `out/ptoken-coupled-role-20260928/source-04/`; the
[operation report](evidence/ptoken-operation-proof-20260928.json) is from
`out/ptoken-coupled-role-20260928/operations-07/`. All 382 calls passed their
expectations: 338 returned and 44 reverted with their expected payloads;
zero INVALID or harness failures were accepted. The constructor and full-array
controls exercised Cancun PUSH0/MCOPY. These are synthetic-call counts, not
observed chain-holder coverage.

The [case inventory](evidence/ptoken-operation-proof-20260928-cases.json) hashes
all 382 full annotated traces. The [compact operation transcripts](evidence/ptoken-operation-proof-20260928-transcripts.json)
retain mutation/refusal effects and their source spans. Getter full traces remain
in the local attempt directory. The [source inventory](evidence/ptoken-operation-proof-20260928-source-inputs.json)
covers the as-run proof tools and their local host dependencies; compiler and
operation attempts use the same inventory. Each attempt's `as-run/` contains
exact compiled-in proof source copies, checked against the checkout before work.

From the repository root, with the hash-verified official compiler saved locally,
use a new output directory on every invocation:

```sh
cargo +1.88 run --locked --offline -p erc20-balances-tools --bin build_ptoken_proof -- \
  ORIGINAL_COMPLETE_CAPTURE OFFICIAL_MACOS_SOLC FRESH_SOURCE_DIRECTORY
cargo +1.88 run --locked --offline -p erc20-balances-tools --bin execute_ptoken_proof -- \
  FRESH_SOURCE_DIRECTORY FRESH_OPERATION_DIRECTORY
cargo +1.88 test --locked --offline -p erc20-balances-tools ptoken --lib --tests
```

The builder reads static immutable official compiler/source URLs to verify the
saved source pins; it makes no chain requests. Fixture tests and the operation
executor require no network. Cargo dependencies remain pinned and unchanged.
