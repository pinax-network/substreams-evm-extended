# One unqualified BurnMint role-path candidate

This candidate replaces only the root-5 legacy width-two role rule for
`0xac23b90a79504865d52b49b327328411a23d4db2` with
`[bytes32, address]`, terminal offset 0, one word. The bound runtime remains
`0x688f1e2193eea752e7eaa2acc9607ede95429383234670d6d84c0ee3336eb3d0`.
Every other profile field is identical to the original historical 431-profile
fixture. Neither that fixture nor the qualified 425 cohort is replaced.

## Saved and primary source bindings

`source-capture.json` is the complete, byte-identical cached response at
`out/ranks201-250-source-review/0xac23b90a79504865d52b49b327328411a23d4db2.json`,
SHA-256 `8c2bf80a6aef279bc19fbb13885c1067247b9fd8b2d6cf3a0e0645b7bd4fd489`.
The verifier requires chain 56, the exact contract, all 14 source files and
their metadata Keccak hashes, complete saved storage layout, and the exact
runtime reconstruction. This rechecks a saved compiler capture; it does not
independently recompile Solidity or establish a current on-chain code hash.

| Immutable AST id | Meaning | Offsets | Replacement |
| --- | --- | --- | --- |
| 54 | `i_decimals` | 3213 | `0x0000000000000000000000000000000000000000000000000000000000000012` |
| 57 | `i_maxSupply` | 439, 2357 | `0x0000000000000000000000000000000000000000204fce5e3e25026110000000` |

Each replacement is exactly 32 bytes; no linked libraries or CBOR replacement
is allowed. Reconstructed bytes must equal the entire saved runtime and its
Keccak hash, not just a selected instruction range.

`primary-sources.json` retains fresh primary GitHub reads at
`smartcontractkit/chainlink@e6287bd5ec0cb86925584b050b6aed0fa9b1b2e8`.
Thirteen files match the cached source byte for byte. The sole token-source
difference is this import path:

```text
primary:  ../../../ccip/interfaces/IGetCCIPAdmin.sol
captured: ../../../shared/interfaces/IGetCCIPAdmin.sol
```

The interface itself matches byte for byte at its primary `ccip/interfaces`
location. The verifier permits exactly that single string replacement in
BurnMintERC20 and no changes in any other source. This is a documented source
relocation, not a claim that all original file paths and bytes match the
primary checkout unchanged. Every primary file retains its exact URL and
SHA-256 digest.

## Getter invariance and reachable role writes

The concrete token inherits ordinary OpenZeppelin ERC20, ERC20Burnable and
AccessControl 4.8.3, without AccessControlEnumerable or a `balanceOf` override.
The inherited getter directly returns `_balances[account]` at root 0.

| Slot | Saved compiler field | Treatment |
| --- | --- | --- |
| 0 | `_balances: mapping(address => uint256)` | Existing balance extraction |
| 1 | nested `_allowances` mapping | Existing allowance rule unchanged |
| 2, 3, 4 | `_totalSupply`, `_name`, `_symbol` | Existing scalar rules unchanged |
| 5 | `_roles: mapping(bytes32 => RoleData)` | Exact membership path only |
| 6 | `s_ccipAdmin` | Existing scalar rule unchanged |

Within `RoleData`, `members: mapping(address => bool)` is at offset 0;
`adminRole: bytes32` is at offset 1. The complete source has only the internal
`_setRoleAdmin` declaration, with no callsite. Constructor initialization
grants DEFAULT_ADMIN_ROLE membership, and the public `grantRole`, `revokeRole`
and `renounceRole` functions reach membership writes. The convenience
`grantMintAndBurnRoles` calls the ordinary authorized `grantRole` twice.
`setCCIPAdmin` writes scalar slot 6, which is distinct from role admin storage.
There is no enumerable array/index state to accept.

Role metadata does not affect the current getter's returned amount. Roles
can authorize future mint/burn operations; those operations still write the
existing root-0 balance mapping. Admitting metadata never bypasses balance
extraction or the runtime-change guard.

The tests cover grant/revoke, default and arbitrary role keys, duplicate/no-op
shapes, failure/revert filtering, concurrent balance writes, malformed word
widths, and genuine code-change records. They reject outer admin writes,
adjacent membership words, extra depths, wrong roots/order, address padding,
and missing/corrupt preimages. Source/candidate tampering is also refused.
These are source-derived Extended-record shape tests, not executed role
transactions. Existing ordinary no-op filtering and metadata value handling
remain unchanged; the typed rule constrains key shape, not boolean values.

## Reproduction and qualification

From the repository root, prepare a fresh source-review output:

```sh
cargo run --offline --locked -p erc20-balances-tools --bin prepare_burnmint_role -- \
  /absolute/original/erc20/balances out/burnmint-preparation-FRESH
```

That tool reads the pinned cached response and only fetches pinned public
GitHub source files. The native replay performs no network requests:

```sh
cargo run --release --offline --locked -p erc20-balances-tools --bin replay_role_candidates -- \
  erc20/balances out/burnmint-replay-FRESH --burnmint /absolute/original/erc20/balances
```

Replay covers `[122288006, 122289030)` with exact saved clocks, full protobuf
parity against current/historical baseline, canonical RPC-capture overlap,
and the shared observed-holder ledger. Failure preserves partial output and
a failed report; no missing block is replaced with an empty event list.
No reference value initializes or repairs retained state.

Live runtime controls, actual package output/RPC comparison, initialized
holder/final-state qualification, and producer visibility of actual role
operations remain independent requirements. Issue #4 stays open.
