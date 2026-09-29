# Mai coupled-role candidate — NOT QUALIFIED

The separate BSC candidate for `0x35803e77c3163fed8a942536c1c8e0d5bf90f906`
selects `mai_solc_0_8_9_oz_4_7_0`. It removes only the historical profile's broad
membership mapping at root 0 and width-two set mapping at root 1. Complete
coupled operations replace those permissions. Balance root 2, allowance root 3,
scalar slots 4/5/6 and the exact runtime hash remain unchanged. The historical
431-profile baseline and qualified 425-profile cohort are not modified.

The helper restores both removed entries and removes the selected rule to
compare the complete profile against the immutable baseline. No independent
membership, admin, proxy, deployment or creation permission is added. All 15
source files contain only the inherited `_setRoleAdmin` definition at
`AccessControl.sol:214`; there is no source callsite for it.

## Source and operation boundary

The [host proof](mai-operation-proof.md) binds solc
`0.8.9+commit.e5eed63a`, with the optimizer disabled and all original settings
preserved. Its complete 1,632-call execution contains 1,427 returns and 205
source reverts, 445,655 instructions and 4,334 exact source annotations.
Fourteen full dependency bodies match OpenZeppelin 4.7.0 at
`8c49ad74eae76ee389d038780d407cf90b4ae1de`. The custom UNLICENSED
`contracts/Mai.sol` still has no independently established maintainer revision.

The capture SHA-256 is
`e5fb99260765d58a5fecdd4d2eccd61c84e6f0032821fc75402f00e93535386b`.
Creation is exactly 12,772 bytes with no constructor append. Runtime is 11,296
bytes, with only cap immutable ID 1258 at byte 1767 patched to the source-derived
10^29. Its Keccak is
`0x1134938644214395aba790bf7d50e13027c30b74b4bd09dcbe167c0ccc2cc9e5`.
No metadata replacement or byte stripping is involved. Synthetic construction
returns that runtime but does not admit deployment or seed roles or holders.

The selected coherent store order is:

- Grant: boolean `0→1`, length increment, appended element, one-based index.
- Non-tail removal: boolean `1→0`, destination replacement, moved index,
  tail clear, length decrement, removed index clear.
- Tail or sole removal: boolean `1→0`, tail clear, length decrement,
  removed index clear. There is no unconditional legacy self-swap.

The exact grant PCs are 3060/5366/5394/5426; non-tail removal uses
5841/7169/7195/7238/7240/7265; tail removal uses 5841/7238/7240/7265.
RoleGranted/RoleRevoked logs occur after the boolean and before set stores.
These locations establish the host evidence. Ingestion uses complete physical
storage, Keccak and frame witnesses; it does not infer authorization or logs.

The older void-super override always invokes the set mutation. Malformed
prestates can therefore succeed with boolean-only grants/revokes or set-only
grants/revokes. All four are refused. Dirty packed booleans, dirty address words,
malformed indices and maximum-length wrapping also remain outside admission.
Checked logical growth is conservative even where the compiled source wraps.
Duplicate/absent calls emit no role changes and do not prove hidden coherence.

Only source-executed equal zero stores may be absent. Their inferred logical
stages still participate in whole-block continuity, protected-alias checks and
foreign-store/call barriers. Moved-tail boolean coherence is a constraint,
never a new permission or an invented runtime preimage. Complete operations
must stay in one account, role and actual frame. This selected template requires
Extended v4/v5 and a positive actual root begin; no v3 fallback is introduced.
Other selected templates retain their prior behavior.

Mai additionally protects preimage-witnessed descendants of its preserved
metadata mappings, including allowance root 3. Observed and inferred equal
array stages cannot collide with those known locations. This restriction is
specific to the new Mai mode; it grants no metadata permission and changes no
older selected template. Unrelated noops outside an operation retain the
mapper's ordinary filtering policy.

## Evidence and validation

The initial unsupported-selector failure is preserved in
`out/mai-coupled-role-20260929/01-pre-mode-red.log`, with the exact test source.
The preparation CLI preserves its own and the helper's as-run sources and
hashes. It rechecks all final Phase A artifacts, complete capture/compiler
bindings, original cache identity and exact candidate restoration.
The raw-byte binding covers all 16 Phase A evidence/fixture files and checks the
compiler/operation artifact links and their common 237-file source inventory.
A reproduced whitespace-only input drift is preserved in `04-raw-input-red.log`;
`source-02` contains the corrected preparation and as-run helper. Final
`source-03` rechecks the same bindings and produces byte-identical candidate
and source-review fixtures.
The first projector attempts also remain saved: `09-projector-tests-first.log`
exposed an allowance-leaf collision and an overstrict unrelated-noop test;
`10-projector-allowance-red.log` isolates the collision. The Mai-only guard
passes all 12 projector groups plus 335 existing library tests in
`11-mai-guard-green.log`.

Dedicated projector tests translate the source proof into synthetic Extended
records. They test complete operations and equal-stage subsets, the four
one-sided successes, malformed effects and preimages, aliases, continuity,
transaction/frame barriers, persistence filtering, metadata preservation and
runtime/creation refusal. Synthetic Extended records do not qualify actual
producer visibility.

The final [saved replay report](evidence/mai-coupled-role-candidate-20260929.json)
and [source inventory](evidence/mai-coupled-role-candidate-20260929-source-inputs.json)
cover **[122288006, 122289030)**, all 1,024 saved Extended v5 blocks. The
candidate matches the unchanged current baseline and historical complete
protobuf output on every block: 110,139 native rows, all clocks and block hashes
equal, and no differences. Across the configured cohort, 110,138 native rows
have same-block canonical matches, one is native-only, and 4,012 reference-only
rows match independently retained state; 66,265 reference observations remain
cold. These coverage differences are preserved rather than hidden.

Mai emits **83 rows**, all with same-block canonical matches, and independently
initializes **49 observed holders**. Its **13 cold reference observations**,
including **8 nonzero** values, remain unknown; there are no retained-only Mai
matches. No persisted membership writes or validated coupled role operations
occur in this window. Reference observations never seed holder state. This
replay therefore establishes saved balance parity, not real role visibility.

The report SHA-256 is
`9ca05775f655b5422315b4d2c40e61db60d3d5cf7e0c7696f64541615d77570f`;
the 230-file inventory SHA-256 is
`f8a5debf4322cbdc8860dc75e99e15c3be8769996a5cda3e190ad1764d258f32`.
All source inputs remained unchanged throughout `replay-01`.

Pinned Rust 1.88 validation with a unique worktree target passed on actual
PR101 merge `86cdda76b1e4699e967ef56d529edea7436daee8`: **1,254 tests in 116
suites (77 nonempty)**, all-target Clippy with warnings denied, workspace WASM
check, formatting and staged whitespace checks. Logs are preserved under
`out/mai-coupled-role-20260929/`; no live service was contacted.

Custom-source attribution, deployed initial-set coherence, zero unused tails,
producer completeness and replacement package/runtime/getter/holder
qualification remain open. This offline candidate makes no live RPC, stream or
sink request and changes no VM, dependency, ABI, protobuf or persistence rule.
