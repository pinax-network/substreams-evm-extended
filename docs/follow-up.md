# Follow-up work

Status reviewed on **2026-09-28**, against main at
`5dca3393d8ea75f9032ff4faf2a14a3b6c6137a5`, GitHub acceptance lists, latest
issue comments and merged evidence. There were **13 open issues and no open
pull requests** when this review began. The earlier
[offline remediation](audit-remediation-2026-09-21.md) already landed in
[PR #46](https://github.com/pinax-network/substreams-evm-extended/pull/46);
PRs #47–#62 added hardening and bounded BSC evidence.

All 13 remaining issues have unmet acceptance criteria. A merged implementation,
passing tests or a successful BSC sample does not complete their broader scope.

The host reference models also distinguish explicitly known zero inputs from
missing ledger facts. [Aave/sDAI source-branch regressions](research/11-Known_zero_reference_inputs.json)
cover zero-valued conversions, required earlier failures and a corrected sDAI
division panic, with synthetic idle-clock/undo controls under #7/#16. These
checks do not establish deployment reachability or new live qualification.

## Open issues

| Issue | Implemented or recorded evidence | Required before closure |
| --- | --- | --- |
| [#2 DSG enumerable sets](https://github.com/pinax-network/substreams-evm-extended/issues/2) | Source-bound operation validator and adversarial regressions; #62 adds actual DSG grants/revocations/renouncements, tail/self-swap observations and sparse package execution. | Zero-address member observation, continuous runtime/initial-set qualification, package/RPC balance and initialized-holder parity. The role-operation sample emits no balances. |
| [#3 APD/DSG qualification](https://github.com/pinax-network/substreams-evm-extended/issues/3) | Typed allowance paths, restored-allowance regression and DSG enumerable operations. [Saved review](../erc20/balances/docs/typed450-offline-review.md) preserves balance comparisons and cold gaps. | Independent balance-versus-metadata controls, packaged balance parity across delivered clocks, initialized holders/final state and negative binding controls. Neither token belongs to #60's historical 431-profile cohort. |
| [#4 Legacy role paths](https://github.com/pinax-network/substreams-evm-extended/issues/4) | Separate candidates cover thirteen of 33 boolean-role profiles: Token/CYS, BurnMint, Point/Bedrock, FHE/B2, BAS, Tagger, Artx, Kgen/Deep and PToken. The [candidate status table](../erc20/balances/docs/role-shape-audit.md#candidate-progress) links source bindings, tests and saved replays. Reviewed admin paths and proxy guards remain explicit. The [PToken coupled rule](../erc20/balances/docs/ptoken-coupled-role-candidate.md) admits complete same-frame boolean/set transitions for the selected compiler template, backed by the separate host operation proof and adversarial projector tests. The separate [BTR](../erc20/balances/docs/btr-operation-proof.md), [SecuritiesToken](../erc20/balances/docs/securities-operation-proof.md) and [GMToken](../erc20/balances/docs/gm-operation-proof.md) host proofs bind selected compiled local behavior without adding ingestion rules or candidates. SecuritiesToken retains eight primary-source gaps and absent deployment/creation evidence; external-client, initializer, transfer/UI and proxy execution remain excluded. GM separately executes local initialization, with five custom source gaps, exact compiler/captured metadata substitutions and absent on-chain creation evidence. | Remaining per-profile reachability/path review and migration; retain independent source gaps for Point, BAS, Tagger, Artx, Kgen/Deep, PToken, BTR, SecuritiesToken and GMToken, alongside the original unresolved cohorts; qualify replacement layouts before promotion. Deep's old proxy initializer and PToken's real initial-set coherence remain unqualified. These candidates are separate from the historical 431/qualified 425 fixtures; source-derived operations do not establish actual producer role-operation visibility, runtime/package/getter or initialized-holder/final-state qualification. |
| [#5 Remaining 19 candidates](https://github.com/pinax-network/substreams-evm-extended/issues/5) | Saved token-specific diagnostics, calculated/reflection models and partial APD/DSG work. | Source/runtime and dependency initialization, exact arithmetic and affected-holder behavior, then package/holder qualification per candidate. These original 19 are separate from the six regressions in #61. |
| [#7 Initialization/completeness](https://github.com/pinax-network/substreams-evm-extended/issues/7) | [Host ledger/spec](initialization-and-completeness.md), atomic application, exact checkpoint identity and undo; #58 adds a native BSC checkpoint replay. Holder/global epoch membership requires actual prior or declared identities, with one consistent cold predecessor. Emitted observation spans now enforce activation, successor and invalidation boundaries atomically, while explicit checkpoint imports retain historical provenance. | Native sink publication/clock-chain verification and live BlockUndoSignal handling. Known observed holders remain distinct from global enumeration; protocol-global evaluation is tracked under #16. |
| [#8 Additional networks](https://github.com/pinax-network/substreams-evm-extended/issues/8) | Chain/asset/producer requirements, source research and explicit ERC-20 host verification network/chain configuration. Every RPC entry checks chain identity; offline replay labels remain unverified. | Independently bind and qualify Ethereum, Base, HyperEVM and Arc: Extended semantics, fork effects, runtime/package/getter controls, holders, clocks and native identities/aliases. Configuration and mock tests do not establish network qualification; BSC evidence does not transfer. |
| [#14 Compound v2](https://github.com/pinax-network/substreams-evm-extended/issues/14) | Share/cash/IRM extraction, compiler layouts, synthetic tests and #52's 2019 cUSDC/cETH corrections. Exclusive invalidation cutoff omits partial storage/native-cash state. [Pinned-source execution](../conformance/fixtures/compound-v2-oracle/README.md) checks uint256 intermediates and revision-specific behavior. The retained bridge separates shares, stored exchange/underlying values and projected underlying claims, including donation-only cash and idle-block changes. | Actual deployed runtime/source and dependency binding, real USDC implementation/activation, captured Ethereum replay and package/getter controls; qualified initialized checkpoints and successor epochs. Synthetic projector/consumer tests do not qualify their fixture declarations. |
| [#15 Compound III](https://github.com/pinax-network/substreams-evm-extended/issues/15) | Principal/market extraction, compiler layouts, exact arithmetic and exclusive invalidation cutoff. The retained bridge separates signed principal, stored/projected indices and supplied balance with missing-state, uint40 clock and undo controls. A [compiled pinned-source oracle](../conformance/fixtures/comet-oracle/README.md) checks 1,515 arithmetic/getter outcomes plus two ABI-width refusals. | Replace implementation/activation placeholders; capture Ethereum inputs and packaged/getter controls. Independently qualify runtime constants, initialized checkpoints and successor epochs. Controlled source execution does not bind a deployment. |
| [#16 Conformance](https://github.com/pinax-network/substreams-evm-extended/issues/16) | Five pure Rust model families with pinned source execution for Aave/static-aToken, OZ, Compound v2, Comet, Lido and SavingsDai. The [Aave/static oracle](../conformance/fixtures/aave-static-oracle/README.md) checks both selected rounding eras, stored-index narrowing and conversion/withdrawal-limit failure order. The host `ProtocolLedger` retains holder/global inputs, exact clocks, model/dependency bindings and bounded undo. Its qualified bridge evaluates initialized holders across idle/global-only updates with explicit runtime attestations, result units and signed principal; logs and current-block derived values remain separate. | Actual runtime/dependency/package/getter controls, qualified deployment checkpoints and successor epochs, and deployment-backed clock/undo validation. Synthetic lifecycle tests, controlled source harnesses and the captured Aave index control do not qualify deployments or global holder coverage. |
| [#17 Native balances](https://github.com/pinax-network/substreams-evm-extended/issues/17) | One-map reducer, saved controls, producer/fork matrix and #55's bounded BSC package/getter checks. | Remaining lifecycle/reward/fork reasons, Base failed-deposit persistence and per-network native identity/alias controls under #8. BSC success does not satisfy the full matrix. |
| [#21 Roadmap](https://github.com/pinax-network/substreams-evm-extended/issues/21) | Packages/shared contracts exist; #11/#12/#13/#18/#19/#20/#22/#24 are closed. | Complete #14/#15/#16/#17/#23 and shared #7/#8 gates. Preserve extraction, evaluated balances and downstream interpretation as separate responsibilities. |
| [#23 stETH](https://github.com/pinax-network/substreams-evm-extended/issues/23) | Shares/global inputs, Aragon guards and exclusive invalidation/undo controls. A [compiled pinned-source oracle](../conformance/fixtures/lido-oracle/README.md) checks getter arithmetic, including wrapping products before division and checked total addition. Spec revision 4 preserves all six observed inputs while omitting an overflowing derived total. Actual projector-to-host-ledger synthetic coverage retains stored getter inputs and separate report logs, expires derived rows and evaluates initialized shares. | Actual runtime/activation/dependency qualification, captured Ethereum rebase/share/fee/burn/external-share cases, qualified checkpoints/successor epochs and package/getter parity. Controlled source execution does not establish deployed reachability; the host bridge does not infer report-time holder state from end-of-block shares. |
| [#61 Six BSC exclusions](https://github.com/pinax-network/substreams-evm-extended/issues/61) | [Offline exclusion review](../erc20/balances/docs/bsc-exclusions-offline-2026-09-28.md): original TAKE/RADR/TOPS refusal fixtures; pinned TAKE source/runtime reconstruction; one unqualified guard-slot candidate with 8,436 saved-reference matches over 1,024 blocks and 207 initialized observed holders. Oversized persisted configured storage words now fail before metadata ignores. | All six remain excluded. TAKE needs fresh runtime/package/getter/holder qualification; TOPS needs a reviewed typed dynamic-array path and independent source pin; RADR lacks bound source; BNC4's replacement beacon and sPro/swkeyDAO2's changed divisors require explicit dependency/holder-state qualification. |

## Completed issue records

These issues are already closed on GitHub. Their evidence retains its recorded
package, runtime, interval and holder scope.

| Issues | Delivered scope |
| --- | --- |
| [#6](https://github.com/pinax-network/substreams-evm-extended/issues/6) | Current ERC-20 package and native sink checks in #60, using 425 admitted profiles; six exclusions remain under #61. |
| [#11](https://github.com/pinax-network/substreams-evm-extended/issues/11), [#12](https://github.com/pinax-network/substreams-evm-extended/issues/12) | Coverage requirements and versioned holder/global balance-state contract. |
| [#13](https://github.com/pinax-network/substreams-evm-extended/issues/13) | Selected BSC Aave aToken epoch and package/getter controls in #54. |
| [#18](https://github.com/pinax-network/substreams-evm-extended/issues/18), [#19](https://github.com/pinax-network/substreams-evm-extended/issues/19) | Execution facts and standard ERC-20 event evidence; further producer/network controls remain in #8. |
| [#20](https://github.com/pinax-network/substreams-evm-extended/issues/20) | Selected Aave lending-action adapter, compiled ABI provenance and BSC receipt-log comparison in #56. |
| [#22](https://github.com/pinax-network/substreams-evm-extended/issues/22) | Non-Transfer ERC-20 balance regression coverage. |
| [#24](https://github.com/pinax-network/substreams-evm-extended/issues/24) | Selected BSC static-aToken conversion package in #57. Ethereum sDAI/OZ declarations and withdrawal-limit extraction are not thereby qualified. |

## Evidence and execution boundaries

The [current ERC-20 package report](../erc20/balances/docs/live-package-bsc-2026-09-23.md)
covers BSC **[123561000, 123562024)**, 425 admitted profiles and 88,534
initialized observed holders. It does not cover APD/DSG or the six excluded
profiles, enumerate all holders or qualify replacement layouts.

The Token/CYS candidate replay remains BSC **[122288006, 122289030)**:
110,139 rows equal the historical/current baseline, with 69 initialized
observed holders for those two candidates and no actual role-membership writes.
The historical 431-profile fixture remains unchanged and still uses legacy
role rules; candidate fixtures do not silently replace it.

BSC live work was separately resumed and recorded on 2026-09-22/23. Older
blanket pause statements describe the earlier phase; they do not erase those
reports. This review ran only offline checks and did not authorize or start
new RPC, Firehose, Substreams or sink work. Other-network qualification and
the explicit dex/pool-state live hold remain separate gates. Preserve all
historical artifacts and failed attempts.

Use the acceptance lists above for issue closure, the
[extraction coverage](extraction-coverage.md) for model boundaries and
[handoff](handoff.md) for historical artifact locations. The
[issue workflow](../skills/roadmap-issue-workflow/SKILL.md) describes delivery.

The review checkout passed pinned Rust 1.88 formatting, **697 workspace tests**
(678 library/binary and 19 integration), all-target Clippy with warnings denied
and the locked WASM workspace check. All Cargo validation used offline mode;
logs are in `out/issue-review-20260928/`. These checks validate the checkout,
not new chain/runtime qualification.
