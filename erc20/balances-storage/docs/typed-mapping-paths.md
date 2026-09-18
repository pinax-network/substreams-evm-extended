# Exact non-balance mapping paths

`other_mapping_paths` describes explicitly reviewed non-balance fields using
their declared mapping keys and terminal record offsets. It addresses the
[role-shape audit](role-shape-audit.md): treating an entire nested role mapping
as two-word records also accepts a word beside each holder's single-word
membership value. A role's admin field belongs to the outer record instead.

For a reviewed `mapping(bytes32 => RoleData)` at root 6, where `RoleData` holds
`mapping(address => bool) members` at offset 0 and `bytes32 adminRole` at offset
1, the two paths are:

```json
{
  "other_mapping_paths": [
    {
      "root": "0x0000000000000000000000000000000000000000000000000000000000000006",
      "key_types": ["bytes32", "address"]
    },
    {
      "root": "0x0000000000000000000000000000000000000000000000000000000000000006",
      "key_types": ["bytes32"],
      "offset": 1
    }
  ]
}
```

This is a shape example, not a qualified token configuration. Omit the admin
path when the reviewed runtime cannot persist admin changes in the supported
interval. Source reachability matters: an inherited setter with no callsite does
not establish that an admin write is possible. Remove the same root from legacy
ignore fields when using a typed path; overlapping configuration is rejected.

## Matching rules

- `root` is exactly 32 bytes of `0x`-prefixed hex in token storage.
- `key_types` lists 1–8 keys from outermost to innermost. Supported types are
  `address`, `bytes32`, and explicitly sized `uint8` through `uint256`, in whole
  bytes. Address and smaller unsigned keys must have canonical zero padding.
- `offset` defaults to 0; `words` defaults to 1 and must be 1–32. The terminal
  word range must fit offsets 0–255. Offsets use Solidity's modulo-256-bit storage
  arithmetic and apply only after the final mapping hash.
- Every mapping level needs its own 64-byte, hash-verified preimage. The matcher
  walks exactly the configured depth. Missing ancestors, additional nesting,
  wrong key order, malformed padding, and array-shaped 32-byte preimages fail.
- Paths sharing a root must agree on shared key types. Terminal ranges at equal
  depth cannot overlap. A shorter path cannot treat offset 0 as a scalar when a
  longer path uses it as an inner mapping anchor.

For the example, the mapper accepts `hash(role, root) + 1` and
`hash(holder, hash(role, root))`. It rejects the outer anchor itself, outer
offset 2, membership offset 1, and mappings under an offset-shifted parent.
A reviewed `mapping(uint32 => FourWordRecord)` uses `["uint32"]`, `words: 4`;
only terminal offsets 0–3 are allowed, with no recursion into its members.

The parser rejects roots reused by balances, scalar fields, legacy mapping
rules, proxy pointers, balance dependencies, voting histories or address lists.
Beacon-account pointers remain in the beacon's separate storage namespace.
Runtime/dependency-change guards still execute before metadata classification;
metadata rules cannot bypass them, even for change-and-restore sequences.

## Qualification boundary

These paths classify storage locations; they do not infer whether a field can
affect `balanceOf`, validate an entire Solidity value type, or prove a runtime's
identity. Independent source/runtime qualification is still required. Explicit
root conflicts do not constitute a general proof against aliases involving
hashed intermediate addresses. Retained legacy rules continue to accept their
original broader shapes, so each complete configuration needs review.

Dynamic mapping keys, signed keys, fixed bytes shorter than 32, intermediate
struct offsets, and mapping-owned enumerable arrays are unsupported. In
particular, an EnumerableSet's address-to-index mapping at outer offset 1 and
its array storage cannot be represented by these paths. Unknown persisted writes
continue to fail closed. A historical interval without such writes does not
establish support for those operations.

## Offline validation

The Rust regressions cover exact role depth/offsets, all supported unsigned key
widths, address padding, zero/max keys, missing/corrupt preimages, parser
ambiguities, protected dependencies, metadata-only blocks, persisted restores,
reverts and system calls. In-memory typed versions of saved MUSD/OLY and USDe
fixtures preserve all 11 captured RPC event expectations. Published profile
fixtures remain unchanged.

The separate [APD/DSG review](typed450-offline-review.md) traces their saved
strict failures to ordinary allowance writes and retains the unsupported DSG
enumerable-role shape explicitly.

The [full saved-data baseline replay](evidence/typed-path-baseline-replay.json)
uses the production source at commit `a8270d5` with the unchanged 431-profile
fixture. All 1,024 full blocks in `[122288006, 122289030)` match the saved
canonical-bound clock identities and parent chain. All 110,139 emitted balances
(15,259 zeros) match the historical storage protobuf output, including every
field and optional-contract presence; only row ordering is normalized.

The separate canonical RPC capture supplies 110,138 same-block matches and
4,012 matches for balances retained from earlier native emissions. There are no
value mismatches. The replay does not seed a holder checkpoint, so 66,265
reference-only observations remain unknown. One storage row absent from that
RPC capture remains explicit: contract `0x32b133ca38c9b410a053f2bcfeea83831c3bcfe0`,
holder `0xa6ee430f253057aa4de8d757f6de081bdaa75e52`, block 122288884, amount
`100000000000000000000000000`. It matches historical storage output and is not
counted as a same-block RPC comparison. These differences are why unchanged
historical output is not described as universal stream-row parity or complete
holder initialization.

The first unoptimized attempt was deliberately stopped and preserved as a
partial run. A separate optimized native run completed in 188.78 seconds with
zero network requests. Input, per-block, output, source and attempt hashes are
bound in the summary; full artifacts and the Rust runner remain under
`out/typed-path-baseline-replay/` relative to this module.

Live Substreams, Firehose and RPC usage is paused. The committed storage SPKGs
predate this source change and are preserved as historical artifacts. Offline
native replay and WASM compilation do not qualify a newly packaged module, new
token, unseen storage operation or global holder set. Packaging, actual-WASM
stream parity and fresh RPC controls remain pending until live testing resumes.
