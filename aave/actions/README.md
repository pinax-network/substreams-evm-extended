# aave/actions

A single RPC-free `map_events` reads Firehose Extended blocks and emits
**`aave.actions.v1.Events`** ([schema](../../proto/v1/aave_actions.proto)):
Aave V3 Pool `Supply`, `Withdraw`, `Borrow`, `Repay`, `LiquidationCall` and
`FlashLoan` facts for explicitly bound Pool epochs
([#20](https://github.com/pinax-network/substreams-evm-extended/issues/20)).
It is separate from aToken balance state
([`aave/balance-state`](../balance-state/README.md)) and from wallet labels.

## Rows

One `Action` per decoded Pool log, with block, transaction, call and log
identity, execution ordinal, the emitting Pool and its epoch, the protocol's
own `kind`, and the actor fields the event carries:

| Kind | `reserve` | `actor` (initiator) | `beneficiary` (position) | `recipient` | Amounts and flags |
| --- | --- | --- | --- | --- | --- |
| Supply | reserve | `user` | `onBehalfOf` | — | `amount`, `referral_code` |
| Withdraw | reserve | `user` | `user` | `to` | `amount` |
| Borrow | reserve | `user` | `onBehalfOf` | — | `amount`, `interest_rate_mode` (raw; 1 is deprecated stable), `borrow_rate` (ray), `referral_code` |
| Repay | reserve | `repayer` | `user` (debtor) | — | `amount`, `use_atokens` |
| LiquidationCall | debt asset | `liquidator` | `user` (liquidated) | — | `amount` = debt to cover, `collateral_asset`, `liquidated_collateral_amount`, `receive_atoken` |
| FlashLoan | asset | `initiator` | `target` (receiver contract) | — | `amount`, `interest_rate_mode`, `premium`, `referral_code` |

Caller, beneficiary and recipient are never merged; wallet-relative direction
is the consumer's. A transaction may carry many rows, including the same
action attempted in a reverted frame and then persisted; `persisted` marks
receipt logs. No row asserts economic success beyond the log having
persisted, and nothing is inferred from calldata.

## Source binding and fail-closed rules

Event shapes are those of `IPool.sol` at aave-v3-origin v3.7.0. On a bound
Pool, a log with a bound signature but a different shape (topic count, data
length, unpadded address, `uint8`/`bool` out of range) fails the block: that
is how an unqualified version announces itself. A Pool or implementation code
change, or a persisted write to the Pool's implementation pointer slot, also
fails the block. Other Pool events and other emitters are ignored; a Pool that
is not configured produces no rows. Reverted frames are emitted as attempts
unless `include_attempted` is false.

## Parameters

```json
{"chain_id":56,"producer_versions":[5],"include_attempted":true,
 "pools":[{"address":"0x6807…e0cB","epoch":1,"implementation_slot":"0x3608…2bbc","implementation":"0x5e2B…3B6d","source_pin":"IPool.sol at aave-v3-origin v3.7.0 …"}]}
```

The default manifest binds no Pool. [`tests/fixtures/bsc-aave-v3-pool.json`](tests/fixtures/bsc-aave-v3-pool.json)
binds the Aave V3 BNB Pool; its runtime code hash is not bound offline.
`producer_versions` must be a nonempty subset of the reviewed Extended versions
4 and 5; explicitly listing another version does not authorize its semantics.

## Evidence

Captured BSC transactions ([provenance](tests/fixtures/cases.json)): a
variable-rate USDT `Borrow` with its ray borrow rate, a BTCB `Supply`, a USDT
`Withdraw`, and a router transaction where `Supply`, `Withdraw` and `Borrow`
are attempted in reverted frames and then persisted (seven rows, three
persisted). `Repay`, `LiquidationCall` and `FlashLoan` do not occur in the
saved window and are covered by synthetic logs built from the pinned shapes;
topic constants are asserted against the canonical signatures. Live
qualification of emitted facts and packaged output waits for explicit
resumption.

```sh
cargo test --locked -p aave-actions
make -C aave/actions build
```

## Boundaries

Stake/unstake, rewards, trading, generic mint, Compound adapters and protocol
cancel events need named protocol and version requirements first; execution
facts (`evm/executions`) carry their raw evidence meanwhile. No prices,
health factors, strategy interpretation or custom sink.
