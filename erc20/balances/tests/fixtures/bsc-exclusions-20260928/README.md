# Preserved BSC exclusions and unqualified TAKE candidate

These fixtures preserve the three September 23 original metadata refusals.
`cases.json` records the original full-block hash/SHA-256, trimmed fixture
SHA-256, transaction hash/index, original profile/refusal, call writes, and the
derived TOPS array path. Each `.pb` keeps **one entire decoded original transaction**
and the original block header. System calls and block effects are omitted;
these are narrow captured fixtures, not complete blocks or synthetic traces.
The generator preserves these frozen fixture bytes and verifies decoded equality
against the full source block's transaction trim, avoiding nondeterministic
protobuf-map wire ordering on subsequent runs.

| Fixture | Original refusal |
| --- | --- |
| `take-123561001-tx66.pb` | ERC-7201 reentrancy guard scalar |
| `radr-123561119-tx53.pb` | Unreviewed mapping root 21 |
| `tops-123561227-tx63.pb` | `lpInfos` mapping root 31, dynamic array element 4, field 0 |

`take-candidate-NOT-QUALIFIED.json` contains exactly one original profile with
one additional `other_slots` entry. The historical 431-profile fixture remains
byte-for-byte unchanged. Do not replace a qualified profile or package with this
candidate based on these tests.

`take-source.json` and `tops-source.json` retain all saved compiler-input source
contents/hashes, compiler settings, saved compiled/on-chain runtime bytes, and
declared immutable transformations. The generator validates captured chain id
56 and contract/implementation address before reading source semantics. Tests
bind this retained provenance and reconstruct the recorded runtime;
they do not compile Solidity or fetch current bytecode. Independent TAKE source
pin: `overtake-dev/take-token-repo@7cfcaf7a2429a83612b92dc5fa2f42b551b600e9
bsc/src/TokenImpl.sol`; OpenZeppelin upgradeable submodule pin:
`OpenZeppelin/openzeppelin-contracts-upgradeable@60b305a8f3ff0c7688f02ac470417b6bbf1c4d27`.
The exact independent TokenImpl, ERC20Upgradeable and ReentrancyGuardUpgradeable
content hashes are asserted in the test. TOPS has no independently established
maintainer pin and remains excluded.

See the [review](../../../docs/bsc-exclusions-offline-2026-09-28.md) for source
reachability, complete replay scope, unresolved cases and live gates. Adversarial
test mutations are synthetic; only the original `.pb` contents are captured.
