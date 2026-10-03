# Retained YBC raw inputs

A separate host reference retains the finite raw inputs of the reviewed YBC
reward model. It keeps the existing three calculated-retention bindings and the
pure YBC arithmetic unchanged. This is not a production layout, sink, complete
holder index or new live qualification.

The [model and historical evidence](ybc-reward-model.md) remain the arithmetic
prerequisite. The [new raw fixtures](https://github.com/pinax-network/substreams-evm-extended/tree/6dade8957887c0c278cfa8da6bef61b9cc22f534/erc20/balances/tools/tests/fixtures/ybc-retention) copy
both complete storage snapshots, the separate getter report, all original raw
state-override maps, token source capture, three runtime byte arrays and the
reviewed helper calls. Every original is checked by raw SHA before parsing.
The token runtime is reconstructed from saved compiler bytes with its sole exact
CBOR substitution. All three whole runtimes, helper literal addresses and
recorded call targets are checked. The helper source remains unavailable; a
complete source or fresh runtime execution proof is not implied.

The two historical boundaries are 122288005 and 122289029, with exactly 12
registered holders and 1,705 raw words each. The new decoder reproduces all 48
separate historical getter results and all 60 results from 30 saved override
cases, including the full ABI bytes for 17 expected reverts. Overrides remain
independent synthetic branches, never historical events. No expected getter or
decoded historical model state becomes a raw fact.

## Input and lifecycle contract

`ybc_retention` reuses PR105's clock, fact, storage/code-effect and persisted-effect
types without extending its three model descriptors. Its closed binding identifies
chain 56, the selected token/helper/pool runtimes, evidence pins, model and
persistence revisions, epoch and activation. Checkpoints carry an exact block
number/hash/timestamp and preserve absent historical parent/producer metadata.
Each observed word adds the exact block, positive ordinal and original PB digest.

The permitted finite key registry is distinct from the currently required getter
inputs. Key names from the final capture may define the bounded replay scope;
their values do not initialize the parent. Known hourly facts survive changes to
the required horizon. No missing word means zero, and no scan or allocation is
proportional to a large cursor gap. Evaluation performs at most 240 hourly steps.
Default limits are 12 holders, 8,192 registered keys, 16,384 effects per block and
16 undo snapshots; all have hard host bounds.

The decoder resolves the below-threshold static-rate, unopened-cycle and
already-claimed/future-cursor exits before requiring unreachable hourly/pool
inputs. Active future-launch subtraction is a known-input arithmetic refusal.
Skipped no-burn hours omit unused rate/reward inputs, while the first-hour rate
identity remains mandatory. Pool reward recursion and unreviewed pair orientation
are explicit scope refusals. It reports the first unresolved required raw key
when dependent branch discovery cannot continue; this is not an exhaustive list
of every potentially missing key.

For an active multi-hour path, the planner gathers required inputs before calling
the unchanged pure arithmetic. If a later required input is missing, it can report
Unknown even when an earlier fully known arithmetic step would revert. This is a
conservative planning limit, not a claim to reproduce every partial-input error
in exact source execution order. It never converts missing evidence into Known.

Outputs distinguish stored raw basis, pending reward with stopping hour, and the
observable raw-plus-reward amount. The stopping hour is not a balance or a state
mutation. A successful reward can coexist with an unknown raw basis or an
observable-amount addition overflow. Missing inputs, known arithmetic failure,
scope refusal and changed-dependency suspension remain separate outcomes.
Each evaluation binds its exact input-state digest and accessed facts/provenance.

Apply is atomic across facts, clocks, counters and undo. Same-key effects require
strictly increasing positive ordinals and exact old/new continuity, including
no-ops. Different keys may commute at equal ordinals. Failed and reverted effects
are filtered by unchanged persistence rules. Gaps, forks, version/timestamp
changes, malformed words, late-block errors and resource limits refuse without
committing a prefix. Code changes, helper-pointer excursions and unreviewed
helper-storage changes suspend, even if restored in the same block. Reset needs
a separately initialized checkpoint at the exact current boundary, a newer epoch,
reviewed runtime state and all currently required inputs; no old facts or undo
history carry. Digest-checked restart snapshots preserve facts, not undo history.

## Saved replay scope

The local replay consumes the original hash-pinned Extended PBs in
**[122288006,122289030)**. It initializes only the parent checkpoint's 12 holders,
retains observed registered words, journals every selected persisted effect and
compares available final facts/getters to the separately held final checkpoint.
The final checkpoint is also imported independently. Each raw PB digest must
match the original manifest; headers alone are insufficient. Compiled source
snapshots prevent a stale executable from attributing newer disk files to a run.
Reports preserve partial attempts and distinguish each metric's Known, Unknown,
model-refusal, scope-refusal and suspension counts.

The captured active holder `0x2aafce28b5552e216811fb5a1cb015178a29b461` advances
from hour 1695 to 3135 between the two captures. Its later hourly keys cannot be
seeded from the final capture during replay. Continuous known evaluation is not
assumed: missing new ranges must be reported, and there are no independent
intermediate getter captures.

The final [saved replay report](evidence/ybc-retained-inputs-20260929.json) and
[41-file source inventory](evidence/ybc-retained-inputs-20260929-source-inputs.json)
record all 1,024 producer-v5 blocks and 12,288 holder evaluations. Stored raw basis
is known for every evaluation. Pending reward and observable amount are known
11,380 times and unknown 908 times, with no model/scope refusals or suspensions in
this particular journal. The active holder above has 116 known evaluations before
its first unknown at block 122288122; the other eleven holders remain evaluable.

There are 264 selected persisted storage effects, of which 181 update registered
retained keys, and no selected code effects. At the final boundary, 746 retained
raw words match independently captured values and 959 independently captured words
remain absent from retained state. Eleven observable balances match the
independently captured final getters; the twelfth remains Unknown in continuation,
while importing its separate
final checkpoint makes it evaluable. These finite counts do not establish
intermediate getter parity, complete history or globally complete holders.

The portable journal regression reproduces these results, restores an exact
midpoint snapshot, compares the continuation, and undoes/reapplies the final sixteen
blocks. Original PB digests, clock chains and regenerated journal bytes are
checked separately. Final replay report SHA-256 is
`21529fa98f1ec4bee23aed0144c1dc2f0276351708855ed6fd27aecd21903028`;
inventory SHA-256 is
`2e05f9e4e3f43932cdfb08e2eeb1b7b6a32dc82ddf43726e304df21c7c5ffb05`.

Run focused host tests with the pinned toolchain and lockfile:

```sh
cargo test --locked --offline -p erc20-balances-tools --test ybc_retention
```

The final pinned, locked offline checks passed on the integrated workspace:
1,416 tests across 141 suites (93 nonempty), zero failed or ignored, all-target
Clippy, formatting and the WASM workspace check. Twenty new host regressions
cover the raw decoder, missing/known-zero inputs, lifecycle/refusal boundaries,
original-PB binding and portable journal continuation. These checks qualify
neither a deployment nor a new live package.

OG retention is deferred. Complete helper source, recursive/unreviewed branches,
current runtime continuity, production affected-holder extraction, new WASM
package parity and globally complete holders remain outside this evidence.
