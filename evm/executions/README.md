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

Persisted trace logs are sorted by their unique, nonzero ordinals and compared
with receipt logs in receipt order. Address, topics, data, transaction and
block log indexes, and ordinal must all agree. The map fails on disagreement
even when log output is disabled; receipts only validate trace data and never
supply fallback rows.

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

## Call context

`Call.address` is the address whose code the frame runs, and `Call.caller`
is the frame's caller. For `CALL_TYPE_DELEGATE` and `CALL_TYPE_CALLCODE`
frames the storage (and `msg.sender` context) is the **caller**'s; for every
other frame it is the frame's own `address`. A proxy upgrade is therefore
visible as a change of `address` among the delegate frames whose `caller` is
the proxy; the package records that fact and does not label it. The replay
below checked this rule against every persisted storage write in the saved
data: 2,093,149 writes in 678,629 frames, 1,147,531 of them in delegate
frames, with no exception. No `CALLCODE` frame occurs in the saved data; the
rule for it is covered by a synthetic regression.

## Producer capabilities (saved BSC data)

What each reviewed producer version supplied in the saved blocks, from the
[replay report](docs/evidence/replay-bsc-v4-v5.json). An absent fact is only
meaningful relative to what the producer records.

| Fact | `Block.ver` 4 (71 blocks) | `Block.ver` 5 (1,438 blocks) |
| --- | --- | --- |
| Transaction types observed | 0–4 | 0–4 |
| EIP-7702 delegated frames / authorizations | 1,640 / 136 | 14,273 / 890 |
| Blob transactions (all with `blob_gas`) | 13 | 217 |
| System calls | 1 per block | 1 per block |
| Block-level balance changes | 142 | 2,876 |
| Block-level code changes | 0 | 0 |
| Logs in reverted frames (attempts) | 1,718 | 24,630 |
| Equal-value storage changes | 0 of 90,703 | 0 of 2,002,446 |
| Frames with a zero `begin_ordinal` | 0 | 0 |

Two consequences for consumers:

- **An `SSTORE` that writes the current value leaves no record** on these
  producers: none of 2,093,149 storage changes is equal-valued. Only changing
  writes are observable, which is why the balance-state packages evidence
  every changing pointer write individually rather than relying on a no-op.
- **Code changes, by contrast, are recorded even when nothing changes.** A
  SetCode authorization that re-delegates an account to its current target
  produces a code change whose old and new code are identical (86 cases, all
  in successful transactions with applied authorizations). The row keeps kind
  `DELEGATION_SET` and the target, with `persisted = false` because no code
  state changed; the frame's `Call.persisted` is `true` and the
  `SetCodeAuthorization` row shows the authorization `applied`, so it is
  distinguishable from an attempt in a reverted frame.

The same facts hold on current live data: 250 contiguous final blocks
123,552,820–123,553,069 fetched from `bsc.firehose.pinax.network` on
2026-09-23 replay with 0 errors ([report](docs/evidence/replay-bsc-live-2026-09-23.json)):
37,214 transactions, 275,928 receipt logs matched, 523,805 storage writes
with 0 storage-context violations, 0 equal-value storage changes, no
CALLCODE frame and no block-level code change, 857 SetCode authorizations,
37 blob transactions, one system call per block, 12,525 logs in reverted
frames.

Producer semantics on other chains and versions are qualified under
[#8](https://github.com/pinax-network/substreams-evm-extended/issues/8).

## Parameters

```json
{"chain_id":56,"producer_versions":[5],"include_input":false,"include_return_data":false,"include_calls":true,"include_logs":true}
```

Selectors and payload sizes are always carried; raw input and return data are
opt-in because they dominate row size. `Call` and `Log` tables can be
switched off for a transaction-only feed. `producer_versions` must be a
nonempty subset of 4 and 5; unreviewed versions are rejected at parse time.

## Evidence

Rust tests replay the complete captured BSC block 122260950 (68 transactions,
853 frames including one system call, 273 logs of which 253 are the receipt
logs and 20 are attempts in reverted frames, one reverted transaction with 11
attempted frames, one blob transaction, 22 EIP-7702 delegated frames, one
nested factory creation), the captured first-CREATE deployment and
clone-factory transactions, the two captured failed SetCode transactions
(applied authorizations, no persisted frame), and synthetic cases for receipt
same-count receipt tampering, receipt ordering, parent/child trace ordering,
ambiguous log ordinals, reverted children, system calls, block records, code-change
kinds and parameter guards, plus a captured same-target re-delegation,
delegate and `CALLCODE` context across a proxy upgrade, and transaction types
outside the named set (raw value kept, `OTHER` when unnamed). Producer
semantics on other chains are qualified under
[#8](https://github.com/pinax-network/substreams-evm-extended/issues/8).

The host-only replay tool (`tools/`, excluded from the WASM path) projects
every saved block, checks one clock per block, determinism and the storage
context rule, and tabulates producer capabilities. Over all 41 cached BSC
directories ([report](docs/evidence/replay-bsc-v4-v5.json)): 1,509 distinct
blocks (71 version 4, 1,438 version 5) project with **0 errors**, including
the receipt/trace log agreement check on 116,951 transactions (1,075,108
persisted trace logs equal 1,075,108 receipt logs), 1,509/1,509 deterministic
re-projections and 0 storage-context violations. This is saved-data evidence,
not a package qualification.

```sh
cargo test --locked -p evm-executions -p evm-executions-tools
cargo run --release --locked -p evm-executions-tools -- replay \
  --blocks <dir> [--blocks <dir> ...] --producer-versions 4,5 --output evm/executions/out/replay
make -C evm/executions build
```

## Boundaries

No RPC, no ABI decoding, no labels, no prices, no `db_out` or custom sink.
Native balance effects are `native/balances`; token transfer and approval
evidence is [#19](https://github.com/pinax-network/substreams-evm-extended/issues/19);
protocol action adapters are [#20](https://github.com/pinax-network/substreams-evm-extended/issues/20).
Packaging and live qualification remain paused.
