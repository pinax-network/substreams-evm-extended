# SecuritiesToken operation proof (Phase A)

This host-only proof concerns the selected BSC implementation
`0xcfed6c4679297ea4889f8183bc057b4a86c64e46`. It adds no ingestion rule, schema,
candidate layout, dependency, persistence rule or protobuf change. The thirteen
separate boolean-role candidates remain unchanged. Compiler regeneration and
the operation matrix pass on the current source: **831 compiled calls**, with
711 returns, 112 exact expected reverts and eight explicit unsupported-path
controls. There are no unexpected `INVALID` or harness failures.

## Recorded execution

The [compiler report](evidence/securities-operation-proof-20260928-compiler.json)
comes from `out/securities-operation-proof-20260928/source-02`; the
[operation report](evidence/securities-operation-proof-20260928.json) comes from
`operations-01`. The [831-case inventory](evidence/securities-operation-proof-20260928-cases.json)
binds each complete saved trace. The
[84 compact transcripts](evidence/securities-operation-proof-20260928-transcripts.json)
retain operation/failure effects and source annotations; getter traces remain
in the full case artifacts. The
[174-file source inventory](evidence/securities-operation-proof-20260928-source-inputs.json)
has SHA256 `5a077b927b4596a8f4da41a6d3390d0a94292a443a602def0fbd2bf2fab052d0`
and includes the as-run tools, tests, actual local host dependencies and four
central documentation files. No source, input or runtime byte changed between
successful compilation and execution.

All tested account state is synthetic. There is no block interval, observed
holder coverage, new canonical replay or claim that a producer emitted these
role writes. Successful local execution and refusal of unsupported paths are
counted separately.

## Source and compiler boundary

The complete original capture has SHA256
`fd44489fbc3e55ef5669b0a90246f32cd8b248079aa1cd58c5af2288a27daef2`.
All 31 literal sources, source IDs, metadata hashes, ABI, layout and saved
compiler objects are checked. Official `solc 0.8.24+commit.e11b9ed9` is bound to
the immutable `ethereum/solc-bin@16a99b8c26ed33a91796e209ff6797ee7baf2b0d`
macOS release manifest. Its binary SHA256 is
`cc2d44c706905ccc382f484625dff61d741e0c24232d226f139a6835fc644f3f`.
Original Cancun, optimizer-enabled/200, via-IR false, IPFS metadata settings and
all six remappings are preserved. Only `outputSelection` is added to obtain
complete compiler artifacts and generated Yul source 31.

The complete fresh compiler output is pinned to SHA256
`0b44f0b9acc0d1f33aa3bc5af37e6d12bf64ed46909568ae73b4fd4d3cf4b739`.
The saved metadata contains literal Unicode while official solc emits Unicode
escapes. Both exact raw strings remain preserved and separately pinned:
saved `44f85de3053c3bcbe52f78c3cbd7ed520c5ec965fd74d4ab642d88894848f3b6`,
fresh `c49c374e44cc250b2d11f362d3b48c5687ba46114d8fd2d2374d91b01189db9b`.
Their complete parsed metadata is equal; raw strings are not claimed equal.
No source or metadata normalization is used.

The entire saved runtime is 10,836 bytes, including its unchanged 53-byte CBOR
at offset 10,783. Its Keccak is
`0x060dc28d4dd8d9bb8a381d4009bccbac129ce743a10b3d8aa0ffef5034b50544`.
There are no links, immutable substitutions or runtime transformations.

**On-chain creation bytes, `creationMatch`, deployer, transaction, block and
transaction index are all null.** Fresh equality with the saved 11,063-byte
compiler creation object supports only a synthetic argument-free constructor
run. It cannot establish deployed constructor equality, initialization history
or a deployment boundary. The original `exact_match` runtime labels and null
creation label remain intact.

## Primary sources and preserved gaps

Twenty literal dependency bodies match immutable OpenZeppelin v5.3.0 revisions:
14 at `OpenZeppelin/openzeppelin-contracts@e4f70216d759d8e6a64144a9e1f7bbeed78e7079`
and six at
`OpenZeppelin/openzeppelin-contracts-upgradeable@60b305a8f3ff0c7688f02ac470417b6bbf1c4d27`.
The remaining eleven sources are classified individually, without normalization:

| Captured custom source | Independent primary status |
| --- | --- |
| `ERC8056/IScaledUIAmountBalances.sol` | Exact at the BEP pin below |
| `ERC8056/IScaledUIAmountConversion.sol` | Exact at the BEP pin below |
| `ERC8056/IScaledUIAmountNewUIMultiplier.sol` | Exact at the BEP pin below |
| `ERC8056/ERC8056BaseUpgradeable.sol` | Preserved differing public body; not exact |
| `ERC8056/IERC8056Scheduled.sol` | Preserved differing public body; not exact |
| `ERC8056/IScaledUIAmount.sol` | Preserved differing public body; not exact |
| `SecuritiesToken.sol` | No recovered exact public revision |
| `compliance/ComplianceClientUpgradeable.sol` | No recovered exact public revision |
| `compliance/ICompliance.sol` | No recovered exact public revision |
| `pauseManager/PauseManagerClientUpgradeable.sol` | No recovered exact public revision |
| `pauseManager/IPauseManager.sol` | No recovered exact public revision |

The six BEP counterparts use
[`bnb-chain/bep-677-contracts@ff17399a4723e2e344f7d4c52a3eecf2ca007f6b`](https://github.com/bnb-chain/bep-677-contracts/tree/ff17399a4723e2e344f7d4c52a3eecf2ca007f6b/contracts).
The fixture retains each exact URL, captured hash, fetched hash and complete
fetched body. Thus 23/31 files have exact independent matches and eight retain
a primary-source gap. A bounded prior search compared the three differing files
at twelve available BEP history commits without finding an exact body; it does
not prove that no public source exists. The five token/client sources retain
their BUSL-1.1 notices; all other captured notices and upstream license files
are preserved. No public token body replaces the captured token.

## Local operation scope

The selected namespaced role membership root is
`0x02dd7bc7dec4dceedda775e58dd541e08a116c6c53815c0bd028192f7b626800`;
the enumerable root is
`0xc1f6fe24621ce81ec5827caf0253cadb74709b061630e6b55e82371705932000`.
Grant/revoke update enumeration only when the parent boolean changes. Public
grant/revoke accept full-width roles under their configured admin; renounce
requires matching caller confirmation. No public arbitrary role-admin setter
or clear operation exists in the selected token. Initializer admin writes are
outside this proof's successful local operation scope.

For the exact selected compiler/runtime, let `M` be the member boolean, `L` the
array length, `A[i]` an array element and `I(member)` its one-based position.
The measured successful stores are:

| Operation | Ordered stores (the role event follows the first store) |
| --- | --- |
| Add | `M=1`, `L=L+1`, `A[old L]=member`, `I(member)=new L` |
| Tail remove | `M=0`, `A[old L-1]=0`, `L=L-1`, `I(member)=0` |
| Non-tail remove | `M=0`, `A[index]=tail`, `I(tail)=index+1`, `A[old L-1]=0`, `L=L-1`, `I(member)=0` |
| Duplicate grant / absent revoke | No stores or logs |

Zero-address cases include executed equal array stores; tail removal skips the
self-swap. This order is evidence for this selected build, not a universal
OpenZeppelin 5.3 semantics promise. Full-width role/admin values, zero members,
all removal positions, two-role repeated sequences, authorization failures and
malformed ABI controls have exact state/log/getter assertions. Raw getters cover
zero, one and uint256 max and perform the expected single storage read.

Deliberately incoherent boolean/index states demonstrate the source's parent
gate; they must not be interpreted as admissible states. Empty/out-of-bounds
sets revert after a boolean store and role log, retaining those attempted
effects while rolling back committed state/logs. A maximum-length push wraps
outside the coherent domain; a future checked-growth admission rule would be a
conservative restriction, not a claim that this source reverts on that push.

The matrix uses explicitly synthetic coherent role membership, arrays and
one-based positions. Raw ERC20 balances/allowances and total supply are coherent
and checked independently; every operation compares the complete storage state,
ordered stores, exact logs and getter readback. Deliberately malformed states
are separate source behavior controls, not qualified/admitted states. The
bounded executor preserves attempted effects on reverts and harness failures
while committing neither writes nor logs.

Raw `balanceOf`, `allowance` and `totalSupply` use the ERC20 namespace
`0x52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace00`.
`balanceOfUI` and scheduled conversions use a different, timed interface.
Successful initializer, external compliance/pause validation, transfers,
mint/burn, UI conversions and beacon/proxy dispatch are excluded. Reaching an
unsupported clock, external account or call path must produce `HarnessFailure`,
never a fabricated successful client response or a Solidity revert.
The excluded initializer performs seven attempted stores before the unsupported
timestamp instruction, and all are rolled back. Client setters stop at unknown
external-account code-size checks. Transfer/mint/burn stop at `GAS` at PC7368,
immediately before `STATICCALL`; that external call is never executed. UI getter
and schedule controls stop at `TIMESTAMP`.

The eight source-binding tests and seven independent matrix tests include
complete input/metadata/primary tampering, fabricated creation refusal, raw
metadata changes with equal parsed content, exact rollback and altered
`SSTORE`/`LOG4` runtime controls. The failed bootstrap `source-01`, the strict
metadata-string failure in `focused-01.log`, and the initial pre-call opcode
expectation failure in `cases-01.log` remain preserved. These failures were not
relabeled as successful proofs.

## Remaining qualification

The seventeen historical profiles share a saved proxy/beacon/implementation
configuration; this single-account implementation proof does not execute that
proxy dispatch or establish all seventeen deployed histories. Proxy runtime,
beacon runtime, both pointer guards, initialization and producer visibility
need independent qualification. Custom name/symbol/identifier setters can write
dynamic string payloads; preserving their scalar roots would not admit those
payloads. No role cache, PToken semantics label, independent membership bypass,
blanket OZ5 rule or canonical saved-block replay is introduced here. A future
candidate needs a separately reviewed complete operation rule and fresh bounded
evidence before any separately authorized live/package qualification.
