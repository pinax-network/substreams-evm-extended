# FHE and B2Token: two NOT-QUALIFIED role candidates

These separate profiles replace legacy role rules with exactly
`[bytes32, address]`, terminal offset 0, width 1. FHE's root 5 occurs in
**both** `other_mapping_slots` and width-two `other_mapping_words`; both
entries must be removed. B2Token removes only the root-9 width-two rule.
Every other profile field, the historical 431-profile baseline and qualified
425 cohort remain unchanged. Neither candidate is promoted.

| Token | BSC contract | Role root | Balance root | Saved compiler |
| --- | --- | --- | --- | --- |
| FHE | `0xd55c9fb62e176a8eb6968f32958fefdd0962727e` | 5 | 0 | `0.8.24+commit.e11b9ed9` |
| B2Token | `0x783c3f003f172c6ac5ac700218a357d2d66ee2a2` | 9 | 0 | `0.8.22+commit.4fc1097e` |

## Complete saved capture and primary sources

`FHE.json` preserves the original `out/ranks351-400-source-review/{address}.json`
capture, SHA-256
`a71797c05722783295bb7dc0dc5a13956e1bb6eb5295b4c11766e693d81f4ecf`.
`B2Token.json` preserves `out/ranks201-250-source-review/{address}.json`,
SHA-256 `c468c616cfc867dab05817ab1af80e31b05575265eb1aaf7c5125dbe7ec6eb99`.
Both files are byte-identical copies, including complete source/compiler
input/output, metadata, ABI, storage layout, source IDs and runtime evidence.
Both captured compiler settings use Paris and disabled optimization/runs 200.

All 51 primary sources match exactly; no source/import normalization is allowed.
`primary-sources.json` retains their immutable URLs, SHA-256 and full content:

- FHE's token is
  [mind-network/mind-token-contracts at e3b91129](https://github.com/mind-network/mind-token-contracts/blob/e3b9112918304a7a00d210609eec34d5390ef2a1/contracts/other-evm/FHE.sol).
  Use this `other-evm/FHE.sol`, not the repository's separate Ethereum token.
  Its 24 dependencies match
  [OpenZeppelin v5.2.0 at acd4ff74](https://github.com/OpenZeppelin/openzeppelin-contracts/tree/acd4ff74de833399287ed6b31b4debf6b2b35527/contracts).
- B2Token is
  [b2network/b2-token-contract at 8369fb65](https://github.com/b2network/b2-token-contract/blob/8369fb6537f85ad746cef56ae616b5cb581c2c0b/contracts/B2Token.sol).
  Its 25 dependencies match
  [OpenZeppelin v5.0.2 at dbb6104c](https://github.com/OpenZeppelin/openzeppelin-contracts/tree/dbb6104ce834628e473d2173bbc9d47f81a9eec3/contracts).
  The distinct `72dc2b060eb986b3fc591bde55a685354a3cbcd4` main variant is not
  an accepted substitute for this pinned AccessControl token.

`source-review.json` records all per-file SHA-256/Keccak hashes. The host
verifier first pins each complete raw capture, then cross-checks source/input/
metadata/source-ID sets, compiler identity/settings/target, output metadata,
ABI/docs/layout and original compiler runtime bytes. This rechecks saved
compiler output; it does not run a fresh Solidity compilation.

## Exact immutable reconstruction

Unlike Point/Bedrock, these two runtimes require constructor-set immutable
substitutions. A separate helper permits only the following exact sets;
Point/Bedrock's no-transformation verifier remains untouched. Each AST ID
has exactly one 32-byte reference. No extra/missing/duplicate reference,
changed value/offset/length, overlapping/out-of-bounds range, linked library
or CBOR substitution is accepted. Original compiler placeholders must be zero.

| Meaning | FHE ID / byte offset | B2Token ID / byte offset |
| --- | --- | --- |
| ERC20Capped cap | — | 1188 / 1850 |
| Cached EIP-712 domain separator | 3455 / 4406 | 2694 / 4209 |
| Cached chain ID | 3457 / 4365 | 2696 / 4168 |
| Cached verifying contract | 3459 / 4279 | 2698 / 4082 |
| Hashed name | 3461 / 6451 | 2700 / 5919 |
| Hashed version | 3463 / 6484 | 2702 / 5952 |
| ShortString name | 3466 / 5043 | 2705 / 5044 |
| ShortString version | 3469 / 5102 | 2708 / 5103 |

Values are recomputed from chain ID 56, each exact contract address, version
`1`, and names `MindNetwork FHE Token` (21 bytes) / `BSquared Token` (14 bytes).
The standard EIP-712 domain type is
`EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)`.
Names/version are encoded with their byte length in the ShortString final byte.
B2's cap is `210000000 * 10^18`. Exact 32-byte values are retained in the review.

The resulting domain separators are FHE
`b6e246a226dd92fdeb64aaaaf6b1d1ca868ecb41da5fb65b497ba907d149778c`
and B2
`9a44467c882b6f41693862a261718969c0bbdc8fbaa9223f038effc9d734c3e1`.
The original CBOR trailers remain unchanged at offsets 10135 / 10404.
Reconstructed bytes must equal the entire saved runtime:

| Token | Runtime bytes | Runtime Keccak-256 |
| --- | --- | --- |
| FHE | 10188 | `0x652b7c5764581dd0fac2d759383df2c06101f0b7edf7f4194737f5b392dcc20b` |
| B2Token | 10457 | `0xb616979feee87559683543b45de4e46f31244303ab158bb343f1759a5f1ce5ab` |

The runtime binding does not establish current role/admin state, deployment
activation or package qualification. Captured constructor data is preserved,
not used to seed balances or claim current ownership.

## Source/layout review and tests

Both use ordinary OZ5 `RoleData.hasRole: mapping(address => bool)` at offset
0 and `adminRole: bytes32` at outer offset 1. Complete source review finds no
`_setRoleAdmin` callsite beyond the internal declaration and no initializer.
Constructors grant default-admin membership. Public grant/revoke and renounce
write the inner mapping; renounce additionally checks caller confirmation.
A comment/string-aware lexical guard counts the one unused setter declaration
in each complete pinned source set. It is not a general call-graph analyzer.

Both inherit direct ERC20 `balanceOf` without overriding it. Cap, pause,
permit and roles affect authorization or future updates, not the current
getter's direct mapping read. All other compiled fields remain unchanged:

| Token | Complete storage slots |
| --- | --- |
| FHE | 0 `_balances`, 1 `_allowances`, 2 `_totalSupply`, 3 `_name`, 4 `_symbol`, 5 `_roles`, 6 `_nameFallback`, 7 `_versionFallback`, 8 `_nonces`, 9 `ccipAdmin` |
| B2Token | 0 `_balances`, 1 `_allowances`, 2 `_totalSupply`, 3 `_name`, 4 `_symbol`, 5 `_nameFallback`, 6 `_versionFallback`, 7 `_nonces`, 8 `_paused`, 9 `_roles` |

The exact names fit ShortString; fallback string writes are constructor
behavior for long names, not observed here. FHE's `setCCIPAdmin` scalar at 9
is distinct from `RoleData.adminRole`; B2's pause state at 8 also stays separate.
Tests preserve those scalar paths, permit nonces, allowances and concurrent
balance writes. They admit default/minter/arbitrary membership grant/revoke/
no-op shapes and reject outer/admin/adjacent words, extra depth, wrong roots/
key order, address padding, missing/corrupt preimages, malformed words and
runtime changes. Failed/reverted writes retain their existing filtering.
A regression requires removing FHE's duplicate broad rule; merely removing
its width-two entry cannot produce a valid typed candidate.

These are synthetic Extended-record shapes, not executed role transactions,
authorization checks or boolean-value validation. Source/immutable tests
reject altered complete captures, source files, primary pins, compiler/layout
bindings and even coherently altered compiler/deployed bytes outside patches.

## Reproduction and limits

Use a fresh isolated target. The output parent must exist and the preparation/
replay directories must be new:

```sh
CARGO_TARGET_DIR=out/fhe-b2-FRESH/target-isolated \
  cargo run --locked --offline -p erc20-balances-tools --bin prepare_fhe_b2_roles -- \
  /absolute/original/erc20/balances out/fhe-b2-preparation-FRESH
CARGO_TARGET_DIR=out/fhe-b2-FRESH/target-isolated \
  cargo run --release --locked --offline -p erc20-balances-tools --bin replay_role_candidates -- \
  erc20/balances out/fhe-b2-replay-FRESH --fhe-b2 /absolute/original/erc20/balances
```

Preparation fetches only pinned public GitHub sources. Replay makes no network
requests and uses the immutable canonical capture. [The bounded report](../../../docs/fhe-b2-role-candidates.md)
records the interval and initialized observed-holder set. Fresh runtime,
replacement-package/RPC, initialized-holder/final-state and actual role-write
visibility controls remain promotion gates. Other legacy cohorts remain
outside this candidate; issue #4 stays open.
