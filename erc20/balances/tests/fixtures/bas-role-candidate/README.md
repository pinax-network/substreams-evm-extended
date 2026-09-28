# BAS: one NOT-QUALIFIED membership and fixed-admin candidate

This separate profile for BSC BASToken
`0x0f0df6cb17ee5e883eddfef9153fc6036bdb4e37` replaces legacy role root 6,
width 2, with `[bytes32, address]`, terminal offset 0, width 1. It also permits
exactly one fixed scalar location:

```text
PAUSER_ROLE = keccak256("PAUSER_ROLE")
            = 0x65d7a28e3265b37a6474929f336521b332c1681b933f6cb9f3376673440d862a
admin word  = keccak256(PAUSER_ROLE || uint256(6)) + 1 (mod 2^256)
            = 0xe09f975e15f8f53f24cbbc282b13c40b84df485fcdb8d3997fa103dc5a4ef842
```

No generic outer role-admin path is admitted. All other profile fields,
runtime and deployment guards remain unchanged. The historical 431-profile
fixture and qualified 425 cohort remain byte-identical. This is not candidate,
deployment or replacement-package qualification.

## Complete source, compiler and constructor binding

`source-capture.json` preserves the original complete response from
`out/ranks101-150-source-review/0x0f0df6cb17ee5e883eddfef9153fc6036bdb4e37.json`
in the original package cache, without reserialization. SHA-256:

```text
8fe36b7af10f037925e8a777e45ac5647d82779c606d99790f467c52e0d8cdd0
```

The Rust verifier requires chain 56/address; all 14 sources and their metadata
Keccaks; exact standard-JSON input, source IDs, compiler metadata, ABI, docs,
layout and runtime/creation objects. The original `match`, `runtimeMatch`
and `creationMatch` labels remain **`match`**, not `exact_match`.

Recorded solc is `0.8.26+commit.8a97fa7a`, Paris, via IR, optimizer enabled
with 1,000,000 runs, metadata bytecode hash disabled, no linked libraries or
remappings. This reconstructs saved compiler output; it is not fresh Solidity
compilation. The complete 9,652-byte saved runtime has Keccak:

```text
0x957bf9d2d5b267f6e4f01ddcbda34c4b3cf9d89f3af6c07c98056ca60fb9733e
```

The sole `_cap` immutable, ID 1169, has exactly two 32-byte zero placeholders
at offsets 3249 and 4710. Each becomes `10^28`, encoded as
`0x0000000000000000000000000000000000000000204fce5e3e25026110000000`.
No other replacement is allowed; the original CBOR trailer at 9640 remains.

The helper independently ABI-encodes the complete constructor append:
`("BNB Attestation", "BAS", 10^28, admin, pauser)`, where both addresses are
`0x9d8796b0ac1064ede1378d785df96970eaf5a2b9`. Exact dynamic offsets, lengths,
string/address padding and values produce 288 bytes. Appending these to the
11,365-byte saved compiler creation object must equal all 11,653 saved
creation bytes. The capture records block 53,994,920, transaction index 117,
transaction `0x5368d4291e1b1427f0f61abe80ff515c5814e88f573a35dd71f4754accfd1c72`.
Those are captured facts, not current admin state or independent deployment
qualification. The profile gains no `deployment` declaration, and matching
runtime creation still refuses under its existing guard.

## Primary-source gap remains explicit

`primary-sources.json` retains immutable URLs, original contents and SHA-256
hashes for all 13 OpenZeppelin dependencies, exactly matching
[OpenZeppelin v4.9.6 at dc44c9f1](https://github.com/OpenZeppelin/openzeppelin-contracts/tree/dc44c9f1a4c3b10af99492eed84f83ed244203f6/contracts).
Their original MIT notices remain intact.

The captured MIT token `contracts/bas/BAS.sol` has SHA-256
`08beba1cb6911308c84bb235219eead860f1246e8913a7652f232ea73495c8c1`.
None of the six recorded public `bnb-attestation-service/bas-erc20` revisions
matches it. Public [BAS.sol at 2a988ca0](https://github.com/bnb-attestation-service/bas-erc20/blob/2a988ca00a4b8301cb2d5d90f773d2fa402054cf/BAS.sol)
lacks the captured `ERC20Recovered` event declaration and emit. Earlier
recorded revisions also differ. Their exact bytes and hashes are preserved;
no normalization or inferred exact token repository pin is permitted.
Dependency matches do not resolve this token-source gap.

## Getter invariance and reachable writes

| Slot | Captured source field | Candidate treatment |
| --- | --- | --- |
| 0 | `_paused` | Unchanged scalar |
| 1 | `_balances` | Unchanged emitted balance mapping |
| 2 | `_allowances` | Unchanged address mapping |
| 3, 4, 5 | `_totalSupply`, `_name`, `_symbol` | Unchanged scalars |
| 6 | `_roles` | Exact inner membership plus one fixed PAUSER admin word |
| 7 | `_isWhitelisted` | Unchanged address mapping |

The token inherits ordinary ERC20 `balanceOf`, reading only `_balances`.
Role, pause and whitelist state govern calls and transfers, but are not
inputs to that getter; persisted balance writes still emit normally.
AccessControl's `RoleData` contains address/bool `members` at offset 0 and
bytes32 `adminRole` at offset 1. Constructor grants and public
`grantRole`/`revokeRole`/`renounceRole` reach the membership mapping.

The captured token's sole `_setRoleAdmin` call is in its constructor:
`_setRoleAdmin(PAUSER_ROLE, PAUSER_ROLE)`. There is no initializer or other
admin setter. `setMinter` is default-admin gated; pause/unpause and whitelist
setters require PAUSER_ROLE. Only the constructor's fixed PAUSER admin word
is added, without authorizing other role admin records. Location admission
does not itself prove constructor context, transaction authorization or a
particular value.

Synthetic Extended-record tests cover grant/revoke/no-op membership shapes,
fixed admin and simultaneous balances, unchanged pause/whitelist/allowance
handling, and failed/reverted writes. They refuse arbitrary admin words,
outer anchors, member+1, wrong fixed words/depth/key order/address padding,
missing/corrupt/oversized preimages, malformed storage words and runtime
changes. Valid-width metadata values are not constrained to 0/1; these are
source-derived storage-shape tests, not executed Solidity role operations.
The legacy adjacent-word over-admission and its fixed-admin refusal without
preimages remain reproducible controls. Source controls mutate capture hashes,
complete sets, immutables, constructor encoding, public pins and candidate scope.

## Reproduction and qualification limits

Prepare fresh evidence using saved capture and immutable public source reads:

```sh
CARGO_TARGET_DIR=out/bas-role-FRESH/target-isolated \
  cargo run --offline --locked -p erc20-balances-tools --bin prepare_bas_role -- \
  /absolute/original/erc20/balances out/bas-preparation-FRESH
```

The helper preserves its as-run CLI and binding-helper source and hashes.
The replay itself makes no network requests:

```sh
CARGO_TARGET_DIR=out/bas-role-FRESH/target-isolated \
  cargo run --release --offline --locked -p erc20-balances-tools --bin replay_role_candidates -- \
  erc20/balances out/bas-replay-FRESH --bas /absolute/original/erc20/balances
```

Output parents must exist; preparation and replay directories must be fresh.
[Replay evidence and limits](../../../docs/bas-role-candidate.md) distinguish
initialized observed holders from cold reference observations and count
membership and fixed-admin writes separately. Canonical references never
seed or repair the ledger. Actual runtime/deployment controls, replacement
package/RPC parity, initialized-holder/final-state checks and role-operation
visibility remain promotion gates. Issue #4 stays open.
