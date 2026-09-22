# Two offline role-path candidates

`layouts.json` contains two **unqualified candidate replacements**, separate
from the published 431-profile `bsc-refined450-layouts.json`. It removes only
the role root's legacy width-two rule, replacing it with one exact
`[bytes32, address]`, offset-zero, width-one membership path. Every other
profile field, including runtime hashes and balance roots, is unchanged.

| Contract | Source contract | Role root |
| --- | --- | --- |
| `0x255e746abb8d9acac00d6d023e5e63e3b8dfa7cd` | `project/contracts/Token.sol:Token` | 8 |
| `0x0c69199c1562233640e0db5ce2c399a88eb507c7` | `src/CYS.sol:CYS` | 6 |

`source-review.json` preserves the complete source text, original per-file
Keccak hashes, compiler metadata, storage layout and runtime-bytecode
bindings from the existing captured source responses. The responses' SHA-256
hashes remain bound to the historical role-shape audit. Token's seven
immutable replacements must reconstruct the exact bound runtime; CYS's
compiled and bound runtime bytes match directly. This is a recheck of saved
compiler/source evidence, not a fresh Solidity compilation or live code
check.

Both complete source bundles contain the internal `_setRoleAdmin`
declaration and no callsite. Both use `mapping(bytes32 => RoleData)` with
`mapping(address => bool)` at member offset zero and `adminRole` at offset
one. The candidates admit membership grant/revoke shapes, and reject admin
writes, adjacent words, malformed address padding, missing preimages and
wrong mapping depths. These shape tests do not assert that every synthetic
write is externally reachable.

The host-only `role_path_candidates` integration tests verify the saved
source hashes and exact config delta. The `replay_role_candidates` host
binary rechecks original local capture bindings, replays the entire saved
BSC interval `[122288006, 122289030)`, and compares candidate output with
both the unchanged current baseline and preserved historical protobuf
output. It also compares with the immutable canonical RPC capture, using
`evm-retention` to keep uninitialized holders unknown.

Run it with a fresh output directory:

```sh
cargo run --release --offline --locked -p erc20-balances-tools \
  --bin replay_role_candidates -- erc20/balances \
  erc20/balances/out/role-path-candidates-replay-new
```

The full 431-profile combined candidate config exists only in that output
directory. Source/runtime changes, new package bytes and live stream,
checkpoint, holder, RPC and sink qualification remain separate gates.
