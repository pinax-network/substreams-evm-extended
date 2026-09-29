# GM coherent role candidates — NOT QUALIFIED

Two BSC beacon-proxy profiles select a separately named
`gm_token_solc_0_8_16_ondo_vendor` template. The historical 431-profile baseline
remains unchanged. This candidate replaces only the broad width-two membership
rule at root 201 with complete coupled boolean/set operations at roots 201/251.

| Binding | Frozen value |
| --- | --- |
| Proxy A | `0x9b8e987e6fec8cf1380c4dca7071e2c7853aeea1` |
| Proxy B | `0xa9ee28c80f960b889dfbd1902055218cba016f75` |
| Shared beacon | `0xc046b05a920e4b412815934dd8e58904dda73315` |
| Implementation | `0x578f397ca4661d1db4d9a65065d6b284a1a850fd` |
| Balance / allowance roots | 51 / 52 |
| Boolean membership / enumerable roots | 201 / 251 |
| Compiler | `0.8.16+commit.07a7930e`, original London / optimizer 200 |

The helper verifies both proxy runtime hashes, the implementation runtime hash,
the beacon hash, the EIP-1967 beacon pointer and beacon implementation slot 1.
It removes only the role rule and then restores it to compare every field of
each candidate against its complete baseline profile. No role-admin permission
is added: the complete frozen sources contain no `_setRoleAdmin` invocation
beyond the inherited internal definition at AccessControlUpgradeable line 257.
The source review is specific to these immutable files, not a generic call graph.

The exact scalar words `keccak(54)`, `keccak(54)+1`, `keccak(401)` and
`keccak(401)+1` remain unchanged finite long-name payload allowances. Their values
are independently recomputed in Rust. They do not permit arbitrary dynamic
string payloads; later payload words remain unknown. Ordinary scalars and the
compliance/pause-manager pointers retain their prior handling.

## Source and operation boundary

The frozen [Phase A proof](gm-operation-proof.md) executed 783 identical local
runtime cases against both the compiler and captured runtime, plus one synthetic
compiler constructor: 1,567 calls. Exact CBOR substitutions remain explicit.
Runtime traces did not consume substituted metadata through executed PCs,
PUSH immediates or CODECOPY. This production change does not rerun source fetches,
modify the VM, or change the Phase A evidence.

The 15 implementation dependencies match
`ondoprotocol/usdy@3912ca0698c2992e4db997d0855e62588c44e2c0`; these are the exact
Ondo-vendored files, not a newly inferred upstream OZ release. Seven unique proxy
dependencies match OpenZeppelin commit
`ecd2ca2cd7cac116f7a37d0e474bbb3d7d5e1c4d`. The five custom BUSL sources have no
recovered exact independent public primary. Captured on-chain creation fields
are null. Synthetic construction and local implementation execution do not prove
proxy dispatch, deployed initialization, compliance or pause-manager behavior.

The selected coherent operation order is:

- Add: boolean `0→1`, length increment, appended element, one-based index.
- Remove: boolean `1→0`, optional non-tail destination and moved index, tail
  clear, length decrement, removed index clear. No legacy DSG self-swap is added.

The older void-super override always invokes its set mutation, so its malformed
prestates can produce set-only grants, boolean-only grants, boolean-only revokes
and set-only revokes. All four are explicitly rejected by ingestion. Dirty
booleans, wrong indices, duplicate/wrong array entries and malformed maximum
length wrapping are also rejected. The checked logical length is a conservative
admission restriction even though the selected compiler wraps at maximum length.

Only source-executed equal stores may be omitted. Inferred equal stages retain
physical continuity and alias protection. Complete root and write preimages,
one account/role/member identity, one actual execution frame, whole-block
continuity, foreign-store/call barriers and moved-tail membership constraints
remain required. Moved-tail boolean coherence grants no write permission and
does not invent a preimage the runtime did not execute. Extended v4/v5 with real
positive root begin is required; v3 fallback remains unavailable for this mode.
PToken, Securities and DSG continue through their existing explicitly selected
semantics, including Securities' separate fixed admin rule.

## Offline validation

Frozen actual-runtime coherent operations and every optional equality subset are
translated through the public projector for both candidate addresses and both
reviewed producer versions. Tests cover one-sided controls, no invented no-op
observations, malformed/missing/reordered effects and preimages, role/account/
frame joins, protected aliases, omitted equality continuity, source-implied
moved-tail coherence, admin and unknown dynamic payload rejection, rollback and
failure suppression, proxy/beacon/implementation changes, restored pointer
writes and creation refusal. Helper tests bind the complete Phase A artifacts
and assert exact two-profile restoration. Replay counters have separate negative
controls for missing/failed/reverted/no-op/dirty witnesses.

Final validation on merged main `d1e6f38c5c48d7f7fbdb1ab27baf3803f7c030c2`
passed formatting, 1,082 tests in 81 suites, all-target Clippy with warnings
denied, and the WASM workspace check. Every build used the worktree-local
`out/gm-coupled-validator-20260928/target-isolated`; no shared-worktree binary
cache was used. Final logs are `fmt-02.log`, `workspace-tests-02.log`,
`clippy-02.log`, `wasm-01.log` and `build-replay-02.log` in that evidence directory.

```sh
export CARGO_TARGET_DIR="$PWD/out/gm-coupled-validator-20260928/target-isolated"
cargo fmt --all -- --check
cargo test --locked --offline --workspace --lib --bins --tests
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
cargo check --locked --offline --workspace --target wasm32-unknown-unknown
cargo build --locked --offline --release -p erc20-balances-tools --bin replay_role_candidates
"$CARGO_TARGET_DIR/release/replay_role_candidates" erc20/balances /fresh/output \
  --gm /path/to/original/erc20/balances
```

The final [replay report](evidence/gm-coupled-role-candidate-20260928.json) and
[source inventory](evidence/gm-coupled-role-candidate-20260928-source-inputs.json)
are exact copies from `out/gm-coupled-validator-20260928/replay-03`. The report
SHA-256 is `aff18f797709e791579c09a7fc48d128d123a780c9996683e4b5726cac01908a`;
the source-inventory SHA-256 is
`e56a306ef50ac1a0dc50acf8b739bf0cc20b151e605f32ff91cbb893811aff3f`.
All source inputs were unchanged during this final rebuilt run.

The complete saved BSC interval **[122288006, 122289030)** contains 1,024
Extended v5 blocks. All **110,139** candidate protobuf rows match the unchanged
current baseline and historical output. Canonical same-block overlap is 110,138
rows; the one native-only row is unchanged from baseline. There are 4,012 further
reference-only retained matches and 66,265 cold observations left unknown.
The ledger initializes 33,591 observed holders, with zero checkpoint or deployment
seeds and no inferred global enumeration.

| Selected proxy | Emitted / same-block matches | Observed holders | Cold reference observations (nonzero) | Retained matches | Role operations |
| --- | ---: | ---: | ---: | ---: | ---: |
| A (`0x9b8e…eea1`) | 239 / 239 | 31 | 137 (89) | 5 | 0 |
| B (`0xa9ee…6f75`) | 103 / 103 | 98 | 12 (8) | 0 | 0 |

Both membership-write and validated-operation counters are zero in this window.
Consequently the replay establishes balance parity for these observed holders,
not actual GM producer role-operation visibility. The synthetic v4/v5 operation
tests and the historical all-v5 replay are separate evidence.

Earlier attempts are retained: `build-preparer-01.log` records a Rust name-shadow
compile failure; `projector-01.log` records a test fixture placing its positive
balance inside a reverted frame, subsequently corrected. `replay-01` passed on
the pre-protocol base. `replay-02` was intentionally aborted because it started
before the post-protocol rebuild completed; its `ABORTED.md` excludes it from
qualification. Only `replay-03` supplies the final source-bound measurements.

No network/source/RPC/stream/sink request, dependency or ABI change, protobuf
change, persistence change, deployment seed or baseline replacement was made by
this candidate. Runtime/package/getter/holder and live qualification remain open.
