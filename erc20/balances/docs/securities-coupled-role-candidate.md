# SecuritiesToken coupled role candidates

These seventeen separate **NOT-QUALIFIED** candidates replace the broad role
permission shared by the selected SecuritiesToken beacon proxies. They preserve
the immutable 431-profile baseline and historical 425-profile qualification
cohort. The [Phase A operation proof](securities-operation-proof.md) is an
independently frozen prerequisite, not evidence of proxy execution or successful
initialization.

## Exact selected template

The opt-in identifier `securities_token_solc_0_8_24_oz_5_3_0` means the exact
reviewed ordered template for the selected compiler/runtime. It does not grant
universal OpenZeppelin 5.3 support. Later builds or profiles need independent
source/runtime binding and execution comparison. PToken's separately named mode
and legacy DSG's `oz_3_4_2` behavior and serialization remain unchanged.

The fixed ERC7201 roots are:

- Boolean membership: `0x02dd7bc7dec4dceedda775e58dd541e08a116c6c53815c0bd028192f7b626800`.
- Enumerable sets: `0xc1f6fe24621ce81ec5827caf0253cadb74709b061630e6b55e82371705932000`.

For the full bytes32 role `r`, boolean membership is
`H(address || H(r || membership_root))`; set length is `A=H(r || set_root)`;
array element `i` is `H(A)+i` modulo 2^256; and one-based position is
`H(address || (A+1))`. Address keys require zero high 96 bits in exact,
hash-verified 64-byte preimages.

| Operation | Ordered logical stages |
| --- | --- |
| Add absent member | Boolean0→1; length increment; append member; set position |
| Remove non-tail | Boolean1→0; copy tail to removed index; update tail position; clear tail; decrement length; delete removed position |
| Remove tail/sole | Boolean1→0; clear tail; decrement length; delete removed position |

Only source-executed equal zero-address append or zero-tail clear stores may be
omitted. Boolean, length and position changes remain required. No DSG self-swap
or clear template is added. Both namespaces are reserved and discovered from
the union of verified outer preimages. Boolean-only, set-only and recognized
equality fragments cannot become independent permissions. Each accepted record
belongs once to a unique complete operation within one account, role, root pair
and structural frame.

Initial boolean/position/array coherence and unused-tail zeroes are qualification
assumptions. The stateless mapper does not reconstruct that history. Moved-tail
membership equal to1 is a derived coherence constraint, not an observed store,
permission or required invented producer preimage. Observed and omitted stages
receive the same protected-key and cross-role alias checks. Block-local logical
and physical values must remain continuous. Logical growth at uint256 maximum
is conservatively refused: Phase A actually executed a wrapping push, so this
restriction must not be described as an EVM overflow revert.

## Fixed initializer admin boundary

`src/SecuritiesToken.sol` has one reachable `_setRoleAdmin` callsite: initialization
sets `ISSUER_ROLE` to `DEFAULT_ADMIN_ROLE` (zero). No public general admin setter
exists. The helper independently derives `ISSUER_ROLE=H("ISSUER_ROLE")` and the
only permitted admin word, `H(ISSUER_ROLE || membership_root)+1`:

`0xecfb03a241cc67499591701273c9840824101e710aea92d3ca0339671f5c64c1`.

The baseline's width-two membership rule implicitly covered this word; it was
not an explicit scalar. Each candidate removes that broad rule, appends this
exact word to `other_slots`, and adds the coupled rule. Selecting the new mode
requires that explicit scalar. Every persisted write to it must have a new value
of zero, even without a mapping preimage or when equal. The previous value is
not fabricated. Other role admin words and nonzero new values remain refused.
The word is protected from array/coherence aliases and remains a metadata
barrier; it cannot bridge an incomplete role operation. This structural rule
does not qualify initialization: successful initializer execution was excluded
from Phase A and deployment history remains unknown.

## Producer and guard boundaries

The selected mode requires Extended versions **4 or 5**, positive actual root-call
begin ordinals and strict structural frame containment. Version3 is refused even
without role writes; legacy DSG's version3 transaction-begin fallback remains
unchanged. Persisted foreign stores, including equal stores, and execution
boundaries including reverted children remain barriers. Numeric old/new words
retain the shared at-most 32-byte normalization; selected-account storage keys
must be exactly 32 bytes. Actual producer visibility of required changing stages
and preimages remains unqualified. Constant-folded roots cannot be repaired with
invented preimages.

The seventeen configurations are derived from exact immutable baseline bytes and
share all fields except contract. Their proxy hash, implementation/runtime,
beacon/hash/pointer slots, raw balance root, allowance root, scalars and every
other guard remain unchanged. No independent membership path, creation seed,
role cache, persistent-state rule, protobuf or dependency change is introduced.
Permitted string scalar roots do not admit unreviewed dynamic payload writes.
Raw `balanceOf` storage is distinct from timed UI conversions.

## Source binding and controls

The preparer and replay bind complete raw capture, fresh compiler output,
compiler report, operation report, compact transcripts and primary-source
artifact by their frozen Phase A hashes. They recheck all source classifications:
20 exact OpenZeppelin and three exact BEP dependencies; three pinned differing
BEP files and five unrecovered custom sources remain explicit gaps. Both raw
metadata strings retain independent exact hashes and complete parsed equality;
no source or metadata normalization is introduced. The full implementation
runtime matches the capture. On-chain creation and deployment evidence are
null; compiler creation equality and synthetic constructor execution do not
change that boundary. The proof's proxy/beacon, initialization, external
compliance/pause/transfer and timed UI limitations all remain.

Projector tests translate frozen actual compiled operations into explicitly
synthetic Extended proxy-storage frames. They cover all 35 permitted equality
representations for 29 selected operations, mixed sequential roles, malformed
source-domain outcomes, missing/corrupt/reordered/extra stages, boolean/set-only
fragments, fixed admin restrictions, metadata barriers, dirty words/preimages,
frame/account/role joins, aliases, conservative growth and unchanged guards.
Negative metadata cases include a valid balance write and require atomic refusal.
The counter control exercises five compiled operations at each of 17 configured
accounts, separately counting changed membership and the fixed admin scalar;
failed/reverted/no-op/malformed records cannot inflate these counts.

## Saved replay and qualification

The fresh [saved replay report](evidence/securities-coupled-role-candidate-20260928.json)
and [196-input source inventory](evidence/securities-coupled-role-candidate-20260928-source-inputs.json)
cover all 1,024 BSC blocks in **[122288006,122289030)** (Extended version 5).
Every clock matches the immutable canonical-bound clocks. All **110,139**
complete balance rows equal both the unchanged current 431-profile baseline
and immutable historical output. Against saved canonical RPC observations,
110,138 same-block rows and 4,012 retained observations match; 66,265 cold
observations remain unknown. The one baseline-only row is preserved unchanged.
The full cohort initializes 33,591 observed token/holder pairs, including
6,197 known zero balances. Canonical RPC values never initialize retained state.

For the seventeen candidates specifically, **2,217 same-block** and **26 retained**
observations match across **685 initialized token/holder pairs**. Another
**1,507 cold observations**, including 673 nonzero values, remain unknown.
The report lists each profile separately. There are **zero persisted role
membership writes, zero validated role operations and zero fixed admin writes**
in this interval: saved balance parity does not exercise those new paths.
Those paths instead have compiled-source and explicitly synthetic projector
controls, with real producer visibility still unqualified.

Fresh preparation reproduces both committed candidate artifacts exactly.
Pinned Rust 1.88 offline validation passed formatting, **1,031 workspace tests**
(76 suites, 53 nonempty), all-target Clippy with warnings denied and the WASM
workspace check. New controls comprise twelve projector tests, two candidate
binding tests and one replay counter test. Logs and failed attempts remain in
`out/securities-coupled-validator-20260928/` in `network-verification-config`.

No live RPC, Firehose/Substreams or sink calls are part of this work. Fresh
runtime/proxy history, producer role-write visibility, coherent initial state,
replacement-package/getter parity and final-holder qualification remain open.
