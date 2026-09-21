# Balance-state contract: the three candidate designs (issue #12)

Before `proto/v1/balance_state.proto` was written, three independent designs
were drafted offline and compared. The committed contract is a synthesis,
closest to design 0. They are kept because they record the trade-offs and the
validation each draft went through (protoc/buf/prost round trips, keccak checks
of example slots), and because the judge panel that was meant to score them
never ran (subagent spend limit).

| Draft | Thesis | Files |
| --- | --- | --- |
| 0 chain-fact | every row cites its persisted source (contract, slot, raw words, bit range, ordinal, tx/call identity); observed vs verified vs derived are distinct enum values; dependency graph and invalidations are rows | `0-chain-fact.md`, `0-chain-fact.proto` |
| 1 protocol-model | per-protocol fidelity first: explicit model epochs, state kinds (known zero vs invalidated), metric names per family | `1-protocol-model.md`, `1-protocol-model.proto` |
| 2 sink-first | minimal flat tables for the native ClickHouse sink; fewest messages and enums; alias rows for Arc | `2-sink-first.md`, `2-sink-first.proto` |

The committed contract is documented in [`../../balance-state-contract.md`](../../balance-state-contract.md).
Paths inside these drafts refer to a session scratchpad that no longer exists.
