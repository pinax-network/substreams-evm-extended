# Finite calculated-balance retention

The host-only `calculated_retention` module retains raw inputs for the existing LBP reward and BabyDoge/10SET reflection models. It does not emit production `Balance.amount` rows or introduce a protobuf state kind. Its closed historical bindings cover 64 token-holder pairs: 33 LBP, 19 BabyDoge and 12 10SET. The immutable historical fixtures, getter expectations and earlier failed attempts remain unchanged.

## State and calculation contract

A checkpoint identifies chain 56, the model and persistence revisions, a positive epoch, activation boundary, exact token/dependency runtime hashes, source capture digests, exemptions, a finite holder registry, raw words and their evidence. Checkpoint words are storage facts, not RPC getter values. Raw LBP balances, hLBP shares and reward indexes remain distinct; reflection balances retain both token and reflection units. Missing facts produce `Unknown`, arithmetic refusal produces `ModelRefusal`, and a protected runtime change produces a sticky `Suspended` outcome. None returns a previous amount as current.

Every accepted block reevaluates every registered holder, including holders without writes. That covers clock-only LBP accrual and reflection global-only changes. LBP balance and pending-reward results are separate: an exempt balance can succeed while the independent pending calculation refuses. Eager decoding can conservatively require facts that a source early-return branch would not read. Unregistered holders remain unknown; the module does not enumerate holders from transfer logs or getter expectations.

Blocks are applied to a staged copy, including clock, provenance, counters, dynamic exclusion closure and runtime status. A late error leaves the entire ledger unchanged; `apply_all` extends that guarantee across all three ledgers. Applied clocks require exact consecutive numbers, parent hashes, nondecreasing timestamps and a stable Extended producer version (4 or 5). The historical parent checkpoint records its known number, hash and timestamp; its unavailable parent-of-parent and Extended version remain explicitly absent. The first accepted block binds the version.

The collector reuses the unchanged ERC20 persisted-effect rules and retains ordinary equal-value storage writes. Each physical key needs positive, strictly increasing ordinals and exact old/new continuity. Distinct physical keys may have tied or differently ordered ordinals because calculation happens only after the full batch. This is a deliberate relaxation of the initial conservative total-order plan, not a source event-order claim. Same-key ambiguity always refuses.

Observed facts preserve their original block identity, ordinal and source PB digest across later idle blocks. Each evaluation includes the current source digest and a deterministic input digest covering its raw finite state, holder, clock, binding, checkpoint identity and suspension/runtime state. Snapshot import requires an independently retained snapshot digest and exact expected binding; canonical duplicate keys, malformed identities and unsupported versions refuse. Snapshots contain raw facts and counters, not cached calculated amounts, and deliberately omit undo history.

Undo is bounded and hash-checked; replay of the removed suffix must rebuild the same state. Runtime changes remain suspended even if code returns to the original hash in the same or a later block. Resumption requires a complete new checkpoint at the current boundary, a strictly newer epoch and the exact currently observed runtime bindings. It discards all prior facts, counters and undo history. A new unreviewed runtime cannot be adopted through these closed presets.

## Binding and qualification boundary

Four complete original captures are pinned before parsing. The checker reconciles their sources with standard compiler input, metadata source Keccaks, source IDs, selected output and storage layouts. It checks the exact getter fields, packed widths and offsets, then reconstructs the complete captured runtimes from saved compiler bytes using only the closed immutable and CBOR substitutions. This is saved-output reconstruction, not fresh compilation.

The LBP exemption branch is exactly self, DEAD, pair, burn vault, referral vault, POL vault, FOMO vault and hLBP. Zero is not an exemption. Nine actual captured constant-getter executions in the existing host VM independently verify the six LBP immutable addresses and hLBP's back references to LBP, pair and referral vault. These executions have empty synthetic storage and no storage reads, writes or logs; they do not execute the calculated getters or qualify current deployed state. The source captures have no AST, so declared immutable IDs are not represented as a newly compiled AST proof.

The adapter conservatively refuses pending calculations that need nonzero unused high-byte padding in hLBP slot 6, preserving the raw word. Exempt raw balances remain known with a separate pending refusal, and the model’s true pending early exits remain valid. The existing arithmetic model is unchanged.

The replay driver accepts only a local original cache package and a fresh output directory. It binds all 1,024 original PB hashes in `[122288006,122289030)` to the historical header/clock evidence, derives a fresh typed journal, and compares the unchanged 594 LBP observations (33 parent and 561 in the interval, comparing both balance and pending) and 329 reflection observations (31 parent and 298 in the interval). Expected getter amounts are never written into checkpoint or replay state. Historical sample agreement does not establish global holder coverage, current deployment/package qualification, arbitrary model epochs or a production calculated-balance implementation.

## Validation

Focused tests cover atomic late failures, global-only and idle calculations, raw-input provenance, missing and zero facts, distinct-key permutations, per-key ambiguity, protected dependency invalidation, complete reset, bounded undo and snapshot import. Reflection tests preserve duplicate and zero exclusion-array entries and source subtraction order.

The [final saved replay report](evidence/calculated-retention-20260929.json) and [source inventory](evidence/calculated-retention-20260929-source-inputs.json) cover every block in `[122288006,122289030)`. The original PB manifest and four complete source captures are independently checked before use. The executable checks its compiled input snapshots against disk, copies the source inventory and requires identical input hashes after replay. The newly derived portable journal exactly matches its committed regression fixture.

| Model | Registered pairs | Parent reference observations | Interval reference observations | Interval evaluations | Retained relevant writes |
| --- | ---: | ---: | ---: | ---: | ---: |
| LBP | 33 | 33 | 561 | 33,792 | 110 |
| BabyDoge | 19 | 19 | 190 | 19,456 | 26 |
| 10SET | 12 | 12 | 108 | 12,288 | 45 |

All 923 reference observations match. Each LBP observation compares both balance and pending reward, for 1,188 scalar comparisons; reflection contributes 329 balance observations. The separate 1,552 raw reflection storage comparisons comprise 145 at the checkpoint and 1,407 in the interval. The driver schedules 65,536 interval evaluations plus 64 initial comparisons, including 986 blocks with no protected storage effects. It observes 394 persisted storage effects across the four bound contracts and no protected code changes; the table counts only raw words retained by each model. The 512-block snapshot continuation and 16-block undo/replay preserve exact final state, including raw facts, provenance and counters.

The report SHA-256 is `d76a8004b6dc7b6429bfb5a19c49e34de915020f1968edc5cdac519a5b346ee6`; the inventory SHA-256 is `89a777735f7718598019b349b7ba9a92570deb8cca3a8fce33ab1866d75695de`. Complete offline workspace gates are recorded with the delivery validation. Neither the synthetic invalidation tests nor this unchanged-runtime historical window qualify a runtime transition.

Run the local diagnostic with the pinned toolchain and an isolated target directory:

```sh
cargo +1.88 run --locked --offline --release -p erc20-balances-tools \
  --bin replay_calculated_retention -- \
  /absolute/path/to/original/erc20/balances /fresh/output/directory
```

This command has no RPC, stream, sink or network client path.

The standalone LBP fixture helper also stages writes before committing and rejects mixed checkpoint hashes, duplicate canonical keys and zero update ordinals. The original atomicity and hash regressions were reproduced before the fix. Hashless synthetic arithmetic fixtures remain supported by that helper; the historical retention loader separately requires the exact pinned hash-bearing checkpoints.
