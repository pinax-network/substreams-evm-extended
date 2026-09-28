# Host reference retention

`Ledger` retains primitive holder amounts or basis observations, including
their initialization origins, canonical identities and bounded undo history.
`protocol::ProtocolLedger` adds the inputs needed for protocol reference
evaluation: full holder/global rows, model and dependency declarations, and
the complete `BlockClock`, including timestamp. Both are host consumer
tooling; the protocol wrapper is excluded from WASM and is not a production
sink or another map.

## Protocol state API

- `ProtocolLedger::new(undo_depth)` begins without initialized state. Only a
  `BOUND` declaration establishes a model; joining at a heartbeat does not
  prove uninterrupted history since activation.
- `ProtocolLedger::from_checkpoint` takes a `Checkpoint` containing explicit
  input descriptors and an independent evidence reference. Its clock binds
  the exact chain, number, hash, parent, timestamp, state root, producer,
  package/version, spec revision and parameters digest. Table counts describe
  the supplied snapshot subset. Each fact records `Origin::Checkpoint`;
  these descriptors are not emitted map rows or counted as new observations.
  The next block must be the checkpoint's exact successor. Omitted inputs
  remain unknown; a getter amount cannot initialize a scaled/share basis by
  inverting rounded arithmetic.
- `apply(&Events)` stages the whole block, including its declared row counts.
  Refusal leaves the clock, facts, reports and both journals unchanged.
  `undo(number)` restores exact prior snapshots; it never inverts arithmetic.
  Replacement blocks must extend the restored parent hash. Checkpoint origins
  cannot be undone, and targets outside `undo_depth` are refused atomically.
- `model`, `holder`, `global` and `dependencies` require an active bound
  market. Missing, unbound, unsupported and suspended inputs are distinct
  `Unavailable` cases. An empty global value invalidates that input until
  a fresh value is supplied; it never initializes zero.

`Fact<T>` preserves the original protobuf row, full observation clock,
initialization origin and an `effective_epoch`. Permitted carryover changes
only the effective epoch, retaining original source/slot/scale provenance.
An invalidation or suspension creates an observation gap: the next `BOUND`
drops prior holder/global inputs regardless of carryover flags. While
suspended, subsequent same-epoch rows cannot be evaluated, and their previous
values are not compared with stale pre-gap values. Other markets can advance.
An incompatible holder interpretation or changed dependency heartbeat fails.

Globals are keyed by market, field, mapping key and observation kind.
Persisted writes and qualified constants are distinct from log evidence.
Storage `CHANGE` rows require a final end-of-block companion; log `CHANGE`
rows are independent evidence, reduced by their execution ordinal with tied
ordinals refused. `DERIVED` facts expire at the next block. Constants require
an explicit declaration under an established model; unbound heartbeats
cannot initialize them. Constants are redeclared across every new epoch.

The bounded reference journal keeps at most `undo_depth` full state snapshots
plus current state. Its memory cost grows with the initialized input set and
that depth; it is intended for bounded qualification, not sink-scale serving.
`ProtocolReport.raw_basis_ledger` counts primitive basis observations, which
may be unbound or suspended. `active_model_basis_holders` excludes those cases
but still does not assert that all globals or external qualification exist.
Neither count enumerates global holders or counts successful getter checks.

## Evaluation and trust

[`conformance::retained`](../../conformance/src/retained.rs) evaluates explicit
metrics from this state. A `QualifiedModel` names the reviewed stream, exact
model/dependency set, holder mapping root and each global input location.
Its separate `RuntimeQualification` supplies code hashes and independent
evidence at the exact model-origin/checkpoint header. The attested timestamp,
state root and producer are checked as well as hash and stream identity;
table counts remain the separate complete-delivery check. The caller must
verify that external evidence and source/runtime equivalence: a source pin,
an arbitrary evidence string or empty emitted runtime hashes cannot establish
it automatically. No RPC or header verification is performed by this API.

Returned evaluations preserve the metric, evaluation clock, model, runtime
attestation and every consumed holder/global fact. Missing inputs return
`Unknown`, including missing globals for a known-zero holder. Read the
[conformance scope](../../conformance/README.md) and
[initialization contract](../../docs/initialization-and-completeness.md) for
remaining deployment, package, holder, sink and live-reorg gates.
