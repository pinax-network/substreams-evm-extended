# Persisted native balance effects

This page is the chain / producer / fork / reason matrix required by
[#17](https://github.com/pinax-network/substreams-evm-extended/issues/17). It
records what the saved data establishes, what the code handles by pinned rule
without a fixture, and what stays unqualified. Nothing here is a fresh live
measurement: live Substreams, Firehose, RPC and sink usage remains paused.

## Scopes

| Scope | Source in `sf.ethereum.type.v2.Block` | Rule | Evidence |
| --- | --- | --- | --- |
| Succeeded transaction | `transaction_traces[].calls[].balance_changes` with `state_reverted == false` | every non-no-op record persists | block 122260950 oracle; saved replay |
| Reverted child frame | same, `state_reverted == true` | dropped | synthetic tests; saved replay continuity |
| Failed / reverted transaction | root call only | `GAS_BUY`, `GAS_REFUND`, `REWARD_TRANSACTION_FEE` persist; reasons known to revert are dropped; any other reason fails the block | 6,655 failed-root gas records in the saved replay; two captured failed SetCode transactions |
| Failed SetCode (EIP-7702) | authorization nonce/code records before root `begin_ordinal` | handled by `evm-persist`; no native balance effect beyond gas | captured `bsc-121114122` / `bsc-121114153` |
| System call | `system_calls[].balance_changes` with `state_reverted == false` | persists | rule only; the saved BSC captures carry no system-call balance record |
| Block level | `Block.balance_changes` | always persists | 2,878 fee-reward records including the fee reset at 122260950 |

Reduction is by execution ordinal across all scopes. Array order is not
execution order: block-level records are serialized after the transactions but
the fee reset at block 122260950 (ordinal 3244) must follow the last
transaction credit (ordinal 3237). Per-account continuity (`old == previous
new`) and strictly increasing ordinals are required; ties and gaps fail the
block. Ordinal 0 is rejected; there is no fixture for a legitimate zero-ordinal
persisted record, so genesis allocations are unsupported rather than guessed.

## Reason matrix

Reason numbers follow `sf.ethereum.type.v2.BalanceChange.Reason` in
[firehose-ethereum `type.proto`](https://github.com/streamingfast/firehose-ethereum/blob/develop/proto/sf/ethereum/type/v2/type.proto).
The pinned `substreams-ethereum` 0.11.1 bindings name reasons 0–16; the
reducer applies records regardless of whether it can name the reason, because
the reason only governs the failed-transaction policy. Counts are from the
saved-data replay of 1,439 BSC producer-version-5 blocks.

| Reason | Number | Succeeded / system / block | Failed transaction root call | Saved records |
| --- | --- | --- | --- | --- |
| `TRANSFER` | 5 | persists | reverts | 127,866 (Tx) |
| `GAS_BUY` | 7 | persists | persists | 95,532 (Tx), 3,344 (failed) |
| `REWARD_TRANSACTION_FEE` | 8 | persists | persists | 95,532 (Tx), 3,344 (failed), 2,878 (block) |
| `GAS_REFUND` | 9 | persists | persists | 85,350 (Tx), 3,311 (failed) |
| `REWARD_BLOB_FEE` | 17 | persists (BNB chain blob processing reward, credited to the system fee account in successful blob transactions) | **fails the block** – no fixture | 218 (Tx); live 2026-09-23: 37 (Tx) in 250 parity-checked blocks |
| `TOUCH_ACCOUNT` | 10 | persists (no-op records are dropped) | reverts | none observed |
| `SUICIDE_REFUND`, `SUICIDE_WITHDRAW` | 11, 13 | persist | revert | none observed |
| `CALL_BALANCE_OVERRIDE` | 12 | persists | reverts | none observed |
| `BURN` | 15 | persists | reverts | none observed |
| `REWARD_FEE_RESET` | 14 | persists | fails the block | none observed; the BSC v5 reset is recorded as `REWARD_TRANSACTION_FEE` at block level |
| `REWARD_MINE_UNCLE`, `REWARD_MINE_BLOCK` | 1, 2 | persist (block level) | fails the block | none observed (BSC has no mining rewards) |
| `DAO_REFUND_CONTRACT`, `DAO_ADJUST_BALANCE` | 3, 4 | persist (Ethereum DAO fork block) | fails the block | none observed; Ethereum fixture required |
| `GENESIS_BALANCE` | 6 | block 0 rejected | fails the block | none observed |
| `WITHDRAWAL` | 16 | persists (Ethereum execution-layer withdrawals, block level) | fails the block | none observed; Ethereum fixture required |
| `INCREASE_MINT`, `REVERT` | 18, 19 | persist if observed | **fails the block** – OP-stack deposit semantics unqualified | none observed |
| `MONAD_TX_POST_STATE` | 20 | persists if observed | fails the block | out of scope |
| `UNKNOWN` / future | 0, >20 | persists if observed | fails the block | none observed |

"Persists if observed" means the producer emitted a persisted record and the
reducer applies its old/new values; it does not mean the chain semantics were
reviewed. Producer fixtures are required before any of these networks or
reasons are claimed.

### Live reason matrix (BSC, 2026-09-23)

250 contiguous final blocks 123,552,820–123,553,069 fetched from
`bsc.firehose.pinax.network` and replayed offline
([report](evidence/live-reasons-bsc-2026-09-23.json)); the packaged output of
the same blocks equals the replay, and every emitted row of these blocks was
checked against `eth_getBalance` at the block hash:

| Scope | Reasons (records) |
| --- | --- |
| Succeeded transaction | `TRANSFER` 67,428, `GAS_BUY` 32,923, `REWARD_TRANSACTION_FEE` 32,923, `GAS_REFUND` 30,266, `REWARD_BLOB_FEE` 37 |
| Failed transaction root call (persisted) | `GAS_BUY` 2,264, `GAS_REFUND` 2,259, `REWARD_TRANSACTION_FEE` 2,264 |
| Block level (fee reset) | `REWARD_TRANSACTION_FEE` 500 |

No system-call balance record, no `BURN`, `SUICIDE_*` or `TOUCH_ACCOUNT`
record, and no failed-transaction reason outside the pinned three occurred;
over the 5,000-block live window the reason guard refused no block.

## Chain / producer / fork matrix

| Network | Producer versions in saved data | Status |
| --- | --- | --- |
| BSC (chain id 56) | 5 | Replayed: 1,439 blocks, 0 projection errors, 0 clock or continuity mismatches, 82/82 same-block RPC rows at 122260950. Fee-reset and blob-fee-reward semantics observed. **Live-qualified on 2026-09-23** (package `fbb46fc7…`): 1,024 saved control blocks equal to the offline replay and to `eth_getBalance` (76,139/76,139), and 5,000 contiguous recent blocks without a refused block, 200,344/200,344 same-block `eth_getBalance` checks (see the README). |
| BSC | 4 | 71 saved blocks at heights 51,995,162–104,975,334 reduce with 0 projection errors and 0 continuity mismatches when version 4 is enabled (4,461 additional continuity checks); no native RPC oracle. Not enabled by default. |
| BSC | 3 | Accepted by the historical prototype; no saved block. Unsupported by design: the version-3 tracer recorded system-call ordinals on a different scale from transaction ordinals and set every root call's `begin_ordinal` to 0 ([geth Firehose tracer](https://github.com/streamingfast/go-ethereum/blob/70f5118d6443624792f49501627a1cd80f51e8e9/eth/tracers/firehose.go), [firehose-ethereum CHANGELOG v2.10.0](https://github.com/streamingfast/firehose-ethereum/blob/9485efe2e6290e525fd4978b50462516ec752672/CHANGELOG.md)). Global ordinal reduction across scopes is not trustworthy on version 3. |
| Ethereum (1) | none | Genesis, uncle/block rewards, DAO redistribution, execution-layer withdrawals, blob fee debits, pre/post-[EIP-6780](https://eips.ethereum.org/EIPS/eip-6780) SELFDESTRUCT and the consensus-layer boundary need fixtures under #8. |
| Base (8453) | none | The [OP deposit specification](https://specs.optimism.io/protocol/deposits.html#execution) persists the mint of a failed deposit. The BSC gas-only failed-root policy would drop it; the reason guard fails such blocks until producer fixtures show where the record appears and how rollback is represented. L1/operator fee vault effects unreviewed. |
| HyperEVM (999) | none | HYPE account changes and the EVM side of HyperCore↔HyperEVM transfers need fixtures; HyperCore spot/perp/staked balances are a separate domain not derivable from EVM state. |
| Arc | none | Native USDC at 18 decimals with an ERC-20 interface at 6 decimals over the same balance. Both raw observations must be preserved with an explicit alias identity; this package would emit the native side only. Chain id, producer and precision binding under #8/#12. |

Accepting a protobuf version does not qualify every state transition of a
network. SELFDESTRUCT records are applied as balance movements only; code or
account deletion is not inferred from them. Version 5 producers also stop
recording no-op state changes and gas changes; the persistence rules already
drop no-op balance records, so version 4 and 5 blocks reduce identically.

## Saved-data replay

Reports produced by `native-balances-tools replay` over the locally retained
Extended block captures (the ERC-20 campaign's cache under
`erc20/balances/out/`, ignored by git) plus the committed block 122260950:

| Report | Producer versions | Blocks | Rows | Continuity checks | Result |
| --- | --- | --- | --- | --- | --- |
| [`replay-bsc-v5.json`](evidence/replay-bsc-v5.json) | 5 | 1,439 replayed; 71 version-4 blocks refused as unqualified | 126,180 (10,226 final zeros; 30,374 accounts) | 86,564 | 0 projection errors, 0 clock or continuity mismatches; oracle 82/82; prototype 82/82 |
| [`replay-bsc-v4-v5.json`](evidence/replay-bsc-v4-v5.json) | 4, 5 | 1,510 replayed | 136,640 (10,753 final zeros; 34,972 accounts) | 91,025 | 0 projection errors, 0 clock or continuity mismatches; oracle 82/82; prototype 82/82 |

Continuity checks compare an account's `old_amount` with its last emitted
amount across consecutive replayed blocks; the 1,024-block window
[122288006, 122289030) contributes most of them. Blocks separated by gaps are
linked only by hash where consecutive. The replay executes the Rust reducer,
not packaged WASM, and reads no network. It does not establish global holder
coverage: 30,374 distinct accounts changed in the replayed blocks, and
accounts without a change are unknown.

## Historical evidence retained

- Block 122260950 with 82 native and 18 WBNB same-block RPC rows:
  [`bsc-122260950.json`](../../../erc20/balances/tests/fixtures/bsc-122260950.json).
- The prototype's 82 native old/new/ordinal rows for that block:
  [`bsc-122260950-prototype-native-rows.json`](../tests/fixtures/bsc-122260950-prototype-native-rows.json),
  extracted from the retained 64-block run output whose SHA-256 is recorded in
  the fixture.
- 192 blocks, 37,086 native before/after RPC checks with zero mismatches for
  the prototype package: [legacy qualification](../../../erc20/balances/docs/legacy-qualification.md),
  [64-block](../../../erc20/balances/docs/evidence/rpc-exhaustive-64.json) and
  [128-block](../../../erc20/balances/docs/evidence/rpc-exhaustive-128.json)
  summaries. The Extended blocks of those windows are not retained locally,
  so they cannot be replayed offline; the checks qualify the removed prototype
  artifact, not this package.
- The failed v0.3.3 reference comparison that overwrote the fee reset
  ([64-audited](../../../erc20/balances/docs/evidence/64-audited.json)). This
  is a finding about that artifact's ordering, not about the current
  `substreams-evm` native module.

## Remaining gates

- Build a versioned SPKG, record source/WASM/package digests and compare actual
  packaged output with these saved controls, then with same-block-hash
  `eth_getBalance` controls, after live usage is explicitly resumed. Extra
  unchanged RPC candidates are accounted separately from state-derived rows.
- Initialization, exact checkpoints, reorg/undo completeness and the
  complete-block clock contract: [#7](https://github.com/pinax-network/substreams-evm-extended/issues/7).
- Per-network fixtures and asset identity: [#8](https://github.com/pinax-network/substreams-evm-extended/issues/8).
