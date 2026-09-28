# Artx: one NOT-QUALIFIED proxy membership candidate

This separate profile for BSC proxy
`0x8105743e8a19c915a604d7d9e7aa3a060a4c2c32` changes only legacy role root 151,
width 2, to `[bytes32,address]`, terminal offset 0, width 1. No scalar or generic
role-admin path is added. Every other field, both runtime hashes, the
implementation address/slot and deployment guard remain unchanged. Neither
the historical 431 fixture nor qualified 425 cohort is modified or promoted.

## Two complete captures and distinct compiler source sets

| Capture | Address | Complete original capture SHA-256 |
| --- | --- | --- |
| `proxy.json` / ERC1967Proxy | `0x8105743e8a19c915a604d7d9e7aa3a060a4c2c32` | `22e604d6c46a14825b5a95f686ffb1875475ad0725fe63d7d77747bb0931d9ab` |
| `implementation.json` / ArtxToken | `0xc401796ec909f7774703528571c3590748ebc563` | `f01037c490dd41a7e656ad3dcf8b7cfd95ee125f3378ceac81d663933a5d7e7e` |

The raw files are unchanged original responses from
`out/ranks151-200-source-review/<proxy>.json` and
`out/ranks151-200-pending-proxies/<implementation>-source.json` in the original
package cache. Both record chain 56 and original `exact_match` labels.
The Rust helper checks those exact raw digests before parsing, then verifies
address/compiler/input/output/metadata/ABI/docs/layout and complete bytecode.

The proxy capture and standard-JSON input contain **14 files**. Its selected
compiler source IDs/output/metadata contain **eight files**. Six input-only
files remain preserved: Ownable, Context, BeaconProxy, UpgradeableBeacon,
TransparentUpgradeableProxy and ProxyAdmin. The selected subset is checked
exactly; input-only files have no selected metadata Keccak and are bound by
the complete capture digest and independently matched primary bytes. The
implementation has **21 files** in all four sets. No missing selected sources,
extra selected sources, dropped inputs or invented metadata are accepted.

## Exact primary dependencies and unresolved token source

`primary-sources.json` preserves the original research proof, SHA-256
`57adbbb35d47187a3edada8f43da5471274f27114652490bb5736528aeaf9bb1`, with full
contents, exact URLs, hashes and selected-output flags for 34 matching files:

- All 14 proxy inputs match [OpenZeppelin v5.2.0 at acd4ff74](https://github.com/OpenZeppelin/openzeppelin-contracts/tree/acd4ff74de833399287ed6b31b4debf6b2b35527/contracts).
- All 20 implementation dependencies match [OpenZeppelin Upgradeable v4.9.6 at 2d081f24](https://github.com/OpenZeppelin/openzeppelin-contracts-upgradeable/tree/2d081f24cac1a867f6f73d512f2022e1fa987854/contracts).

Preparation fetches those immutable URLs again and requires byte equality
with both proof and captured sources. Original MIT dependency notices remain.
The captured BUSL-1.1 token `contracts/ArtxToken.sol` has exact SHA-256
`ba971c5212b7cc1dd5a679bf6501fb4588540fb8a1f2c42325d72248575693d8`.
Its independent public token pin remains unresolved after the bounded
ULTILAND/public-code search. That search does not prove no source exists
elsewhere. No normalization or token pin is inferred from exact dependencies.
The original token license and source bytes remain intact.

## Saved bytecode and independent constructor encoding

| Capture | Recorded compiler/settings | Runtime bytes / Keccak |
| --- | --- | --- |
| Proxy | solc 0.8.29+ab55807c; optimizer 200; Paris; IPFS metadata; no links | 170 / `0x864cc9ad53b338b82da1f7cab85ab0b3d5c8861acb422b6fec63cf36234f36a6` |
| Implementation | solc 0.8.20+a1b79de6; viaIR; optimizer 1; Paris | 10,348 / `0xf8ef8aafe948cf5240fb04631cd152673a48a13c9c6be2e638e2642250d4f03c` |

Proxy runtime is untransformed. Implementation runtime has only three
UUPS `__self = address(this)` immutable replacements, ID 1097, 32-byte sites
1785/2035/2980. Each zero placeholder becomes the padded implementation
address, independently constructed from the bound address above. Full
reconstructed bytes equal the saved runtime. Both original CBOR trailers
and compiler source maps remain bound; no linked or metadata replacement
is accepted. These checks reconstruct saved compiler output and do not run
a fresh Solidity compilation.

The helper independently ABI-encodes the proxy's complete constructor
`(implementation, initCalldata)`. The 356-byte payload uses selector
`0x1e5c013d` for
`__ArtxToken_init(string,string,uint256,address[],uint256[])`, with
`("Ultiland","ARTX",0,[],[])`. It checks offsets 160/224/288/320, lengths,
empty arrays and every padding byte. The 480-byte constructor append plus
1,040 saved compiler creation bytes must equal all 1,520 saved creation bytes,
Keccak `0xe8a46410e3efa2bfef8c9d29bbc3c99cf959b5825df1d9c898b2dc505d142a2d`.
Implementation creation is untransformed, 10,574 bytes, Keccak
`0xc23c8191323365eee633715e963eb0f4df6fa1d0d1377808532c4d0552bce19b`.

The captures record proxy block 67,828,331/transaction 141 and implementation
block 67,828,310/transaction 162, with exact transaction hashes and common
recorded deployer `0x030b08e1f870362453062d2920fb651c25b96ea1` in the review.
They do not attest current proxy pointer, owner/roles, successful initialization
or independently qualified deployment. Zero initial supply and empty arrays
never seed a balance or holder. Matching captured creation still refuses.

## Getter invariance and bounded reachability

The inherited ERC20Upgradeable `balanceOf` reads only balances root 51.
The token constructor disables initializers; `__ArtxToken_init` initializes
bases and grants default-admin/operator memberships. Its initial mint arrays
are memory arguments, not stored role enumeration. Public inherited grant,
revoke and renounce reach the exact inner membership mapping.

Review of all 21 implementation sources finds only the unused
`_setRoleAdmin` declaration in AccessControlUpgradeable. The lexical regression strips comments/literals
before checking that single identifier reference; it is a guard for this
exact source bundle, not a general Solidity call-graph analyzer. No outer
admin word or adjacent membership word is admitted. `_authorizeUpgrade`
remains owner-gated, and both source-bound proxy/implementation guards remain.

| Stored fields | Existing candidate treatment |
| --- | --- |
| Packed initializer flags at slot 0 | Unchanged scalar |
| Balances at 51, allowances at 52 | Unchanged balance/address mappings |
| Supply at 53, name at 54, symbol at 55 | Unchanged scalars |
| RoleData at 151 | Exact bytes32/address membership, offset 0, width 1 |
| Owner at 201, totalMinted at 351, totalBurned at 352 | Unchanged scalars |
| Fixed inherited gap arrays | Remain unsupported; no new permission |
| ERC1967 implementation slot | Existing fail-closed dependency guard |

Typed membership constrains key shape and depth, not boolean values or
transaction authorization. Synthetic Extended-record tests cover grant,
revoke, no-op and simultaneous balances; admin/member+1/depth/root/key-order,
padding/preimage/33-byte refusals; retained metadata/gap handling; persistence
filtering and both code/pointer/creation guards. The legacy adjacent-word
acceptance remains a control. These tests do not execute role transactions
or qualify actual producer visibility.

## Reproduction and remaining gates

```sh
CARGO_TARGET_DIR=out/artx-role-FRESH/target-isolated \
  cargo run --offline --locked -p erc20-balances-tools --bin prepare_artx_role -- \
  /absolute/original/erc20/balances /absolute/artx-primary-sources-20260928.json \
  out/artx-preparation-FRESH
```

The output parent must exist; the preparation directory must not exist.
Preparation preserves its exact as-run helper/CLI and hashes, both raw
captures, primary proof, freshly retrieved primary files and generated review
and candidate. The replay makes no network requests:

```sh
CARGO_TARGET_DIR=out/artx-role-FRESH/target-isolated \
  cargo run --release --offline --locked -p erc20-balances-tools --bin replay_role_candidates -- \
  erc20/balances out/artx-replay-FRESH --artx /absolute/original/erc20/balances
```

[Replay results and limits](../../../docs/artx-role-candidate.md)
report the exact interval, initialized observed holders, cold observations
and real membership-write counts. Canonical values never seed the ledger.

Fresh runtime/dependency controls, replacement-package/getter parity,
initialized-holder/final-state checks and actual role-operation visibility
remain promotion gates. The primary token-source gap is explicit; issue 4
remains open. No production parser, package or wire contract changes here.
