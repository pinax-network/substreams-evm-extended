# Session handoff (2026-09-21): roadmap state, evidence, and how to continue

**Current issue status (2026-09-28):** use [Follow-up work](follow-up.md).
The earlier remediation is merged in #46, subsequent hardening/BSC evidence
is merged through #62, and 13 issues remain open for explicit acceptance gaps.
The dated checkpoints below preserve historical counts and artifact locations;
they are not the current issue-status list.

The ERC-20 host verifier now takes explicit network/expected-chain options,
checks the RPC identity before data requests, and keeps caller-labeled offline
blocks distinct from chain-verified reports. Non-BSC capture requires explicit
RPC and stream endpoints. The loopback CLI regression covers every RPC command;
it does not resume or qualify any network under #8. Authentication fallback is
restricted to the canonical HTTPS BSC Pinax RPC origin; custom providers need an
explicit RPC key if they require authentication. See the
[host verification instructions](../erc20/balances/README.md).

The five protocol balance-state extractors now use an exclusive invalidation
cutoff and omit partial end-of-block state for invalidated markets. Pre-cutoff
unknown writes still fail, every guarded transition remains evidence, and
unaffected markets keep their rows. The host consumer discards basis when
rebinding after suspension, including when storage is compatible, because
missing observations break continuity. Synthetic projector-to-consumer tests
cover suspension, rebound, undo and replacement blocks. Aave's specification
revision became 2; Compound v2/v3, Lido and ERC-4626 became revision 3.
The later Lido arithmetic correction described below advances Lido to revision 4. Historical
live evidence keeps its original package digests and does not qualify these
newly built packages. See the [balance-state contract](balance-state-contract.md).

Validation uses a separate Cargo target directory for each checkout. During
this follow-up, a shared `CARGO_TARGET_DIR` reused protocol test binaries in a
different worktree whose source had fewer tests. Those local runs were discarded
as branch validation and replaced with fresh isolated builds; independent GitHub
CI still gates every merge. Preserve the earlier logs as failed validation
attempts, and do not share build output between worktrees.

The host-only `common/retention::protocol::ProtocolLedger` now retains original
holder/global rows, complete clocks, epoch declarations and dependencies with
atomic block application and bounded snapshot undo. `conformance::retained`
evaluates Aave, static-aToken, sDAI, OZ virtual-offset, Lido, selected 2019
cUSDC/cETH and USDC Comet inputs. Qualification is explicit:
the caller supplies exact stream/model/storage bindings and external runtime
hash evidence at the model's BOUND or checkpoint origin. The library validates
that binding but does not fetch or establish its chain truth. Missing inputs
remain unknown; log evidence never initializes stored getter inputs, derived
values expire, and interrupted epochs cannot carry stale state into a rebind.
Tests cover synthetic lifecycle cases and one captured Aave Pool-index oracle
with synthetic holder/bootstrap inputs. This is a host reference implementation
with full-state undo snapshots, not a production sink or package qualification.
See [initialization](initialization-and-completeness.md) and
[conformance](../conformance/README.md) for supported metrics and limits.

The Compound adapters keep raw shares/signed principal, stored conversions,
projected claims and index values distinct, with explicit result units. Actual
projectors feed synthetic blocks 10–13 through donation-only cash changes,
idle accrual, signed-principal transitions, missing inputs, invalidation and
undo. Comet projection checks the source's uint40 timestamp limit and accepts
an explicitly observed zero index; absence remains unknown. The cETH WhitePaper
constructor-set storage values need independent attestation as well as code
identity. A changed parameter digest is rejected by the old ledger: switching
streams requires a separately qualified checkpoint and new ledger, not an
assumed same-stream successor. These tests use synthetic runtime attestations
and initialized observed holders; they supply no Ethereum qualification.
Validation logs are under `out/compound-retention-20260928/` in the
`network-verification-config` worktree, including the reproduced zero-index
failure and its regression. No extractor, protobuf or package changes.

The Compound v2 host model now checks every uint256 arithmetic intermediate,
including products later divided down and additions later reduced by reserves.
Eight regressions reproduced the previous overflow acceptance. Five independent
source-execution tests make 2,676 calls into controlled harnesses compiled from
the pinned current CToken, 2019 CToken/WhitePaper and legacy jump-rate sources.
They distinguish legacy returned errors and unchanged state from current
reverts, and preserve revision-specific same-block behavior. The full isolated
workspace run passed 769 tests. See the
[oracle report](../conformance/fixtures/compound-v2-oracle/README.md) for exact
harness substitutions, official compiler/source/artifact hashes and limits.
Logs, earlier failures and an initial compiler build with unverified provenance
remain under `out/compound-checked-20260928/` in the
`protocol-invalidation-boundary` worktree; `source-04/` is the final official
compiler regeneration. This is arithmetic evidence for #14/#16, not deployed
runtime, holder, getter or package qualification.

The [Comet source oracle](../conformance/fixtures/comet-oracle/README.md)
adds 1,515 model comparisons and two ABI-width refusal controls against pinned
compiled source. It preserves whole rate, utilization, both-index and getter
functions, with controlled rate/principal storage and an explicit timestamp;
constructors and holder mapping reads remain outside the harness. The official
solc 0.8.15 binary, six source files, generated harness, selectors, packed layout
and runtime are recorded. The bounded VM adds only TIMESTAMP, SIGNEXTEND and
SGT, with direct controls and its original zero-clock entrypoint retained for
the existing oracles. Logs/builds live under `out/comet-oracle-20260928/` in
`network-verification-config`; `source-03/` is the final capture and earlier
attempts remain preserved. No deployed runtime, Ethereum interval, initialized
holder or package qualification is inferred from this arithmetic evidence.

The [Lido source oracle](../conformance/fixtures/lido-oracle/README.md)
reproduces three accepted-input mismatches against the pinned Solidity 0.4.24
getter bodies: the holder and external-share products wrap to uint256 before
division, while the final pooled-ether addition uses checked `SafeMath.add`.
The host model now follows that order. The production extractor's derived
total uses the same wrapping and omits only that derived row on addition
overflow, preserving the six raw observed fields. Lido's specification
revision is 4; its contract/model version remains 4, with no wire or dependency
change. The bounded harness relocates three packed storage positions and
substitutes one holder read; it does not qualify a deployed runtime or package.
The edge cases are synthetic storage-domain controls, not a claim that these
states occurred on Ethereum. Reproduced failures and fresh validation remain
under `out/lido-oracle-20260928/` in `bsc-exclusion-review`.

The [known-zero reference review](research/11-Known_zero_reference_inputs.json)
removes numeric missing-state sentinels from Aave index and SavingsDai `chi`
evaluation. The host ledger still rejects absent facts. A projected zero `chi`
previously caused a Rust division panic; floor conversion now returns an
explicit error, while SavingsDai's rounded-up helper preserves its source
zero-numerator branch. Required normalization and projection errors still run
before a zero result. Synthetic pure/retained regressions preserve those
branches, idle clocks and undo for one explicitly initialized observed holder
per case over blocks 10–11. Logs and reproduced failures are under
`out/known-zero-reference-20260928/` in `protocol-invalidation-boundary`.
This changes host reference evaluation only and makes no claim that these
zero states were observed on-chain or qualify a deployment.

The [SavingsDai source oracle](../conformance/fixtures/sdai-oracle/README.md)
executes the pinned `_rpow`, `_divup`, conversions, previews and `maxWithdraw`
with controlled Pot scalar inputs, a holder value and an explicit clock.
It preserves their source arithmetic and branch order. A source-domain
regression found that the retained bridge narrowed uint256 `Pot.rho` to u64:
an observed value above the current clock should use stored `chi`, but was
rejected. The pure model and bridge now preserve full uint256 `rho`, while
the block clock remains u64. Missing facts remain unknown. The test-only VM
now distinguishes return, revert and invalid-opcode exits; the modern entrypoint
accepts empty source reverts but rejects invalid opcodes, while older oracle
callers keep their existing behavior. Source/compiler artifacts, reproduced
failures and isolated validation remain under `out/sdai-oracle-20260928/` in
`network-verification-config`. These synthetic input and source-execution
controls do not qualify Pot storage history, a deployed vault or a package.

The [Aave/static-aToken source oracles](../conformance/fixtures/aave-static-oracle/README.md)
add 5,103 compiled comparisons across the selected current Floor and v3.4
HalfUp Aave getters, reserve normalization/liquidity updates and static-token
conversion, preview and withdrawal-limit paths. The stored index now checks
the source's uint128 cast while read-only normalization remains uint256.
Static division now evaluates checked numerator multiplication, addition and
subtraction before division, preserving exact overflow/underflow failure order.
Both pinned aToken bases use uint120 holder storage; controlled reserve inputs
use uint128 lanes and uint40 timestamps. Full source, licenses, official solc
0.8.27 provenance and the SafeCast dependency Gitlinks are retained. The Paris
compiler target is explicit. These harnesses substitute external/holder reads
and keep the debt cache zero; they do not execute full reserve updates or
qualify deployed Pool dependencies, package output or initialized checkpoints.
The final regeneration and preserved red tests/earlier attempts are under
`out/aave-static-oracles-20260928/source-04/` and its parent in
`bsc-exclusion-review`. No production extractor or VM change is involved.

The [offline six-exclusion review](../erc20/balances/docs/bsc-exclusions-offline-2026-09-28.md)
preserves the original TAKE/RADR/TOPS refused transactions and prepares only a
TAKE guard-slot candidate. Exact complete source-capture hashes bind saved
compiler/runtime reconstruction; this is not a new compiler or chain run.
Across saved BSC [123561000, 123562024), all 8,436 emitted candidate rows match
the immutable canonical reference. The emitted-only ledger initializes 207
observed holders; 6,586 reference observations remain cold. The 431-profile
baseline and qualified 425-profile set are unchanged, and all six exclusions
remain open under #61. TOPS's production array path remains unimplemented;
RADR source and BNC4/sPro/swkeyDAO2 replacement dependencies remain unqualified.
The associated production fix rejects oversized persisted storage keys and
values, including no-ops, before configured-token or beacon metadata ignores.
See the linked report for preserved failed attempts, isolated validation and
the separate runtime, package and holder gates before candidate promotion.

The separate [TOPS host operation proof](../erc20/balances/docs/tops-operation-proof.md)
reproduces the complete original compiler output and saved creation/runtime
binding, including all 13 immutable identities and 91 replacement sites. Eight
dependencies have exact primary matches; the custom token's independent source
pin remains unresolved. The original runtime executes append and getter controls
against synthetic state. An independently compiled harness preserves the exact
private cleanup body, root-31 three-word records and root-32 credit storage.
Its cleanup evidence proves source semantics, not the deployed transfer path or
its write order. It includes prefix compaction, wrapping sums and credits,
checked getter failures and the no-mutation branch when the expired sum wraps
to zero. Original constructor and transfer attempts stop before unsupported
external calls and roll back; they are not successful execution evidence.
Optional host TIMESTAMP is explicit, with absent context still failing closed.
The captured five-pop subsequence remains a separately bounded historical
comparison. This proof adds no production array permission or candidate; all
six exclusions and their runtime/package/getter/holder gates remain open.

The subsequent [offline issue-audit remediation](audit-remediation-2026-09-21.md)
tracks retention correctness, executed layout tests, ERC-4626 arithmetic and
asset bindings, Aragon upgrade guards, execution receipt validation and a
bounded legacy-profile candidate migration. Consult that record for the
follow-up implementation and validation; the historical evidence below keeps
its original scope.

The [BurnMint role candidate](../erc20/balances/docs/burnmint-role-candidate.md)
extends #4's offline per-profile review to
`0xac23b90a79504865d52b49b327328411a23d4db2`. Its root-5 rule permits only the
source-bound `[bytes32 role, address member]` mapping word. All other profile
fields and both historical/qualified cohorts are unchanged. The complete
cached capture, 14 primary source files, saved layout/compiler metadata and
three immutable substitutions are bound; the only primary-source difference
is an explicitly checked interface import relocation. This is saved runtime
reconstruction, not a new compilation or current runtime check. Source-derived
shape tests preserve balance writes and reject unreachable admin/adjacent and
malformed paths. The saved replay tool now independently binds reference
network, interval, fork boundaries and its original historical digest before
comparison. New artifacts and validation remain under
`out/burnmint-role-20260928/` in `bsc-exclusion-review`; the report preserves
the exact interval, observed-holder counts and remaining promotion gates.

The separate [Point/Bedrock role candidates](../erc20/balances/docs/point-bedrock-role-candidates.md)
narrow only the legacy role rules at roots 0 and 5 to the exact two-key
membership path. Complete cached captures bind all 23 source files, compiler
inputs/outputs, layouts and saved runtimes without immutable substitutions or
metadata transformations. Bedrock's token and dependencies match immutable
public source pins; Point's ten dependencies also match, but its independent
token repository remains an explicit gap. Shape tests distinguish membership
from role-admin writes and preserve Bedrock's freeze scalar and user mapping.
The Rust preparation and replay mode preserve both earlier candidate modes
and the immutable canonical-reference checks. Fresh outputs and failed attempts
remain under `out/point-bedrock-roles-20260928/` in
`network-verification-config`. The final replay covers BSC
[122288006, 122289030), matching all 110,139 baseline rows. Point contributes
28 same-block reference matches and 11 initialized observed holders; Bedrock
contributes 30 and 18. Their 18 and 19 cold observations remain unknown, and
neither has a persisted role-membership write in this window. No candidate is
promoted or independently qualified by these source-shape and historical
replay controls.

The separate [FHE/B2 role candidates](../erc20/balances/docs/fhe-b2-role-candidates.md)
bind all 51 source files to exact public commits. Their dedicated verifier
reconstructs only the seven FHE and eight B2 immutable words, independently
computing EIP-712/ShortString values and B2's cap while preserving original
metadata and full runtime hashes. Point/Bedrock's no-transformation policy
remains intact. FHE's role root appears in both legacy mapping lists; both
permissions are removed before adding its exact membership path. B2's single
width-two rule is narrowed separately. CCIP administration, pause state,
nonces, allowances and balance handling remain unchanged. The complete saved
BSC interval [122288006, 122289030) matches all 110,139 baseline rows. FHE has
18 same-block reference matches and 6 initialized observed holders; B2 has
40 and 12. Their 18 and 41 cold observations remain unknown, and neither has
a captured membership write in this window. New outputs and preserved attempts
remain under
`out/fhe-b2-role-20260928/` in `network-verification-config`; neither candidate
is promoted by source-shape tests or saved compiler reconstruction.

The separate [BAS role candidate](../erc20/balances/docs/bas-role-candidate.md)
replaces only root 6's legacy width with exact membership and the single
source-derived PAUSER admin word. Its constructor sets that word; other role
admins remain guarded. Fixed-slot permission does not enforce authorization,
the stored role value or constructor context, and the existing runtime guard
still refuses unqualified creation. All 14 captured source files, saved
compiler/runtime bytes, two cap substitutions and the independently encoded
288-byte constructor append are bound. Thirteen dependencies match pinned
OpenZeppelin source, but none of six checked public token revisions matches
the captured token; the missing event declaration/emit is not normalized away.
The original capture's `match` labels remain unchanged. As-run preparation,
validation and replay artifacts are under `out/bas-role-20260928/` in
`bsc-exclusion-review`; `source-03/` preserves the final preparation source.
The candidate remains NOT-QUALIFIED, and both published cohorts are unchanged.

The separate [Tagger role candidate](../erc20/balances/docs/tagger-role-candidate.md)
preserves the source's owner-authorized arbitrary-role admin setter through
an exact outer `[bytes32]` path at offset 1, alongside root 6's nested
membership path. Owner bypass of membership checks and self-only renouncement
remain source facts; the mapper validates storage shapes, not authorization.
Every other metadata field and both historical cohorts remain unchanged.
The complete flattened capture binds compiler input/output, layout and full
runtime/creation bytes. Only one exact 53-byte CBOR replacement per bytecode
array is permitted, including an unchanged creation suffix; no immutable,
link or constructor-argument substitution is allowed. The original `match`
labels remain intact. An independent public token pin and separately verified
upstream dependency files remain unresolved. Final preparation and validation
artifacts are under `out/tagger-role-20260928/` in
`network-verification-config`; historical package evidence does not qualify
this candidate or establish actual admin/member operation visibility.

The separate [Artx role candidate](../erc20/balances/docs/artx-role-candidate.md)
narrows only root 151 to exact membership, preserving the proxy's runtime,
implementation address/hash, EIP-1967 pointer guard and all unrelated storage.
It introduces no admin permission. Both complete captures bind 35 input files;
34 dependencies match immutable OpenZeppelin pins, while the BUSL-1.1 token's
independent public source remains unresolved. The proxy capture has 14 input
files but exactly eight selected output/metadata source IDs; the implementation
has 21 aligned files. The verifier preserves those different sets explicitly.
Saved bytecode reconstruction permits only three implementation self-address
substitutions and independently encodes the exact 480-byte proxy constructor
append. Zero initial supply and empty mint arrays do not seed holder state or
qualify deployment, current pointer or owner. Both creation guards remain
active. Preparation, failed attempts and validation artifacts are under
`out/artx-role-20260928/` in `bsc-exclusion-review`; the candidate and both
unchanged historical cohorts retain their separate qualification boundaries.

The separate [Kgen/Deep candidates](../erc20/balances/docs/oft-role-candidates.md)
narrow only the plain role namespaces: root 10 membership for Kgen, and
ERC-7201 membership plus three fixed initializer admin words for Deep.
Kgen's forwarder-array and both tokens' long option bytes retain their existing
handling; role-path permission does not establish authorization or initializer
context. Three complete captures preserve 133 source entries, including 123
exact direct upstream matches and one exact Kgen vendored interface. Kgen's
public token differs in whitespace; Deep retains seven custom-source gaps and
a nonmatching upstream interface. These differences are never normalized away.
All 26 immutable sites and complete constructor appends are independently
reconstructed from saved evidence. Deep's proxy constructor targets an older
implementation with an opaque initializer; a matching nonce-one ProxyAdmin
derivation does not establish its CREATE history. The later implementation
guard and creation refusals for all three bound addresses remain. Preparation,
failed attempts and validation are retained under `out/oft-role-candidates-20260928/` in
`bsc-exclusion-review`. Both candidates remain NOT-QUALIFIED, with actual
runtime/package/holder and producer role-operation gates outstanding.

The host-only [PToken operation proof](../erc20/balances/docs/ptoken-operation-proof.md)
is the source-execution prerequisite for the subsequent coupled-role rule.
Official solc 0.8.28 reproduces the complete saved runtime and creation bytes
with original settings and only an output-selection addition. Twenty pinned
OpenZeppelin files match exactly; the token's independent primary source remains
unresolved. The bounded Rust executor checks constructor and ABI operations
against synthetic state, preserving ordered stores, logs, Keccak inputs,
source locations and attempted effects on rollback. It distinguishes coherent
role operations from malformed-prestate controls, including wrapping maximum
array length. Selected tail removal clears the array cell before decreasing
length and has no DSG-style self-swap stores. The Phase A proof itself added
no production validator or profile. Failed attempts and full traces remain
under `out/ptoken-coupled-role-20260928/` in
`network-verification-config`.

The subsequent [PToken coupled candidate](../erc20/balances/docs/ptoken-coupled-role-candidate.md)
links root-5 membership and the root-6 set under an explicit selected-runtime
semantics value. Both roots are reserved; complete ordered same-frame operations
must consume every recognized record. Only source-proven zero-member equality
stores may be omitted. Moved-tail membership is a coherence constraint, never
an extra write permission. Observed and inferred stages retain alias, barrier
and block-local continuity checks. Logical length overflow is conservatively
refused, without claiming the compiled malformed-state push reverts.
The mode requires Extended v4/v5 and positive actual call boundaries; DSG's
legacy semantics and v3 fallback remain separate. Its candidate removes the
independent boolean allowance while preserving balance, metadata, runtime and
creation guards. Validation and saved replay are retained separately under
`out/ptoken-coupled-validator-20260928/` in `network-verification-config`.
The candidate remains NOT-QUALIFIED: source provenance, real initial-set
coherence, actual producer visibility and runtime/package/getter/holder gates
remain open. Historical cohorts and Phase A evidence are unchanged.

The separate [BTR host operation proof](../erc20/balances/docs/btr-operation-proof.md)
reconstructs both saved implementation and proxy artifacts using official solc
0.8.24 and the complete original inputs. Thirty implementation dependencies and
eight proxy dependencies match their pinned primary sources; the custom
UNLICENSED token source remains an explicit independent-provenance gap. Five
UUPS self-address sites and the independently encoded proxy initializer append
bind the complete saved bytecode without metadata replacement. Local execution
uses explicit installed self-code sizes, keeping implementation bytes distinct
from the synthetic proxy account's code. It does not execute proxy dispatch.
The operation matrix separates coherent role changes, four incoherent
boolean/index controls, wrapper authorization, direct whitelist operations,
initialization and rollback. This host proof adds no BTR production candidate. The
whitelist's existing length/index permissions do not establish complete
array-operation admission; the role-only candidate below records the inherited
zero-member/equality exception explicitly.
Fresh attempts, full traces and failed attempts remain under
`out/btr-operation-proof-20260928/` in `bsc-exclusion-review`. Actual initial state,
runtime/package/getter/holder qualification and producer visibility remain open.

The separate [SecuritiesToken host proof](../erc20/balances/docs/securities-operation-proof.md)
rebuilds the complete selected implementation with official solc 0.8.24 and
all 31 captured sources. Twenty-three bodies match immutable primary revisions;
three differing BEP bodies and five unresolved token/client sources remain
explicit gaps. Full runtime equality includes the metadata trailer. Saved and
fresh metadata differ in Unicode escaping, so both raw strings are separately
pinned alongside complete parsed equality. On-chain creation and deployment
fields are absent; constructor execution is synthetic only.
The local matrix exercises namespaced roles, enumerable-set ordering, raw
ERC-20 getters and rollback. Unsupported initializer/client/transfer/UI paths
remain explicit harness failures; no external-call or clock behavior is
fabricated. That host proof adds no ingestion permission by itself. Proxy/beacon traversal, real initial state, dynamic
metadata and runtime/package/holder qualification remain separate requirements.
Source builds, execution traces and failed attempts are preserved under
`out/securities-operation-proof-20260928/` in `network-verification-config`.

The separate [SecuritiesToken coupled candidate](../erc20/balances/docs/securities-coupled-role-candidate.md)
adds source-selected solc 0.8.24/OZ 5.3 operation admission for seventeen
otherwise identical historical profiles. It replaces broad membership width
with complete same-frame boolean/set operations. The one source-fixed ISSUER
admin word becomes an explicit scalar permission restricted to a zero new
value; it remains protected from aliases and acts as a barrier between stages.
Every proxy, beacon, implementation, balance, allowance and unrelated metadata
field is preserved. Historical finite metadata permissions do not grant new
dynamic-string payloads. Extended v4/v5 and actual positive frame boundaries
remain required; the independent DSG and PToken modes retain their own scope.
Source-derived projector tests cover complete operations, optional equality
stores, malformed/incomplete changes, cross-frame/account/role fragments,
coherence, aliases and dependency guards. Saved canonical replay checks balance
parity separately from role-operation visibility and never seeds cold holders
from reference values. Fresh evidence and failed attempts are under
`out/securities-coupled-validator-20260928/` in `network-verification-config`.
The SecuritiesToken candidates are NOT-QUALIFIED. They raised separate
candidate coverage to thirty of 33 boolean-role profiles. Actual coherent initial
state, initializer/client/proxy history, producer role witnesses, independent
source gaps and runtime/package/getter/holder qualification remain open.

The separate [GMToken host proof](../erc20/balances/docs/gm-operation-proof.md)
rebuilds the implementation and both proxy captures with official solc 0.8.16.
Its 34 source records cover 27 unique source/path profiles: fifteen exact Ondo
vendor dependencies, seven unique upstream proxy dependencies and five custom
BUSL source bodies whose independent primary revisions remain unresolved.
Compiler and captured runtimes differ only through their exact recorded CBOR
substitutions. Both full implementation runtimes run the same local matrix;
per-case traces, state and outcomes must match, and executed paths must avoid
the substituted bytes. Synthetic compiler construction returns the compiler
runtime, while all saved on-chain creation bindings remain null.
Local initialization and role operations retain exact errors and rollback.
They do not execute beacon dispatch or successful external pause/compliance
calls. The latter compliance call is non-view and is never mocked as a static
boolean. Source builds, full traces, the original creation-metadata failure and
the malformed-initializer expectation failure remain under
`out/gm-operation-proof-20260928/` in `bsc-exclusion-review`. No ingestion rule or
candidate is added. Actual initial-state, producer, runtime/package/getter and
holder qualification remain open.

The separate [GM coupled candidates](../erc20/balances/docs/gm-coupled-role-candidate.md)
replace only the legacy width-two role rule in two profiles. Their independently
named solc 0.8.16/Ondo-vendored template couples membership root 201 and enumerable
sets root 251. Complete coherent source operations are required; four one-sided
outcomes reachable from incoherent synthetic state remain refused. No reachable
role-admin setter is present, so the candidates add no admin permission.
All runtime/proxy/beacon/pointer/balance/allowance guards and finite long-name
payload words remain unchanged. The saved host proof is independently frozen;
producer visibility, initial coherence and package/getter/holder qualification
remain open. Fresh artifacts and failed attempts are under
`out/gm-coupled-validator-20260928/` in `bsc-exclusion-review`.
These candidates raised separate NOT-QUALIFIED coverage to thirty-two of 33
boolean-role profiles. Full cohort and historical qualification
fixtures remain unchanged.

The separate [BTR role-only candidate](../erc20/balances/docs/btr-coupled-role-candidate.md)
completes candidate migration for all 33 boolean-role profiles. It replaces only
the broad root-101 membership permission with complete root-101/root-151 operations
and the exact PAUSER self-admin scalar. Every persisted admin write must set the
source-bound role value, including equal writes and absent preimages. Unrelated
runtime/proxy/pointer/balance and whitelist fields remain unchanged. Direct
whitelist admission remains partial: nonzero array mutations refuse; zero-member
add/sole-remove can pass with omitted or ignored equal array stores. Full direct
whitelist admission is separate work. Fresh evidence is under
`out/btr-coupled-validator-20260928/` in `network-verification-config`.
All candidates remain NOT-QUALIFIED. The enumerable-role follow-up now has
three separate candidates described below; five profiles across five other
runtime groups still need source-selected migration. Four legitimate
terminal records and six unresolved-source profiles retain their classifications.
Source gaps, initial coherence, actual producer visibility and replacement
runtime/package/getter/initialized-holder qualification remain open.

The separate [ERC20TokenX host proof](../erc20/balances/docs/erc20tokenx-operation-proof.md)
binds ORI and FNA's complete source/creation records to the shared 7,896-byte
runtime without substitutions. PHI has only an exact historical runtime capture;
its absent individual source and creation records remain explicit. Four pinned
OpenZeppelin dependencies match exactly, while the custom token's independent
primary revision remains unresolved. The 520 synthetic calls measure legacy
root-8 role operations, self-swaps and equal stores, full-width authorization,
raw getters and exact return/revert/INVALID behavior. Captured constructor
attempts stop at unsupported CHAINID before role setup; fee callback and permit
attempts retain their separate unsupported boundaries and roll back. No VM or
production permission changes accompany this proof. A source matrix does not
establish deployed initialization or actual producer visibility.

The subsequent [ERC20TokenX enumerable candidates](../erc20/balances/docs/erc20tokenx-enumerable-candidate.md)
cover ORI, FNA and PHI separately. Each removes only the broad three-word
root-8 permission and selects the unchanged `oz_3_4_2` complete-operation rule,
without a membership root or role-admin permission. All balance, metadata and
runtime fields are preserved. Source-derived projector controls cover ordered
grant/revoke/renounce operations, omitted equal stages, malformed state and
actual call boundaries. The existing v3 root-call fallback remains unchanged;
v4/v5 requires positive actual root-call begin ordinals. Recognized standalone
length/index/admin no-ops refuse, while unrecognized equal array words retain
ordinary no-op handling. Captured creation remains refused. These separate
NOT-QUALIFIED candidates cover three of the eight enumerable profiles; five
profiles across five other runtimes remain. The custom primary-source gap,
PHI's runtime-only attribution, coherent initialization and actual
producer/runtime/package/getter/initialized-holder qualification remain open.

The separate [WKEYDAO/GOT host proof](../erc20/balances/docs/wkey-got-operation-proof.md)
binds each complete source/compiler/runtime and constructor append independently.
The two runtimes use different role roots (9/8) and domain handling, while both
keep MaxSupply at slot 6 and nonce mapping at 7. Their 1,263 synthetic calls
yield 1,000 returns, 252 exact expected reverts, six named source INVALID
controls and five explicit unsupported boundaries. Raw source-mapped effects,
full committed state and rollback preserve each token's role, cap and fee rules.
WKEYDAO construction stops at CHAINID PC 222 after five attempted stores and no
logs. GOT's original synthetic construction returns its exact runtime after
17 stores and three role logs; invalid argument controls roll back. This does
not establish either deployed initial set. Both custom primary-source revisions
remain unresolved, despite exact pinned dependency matches. No production
candidate or VM extension is added. A later GOT migration must remove both
broad root-8 rules, and both candidates need separate projector/replay review.
Fresh evidence remains under `out/wkeydao-got-operation-proof-20260929/` in
`protocol-invalidation-boundary`; previous attempts are preserved.

The host ledger now validates exact epoch membership for holder and global
rows. A transition from epoch 1 to epoch 3 cannot introduce undeclared epoch 2
or carry its unchecked value into the successor. Nonconsecutive IDs remain
valid. Cold input may adopt one epoch, or infer one consistent predecessor
before its first BOUND for cleanup; the inferred predecessor never overwrites
the final bound epoch. The protocol consumer inherits this validation before
staging holder, global, model or dependency changes. Red regressions and atomic
rejection/undo controls are retained under
`out/retained-epoch-membership-20260928/` in `protocol-invalidation-boundary`.
The membership correction is separate from execution-position validation.

The subsequent host position check requires every emitted observation span to
stay inside its epoch's activation, successor and current-block invalidation
boundaries. It checks discarded intermediate/log rows before selection and
requires intermediate effects to lie within the final global row's range.
Constants use their declaration positions without claiming writes. Explicit
checkpoint imports keep historical or neutral descriptors; later normal
blocks always resume strict emitted-position validation. Valid legacy prefixes
and later stateless output from suspended markets remain quarantined or are
dropped at rebinding, without restoring evaluable state. Eight pre-fix failures,
boundary/checkpoint regressions and full validation logs are preserved under
`out/retained-epoch-ordinals-20260928/` in `protocol-invalidation-boundary`.
The five protocol extractors now accept complete reset-only successor schedules
through the dependency-free `common/epochs` helper. Entries normalize by market
and activation; IDs strictly increase but may have gaps. Effects at a successor's
start belong to it, including invalidating same-value pointer writes and code
changes. Raw storage/noop/native-cash continuity is checked before splitting or
cutoffs; retired prefixes still validate but cannot become end-of-block state.
Every successor and its later heartbeats set both carryover flags false.
Unknown suffix inputs stay unknown; only fresh observations or an independently
qualified checkpoint initialize them. Complete raw parameters remain part of
the stream identity, so appending an epoch cannot silently reuse the old ledger.

All five packages are version 0.2.0: Aave spec 3, Compound v2/Comet/ERC4626 spec 4,
and Lido spec 5. Actual-projector synthetic tests cover three-epoch schedules,
changed decoders/dependencies, shared physical effects, exact boundaries,
invalidations, quiet blocks, cold versus zero, checkpoint restore and undo.
The Aave evaluated bridge additionally rejects missing successor globals,
reinitializes a changed holder root, and refuses old qualification/stream data.
Four pre-fix ordinary-noop continuity failures and validation logs remain in
`out/protocol-successor-epochs-20260928/` in `protocol-invalidation-boundary`.
Source-qualified carryover, actual initialized checkpoints and deployed
runtime/package/getter/clock/holder qualification remain open under #7/#16
and the individual package issues. Historical package digests and live reports
continue to describe their original builds; these revisions are offline only.

This page is the entry point for anyone picking up the roadmap in
[#21](https://github.com/pinax-network/substreams-evm-extended/issues/21). It
records what was built between 2026-09-18 and 2026-09-21, what is verified and
what is only inferred, the open review findings, where every artifact lives,
and the ordered next steps. Procedural know-how is in [`../skills/`](../skills/README.md).

## 1. Read this first: the constraints

- **Live use on BSC resumed on 2026-09-22.** The owner supplied the Pinax
  BSC endpoints (`bsc.substreams.pinax.network:443`,
  `bsc.firehose.pinax.network:443` and a key-bearing RPC URL). Keep the key
  and the RPC URL in environment variables only (`SUBSTREAMS_API_KEY`,
  `RPC_URL`); never write them to the repository, evidence or logs, keep
  live ranges modest and always pass a stop block to `substreams run`. Other
  networks (#8) have no endpoints yet, and the `dex/pool-state` live hold in
  `AGENTS.md` still needs its own explicit reauthorization. The first live
  qualification is `aave/balance-state` (#13, see its README); everything
  else below was done offline and says so.
- **Production boundary** ([`../AGENTS.md`](../AGENTS.md)): one RPC-free
  `map_events` per package; `evm.balances.v1` field numbers frozen; protocol
  packages emit the companion `evm.balance_state.v1`; no `db_out`, no custom
  sink; unknown writes and unreviewed runtime/dependency changes fail closed;
  all tooling in Rust and excluded from the WASM path; never print
  access-bearing endpoint URLs or secrets.
- **Two agents work in this repository.** Branches prefixed `codex/` (PRs #9,
  #10, #40 `dex/pool-state`) come from another assistant. Coordinate through
  the GitHub issues; do not rewrite their evidence or packages.

## 2. What exists now

| Package (crate) | Output | Issue | Evidence | Slot provenance |
| --- | --- | --- | --- | --- |
| `erc20/balances` | `evm.balances.v1` | pre-existing production module; #2–#6, #22 | RPC-qualified historical evidence under `erc20/balances/docs`; typed-path baseline replay (1,024 blocks, 110,139 rows, 4,012 retained matches, 66,265 cold unknowns); live current package (#6): 425 of 431 profiles, 1,024 BSC blocks, 253,503 rows with 0 differences from the RPC reference, 88,534 holders checkpointed and final-state equal ([report](../erc20/balances/docs/live-package-bsc-2026-09-23.md)) | RPC-qualified layouts |
| `native/balances` | `evm.balances.v1` (`contract` absent) | #17 open (BSC live-qualified; other networks #8) | **live 2026-09-23**: 1,024 saved control blocks equal to the offline replay and `eth_getBalance` (76,139/76,139); 5,000 live blocks, 200,344/200,344 same-block `eth_getBalance` ([evidence](../native/balances/docs/evidence)); saved replay 1,439 blocks | n/a |
| `erc20/events` | `erc20.events.v1` | #19 closed | BSC single-tx fixtures | n/a |
| `evm/executions` | `evm.executions.v1` | #18 closed; producer semantics under #8 | saved-block replay: 1,509 BSC v4/v5 blocks, 116,951 txs, 1,075,108 receipt logs matched, 2,093,149 writes in storage context, 0 errors, determinism checked ([evidence](../evm/executions/docs/evidence/replay-bsc-v4-v5.json)); live: 250 final v5 blocks, 0 errors ([evidence](../evm/executions/docs/evidence/replay-bsc-live-2026-09-23.json)) | n/a |
| `aave/actions` | `aave.actions.v1` | #20 | **live 2026-09-23**: 459/459 Pool receipt logs equal persisted rows field by field over 2,397 blocks, all six kinds ([evidence](../aave/actions/docs/evidence/live-parity-bsc-2026-09-23.json)); compiled v3.7.0 event ABI with V3.0/V2 comparison | n/a |
| `aave/balance-state` | `evm.balance_state.v1` | #13 | **live-qualified on BSC** for epoch 101,087,794/3347 over 2,060 blocks: packaged output, same-block `scaledBalanceOf`/`balanceOf`/reserve parity, 0 mismatches ([evidence](../aave/balance-state/docs/evidence/live-parity-bsc-2026-09-22-rev3.json)); saved-block replay 1,433 blocks | **compiler-verified** (aave-v3-origin `8305565a`, solc 0.8.27): aToken 52/53/54/58, Pool `_reserves` 52 |
| `compound-v2/balance-state` | `evm.balance_state.v1` | #14 open | synthetic tests only | **compiler-verified**: deployed cUSDC/cETH against the 2019 tree (`solc 0.5.17`, `_guardCounter` at slot 0), cUSDC IRM `LegacyJumpRateModelV2`, delegators against `solc 0.8.10`; USDC FiatToken 0.6.12; `tests/storage_layout.rs` |
| `compound-v3/balance-state` | `evm.balance_state.v1` | #15 open | synthetic tests only; review findings fixed (§4) | **compiler-verified** (`solc 0.8.15`, `tests/storage_layout.rs`) |
| `lido/balance-state` | `evm.balance_state.v1` | #23 open | synthetic tests; named slots asserted `== keccak256(name)` | **ast-derived** (`solc 0.4.24` AST; all 16 position constants configured or reviewed) |
| `erc4626/balance-state` | `evm.balance_state.v1` | #24 | **live 2026-09-23 (BSC static aToken)**: 162/162 shares, `convertToAssets` and `rate()` via conformance, 297 reserve words, 63 supplies over 2,064 blocks ([evidence](../erc4626/balance-state/docs/evidence/live-parity-bsc-stata-2026-09-23.json)); sDAI and OZ epochs are Ethereum placeholders (#8) | **compiler-verified** (StaticATokenLM 0.8.20, SavingsDai 0.8.17, Pot 0.6.12, OZ constants) |
| `common/persist` | library | – | shared copy of `erc20/balances` persistence rules; fixtures reused | – |
| `common/retention` | host library | #7 open | atomic application, exact checkpoint/stream identity, undo and retained protocol-input evaluation; synthetic lifecycle controls and separately bounded BSC evidence ([spec](initialization-and-completeness.md)) | – |
| `conformance` | host library | #16 open | Aave/static-aToken, Compound v2, Comet, Lido, OZ and SavingsDai have compiled pinned-source controls with explicit harness limits, alongside the captured Aave index oracle ([details](../conformance/README.md)); deployment/package qualification remains separate | – |
| `dex/pool-state` | `dex` protos | (other agent, PR #40) | see its README | – |

The initial implementation pass merged PRs #25–#39 and #40, followed by
#46's audit remediation, #47's pointer contract and execution regressions.
Later qualification and offline fixes are recorded above and in the
[current issue tracker](follow-up.md). Issues #6, #11, #12, #13, #18, #19,
#20, #22 and #24 are closed with their bounded evidence preserved. Thirteen
issues remain open; their implementation progress and outstanding gates are
listed separately in that tracker.

## 3. Where the artifacts live

| What | Where |
| --- | --- |
| Roadmap requirements and open questions | [`extraction-coverage.md`](extraction-coverage.md), [`follow-up.md`](follow-up.md) |
| Balance-state contract and its rejected alternatives | [`balance-state-contract.md`](balance-state-contract.md), [`design/balance-state-alternatives/`](design/balance-state-alternatives/README.md) |
| Initialization / completeness spec | [`initialization-and-completeness.md`](initialization-and-completeness.md) |
| Pinned-source research notes (JSON) | [`research/`](research/README.md) |
| Saved-block scans | [`evidence/scans/`](evidence/scans/README.md) |
| Storage-slot provenance and the solc verification plan | [`storage-layout-provenance.md`](storage-layout-provenance.md) |
| Open review findings | [`review-findings-2026-09-21.md`](review-findings-2026-09-21.md) |
| Ad-hoc block scanners (trim, probe, aave, stata) | [`../tools/scratch-scan/`](../tools/scratch-scan/README.md) |
| Per-package replay tools | `native/balances/tools`, `aave/balance-state/tools`, `erc20/balances/tools` |
| Skills (procedures) | [`../skills/`](../skills/README.md) |

### Locally cached Extended blocks (not in git; on the original machine only)

41 directories, 6,093 `<height>.pb` files, all BSC (chain id 56), all
`Block.ver` 5 except the 71 v4 blocks in the vote/voting/hlbp/star directories.
The main contiguous window is `erc20/balances/out/top50-full-holder-blocks`
= [122288006, 122289030), 1,024 blocks, complemented by
`top50-holder-extension512` [122288458, 122288970) and `-extension60`
[122288970, 122289030). Other directories are samples of the same window
(`ranks*-sample-blocks`, `next50-sample-blocks`, `computed/clone/deployment/
fallback-holder-blocks`) or single blocks for specific tokens (`4stock-*`
120607788+, `hlbp-active-blocks` 104727168+, `shareholder-removal-*`
122264480+, `star-mint-blocks` 58597371+, `vote-history-blocks`). If the
machine is gone, these must be recaptured with the `erc20/balances/tools`
capture command once live use resumes.

## 4. Hardening review status

On 2026-09-21 a review workflow (20 finders = 5 crates x 4 dimensions, 3
adversarial verifiers per finding, 6 solc layout extractors) was started
against PRs #33–#38. The subagent spend limit stopped it after 4 of 110 agents;
only the four `compound-v3` finders returned. Their 28 unverified findings are
in [`review-findings-2026-09-21.md`](review-findings-2026-09-21.md). The
dimensions were: pinned-source conformance; fail-closed and persisted-effect
semantics; balance-state contract conformance; test adequacy versus the issue
acceptance list.

Must-fix, in order (the first was confirmed by four independent reviewers and
re-derived by the maintainer from `CometCore.sol:60` and `keccak256` of the label):

1. **Done for `compound-v3` (PR "compound-v3 hardening").** The guard slot is
   reviewed by name (`other_slot_names: ["comet.reentrancy.guard"]`), asserted
   equal to `0xc98c7730…53ac` in a test, and a routine supply in a
   delegatecall frame with the `0→1→0` guard writes is a test case.
2. Row semantics for packed words: the proto says one row per decoded field
   of a written slot. **Done everywhere** (`compound-v3` market words and
   holder rows; `lido` packed halves; `erc4626` Aave reserve fields).
3. `producer_versions` restricted to 4 and 5: **done in every map crate**
   (`compound-v3`, `compound-v2`, `lido`, `erc4626`, `aave/balance-state`,
   `native/balances`), each with a parse test.
4. INVALIDATED instead of failing the block on pointer writes and code
   changes: **done in every balance-state package.** The first version for
   `compound-v3` and `aave` treated a write onto the bound implementation as
   the binding, which violated the proto's `STORAGE_POINTER` rule and would
   have concealed an in-block excursion X→Z→X. All five packages now
   invalidate on every persisted pointer write, including equal-value and
   restored writes, with per-write evidence, and honour `activation_ordinal`
   so an epoch can be bound at its own upgrade block (see "Activation position
   and pointer writes" in `balance-state-contract.md`). The Aave saved-block
   replay reproduces the committed rows byte for byte after the change
   (`aave/balance-state/docs/evidence/replay-bsc-v5-rev3.json`).
5. Constant cross-checks and carryover flags: **done for `compound-v3`**
   (`base_index_scale == 1e15`, `factor_scale == 1e18`, `base_scale ==
   10^decimals`, `global_carryover = false`) and **`compound-v2`**
   (`blocks_per_year == 2102400`, `global_carryover = false`); `erc4626` has
   no pinned constant to check beyond the ERC-7201 namespace slot, which is
   now re-derived in a test.
6. `conformance::comet`: **done** (int104-min negation refused; exact rates
   on both sides of the kink and at the market's utilization; exact indices
   after 3600 s and one year; uint64 overflow arms).
7. Tests listed in the findings: **done everywhere.** `compound-v3` has 14
   tests; the shared subset (producer versions, delegatecall frame shape,
   FAILED and REVERTED transactions, pre-activation blocks, contextual
   `reduce()` diagnostics, determinism under permutation) and then the full
   `validate_block` refusal table, same-block provenance and multi-market
   attribution were mirrored into `compound-v2`, `lido`, `erc4626` and
   `aave`.
8. Re-run the review for `compound-v2`, `lido`, `erc4626`, `common/retention`.
   **Done 2026-09-22: `common/retention` (12 findings), `erc4626` (10, two
   high), `lido` (7) and `compound-v2` (10, one high: the deployed cUSDC and
   cETH run the 2019 layout) reviewed independently; all fixed (see the
   findings doc).**
   Earlier state: blocked twice. A second workflow (16 finders,
   no verifiers) was launched on 2026-09-21 and every one of its 16 agents
   died on the account spend limit, so those four crates have never been
   reviewed by an independent reader. Their fixes so far came from applying
   the `compound-v3` findings by analogy, not from evidence about them. When
   credit is available, re-run
   `workflows/scripts/review-remaining-balance-state-crates-wf_2c8ed0bb-fb7.js`
   (run id `wf_2c8ed0bb-fb7`, nothing cached) or review by hand with the four
   dimensions above. Treat this as the highest-value open item: the one crate
   that *was* reviewed turned out to have a defect that would have failed
   every real block.

### Mixed-provenance pull request (merged)

[PR #46](https://github.com/pinax-network/substreams-evm-extended/pull/46)
combined the offline issue-audit remediation (authored by a concurrent session
in this same working tree; see
[`audit-remediation-2026-09-21.md`](audit-remediation-2026-09-21.md)) with the
sibling-crate refusal tests. It was reviewed and merged on 2026-09-22. It
changed the `common/retention` host API: `seed_checkpoint` and
`seed_deployment_zero` now require explicit identities.

## 5. Facts learned that are not written anywhere else

- USDC (FiatToken V2.2) stores the blacklist flag in bit 255 of the same
  mapping word as the balance (`balanceAndBlacklistStates`, slot 9) and
  `_balanceOf` masks it; any "raw balances slot" model of USDC must decode
  255 bits, which the compound-v2 cash model does via `value_bits`.
- The BSC producer (versions 4 and 5) records **no** equal-value storage
  change: none of 2,093,149 storage changes in the saved data is equal-valued
  (`evm/executions/docs/evidence/replay-bsc-v4-v5.json`). An earlier version of
  this page claimed the opposite; that was wrong. `common/persist` would route
  such a record to `storage_noop` anyway. A reentrancy flag that goes 0→1→0
  within one frame is two *changing* writes that reduce to `old == new`; it is
  still a write and must be a reviewed slot.
- The same producer **does** record a code change whose old and new code are
  identical: a SetCode authorization re-delegating an account to its current
  target (86 cases in the saved data). `evm/executions` keeps the row as
  `DELEGATION_SET` with `persisted = false` (it compares the hashes).
  `common/persist` does not compare them and passes the change on, so a
  balance-state package would invalidate a watched contract's epoch on it:
  fail-closed, like an equal-value pointer write. (A watched protocol contract
  has code and cannot be a SetCode authority, so this is not expected.)
- Storage context: a `DELEGATE` or `CALLCODE` frame writes its `caller`'s
  storage, every other frame its own `address`; this held for all 2,093,149
  saved writes.
- Firehose facts observed in saved blocks: a contract-creation transaction
  has `to` equal to the created address; logs of reverted frames keep their
  receipt `index` (with `block_index` 0) and are absent from the receipt;
  the root call is not necessarily call index 0 (use depth 0 / minimum
  index); BSC fee resets appear as `REASON_REWARD_TRANSACTION_FEE`; reason 17
  is `REWARD_BLOB_FEE` (from the upstream proto), 18/19 are OP-stack, 20 Monad.
- Lido v4 computes the share rate as `internalEther / internalShares`
  (`totalShares - externalShares`), not `totalPooledEther / totalShares`; the
  two differ in integer truncation. Forward and external-share products wrap
  to uint256 before division; the total getter's final addition is checked.
- Aave V3 BSC: Pool `_reserves` mapping is slot 52; `ReserveData` word +1 is
  `liquidityIndex` (low 128) | `currentLiquidityRate` (high 128) and word +3
  holds `lastUpdateTimestamp` at bits 128..168; aToken `_userState` is slot
  0x34 (scaled balance low 120 bits, `additionalData` high 128), allowances
  0x35, `_totalSupply` 0x36; EIP-1967 implementation slot
  `0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc`.
- The captured Aave index oracle `rayMul(linearInterest(prev rate, prev ts,
  now), prev index)` matched every stored index update in the replay.
- zsh does not word-split unquoted `$VAR`; use `${=VAR}` in scripts.
- `gh pr checks` output is tab-separated; the repository has no auto-merge;
  CI runs `clippy --all-targets -D warnings` (rustdoc lints included) and a
  `--locked` WASM check, so new crates need a committed `Cargo.lock` update and
  `#[cfg(target_arch = "wasm32")] mod handler`.

## 6. Next steps, in order

1. **Done:** the `compound-v3`-only tests (validate_block table, same-block
   provenance, multi-market attribution) are mirrored in every sibling.
2. **Done:** the independent review of `compound-v2`, `lido`, `erc4626` and
   `common/retention` (§4 item 8), PRs #49–#52.
   Its offline follow-ups are implemented: all five extractors now accept
   complete reset-only successor schedules and use exclusive invalidation
   cutoffs. No predecessor prefix is emitted as end-of-block state. The
   current review includes Aave and the projector-to-consumer lifecycle.
   Source-qualified carryover and actual deployed successor qualification
   remain separate; the complete parameter hash still binds the stream.
3. **Done 2026-09-21:** solc storage layouts for every pinned contract are
   committed under `docs/evidence/storage-layouts/` with a `tests/storage_layout.rs`
   per package ([provenance](storage-layout-provenance.md)). The USDC FiatToken
   family was compiled the same day (v2.2.0): slot 9 is
   `balanceAndBlacklistStates`, so the compound-v2 cash row now decodes only
   the low 255 bits (`value_bits`). No inferred slot remains.
4. Live on BSC: **done for `aave/balance-state`** (runtime epoch bound
   from the upgrade writes, compiled layout, 2,060 packaged-output blocks
   with same-block getter parity; it also found an unreviewed `_nonces`
   write that halted a real block). Next with the same method
   (`aave-balance-state-tools live-parity` is the template): **done for
   `native/balances`** (#17, `native-balances-tools live-parity`, batched
   `eth_getBalance`) and **`aave/actions`** (#20, `aave-actions-tools`,
   receipt-log parity from the compiled ABI) and the **BSC static aToken**
   (#24, `erc4626-balance-state-tools`) and the **`evm/executions` producer
   checks** (250 live final blocks, `evm-executions-tools replay`, 0 errors)
   and the **current `erc20/balances` package** (#6: `runtime-status`,
   `refusal-scan`, `compare`, `audit-rpc`, `holder-coverage` and the native
   ClickHouse smoke; three profiles refuse unreviewed writes on live blocks
   and three no longer match their runtime bindings). Six profiles are under
   #61. #2's producer check on actual role operations is done
   (`role-operations`, DSG's 411 operations including two tail removals, plus
   236 OpenZeppelin 4.x/5.x operations); only a zero-address member remains
   unobserved. Next: #61, then #2/#3's packaged DSG parity and holder checks. Ethereum and the
   other networks (#8) wait for endpoints.

The [APD/DSG captured ledger controls](../erc20/balances/docs/typed450-offline-review.md#captured-projector-to-ledger-controls)
use the two original full blocks and independently captured canonical rows. Cold
application retains five emitted holders and keeps six reference-only values
unknown, including two nonzero token-owned balances. Separate explicit snapshots
seed six APD holders at 122288154 and five DSG holders at 122288046, including five
known zeros in total. Only synthetic successors follow those checkpoint clocks;
reapplying the seed block or a wrong parent is refused. Derived DSG allowance
restoration changes neither cold nor checkpoint holder state. This adds no
production permission, continuous initialized interval or new package/live
qualification. Bound-bytecode metadata/getter independence remains separate work.
