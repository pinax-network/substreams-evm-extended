# BTR: bounded host operation proof (Phase A)

This is a **NOT-QUALIFIED host source execution proof** for BTR implementation
`0xc8b5a0c5453c15157328b6cc1f1452be032a41f1`, configured behind proxy
`0xfed13d0c40790220fbde712987079eda1ed75c51`. It adds no layout candidate,
production ingestion rule, protobuf, dependency version or chain request.
Implementation bytecode runs against explicitly constructed local accounts;
the proof does not execute the proxy's delegatecall dispatcher.

## Captures, primary sources and compilation

The implementation's complete original capture has SHA-256
`b45110e73361a6bdaf50181ec265b0c9cc00cab6dbafbe1c76f1262a57139b21`;
the proxy capture has SHA-256
`9c9fcd09b032a0ac632041217f5ce4000a20492c555e30e5ca5896fb14b6b320`.
All 31 implementation and eight proxy source bodies bind to the captured input,
source IDs, metadata Keccaks, ABI, storage layout and compiler output. The
original `match`, `runtimeMatch` and `creationMatch` labels are `exact_match`.

Thirty implementation dependencies independently match
[OpenZeppelin Upgradeable v4.9.3](https://github.com/OpenZeppelin/openzeppelin-contracts-upgradeable/tree/3d4c0d5741b131c231e558d7a6213392ab3672a5).
Eight proxy dependencies match
[OpenZeppelin v5.2.0](https://github.com/OpenZeppelin/openzeppelin-contracts/tree/acd4ff74de833399287ed6b31b4debf6b2b35527).
**The exact custom `contracts/BTRToken.sol` primary repository revision remains
unresolved.** Its captured source has SHA-256
`f6b70c8e72aa4519db417da71e4b827e216ddf5f3eaa2867d428d8db026c42aa`
and SPDX `UNLICENSED`. The independently found official
[public BTR.sol](https://github.com/bitlayer-org/bitlayer-contracts/blob/079b0082af96ce14ea1d7b45e73a92d2466ac667/contracts/basic/BTR.sol)
is a different, non-upgradeable distribution
token. Dependency equality and fresh bytecode reconstruction do not close this
custom-source gap. The captured notices and both upstream MIT licenses are
retained without rewriting source bodies.

Fresh compilation uses official solc `0.8.24+commit.e11b9ed9`, whose macOS amd64
SHA-256 is
`cc2d44c706905ccc382f484625dff61d741e0c24232d226f139a6835fc644f3f`.
The binary's SHA-256 and Keccak are checked against the immutable
[official release manifest](https://github.com/ethereum/solc-bin/blob/16a99b8c26ed33a91796e209ff6797ee7baf2b0d/macosx-amd64/list.json)
before execution. Both original inputs keep Paris, optimizer enabled with 1,000
runs, IPFS metadata, empty libraries and remappings, and no via-IR setting.
Only `outputSelection` is added for complete compiler evidence. Removing it
must recover the original input exactly.

The implementation has a 15,308-byte runtime with Keccak
`0x44d6248ac6cfc67512326547ee54cf5e5d5a5573d4d65a8e94b29cb2f8e42949`.
The five 32-byte UUPS `__self` sites, immutable ID `1168` at byte offsets
3,275, 3,408, 3,939, 4,072 and 4,302, are bound to the implementation address.
Patching exactly those zero placeholders must reproduce the entire saved
runtime, including metadata. The 15,595-byte creation code has no constructor
append. No links or metadata rewriting are permitted.

The proxy's 183-byte runtime has no immutables and Keccak
`0xbd576604a6f2b4dac58ff676c3912bd2ef37d52f93640e2d20c8d47de43d3418`.
Its 1,047-byte creation code plus an independently encoded 416-byte constructor
append must reproduce all 1,463 saved bytes. That append selects the captured
implementation and a 292-byte `initialize(string,string,address,address,uint256)`
payload: name `BTR token`, symbol `BTR`, owner and pauser
`0x27e939c9a85afd8d643eeda0bdecb193683bcda5`, and quota
`500000000000000000000000000`. This proves the saved ABI binding; it does not
execute proxy construction or establish deployed initialization state.

## Role and whitelist semantics under test

The selected layout has boolean membership/admin root 101, enumerable role-set
root 151, balances 201, allowances 202, nonce mapping 303, pause state 404,
mint quota 554, and a separate whitelist `AddressSet` at roots 555/556.
The complete original storage layout remains unchanged.

Unlike the separately proved OZ 5 PToken path, this OZ 4 enumerable extension
always invokes `add` or `remove` after the void parent role method. The parent
conditionally changes the membership boolean and emits its role event; the set
independently tests its one-based index. The four deliberately incoherent
boolean/index controls therefore remain distinct from coherent-set operations.
No universal OZ 4 or PToken ingestion permission follows from them.

`setMinter` and `setBurner` require default-admin membership. `setWhitelister`
requires `PAUSER_ROLE`, despite the source comment referring to an owner. These
wrappers emit their own event even when the role or set operation is a no-op.
Only initialization sets an admin relation, making `PAUSER_ROLE` self-admin.
Ordinary whitelist calls operate on the direct set rather than a coupled role
boolean. Zero-address members are legal in both sets.

The strengthened matrix passed **910 constructor/ABI calls**: 836 returns and 74
reverts with exact expected payloads. No INVALID or harness failure is accepted
as source agreement. It covers empty/nonempty adds, duplicates, absent removals,
first/middle/tail/sole removals, zero at every array position, grant/revoke/
renounce and wrapper routes, sequential changes, distinct owner/pauser
authorization, malformed ABI, getters and bounds, balance/supply/allowance/nonce
preservation, pause behavior, whitelisted transfers and mint/burn exemptions.

Measured coherent-set effects are:

| Operation | Ordered stores and logs |
| --- | --- |
| Role add | Boolean `0→1`; `RoleGranted`; length increment; append element; one-based position |
| Role non-tail removal | Boolean `1→0`; `RoleRevoked`; copy tail into removed position; update moved position; clear tail; decrement length; delete removed position |
| Role tail/sole removal | Boolean `1→0`; `RoleRevoked`; clear tail; decrement length; delete position; no self-swap stores |
| Role duplicate/absent | No stores or role logs; authorization, boolean and set-position reads still occur |
| Direct whitelist mutation | The same selected set stores without a role boolean/log, followed by `WhitelistedSet` |
| Wrapper duplicate/absent | No stores, but the wrapper's event still occurs |

Physical equal stores, including zero-address append and zero tail clear, remain
in the exact ordered transcript. Deliberately inconsistent boolean/index states
demonstrate set-only grant, boolean-only grant, boolean-only revoke and set-only
revoke. Empty or out-of-range sets preserve the exact attempted boolean clear
and `RoleRevoked` before `Panic(0x11)` or `Panic(0x32)` rolls back all effects.
A maximum-length push wraps in this selected compiler; its incoherent result
is recorded without admitting it as a valid initialized set.

Implementation construction returns the exact runtime and locks initialization
with one slot-0 `0→255` store and `Initialized(255)`. Synthetic proxy-context
initialization, including valid zero quota, checks all 21 stores and five logs.
Those stores include packed flag `0→1→257`, parent metadata, physical zero
stores, both role sets, PAUSER admin, pause, quota, then flag `257→1`. Zero
owner/pauser refuses after exactly nine parent stores and no logs; excessive
quota refuses after 19 stores and four logs, before quota storage or final
`Initialized`. Exact interleaving, full event topics/data and rollback are
asserted, not just final state or counts. Repeated proxy initialization and
locked implementation initialization fail before effects.

## Explicit account context and limits

The existing bounded host VM gains only optional self `EXTCODESIZE` context.
Constructor execution supplies installed self size zero, direct implementation
execution supplies 15,308, and implementation execution at the synthetic proxy
address supplies 183. Executed bytes and installed self code size are different
inputs. This follows the distinction between
[`EXTCODESIZE` and `CODESIZE` in geth](https://github.com/ethereum/go-ethereum/blob/36b2371c59cd91a9b1da062b3e382f05a6d8687e/core/vm/instructions.go#L341):
the former queries the account selected by the low 160 operand bits, while the
latter measures the current execution bytes. The referenced file's SHA-256 is
`ce0db58b41368a18df9b31e40f8f2bbc796ea627dccbb21fd5c6f9ccae3a0610`.

Missing self context, a foreign-account code-size query, and unsupported opcodes
are harness failures. Existing PToken entrypoints supply no new context and keep
their previous behavior. The VM retains its instruction, stack, memory and
witness bounds; only RETURN commits state and logs. Reverts, INVALID and harness
failures retain attempted effects but restore the prestate. No external calls,
UUPS upgrade execution, permit/ecrecover, gas/refund model or fork-cost model are
provided. Unknown external accounts are not assigned a guessed zero code size.

All prestate is synthetic, including empty cells. Coherent role membership,
array contents, one-based positions and unused-tail zeros are explicit initial
conditions; deliberately broken states test source behavior rather than
qualifying real accounts. Deployment/runtime qualification, existing-chain
initialization, call-frame visibility, ingestion validation and canonical saved
replay remain separate work. **Issue #4 remains open.**

The historical profile's broad root-101 role declaration and separate whitelist
length-555/index-556 permissions remain untouched. That profile has no explicit
whitelist array-data operation rule. A future coupled-role candidate must assess
those separate permissions deliberately; changing role handling alone cannot
claim whitelist-operation admission from this host proof.

## Evidence and reproduction

The final [compiler report](evidence/btr-operation-proof-20260928-compiler.json)
comes from `out/btr-operation-proof-20260928/source-02/`; the
[operation report](evidence/btr-operation-proof-20260928.json) comes from
`out/btr-operation-proof-20260928/operations-03/`. Both use the same frozen
[168-file source inventory](evidence/btr-operation-proof-20260928-source-inputs.json),
including the four central status documents. Both complete compiler output
digests remain identical to the original successful compilation, and all 13
committed source/compiler fixture artifacts match the final source directory.
The [case inventory](evidence/btr-operation-proof-20260928-cases.json) binds all
910 complete annotated traces, and the
[compact transcripts](evidence/btr-operation-proof-20260928-transcripts.json)
retain mutation/refusal effects and source spans. Full PC/stack traces remain
in the local attempt directory. These are synthetic call counts, not chain
holder coverage.
Each attempt uses a fresh directory under `out/btr-operation-proof-20260928/`.
The tools preserve original and augmented compiler inputs, all compiler outputs,
as-run Rust sources, local source inventories, raw failed case records before
expectations, complete PC/stack traces, attempted reads/stores/logs/Keccak inputs,
source-map annotations, and hashes. Initial build failures remain preserved.
`source-01` has the initial matrix skeleton; `operations-01` precedes the exact
prefix strengthening and `operations-02` passes the strengthened 910-call
matrix. Final `operations-03` repeats those exact call results and effect
artifacts using the final source capture. A deliberately broken local program
with expected stores but no role
event reproduced an indexing panic (`07-missing-log-red.log`); validation now
returns a reportable error after saving that attempt. This robustness program
is separate from the 910 source calls. The intermediate syntax failure in
`08-missing-log-green.log` is also preserved and is not a green result.

```sh
cargo +1.88 run --locked --offline -p erc20-balances-tools --bin build_btr_proof -- \
  IMPLEMENTATION_CAPTURE PROXY_CAPTURE OFFICIAL_MACOS_SOLC FRESH_SOURCE_DIRECTORY
cargo +1.88 run --locked --offline -p erc20-balances-tools --bin execute_btr_proof -- \
  FRESH_SOURCE_DIRECTORY FRESH_OPERATION_DIRECTORY
cargo +1.88 test --locked --offline -p erc20-balances-tools btr --lib --tests
```

The builder reads immutable public compiler/source URLs only. The executor and
fixture tests need no network. Cargo uses the pinned toolchain and lockfile;
all new tooling and dependencies remain outside WASM ingestion.
