# ERC20TokenX: bounded host operation proof (Phase A)

This **NOT-QUALIFIED host proof** binds the selected ERC20TokenX runtime shared
by ORI `0xda033999bb6165e64db01bd9be14b40f5653092e`, FNA
`0x08332a515cb2a57884176e887b682e7da2eb114e`, and PHI
`0xa71add46ea4fbf0058b36e6baa39530f8e48b103`. It adds no production admission,
layout, candidate, dependency or protobuf change. Operations execute unchanged
runtime bytes in synthetic local state. No RPC, stream or sink is called.

## Source and compiler binding

Both complete original ORI/FNA source responses are preserved in the
[fixture directory](../tests/fixtures/erc20tokenx-operation-proof/README.md).
They contain the same five source bodies and original compiler input, but retain
their distinct deployment records and exact constructor arguments. Their four
OpenZeppelin dependency files match
[OpenZeppelin 3.4.2](https://github.com/OpenZeppelin/openzeppelin-contracts/tree/8e0296096449d9b1cd7c5631e917330635244c37)
byte for byte: `AccessControl.sol`, `EnumerableSet.sol`, `Address.sol`, and
`Context.sol`. The exact pinned MIT license and original SPDX notices remain.
No independent public revision for custom `contracts/core/ERC20.sol` is
established in the reviewed material. This is a scoped provenance gap, not a
claim that no public source exists. Compilation uses the complete captured body
without normalization, rewriting or substitution, retaining its AGPL-3.0-or-later
SPDX notice alongside the four MIT dependency notices.

Official `0.7.5+commit.eb77ed08` is checked against the immutable
[compiler manifest](https://github.com/ethereum/solc-bin/blob/16a99b8c26ed33a91796e209ff6797ee7baf2b0d/macosx-amd64/list.json)
with SHA-256 `1c100ce86a3167fd4c194290aafec0d3d94fe86c7a1aa0837c1346cc93d8b6ce`
and Keccak `ff52724ea7d3e0913219c765b40f311f72fe1e5c95389020a165a1959e57e24f`.
Original settings retain optimizer enabled/200 and empty libraries. No absent
setting is inserted except outputSelection; compiler metadata identifies default
Istanbul. Complete compiler output, raw metadata, ABI, layout, creation object,
runtime and source maps must match the saved artifacts.

The full **7,896-byte runtime** has Keccak
`0xbaaa46c4d0133a30bd1020c46f86c0ea4ac0609d18317a95fc923efb5b22afdf`.
There are **zero substitutions**: no metadata stripping, immutable patch or
executable-byte equivalence shortcut. The 9,347-byte creation object plus each
capture's exact argument append reconstructs its complete saved creation bytes.
Those saved deployment records are capture evidence, not newly verified chain
history.

PHI's saved source response has null match labels and no sources, creation or
deployment record. A separately preserved historical runtime capture/report at
block 122288067 matches all runtime bytes. That establishes the explicit
runtime-equivalent attribution used here; it does not manufacture an individual
PHI source record, creation binding, or historical initialization proof. The old
runtime/getter report remains historical evidence, not a fresh live check.

## Local state and execution boundary

Compiler layout binds balances root 0, allowances root 1, supply 2, name 3, symbol 4,
decimals 5, nonces 6, domain separator 7, combined roles root 8, main pair 9, fee
receiver 10, sell ratio 11 and buy ratio 12. A role at `R = keccak(role || 8)` stores
length at `R`, array words at `keccak(R)+i`, one-based indexes under `R+1`, and
admin at `R+2`. Membership is index nonzero; there is no separate boolean map.
The complete source has no `_setRoleAdmin` callsite. No writable admin permission
is proposed by this proof.

Source similarity to DSG is only motivation for this measurement. The different
runtime must independently prove store order, self-swap behavior, equality
writes, logs, errors and rollback. A later candidate requires a separately reviewed implementation,
adversarial projector tests and saved replay.

The measured matrix contains **520 local calls**: 431 returns, 82 exact reverts,
three narrowly identified source `INVALID` controls and four explicit harness
limitations. It records 81,550 opcode steps and 2,043 mapped storage/log/hash
effects. Every attempted store and log is compared against an independent
ordered expectation; complete committed storage, getter readback, physical
read/write continuity, preimages, and rollback are also checked. These counts
describe synthetic execution, not chain intervals or holder coverage.

Append stores length, tail element and one-based index at PCs **6309, 6325,
6344**, followed by `RoleGranted`. Removal stores destination element, moved
index, tail clear, decremented length and removed index at PCs **7072, 7092,
7123, 7125, 7150**, followed by `RoleRevoked`. Tail and sole removal still execute
the self-swap: its first two stores are equal-value writes. Zero-address append
and tail clearing preserve executed zero-to-zero stores. Duplicate grants and
absent removals execute reads but no stores or logs. This measured coherent
sequence agrees with the existing legacy DSG template; it does not enable that
template for these profiles.

Controls cover empty/nonempty, first/middle/tail/sole and moved-zero sets,
renunciation, sequential restoration, arbitrary full-width roles/admins,
authorization errors and loss of the last default admin. Solidity 0.7.5 masks
dirty address ABI words, wraps malformed maximum-length append, and executes
`INVALID` for three internal malformed length/index combinations. Those effects
are recorded as source controls, never coherent admission. The public indexed
getter instead returns the exact `EnumerableSet: index out of bounds` revert.
Raw balance, allowance, supply and nonce getters return the complete selected
word, including zero, the high bit and maximum uint256, with exactly one read.

The unchanged bounded host VM has no CHAINID context. Exact captured construction
therefore stops at that unsupported instruction in `ERC20Permit` before role
setup. Its attempted prefix is recorded and rolled back; this is a
**HarnessFailure**, not a Solidity revert or a claim that real construction is
impossible. Both saved constructor appends are independently decoded and
re-encoded: exact fee receiver, ratios, dynamic offsets, name, symbol and zero
padding. Each attempt stores only name, symbol and decimals (9), then stops at
CHAINID PC 509 without logs; all three attempted writes roll back. No constructor
success or initial-set coherence is inferred.
Successful fee-receiver calls, permit execution, chain context, gas/refunds/fork
costs and real producer observations remain outside this proof. Unsupported
execution is never silently treated as a source revert or successful callback.
The fee control reaches foreign EXTCODESIZE PC 5169 after two stores and two
logs; the permit control stops at TIMESTAMP PC 3331 before any store/log. Exact
attempted prefixes and complete rollback are asserted, and no external call is
executed or mocked. The fee-control prestate keeps supply equal to its balances.

## Reproduction and qualification boundary

Run the host builder with the complete fixture directory, the exact official
macOS compiler, and a fresh output directory. It performs only immutable public
source/compiler-manifest reads. The operation executor accepts a frozen complete
compiler artifact and performs no network requests. Both snapshot their compiled
Rust sources and reject a stale executable before generating proof evidence.
All attempts use fresh directories; raw execution evidence is saved before
expectations are checked.

```sh
cargo run --locked --offline -p erc20-balances-tools --bin build_erc20tokenx_proof -- \
  erc20/balances/tests/fixtures/erc20tokenx-operation-proof /path/to/solc-0.7.5 out/fresh-source
cargo run --locked --offline -p erc20-balances-tools --bin execute_erc20tokenx_proof -- \
  out/fresh-source out/fresh-operations
```

The first source attempt retained an incorrect later-OpenZeppelin license hash
and refused before compilation. The corrected exact 3.4.2 notice is pinned; the
failed attempt remains under `out/erc20tokenx-operation-proof-20260929/source-01`.

Final evidence comes from fresh `source-03` and `operations-03` directories:

- [Compiler report](evidence/erc20tokenx-operation-proof-20260929-compiler.json)
  binds the complete official compiler output and all captured sources.
- [Operation report](evidence/erc20tokenx-operation-proof-20260929.json) has SHA-256
  `340d3d1b0ed1542a8e2328b05560bddb2d61d4e712411e399c9bcaf5628033a1`.
- [Source inventory](evidence/erc20tokenx-operation-proof-20260929-source-inputs.json)
  binds 203 current inputs, including the central status docs, with SHA-256
  `b83de18f701ecf8641a638f68a0821598b261088870de9bdc37dd5fb1f20b0f0`.
- [Compact transcripts](evidence/erc20tokenx-operation-proof-20260929-transcripts.json)
  preserve source-mapped effects; the [case inventory](evidence/erc20tokenx-operation-proof-20260929-cases.json)
  hashes every complete local trace, including getters.

The two final source inventories are identical. Final transcripts and case
inventory are byte-for-byte equal to the preceding expanded matrix, including
after integration of the optional timestamp VM entrypoint. This proof keeps its
existing entrypoint and supplies no timestamp; the legacy failure text remains
unchanged. Earlier successful attempts and all failed attempts are preserved.

Pinned offline validation passed on the merged main tree: formatting, **1,129
workspace library/binary/integration tests across 92 suites**, all-target Clippy
with warnings denied, and the full WASM workspace check. The 16 new focused tests
cover binding mutations and the operation matrix, including reportable corrupted
store/log/INVALID failures. Runs used the dedicated
`out/erc20tokenx-operation-proof-20260929/target-isolated` directory; the shared VM
and production ingestion files are unchanged by this Phase A patch.

Initial-state coherence, runtime continuity, actual Extended operation framing,
preimage/equal-write visibility, replacement package/getter parity and holder
qualification remain separate. This phase does not change the immutable
431-profile baseline, migrate any legacy role rule or close issue #4.
