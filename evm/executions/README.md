# evm/executions

A single RPC-free `map_events` reads Firehose Extended blocks and emits
**`evm.executions.v1.Events`** ([schema](../../proto/v1/executions.proto)):
chain-observed execution facts for downstream activity interpretation. It is
not a wallet activity classifier. Wallet-relative send/receive, grouping,
trade or stake intent, protocol priority and labels stay with the consumer.

## Tables

Every top-level repeated message is one native ClickHouse sink table. Rows are
flat; log topics are four fixed columns plus a count.

| Table | Row | Identity |
| --- | --- | --- |
| `BlockClock` | exactly one per block: number, hash, parent, timestamp, state root, coinbase, gas, base fee, producer version, package identity, parameters digest, row counts | `(chain_id, number)` |
| `Transaction` | hash, index, named and raw type, status, from/to, nonce, value, gas, fee caps, input selector and size (full input opt-in), return data (opt-in), ordinals, root-created contract, call/log/reverted counts, authorization count, blob gas and count | `(hash)` |
| `Call` | one node of the call tree: index, parent, depth, call type, caller, code address, EIP-7702 delegation target, value, gas, selector, sizes, execution flags, `state_reverted`, `persisted`, ordinals, effect counts | `(scope, transaction_hash, index)` |
| `Log` | every emitted log, including logs of reverted frames: call index, producer log index, block index (0 for attempts), ordinal, address, topics, data, `persisted` | `(scope, transaction_hash, ordinal)` |
| `CodeChange` | address, old/new code hash and size, lifecycle kind (created, replaced, cleared, delegation set, delegation cleared), delegation target, `persisted` | `(scope, transaction_hash, ordinal, address)` |
| `SetCodeAuthorization` | one EIP-7702 tuple: position, authority, target, nonce, authorization chain id, `discarded`, `applied` | `(transaction_hash, position)` |

System calls carry `SCOPE_SYSTEM_CALL` and no transaction hash; block-level
code changes carry `SCOPE_BLOCK`.

## Attempted execution versus persisted effect

`persisted` is computed once, from the shared
[`common/persist`](../../common/persist) rules: a frame's effects survived
when the transaction succeeded and the frame was not reverted, when it is a
non-reverted system call, or when it is a block record. Accepted SetCode
authorizations persist even when the transaction fails, so their code
changes are `persisted` while every frame of that transaction is not. A
failed transaction's root call is not `persisted` although its fee and nonce
effects are (those are native balance and nonce facts, owned by
`native/balances`). Reverted frames and their logs are emitted as attempts
with `persisted = false`; they are never completed actions.

Persisted logs equal the receipt logs exactly; the map fails the block when a
producer disagrees, rather than choosing one side.

## Lifecycle facts

- A `CALL_TYPE_CREATE` frame is a creation attempt; `Transaction.created_contract`
  is set only for a root creation whose code persisted, and nested factory
  creations appear as `CodeChange` rows of kind `CREATED`.
- `Call.suicide` records a SELFDESTRUCT invocation. Under
  [EIP-6780](https://eips.ethereum.org/EIPS/eip-6780) the code and account are
  deleted only within the creating transaction; a `CodeChange` of kind
  `CLEARED` is the deletion fact, and its absence means the account survived
  with its balance swept.
- EIP-7702 delegation is a `CodeChange` of kind `DELEGATION_SET` with the
  target parsed from `0xef0100 || address` (or `DELEGATION_CLEARED`);
  `Call.address_delegates_to` shows frames that executed delegated code; the
  authorization tuples are their own table. None of this is an ERC-20
  allowance, an off-chain signature or wallet intent.
- Mempool replacement or cancellation and unseen signatures leave no fact in a
  canonical block and are not modeled. A "cancel" is whichever transaction
  consumed the nonce.

## Parameters

```json
{"chain_id":56,"producer_versions":[5],"include_input":false,"include_return_data":false,"include_calls":true,"include_logs":true}
```

Selectors and payload sizes are always carried; raw input and return data are
opt-in because they dominate row size. `Call` and `Log` tables can be
switched off for a transaction-only feed. Producer versions are explicit;
ordinals are trustworthy on versions 4 and 5 only.

## Evidence

Rust tests replay the complete captured BSC block 122260950 (68 transactions,
853 frames including one system call, 273 logs of which 253 are the receipt
logs and 20 are attempts in reverted frames, one reverted transaction with 11
attempted frames, one blob transaction, 22 EIP-7702 delegated frames, one
nested factory creation), the captured first-CREATE deployment and
clone-factory transactions, the two captured failed SetCode transactions
(applied authorizations, no persisted frame), and synthetic cases for receipt
disagreement, reverted children, system calls, block records, code-change
kinds and parameter guards. Producer semantics on other chains are
qualified under [#8](https://github.com/pinax-network/substreams-evm-extended/issues/8).

```sh
cargo test --locked -p evm-executions
make -C evm/executions build
```

## Boundaries

No RPC, no ABI decoding, no labels, no prices, no `db_out` or custom sink.
Native balance effects are `native/balances`; token transfer and approval
evidence is [#19](https://github.com/pinax-network/substreams-evm-extended/issues/19);
protocol action adapters are [#20](https://github.com/pinax-network/substreams-evm-extended/issues/20).
Packaging and live qualification remain paused.
