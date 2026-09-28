# Kgen/Deep: two NOT-QUALIFIED plain-role candidates

These separate profiles change Kgen root 10 and Deep's ERC-7201 role root from
legacy width 2 to exact `[bytes32,address]` membership, offset 0, width 1.
Deep additionally admits exactly three fixed admin words. No generic admin
path, parser extension, coupled enumerable support or holder seed is added.
Every unrelated baseline field and all runtime/pointer/deployment guards stay
unchanged. Historical 431 and qualified 425 fixtures remain unchanged.

## Complete captures

| Fixture | Address | Files | Original capture SHA-256 |
| --- | --- | ---: | --- |
| `kgen.json` | `0xf3d5b4c34ed623478cc5141861776e6cf7ae3a1e` | 55 | `e71ba802e1b6158d14ee38e47c793fb7ef025a484f9a3221a95a70a9bd0190df` |
| `deep.json` | `0x76ec4be0109305fb4543f1d1a8a515ac3bcabd60` | 66 | `3f2b8fa6fcf517c932f4793951a3aba6ff4567ef0c23c6dd989c33d0ba102a7e` |
| `proxy.json` | `0x9b6a1d4fa5d90e5f2d34130053978d14cd301d58` | 12 | `41fbd48e0c4d9dd498b6c3c5b6239e98caa2c9df1d1d3d575bb473f93a094c48` |

Raw responses come from the original package's `out/ranks101-150-source-review`
(Kgen) and `out/proxy400-source-review` (Deep implementation/proxy). The Rust
helper pins each raw digest before parsing, then checks chain 56, address,
original match labels, exact complete source/input/source-ID/metadata sets,
all source Keccaks, compiler settings, ABI/docs/layout and complete bytecode.
Original licenses and all captured source bytes remain intact.

## Independent source proof and explicit gaps

`primary-sources.json` keeps all 133 per-capture source entries and full fetched
bodies. **123 entries match direct upstream bytes**: 53 Kgen, 58 Deep and all
12 proxy files. They use immutable pins:

- OpenZeppelin contracts: `c64a1edb67b6e3f4a15cca8909c9482ad33a02b0`.
- OpenZeppelin upgradeable: `e725abddf1e01cf05ace496e950fc8e243cc7cab`.
- LayerZero devtools: `7af2d6e17c9ea84468dda803c380fc52c5548e68`.
- LayerZero v2 protocol: `9c741e7f9790639537b1710a203bcdfd73b0b9ac`.

Kgen's `IMessageLibManager.sol` is an additional exact **vendored snapshot**
in kgen-protocol/smartcontracts at `5fc4a4250ff7039e68ee06b83ac1376ed779546b`,
`KGeN-Token/BASE/deployments/bsc-mainnet/solcInputs/23ba6c1962a4e8042bae5691a051440d.json`.
The entire saved input has SHA-256
`5adf0df4ff9f8d5ffc81de036930455d23ce9ed7fc7904f4943844a799600c48`.
Only that interface is bound to this bundle; its other token source is different.
The pinned upstream interface differs in parameter names and is not an exact
upstream match. Deep's corresponding interface remains an explicit upstream
mismatch, not an inferred vendored match.

Kgen's captured token SHA-256 is
`146cc456e313a2f21776fa010ed08196423a52c0b3a0d372fe263316217d4995`.
The public `KGeN-Token/BSC/contracts/KgenOFT.sol` at the same commit has SHA-256
`bc7e2713fcf3beb380dd5c19259b10acc701c2114a489d0536cbad65a6fe7eb1`.
The whitespace mismatch remains explicit; no normalization or exact token pin
is claimed. Deep retains seven independent custom-source gaps: its token,
three control bases and three control interfaces. Its captured token SHA-256
is `4231e0e6246c5474093a383bf33c0deb29a3989bd0e2dfb0f8b624490ac8c0a0`.
Complete capture/runtime binding does not erase these primary-source gaps.

## Saved compiler/runtime/constructor bindings

| Capture | Recorded compiler | Runtime bytes / Keccak | Creation object + append |
| --- | --- | --- | --- |
| Kgen | 0.8.22+4fc1097e | 19,452 / `0xfc77b3c0763583345ead87010618488c5eb5f292e12c0857b94bd200761938cd` | 21,978 + 288 |
| Deep implementation | 0.8.28+7893614a | 18,925 / `0x65a4e8836b6ad79fd414fbbfe50ce30aa221ef90703d21e19327bb0f8c78340c` | 19,679 + 32 |
| Deep proxy | 0.8.28+7893614a | 1,166 / `0xd880cb6fa8a18faabeb57bb1cb2b1900371f7338c8c3c3f7add63574dad08e3c` | 3,714 + 288 |

All three use recorded optimizer 200, Paris and literal-content metadata.
The helper checks exact settings and original CBOR bytes, source maps and no
link references. It reconstructs saved compiler output; no fresh compilation
or deployed-code lookup is performed.

Kgen has endpoint immutable 1386 at eight sites, decimal conversion 2778 at
four sites and trusted forwarder 4703 at one site. Values are independently
constructed from the recorded endpoint, decimal conversion 100 and forwarder.
Its independent constructor ABI encoding is `(KGEN,KGEN,endpoint,delegate,forwarder)`.
Deep implementation has endpoint 1356 at eight sites and conversion 3194
at four sites (10^12); its append is the endpoint word. All zero placeholders,
declared sites/widths and full reconstructed bytes must match exactly.

Proxy immutable 621 at offset 16 is the padded ProxyAdmin address
`0xcc1c3381fde6bc19596f7f6119fc4a44c5a3d636`. Independent `RLP(proxy, nonce 1)`
reconstruction matches that value. This is a **matching nonce hypothesis**,
not proof of constructor execution history. The captured transparent proxy
invokes `ERC1967Proxy(_logic,_data)` first; the base can delegatecall the old
implementation before the later `new ProxyAdmin(initialOwner)`.

The independent proxy append encodes the **old implementation**
`0x563fe7f145f39e31cd7138193df6fe03f5b14253`, owner
`0x40e0400d7f4cb921635bf241b7ea0af4914d59be` and the exact opaque 132-byte
initializer. Its ABI container offsets, length and padding are checked, but
the payload is never interpreted as the later configured Deep implementation.
Whether the old initializer performed CREATEs, what it initialized and whether
holders existed remain unverified. The candidate guard still binds the later
`0x76ec…bd60` implementation. Matching captured creation is refused.

## Reachable role writes and separate metadata limits

Kgen's constructor grants memberships, with no `_setRoleAdmin` call outside
the inherited declaration. Public grant/revoke/renounce reach one inner bool.
Its `_msgSender` reads the trusted-forwarder set without mutating that set.
The reconstructed forwarder immutable alone does not establish the current
trusted-forwarder set or role-call authorization.
Its inherited ERC20 getter reads balance root 5, independently of roles.

Deep's unoverridden grant/revoke/renounce likewise update one inner bool and
emit a log; duplicates/absent revocations are no-ops. Its initializer sets
ADMIN_ROLE→ADMIN_ROLE, GUARDIAN_ROLE→ADMIN_ROLE and
BANLIST_OPERATOR_ROLE→ADMIN_ROLE. The role namespace is
`0x02dd7bc7dec4dceedda775e58dd541e08a116c6c53815c0bd028192f7b626800`.
Only `keccak(keccak(roleName) || roleRoot) + 1` for those three roles is added:

| Role | Exact admin word |
| --- | --- |
| ADMIN_ROLE | `0xb16e88c42fd4e48df2dd6a2eabd6bc9aec654ec170056b470819f8892cc6431d` |
| GUARDIAN_ROLE | `0x18476f5b3d6d00091ddd56161ac5e9ba807d29b59f48f8df98938ee352a7cf24` |
| BANLIST_OPERATOR_ROLE | `0xcc685bd430dd10cd13db9f52a68d39fc6e8fa7409ed69c8c1ed8aecaff3c8832` |

These are location permissions, not value/boolean, authorization or initializer
context checks. Arbitrary role admins, outer anchors and membership+1 refuse.
Deep's balance getter uses the unchanged ERC20 namespace; role calls have no
metadata hooks. Its full initializer also touches ERC20/EIP712/owner/initializer
state and calls the endpoint; this candidate is not full initialization or
bridge qualification.

Kgen forwarder-array elements at `keccak(15)+i` remain unsupported, even though
the original length/index permissions remain. Long enforced-options bytes
remain unsupported for both profiles. Deep's source-correct options namespace
is `0x8d2bda5d9f6ffb5796910376005392955773acee5548d0fcdb10e7c264ea0000`.
Those separate operations are not coupled to ordinary role writes, so their
remaining limitations do not erase this narrow role candidate's usefulness.

Ten projector groups cover per-profile grants/revokes/no-ops, simultaneous
balances, fixed admins, exact baseline restoration, adversarial depth/key/order/
padding/preimages, malformed 33-byte words, all old scalars/mappings, independent
metadata refusals, persistence and proxy/runtime/creation guards. Seven helper
groups mutate capture/source/settings/layout/immutables/runtime/constructor/
primary proof/candidate bindings. They test synthetic Extended shapes, not
executed role transactions or actual producer visibility.

## Reproduction

```sh
CARGO_TARGET_DIR=out/oft-role-FRESH/target-isolated \
  cargo run --offline --locked -p erc20-balances-tools --bin prepare_oft_roles -- \
  /absolute/original/erc20/balances out/oft-source-FRESH
CARGO_TARGET_DIR=out/oft-role-FRESH/target-isolated \
  cargo run --release --offline --locked -p erc20-balances-tools --bin replay_role_candidates -- \
  erc20/balances out/oft-replay-FRESH --oft /absolute/original/erc20/balances
```

Output directories must be fresh with existing parents. Preparation preserves
its as-run Rust helper/CLI and hashes, original captures, full vendored input,
all fetched immutable primary bodies, review and candidate. Only preparation
requests public source bytes; replay is wholly offline. See the
[saved-window report and remaining gates](../../../docs/oft-role-candidates.md).
