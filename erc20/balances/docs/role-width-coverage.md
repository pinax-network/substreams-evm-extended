# MUSD and OLY role-write guard requalification

MUSD (rank 386, `0x22a2c54b15287472f4adbe7587226e3c998cdd96`) and OLY (rank 68, `0x544028231562a43b106fbceca722b65cb5c861b0`) passed fresh standalone checks for narrower role metadata rules across finalized BSC blocks **122288006–122289029**, a complete 1,024-block interval. These are replacements for two existing profiles in the 426-profile `bsc-access450-layouts.json` baseline; they add no profiles or production code.

The correction removes only the two-word role rule. Ordinary mapping support remains, including MUSD's existing plain role-root rule. It rejects the reviewed adjacent membership words and the admin-role field, which has no reachable writer in either pinned contract. The remaining plain mapping rule does not enforce an exact nesting depth.

## Source and exact scope

Both complete verified sources inherit an unchanged direct `balanceOf(address)` getter. MUSD uses balance root 1 and OLY uses root 0. The source-returned compiler runtime equals the canonical on-chain runtime byte-for-byte; there are no immutable or CBOR transformations. The artifacts were compared, not locally recompiled. Fresh checks bind code at parent block 122288005 and final block 122289029; getter traces show one balance SLOAD and no external calls.

| Contract | Role root | Balance root | Allowance root | Preserved scalar metadata |
| --- | ---: | ---: | ---: | --- |
| MUSD | 0 | 1 | 2 | Supply 3; name/symbol heads 4–5 |
| OLY | 6 | 0 | 1 | Supply/name/symbol/decimals 2–5; pair, fee receiver and fee ratios 7–10 |

Each `RoleData` has a nested membership mapping at record offset 0 and a genuine `adminRole` word at offset 1. The public grant, revoke and renounce functions change membership. The sole `_setRoleAdmin` definition is internal, and neither complete inheritance graph calls it. OLY's fee callbacks are normal external calls on transfer paths; they provide no direct storage-writing or delegatecall path to the admin word.

The former recursive width-2 rule accepted both the genuine outer `adminRole +1` and `membership_hash +1`, even though the nested boolean has no second word. The replacement keeps plain root classification and rejects outer +1, outer +2 and inner membership +1. The admin-role getter still reads its real field; independent getter controls do not authorize writes there. The discovered issue was extra acceptance by an unknown-write guard, with no observed historical balance mismatch.

[Source evidence](evidence/role-width-source-review.json) preserves full primary sources, compiler layout, runtime bytes and getter paths. Other profiles with multiword mapping records remain outside this replacement cohort; terminal multiword records can legitimately require offsets.

## Fresh controls and parity

[Exact RPC control requests and responses](evidence/role-width-rpc-requests.json) retain 185 canonical hash-pinned read-only calls:

- 40 raw balance cases: five addresses per token, each with 0, 1, 123 and MAX, including the null address.
- 100 metadata independence cases: 76 configured cases and 24 genuine admin-role overrides whose synthetic writes must reject.
- 36 role getter cases spanning default, named minter and arbitrary roles; three OLY decimals-byte controls.
- Six rejected calls covering short calldata, call value and noncanonical address bits.

| Completed gate | MUSD | OLY | Total |
| --- | ---: | ---: | ---: |
| Emitted balances checked against RPC | 16 | 188 | 204 |
| Initialized reference observations | 32 | 308 | 340 |
| Parent-checkpoint holders | 7 | 81 | 88 |
| Cold unknown observations | 16 | 114 | 130 |
| Cold unknown observations with nonzero balance | 2 | 89 | 91 |

All emitted and initialized-holder values matched. The packaged audit includes 26 emitted zeros. The explicit parent checkpoint uses 176 balance/storage reads. Retaining actual packaged output matches 136 initialized observations without a new event; a fresh final RPC snapshot matches all 88 observed holders, including 25 zeros.

Reference rows never initialize or repair retained state. The cold unknown observations remain unknown, and processing uses no balance RPC calls. This covers the observed holder set, not global enumeration.

Successful local gate directories are `out/role-width-qualified`, `out/role-width-rpc`, `out/role-width-holder-coverage` and `out/role-width-wasm-holders`. Compact evidence: [source/control/native](evidence/role-width-qualified.json), [packaged RPC](evidence/role-width-rpc.json), [holder checkpoint/replay](evidence/role-width-holder-coverage.json), [actual-WASM retention](evidence/role-width-wasm-holders.json), and [summary](evidence/role-width-summary.json).

## Captured regressions and replacement artifact

`tests/fixtures/role-width` contains two captured Extended blocks with four independently audited RPC expectations, the replacement layouts, and all controls. Seven Rust regressions pass, covering captured and raw values, null filtering, exact metadata/preimages, required ordinary metadata, metadata-only writes, all three rejected role offsets, and valid/missing/corrupt preimages, including a missing outer mapping preimage.

`cargo test -p erc20-balances --lib role_width_tests --locked`

The [final qualification](evidence/role-width-final-qualified.json) binds canonical boundaries, all 1,024 clocks and every emitted row, the independently initialized holder set and final snapshot, fixture expectations, source/layout/package digests and focused tests. Replacement layouts are `out/role-width-final-qualified/added.json`; root-level combined replacement validation is separate and must not double-count these existing profiles.

- Replacement-layout SHA-256: `d7227597b5a13d4cede6d67e954f073bab497ce340d9f5ddcd365342b2d72de0`.
- Final qualification SHA-256: `0956319b4e5180c0d4ef2bd92aee3ae77228be2ecfc8205d50d674058cd70651`.
- Unchanged production SPKG SHA-256: `f1d57bdff549947cd69e47d117ad8cd74933c1e065b687f958fea4de39004d81`.

The initial compile-only scratch-helper failure is retained in `out/role-width-qualify.log`; the successful corrected run is `out/role-width-qualify-v2.log`. Previous published fixtures and reports remain unchanged. Rust helpers are archived under `out/next-candidate-rust-scripts/role-width-followup/`.
