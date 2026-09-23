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
saved window and are covered by synthetic logs built from the pinned shapes.

**ABI provenance** ([fixture](tests/fixtures/pool-event-abi.json)): the six
bound events are taken from the solc 0.8.27 ABI of `IPool.sol` at
aave-v3-origin v3.7.0 (`cff15de6`); the tests derive every topic from that
ABI, decode a log of each exact ABI shape and fail one with a topic missing.
The original V3 (`aave-v3-core`) declarations have identical types and
indexing. Aave V2 `Deposit`, `Borrow`, `Repay` and `FlashLoan` have other
signatures and yield no row even from the bound Pool, while V2 `Withdraw` and
`LiquidationCall` have **exactly the V3 signatures**: a log's shape cannot
tell the version, only the bound Pool address and its implementation pointer
can, and an unbound V2 pool yields no row.

**Live qualification (BSC, 2026-09-23)** ([report](docs/evidence/live-parity-bsc-2026-09-23.json)):
the packed map (`spkg` sha256 `ca6c9cb0…`, wasm `acd5a4e0…`, parameters
[`bsc-aave-v3-pool.json`](tests/fixtures/bsc-aave-v3-pool.json)) was streamed
from `bsc.substreams.pinax.network` over 2,397 blocks: the 2,000 contiguous
blocks 123,553,459–123,555,458, every other block of the preceding 20,000
with a bound Pool event, and the five blocks with a `LiquidationCall` in
the preceding 600,000 blocks. `aave-actions-tools live-parity` decodes the Pool's
receipt logs from `eth_getLogs` with the compiled ABI (not the map's
decoder) and matches them by transaction hash and block log index in both
directions: **459/459** receipt logs equal a persisted row field by field
(148 `Supply`, 290 `Withdraw`, 7 `Borrow`, 7 `Repay`, 5 `LiquidationCall`,
2 `FlashLoan`), no row without a log, no log without a row; 2,397 clocks
equal the headers; the Pool implementation pointer equals `0x5e2B…3B6d` at
the first and last block. No attempted row occurred in these blocks; attempts
of reverted frames are not in receipts and are covered by the captured router
fixture only.

```sh
cargo test --locked -p aave-actions -p aave-actions-tools
make -C aave/actions build
# live, credentials in the environment only (SUBSTREAMS_API_KEY, RPC_URL)
substreams run -e bsc.substreams.pinax.network:443 <spkg> map_events -s <N> -t +1 \
  -p "map_events=$(jq -c . aave/actions/tests/fixtures/bsc-aave-v3-pool.json)" -o jsonl --bytes-encoding hex > events.jsonl
cargo run --locked -p aave-actions-tools -- --events events.jsonl --params aave/actions/tests/fixtures/bsc-aave-v3-pool.json \
  --spkg <spkg> --endpoint bsc.substreams.pinax.network:443 --output <fresh dir>
```

## Boundaries

Stake/unstake, rewards, trading, generic mint, Compound adapters and protocol
cancel events need named protocol and version requirements first; execution
facts (`evm/executions`) carry their raw evidence meanwhile. No prices,
health factors, strategy interpretation or custom sink.
