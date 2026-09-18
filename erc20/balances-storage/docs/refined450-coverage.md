# Five BSC additions and two role-layout corrections

USDe, CONCILIUM, GIGGLE, LABUBU and ARKIE pass independent packaged RPC and
initialized-holder checks across **122288006–122289029**, inclusive. The
explicit test configuration contains **431 profiles among the first 450
RPC-stream candidates**. MUSD and OLY replace two existing configurations;
they do not increase that count. Ranks and token labels identify the sampled
contracts and activity, not market capitalization or issuer identity.

| Cohort | New profiles | Emitted RPC checks | Initialized observations | Carry-forward matches | Final holders | Final zeros |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| USDe and CONCILIUM | 2 | 34 | 54 | 20 | 24 | 10 |
| GIGGLE, LABUBU and ARKIE | 3 | 51 | 87 | 36 | 35 | 9 |
| **New profiles total** | **5** | **85** | **141** | **56** | **59** | **19** |
| MUSD and OLY replacements, counted separately | **0** | **204** | **340** | **136** | **88** | **25** |

All completed gates have zero value mismatches. The additions include 12
emitted zeros. Their parent checkpoints use 118 independent balance/storage
reads; processing makes no balance RPC calls. Reference observations never
repair retained state. The replacement cohort separately rechecks 26 emitted
zeros and uses 176 checkpoint reads.

## Storage behavior covered

[USDe and CONCILIUM](bridge450-coverage.md) retain direct balance getters with
reviewed bridge, fee and pool metadata. USDe has a four-word terminal rate
record; CONCILIUM's fixed address array requires four explicit slots.
Dynamic option payloads remain unqualified.

[GIGGLE, LABUBU and ARKIE](trade450-coverage.md) cover transfer metadata,
immutable runtime fields, mixed packed words and LABUBU's fixed six-element,
three-word price array. LABUBU's original refusals came from its reentrancy
guard, which does not affect `balanceOf`. Captured regressions preserve both
original failures and verify the explicit slot correction. A transport failure
in the first RPC audit is retained separately from the successful fresh run.

[MUSD and OLY](role-width-coverage.md) remove the recursive two-word role
exception. Their pinned sources have no reachable admin-role writer. The
narrower layouts reject the reviewed admin and adjacent membership writes
while retaining the same historical output. This fixes an overly permissive
guard, not an observed balance mismatch.

The [wider role-storage audit](role-shape-audit.md) records remaining boundaries
in 33 boolean-role and eight enumerable-role profiles. Some contracts
legitimately write role-admin fields, so the same narrowing cannot apply to
all of them. Exact-depth field recognition remains a separate follow-up;
the current results do not establish that every unknown nested write rejects.

## Combined evidence and limits

The [combined report](evidence/refined450-combined.json) uses
[`bsc-refined450-layouts.json`](../tests/fixtures/bsc-refined450-layouts.json).
The fresh actual-WASM capture matches every protobuf event field for
**110,139 previously RPC-verified balances**, including **15,259 zeros**.
There are 430 emitting profiles; hLBP retains separate older mint evidence.
Identical histories preserve evidence for **180,415 initialized observations**,
including **70,277 without a new event**.

The combined comparison binds the previous 426-profile output, the five
newly audited profiles and both replacement layouts to canonical clocks,
runtime/package hashes and immutable evidence. The replacement cohort's
204 freshly rechecked rows must equal the corresponding baseline rows in
every field and every block. Its rows and holder observations are not added
twice. Qualifier, layout, RPC, checkpoint, reference and retained-holder
digests are checked before export.

The Rust capture tool now checks complete streamed block identities even when
every block emits a row. Previously, this extra clock check ran only for sparse
output. A dense-output regression verifies rejection of a wrong block identity
and exact preservation of every event after successful validation. The fresh
combined capture binds all 1,024 clocks to canonical headers and the parent
checkpoint, in addition to comparing output fields.

This reuses historical RPC evidence with a fresh packaged capture; it is not
a new full-431 holder checkpoint or global holder enumeration. Cold replay
of the additions retains **56 unknown observations, including 33 nonzero
balances**. The replacement cohort separately retains 130 unknown
observations, 91 of them nonzero. Neither set is assumed zero.

The [summary](evidence/refined450-summary.json) lists 19 unqualified candidates:
seven from the first 400 and 12 from ranks 401–450. Calculated and reflection
balance models retain their separate host-only evidence. Ethereum, Base,
HyperEVM and Arc still require independent qualification after BSC.

The production SPKG remains
`f1d57bdff549947cd69e47d117ad8cd74933c1e065b687f958fea4de39004d81`;
its WASM remains
`36f5c502ec6546fc842ffaf8cfc692140d10e7f8cb58ff731c259a1b6d48c117`.
The interface remains one RPC-free `map_events`, the shared balance protobuf,
explicit layouts and default parameters `[]`. All executable diagnostics and
regressions are Rust.
