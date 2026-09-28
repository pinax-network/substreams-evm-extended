# Point and Bedrock: two NOT-QUALIFIED role candidates

These separate profiles change only each token's legacy role root from width
two to `[bytes32, address]`, terminal offset 0, width 1. The historical
431-profile fixture and qualified 425 cohort remain byte-identical. This
fixture does not promote either candidate or qualify a replacement package.

| Token | BSC contract | Role root | Balance root | Compiler |
| --- | --- | --- | --- | --- |
| Point / MCoin | `0x826923122a8521be36358bdc53d3b4362b6f46e5` | 0 | 1 | `0.8.9+commit.e5eed63a` |
| Bedrock / BR | `0xff7d6a96ae471bbcd7713af9cb1feeb16cf56b41` | 5 | 0 | `0.8.17+commit.8df45f5f` |

## Complete capture and primary sources

`Point.json` and `Bedrock.json` preserve the complete original saved responses
from `out/ranks251-300-source-review/{address}.json` in the original package
cache, without reserialization. Their SHA-256 digests are respectively:

```text
379a18f2648c684062838d90735bc4bfee90187d62f15e64e18c5ce6b1f1ae88
4a1d4290e378ee9bf416f83b60097b9bed9ac1c43a22e42137d3874d9c936c0a
```

The host Rust verifier checks chain 56/address, complete source sets (11 and
12 files), every source's metadata Keccak hash and recorded SHA-256, compiler
identity/settings/target, standard-JSON input equality, source IDs, saved
compiler metadata/ABI/docs/layout, and exact complete deployed runtime bytes.
The corresponding runtime Keccak hashes are:

```text
Point:   0xa099dede5abb0391e457e8f49f4b61061005b3aea81cd5be261380d99b5659ec
Bedrock: 0xf5a9b1fe6edbc5caf0c7e395f1fca9d2bb67f7d232ca90e149507a8b842256c4
```

Saved compiler bytes equal saved runtime bytes **without any transformation**.
Immutable and library references are empty. CBOR metadata stays intact and
its recorded offset/content must match the original trailer. This is a
recheck of saved compiler output, not fresh Solidity compilation or a new
observation of deployed runtime/activation.

`primary-sources.json` contains exact bytes, immutable URLs and SHA-256 for
22 independently fetched primary files; no source normalization is allowed:

- Bedrock `contracts/BR.sol` matches
  [Bedrock-Technology/BR at 48f3eb27](https://github.com/Bedrock-Technology/BR/blob/48f3eb27d953a00ac7d2b6bc2719052b95ad18bd/contracts/BR.sol).
  Its 11 OpenZeppelin dependencies match
  [openzeppelin-contracts at 0a25c194 (v4.8.3)](https://github.com/OpenZeppelin/openzeppelin-contracts/tree/0a25c1940ca220686588c4af3ec526f725fe2582/contracts).
- Point's 10 OpenZeppelin dependencies match
  [openzeppelin-contracts at 8c49ad74 (v4.7.0)](https://github.com/OpenZeppelin/openzeppelin-contracts/tree/8c49ad74eae76ee389d038780d407cf90b4ae1de/contracts).
  **Point's token repository remains unresolved.** Its `contracts/Point.sol`
  is retained with the exact saved source/compiler/runtime binding; no public
  token repository pin is inferred from dependency matches.

## Complete layout and source review

| Token | Saved fields by slot | Existing treatment |
| --- | --- | --- |
| Point | 0 `_roles`; 1 `_balances`; 2 `_allowances`; 3 `_totalSupply`; 4 `_name`; 5 `_symbol` | Only role root 0 changes; other fields stay identical |
| Bedrock | 0 `_balances`; 1 `_allowances`; 2 `_totalSupply`; 3 `_name`; 4 `_symbol`; 5 `_roles`; 6 `freezeToRecipient`; 7 `frozenUsers` | Only role root 5 changes; separate freeze scalar/address mapping remain |

Both inherit the direct ERC20 `balanceOf` getter without overriding it.
Point initializes default-admin and minter membership in its constructor.
Bedrock initializes default-admin, freezer and minter membership; freezing
constrains future transfers but does not alter the getter's current amount.
Both use ordinary, non-enumerable AccessControl: `members` is an address/bool
mapping at RoleData offset 0, while `adminRole` is bytes32 at outer-record
offset 1. Public grant/revoke/renounce reach membership writes. Complete
concrete-source review finds no `_setRoleAdmin` callsite beyond the unused
internal declaration. The Rust lexical check excludes comments/literals and
requires exactly that one identifier reference in each pinned source set;
it is a bounded review guard, not a general Solidity call-graph analyzer.

Synthetic Extended-record tests admit grant/revoke/no-op shapes for default,
minter, freezer and arbitrary role keys, including concurrent balance writes.
They reject outer admin records, adjacent membership words, extra depth,
wrong roots/key order, malformed address padding, missing/corrupt preimages,
malformed storage words and runtime changes. Failed/reverted writes and
ordinary no-op filtering retain their existing behavior. Bedrock's separate
freeze scalar/mapping controls still pass. These tests exercise source-derived
storage shapes, not executed role transactions or boolean-value validation.

## Reproduction and remaining gates

Prepare fresh evidence from saved captures and pinned public GitHub sources:

```sh
CARGO_TARGET_DIR=out/point-bedrock-roles-FRESH/target-isolated \
  cargo run --offline --locked -p erc20-balances-tools --bin prepare_point_bedrock_roles -- \
  /absolute/original/erc20/balances out/point-bedrock-preparation-FRESH
```

The replay itself makes no network requests:

```sh
CARGO_TARGET_DIR=out/point-bedrock-roles-FRESH/target-isolated \
  cargo run --release --offline --locked -p erc20-balances-tools --bin replay_role_candidates -- \
  erc20/balances out/point-bedrock-replay-FRESH --point-bedrock /absolute/original/erc20/balances
```

The output parent must already exist; the preparation/replay folder must be
fresh. [Bounded replay results](../../../docs/point-bedrock-role-candidates.md)
separate current/historical protobuf equality, canonical capture overlap,
initialized observed holders and cold unknown observations. Canonical values
never initialize or repair the ledger.

Fresh authorized runtime checks, actual replacement-package/RPC parity,
initialized-holder/final-state controls and real role-operation visibility
remain promotion gates. Other legacy cohorts and unresolved-source cases
remain outside this candidate. Issue #4 stays open.
