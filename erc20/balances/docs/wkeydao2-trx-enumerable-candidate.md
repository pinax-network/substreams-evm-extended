# wkeyDAO2 and TRX enumerable candidates

Two separate **NOT-QUALIFIED** candidates narrow the immutable historical
431-profile baseline. They use the unchanged `oz_3_4_2` complete-operation
validator. No production, parser, VM, schema, dependency, persistence or protobuf
change is introduced. The [Phase A proof](wkeydao2-trx-operation-proof.md), these
synthetic projector controls and saved replay are distinct evidence.

| Target | Address | Only removed permission | New legacy rule |
| --- | --- | --- | --- |
| wkeyDAO2 | `0xe0a281deff5c9d8d67af09d39340e134ac81b82e` | root 8 width **2** | root 8, `[bytes32]`, `oz_3_4_2` |
| TRX | `0xce7de646e7208a4ef112cb6ed5038fa6cc6b12e3` | root 6 width **3** | root 6, `[bytes32]`, `oz_3_4_2` |

Neither role root also occurs in `other_mapping_slots`. All fields and array
positions restore exactly to the original profiles; the 431 and qualified 425
fixtures are unchanged. There is no `membership_root` (including null), boolean
path, admin scalar/path, standalone index permission, creation or deployment
rule. These are distinct from WKEYDAO/GOT and issue #61's **swkeyDAO2**.

Both preserve balance root 0 and allowance root 1. wkeyDAO2 preserves nonce root
6 and scalars 2,3,4,5,7,9,10,11,12,13; TRX preserves nonce root 10 and scalars
2,3,4,5,7,8,9. Neither has MaxSupply. Source decimals are 9 and 6 respectively;
balance outputs remain raw stored integers. All existing runtime guards remain
unchanged, with no proxy or deployment fields inferred.

## Exact source and program bindings

The preparer pins **28 final Phase A artifacts** before parsing, including both
complete captures, original/expanded compiler inputs, full compiler outputs,
raw metadata, source notices, primary evidence and five final report files.
Both original primary-cache captures must equal their committed copies.
The compiler report, execution report and source inventory bind the same 243
historical inputs; the report's compiler-report and inventory entries are
checked by their exact raw digests.

Final Phase A report SHA-256:
`e31c76e8a17f75531a2af286a6c04418b663ecd75cd4bd75a628d743169764ef`;
compiler report `842b71fd927ccbd8332f437d14003e0a27e9a540553e6038292792f40e83af56`;
inventory `829cc8b9d643071169bf2c4e3c5c71a22c314334b4eb46f4b25aa14d7a9bc977`.
The proof contains 2,460 calls, 1,230 exact compiled/captured pairs: 1,946 returns,
488 reverts, 12 explicit source INVALID controls and 14 unsupported boundaries.
Its 872 compact transcripts do not represent saved-chain role activity.

Official solc 0.7.5 and 0.6.6 preserve their original settings and full source
bytes. wkeyDAO2 has four exact complete OpenZeppelin 3.4.2 dependencies at
`8e0296096449d9b1cd7c5631e917330635244c37`; its custom AGPL source's independent
primary revision is unestablished. TRX retains its full raw CRLF flattened file.
Only its complete AccessControl and EnumerableSet declarations match that pin
after comparison-only CRLF normalization. These are not whole-token, all-component
or overall-license provenance claims.

| Target | Runtime bytes / Keccak | CBOR runtime / creation offsets | Creation tail / arguments |
| --- | --- | --- | --- |
| wkeyDAO2 | 8,495 / `0x9054d0efc5311cd08c2d5c1204bc9f600f43e301ea5fe6446b4d581c44d6e6a3` | 8,442 / 9,764 | 32 / 96 bytes |
| TRX | 7,088 / `0x84f4834aef7376b01cc68f967cd357e4bfbe13ba2b775757ed1f8e28b67be82c` | 7,035 / 8,761 | 79 / 192 bytes |

Each exact 53-byte CBOR replacement preserves all remaining runtime/creation
bytes and the independently encoded constructor append. Original match labels
remain `match`; the reason for the captured IPFS digest differences remains
unestablished. No arbitrary stripping, normalization, links or immutable patches
are allowed. The paired executed paths exclude the changed spans from executed
PCs, PUSH immediates and CODECOPY. This is bounded equivalence evidence.

Both constructors stop at unsupported CHAINID with rollback. TRX's predicate
grant occurs before that boundary; its synthetic default-admin states are not
constructor-derived authority. Synthetic self-address suffix dispatch does not
execute a meta-transaction, signature or external self-call. Candidate creations
remain refused. No constructor result initializes retention.

## Source-specific structural controls

For `R=H(role||root)`, length is `R`, elements `H(R)+i`, positions
`H(address||(R+1))`, and admin `R+2`. Actual compiled and captured operations use
length→element→position for add and destination-copy→tail-position→tail-clear→
length-decrement→removed-position for removal. Tail/sole removal executes equal
self-swaps. Changing stages are mandatory; only logically derived equal stages
may be omitted.

| Target | Actual add store PCs | Actual removal store PCs |
| --- | --- | --- |
| wkeyDAO2 | 6941, 6957, 6976 | 7704, 7724, 7755, 7757, 7782 |
| TRX | **6176**, 6192, 6211 | 6321, 6341, 6372, 6374, 6399 |

TRX's first add store is the compiler-generated length write at PC6176, not an
optional or absent stage. Solc 0.6.6 supplies no generated source text. Only the
Phase A pinned runtime SLOAD6169/SSTORE6176 and constructor SLOAD1372/SSTORE1379
have exact `27:10:-1` / `45:23:-1` spans without Solidity attribution. Other
effects require exact source attribution. The candidate introduces no exception.

Thirteen projector tests select target, compiled/captured variant, code kind,
name and ABI signature from the raw-pinned final records. Both programs' complete
operation pairs and each target's source PCs are asserted. Thirty coherent calls
per target/variant cover every optional equality subset, plus nine sequential
transitions. Controls include zero/sole/moved members, full-width roles, source
no-ops, malformed source successes, aliases, old-value continuity, missing/extra
stages, dirty words/preimages, runtime/creation refusal, frame/transaction/role/
account separation and barriers. Every negative carries a valid balance update
and requires whole-projection refusal. Complete baseline restoration and all
raw artifact/cache mutations are checked separately by host tests.

Legacy Extended v3 root-begin fallback and v4/v5 positive real-frame behavior
are preserved. Numeric words normalize up to 32 bytes; 33-byte words refuse.
Recognized standalone head/index/admin/anchor fragments refuse. An unrecognized
equal array write without a changing length witness retains ordinary no-op
handling. No coupled-mode fragment policy is imported. Checked growth is a
conservative admission restriction: malformed source MAX growth wraps.
No-write malformed state cannot establish unseen initial coherence.

## Saved replay and qualification

The new counter binds wkeyDAO2 only to root 8 and TRX only to root 6. It counts
changed persisted outer length words with exact verified Keccak preimages;
array/index/admin/equal/foreign/failed/reverted writes cannot supply the count.
Only successful complete projection labels these witnesses validated legacy
operations. The diagnostic grants no permission or authorization claim.

The [replay report](evidence/wkeydao2-trx-enumerable-candidate-20260929.json) and
[source inventory](evidence/wkeydao2-trx-enumerable-candidate-20260929-source-inputs.json)
are exact copies from `out/wkeydao2-trx-enumerable-candidate-20260929/replay-01`.
Report SHA-256: `eea3241018c9a23a9f7a7039b7cc59e537dccf933a92b6a7de40ffaadbeb09ee`.
Inventory SHA-256: `2db161f0411703fe6dca465ee4fa9fa88a31a851a0d4856cd337e23fe62c92e9`.
Release `prepare-02` reproduced both candidate files and the helper/CLI snapshots
exactly. All **240** frozen source and artifact identities remained unchanged
during replay and were rechecked after integrating actual merged main.

All **1,024 Extended v5 blocks in [122288006,122289030)** passed exact clock,
parent-link and canonical-reference identity checks. All **110,139 full protobuf
rows** equal the unchanged baseline and historical output. The whole configured
replay has 110,138 same-block canonical matches, one unchanged native-only row,
4,012 reference-only retained matches and 66,265 cold observations left unknown.

| Bounded observed scope | wkeyDAO2 | TRX |
| --- | ---: | ---: |
| Emitted rows / same-block canonical matches | 108 / 108 | 27 / 27 |
| Initialized observed holders | 24 | 18 |
| Reference-only retained matches | 1 | 0 |
| Cold reference observations | 63 | 19 |
| Nonzero cold observations | 47 | 10 |
| Changed role lengths / validated legacy operations | 0 / 0 | 0 / 0 |

Canonical RPC values never initialize retained state. These are observations and
initialized holders in this interval, not global-holder support. No synthetic
calls were inserted into saved activity, and zero captured role operations does
not qualify real role/preimage/equality visibility.

The pinned, locked offline workspace run passed **1,235 tests across 113 suites**
(75 nonempty), with formatting, all-target Clippy under `-D warnings` and the
WASM workspace check passing. Validation used the unique
`out/wkeydao2-trx-enumerable-candidate-20260929/target-isolated` target. Logs are
`fmt-check-02.log`, `workspace-tests-01.log`, `workspace-clippy-01.log` and
`wasm-check-01.log` under that output parent. Actual merged main
`5c601dc89dccafbdda90ef120566925b46cd571d` is tree-identical to integrated source
`feec2441667490337ce75a3526388fa76449e78b` used for the replay/test build. The
fast-forward changed no source or fixture bytes. These offline checks do not
qualify a new WASM package.

Initial set coherence, uniqueness and unused-tail zeros, complete source history,
runtime continuity, actual producer framing/preimage/equality visibility, and new
package/getter/initialized-holder/final-state qualification remain open. These
offline candidates do not establish live or global-holder support.
