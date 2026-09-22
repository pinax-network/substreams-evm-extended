# lido/balance-state

A single RPC-free `map_events` reads Firehose Extended blocks and emits
**`evm.balance_state.v1.Events`** ([contract](../../docs/balance-state-contract.md))
for one explicitly qualified Lido stETH contract-version epoch. It does not
emit `evm.balances.v1`. Three metrics stay distinct:

| Metric | What it is | Rows |
| --- | --- | --- |
| shares | `shares[holder]`, the retained basis | `HolderBasis` SHARES |
| stETH `balanceOf` | `shares × internalEther / internalShares` (version 4 getter) | `GlobalState` packed words |
| redemption / wstETH | separate models (withdrawal-queue NFTs, wstETH units) | not emitted here |

The consumer evaluates with [`conformance::lido`](../../conformance/src/lido.rs)
at a canonical block clock. A `TokenRebased` report changes every holder's
balance without any holder write; the map emits the report's global words and
the log evidence, never fabricated holder rows. A holder without a row is
unknown, not zero.

## What is emitted

| Table | Row | Source |
| --- | --- | --- |
| `HolderBasis` (`SHARES`) | `shares[holder]` before the first and after the last write of the block | stETH proxy storage, verified Keccak preimage `(holder, shares_slot)`; delegate-call context is the proxy address |
| `GlobalState` `LIDO_TOTAL_SHARES` / `LIDO_EXTERNAL_SHARES` | low / high 128 bits of `keccak256("lido.StETH.totalAndExternalShares")`, one row per half of every written word | stETH storage |
| `GlobalState` `LIDO_BUFFERED_ETHER` / `LIDO_DEPOSITED_POST_REPORT` | low / high 128 bits of `keccak256("lido.Lido.bufferedEtherAndDepositedPostReport")`, one row per half of every written word | stETH storage |
| `GlobalState` `LIDO_CL_VALIDATORS_BALANCE` / `LIDO_CL_PENDING_BALANCE` | low / high 128 bits of `keccak256("lido.Lido.clValidatorsBalanceAndClPendingBalance")`, written by `processClStateUpdate` on each oracle report | stETH storage |
| `GlobalState` `LIDO_TOTAL_POOLED_ETHER` (`DERIVED`) | `internalEther + externalShares × internalEther / internalShares`, only when all three words were written in the block and `internalShares > 0` | pure function of the rows above |
| `GlobalState` `LIDO_CONTRACT_VERSION` | observed write of `keccak256("lido.Versioned.contractVersion")`, and the qualified value as a constant at BOUND / REAFFIRMED | stETH storage, parameters |
| `GlobalState` `LIDO_REPORT_*` (`OBSERVED_LOG`) | `TokenRebased` `reportTimestamp`, `postTotalShares`, `postTotalEther`, `sharesMintedAsFees` from receipts of succeeded transactions | stETH logs |
| `ModelEpoch` INVALIDATED | each contract-version transition to an unqualified value (`CONTRACT_VERSION_SET`); each Aragon resolution-pointer transition (`IMPLEMENTATION_POINTER_WRITE` / `DEPENDENCY_POINTER_WRITE`); proxy or dependency code changes (`CODE_CHANGE` / `DEPENDENCY_CODE_CHANGE`), with evidence | persisted writes and code changes |
| `ModelEpoch` + `Dependency` | binding rows at the activation block and on the heartbeat: Kernel, stETH implementation and Kernel implementation via storage pointers; Accounting declared | parameters |
| `BlockClock` | exactly one per block | header |

Since contract version 3 the share rate is `internalEther / internalShares`,
not `totalPooledEther / totalShares`; the two are equal as rationals but not
as truncated integers, so the consumer must use the getter's form. The
pre-V3 slot `keccak256("lido.StETH.totalShares")` is zeroed at migration and
is **not** a reviewed slot of this epoch: a write to it fails the block, which
is the intended behaviour for an epoch that does not cover that version.

## Parameters

The default manifest parameters bind no epoch and emit only `BlockClock`.
[`tests/fixtures/mainnet-steth-v4-epoch.json`](tests/fixtures/mainnet-steth-v4-epoch.json)
binds the stETH proxy `0xae7a…fE84` at contract version 4 with the listed
implementation `0x0282…cDb0` and Accounting `0x23ED…cdDf`. Every unstructured
slot is checked in the tests against `keccak256` of its pinned name;
`other_slot_names` lets a caller review further named slots without hex. The
`shares` mapping slot `0` is **AST-derived** with `solc 0.4.24` over the
22-contract linearized chain of `Lido` (only `StETH` and `StETHPermit` declare
regular state; every Aragon base uses unstructured storage), and all 16
`*_POSITION` constants of that chain are configured or reviewed
([evidence](../../docs/evidence/storage-layouts/lido-core@2da0f48f.json),
[`tests/storage_layout.rs`](tests/storage_layout.rs)). The implementation's
runtime code hash, the version-4 enactment block and the `activation_block`
placeholder remain unverified; live Firehose and RPC use is paused.

Each epoch requires an `aragon` object containing `kernel`, `app_id`,
`kernel_implementation`, and a `source_pin` including the implemented
`aragon/aragonOS@f3ae59b00f73984e562df00129c925339cd069ff` (OS 4.4.0) layout.
The fixture takes these identities from Lido's pinned `deployed-mainnet.json`;
their actual storage values at activation and deployed runtimes are **not
verified**. Older parameter documents without this binding are rejected.

The resolution path is derived from the pinned source, with no configurable
slot overrides ([source evidence](../../docs/research/09-Lido-Aragon-resolution.json)):

| Location | Bound selector / value | Handling |
| --- | --- | --- |
| stETH `keccak256("aragonOS.appStorage.kernel")` | Kernel address | `BEACON` dependency, depth 1, storage pointer |
| stETH `keccak256("aragonOS.appStorage.appId")` | configured `app_id` | guarded selector, included in the parameter digest; determines the exact app-base mapping member |
| Kernel `apps[keccak256("base")][app_id]` (mapping base slot 0) | stETH implementation | `IMPLEMENTATION` dependency, parent Kernel, depth 2, storage pointer |
| Kernel `apps[keccak256("core")][KERNEL_CORE_APP_ID]` | Kernel implementation | `IMPLEMENTATION` dependency, parent Kernel, depth 2, storage pointer |

Every persisted write at those four locations invalidates the model,
including a transition into a configured identity and a change restored in
the same block, as well as an equal-value SSTORE. Each write keeps its own old/new words, ordinal and
transaction/call provenance; the end-of-block value cannot erase an upgrade.
Writes are checked for continuity before evidence is emitted. The Kernel
and app-id slots cannot also appear among ordinary reviewed slots.
Code changes on stETH, its implementation, the Kernel, the Kernel
implementation and declared Accounting dependency invalidate the model.
Other Kernel storage (including other app bases) is not a conversion input.
The map never follows a newly observed, unqualified pointer: consumers must
retain the invalidation until an independently qualified epoch replaces it.

## Fail-closed rules

| Condition | Result |
| --- | --- |
| Non-Extended block, `Block.ver` not listed (only 4 and 5 may be listed), incomplete transaction data | block fails |
| Persisted stETH write that is not a configured word, a `shares` entry or a reviewed slot / mapping member | `unresolved storage … refusing incomplete balance state` |
| `TokenRebased` log with the wrong topic count or data length | `malformed TokenRebased log` |
| Two writes to one key with equal ordinals, or a write whose old value is not the previous new value | `ambiguous` / `discontinuous`, naming the contract, key and ordinals |
| Any persisted Aragon resolution write, including same-block restoration | epoch invalidation with per-transition evidence |
| Missing Aragon binding, unsupported Aragon source pin, zero/malformed Aragon identities, overlapping slots (including guarded named slots), unknown fields, zero version | parameters rejected |

Reverted frames and failed transactions never contribute, per the shared
[`common/persist`](../../common/persist) rules. Ordinary no-op writes are
omitted; the shared no-op hook preserves equal-value writes at guarded
resolution slots, as required by the `STORAGE_POINTER` binding contract.

## Validation

```sh
cargo test -p lido-balance-state
cargo clippy -p lido-balance-state --all-targets -- -D warnings
cargo check -p lido-balance-state --target wasm32-unknown-unknown
make -C lido/balance-state build
```

Tests are synthetic: slot names, share writes, packed halves at 128-bit
extremes, derived pooled ether with truncation and the zero-internal-shares
refusal, report logs from succeeded transactions only, version and code
invalidations, the pre-V3 slot refusal, reviewed names and nested allowance
mappings, Aragon pointer changes/restorations and source bindings, dependency
code changes, shared-Kernel market attribution, deterministic ordering,
reverts, ties, discontinuities and parameter refusals. Unwritten pointer
values cannot be checked from an Extended block; binding rows declare the
required qualification rather than proving it. There is no
captured-block replay yet; see issue
[#23](https://github.com/pinax-network/substreams-evm-extended/issues/23).
