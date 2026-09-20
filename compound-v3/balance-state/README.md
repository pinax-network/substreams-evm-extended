# compound-v3/balance-state

A single RPC-free `map_events` reads Firehose Extended blocks and emits
**`evm.balance_state.v1.Events`** ([contract](../../docs/balance-state-contract.md))
for explicitly qualified Compound III (Comet) market epochs. It does not
emit `evm.balances.v1`; the market's observable `balanceOf` / `borrowBalanceOf`
is evaluated by the consumer from the rows below at a canonical block clock,
using the exact model in [`conformance::comet`](../../conformance/src/comet.rs).

## What is emitted

| Table | Row | Source |
| --- | --- | --- |
| `HolderBasis` (`SIGNED_PRINCIPAL`) | the signed int104 `UserBasic.principal` (low 104 bits, two's complement) before the first and after the last write of the block; emitted only when the principal changed | Comet storage, verified Keccak preimage `(account, user_basic_slot)` |
| `GlobalState` `COMET_BASE_SUPPLY_INDEX` / `COMET_BASE_BORROW_INDEX` | bits 0..64 and 64..128 of the indices word | Comet storage, `indices_slot` |
| `GlobalState` `COMET_TOTAL_SUPPLY_BASE` / `COMET_TOTAL_BORROW_BASE` / `COMET_LAST_ACCRUAL_TIME` / `COMET_PAUSE_FLAGS` | bits 0..104, 104..208, 208..248 and 248..256 of the totals word; only fields that changed | Comet storage, `totals_slot` |
| `GlobalState` kinks, rate slopes, rate bases, scales | `QUALIFIED_CONSTANT` / `DECLARATION` rows of the bound implementation's immutables | parameters |
| `ModelEpoch` + `Dependency` | binding rows at the activation block and on the heartbeat | parameters |
| `BlockClock` | exactly one per block | header |

`balanceOf(account)` is `principal > 0 ? principal * accruedSupplyIndex / 1e15 : 0`
and `borrowBalanceOf` is `principal < 0 ? -principal * accruedBorrowIndex / 1e15 : 0`,
where the accrued index projects the stored index to the evaluation timestamp
with the per-second rate derived from utilization and the implementation's
immutable kink / slope / base constants. Comet rates are immutables of the
implementation, so every governance rate change deploys a new implementation
and starts a new epoch. Tracking indices (bits 128..256 of the indices word
and bits 104..168 of the user word) are reward state and are not carried.
An idle block changes the evaluated balance of every account without any row;
an account without a row is unknown, not zero.

## Parameters

The default manifest parameters bind no market and emit only `BlockClock`.
[`tests/fixtures/mainnet-cusdcv3-epoch.json`](tests/fixtures/mainnet-cusdcv3-epoch.json)
is the Ethereum cUSDCv3 configuration the synthetic tests use: Comet
`0xc3d688B6…cdc3`, USDC base, immutables from the pinned
`deployments/mainnet/usdc/configuration.json` rate parameters scaled to
per-second factors, and storage slots `0` (indices), `1` (totals) and `5`
(`userBasic`) **inferred from the pinned `CometStorage.sol` declaration
order**. The slot inference, the implementation address, its code hash and
the activation block are **not verified** against a compiler storage layout
or saved Ethereum blocks: no Ethereum Extended blocks are cached locally and
live Firehose and RPC use is paused. The fixture uses placeholder
`implementation` and `activation_block` values for that reason; a real epoch
must replace them after qualification.

## Fail-closed rules

| Condition | Result |
| --- | --- |
| Non-Extended block, unlisted `Block.ver`, incomplete transaction data | block fails |
| Code change on a bound Comet or its implementation | `code changed; requalify the epoch` |
| Write to the Comet's EIP-1967 implementation pointer slot | `implementation pointer changed; requalify the epoch` |
| Persisted Comet write that is not the indices word, the totals word, a `userBasic` entry or a reviewed slot / mapping (`other_slots`, `other_mapping_slots`, chained through verified preimages) | `unresolved storage … refusing incomplete balance state` |
| Two writes to one key with equal ordinals, or a write whose old value is not the previous new value | `ambiguous` / `discontinuous` |
| Overlapping slots, non-decimal immutables, unknown parameter fields, empty producer versions | parameters rejected |

Reverted frames and failed transactions never contribute, per the shared
[`common/persist`](../../common/persist) rules.

## Validation

```sh
cargo test -p compound-v3-balance-state
cargo clippy -p compound-v3-balance-state --all-targets -- -D warnings
cargo check -p compound-v3-balance-state --target wasm32-unknown-unknown
make -C compound-v3/balance-state build
```

The tests are synthetic: they pack int104 principals, indices and totals
words and verify the bit extraction, sign handling, epoch rows, reviewed
mapping chains and every refusal above. There is no captured-block replay
for this package yet; see issue
[#15](https://github.com/pinax-network/substreams-evm-extended/issues/15)
for the live qualification steps that remain.
