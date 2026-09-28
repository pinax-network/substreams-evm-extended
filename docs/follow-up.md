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

## Open issues

| Issue | Implemented or recorded evidence | Required before closure |
| --- | --- | --- |
| [#2 DSG enumerable sets](https://github.com/pinax-network/substreams-evm-extended/issues/2) | Source-bound operation validator and adversarial regressions; #62 adds actual DSG grants/revocations/renouncements, tail/self-swap observations and sparse package execution. | Zero-address member observation, continuous runtime/initial-set qualification, package/RPC balance and initialized-holder parity. The role-operation sample emits no balances. |
| [#3 APD/DSG qualification](https://github.com/pinax-network/substreams-evm-extended/issues/3) | Typed allowance paths, restored-allowance regression and DSG enumerable operations. [Saved review](../erc20/balances/docs/typed450-offline-review.md) preserves balance comparisons and cold gaps. | Independent balance-versus-metadata controls, packaged balance parity across delivered clocks, initialized holders/final state and negative binding controls. Neither token belongs to #60's historical 431-profile cohort. |
| [#4 Legacy role paths](https://github.com/pinax-network/substreams-evm-extended/issues/4) | Separate Token/CYS candidates, source/runtime review, adversarial tests and [complete saved replay](../erc20/balances/docs/role-path-candidates.md) landed in #46. | Remaining per-profile reachability/path review and migration; retain unresolved source cases; qualify replacement layouts before promotion. No role-membership write occurs for the two candidates in their replay interval. |
| [#5 Remaining 19 candidates](https://github.com/pinax-network/substreams-evm-extended/issues/5) | Saved token-specific diagnostics, calculated/reflection models and partial APD/DSG work. | Source/runtime and dependency initialization, exact arithmetic and affected-holder behavior, then package/holder qualification per candidate. These original 19 are separate from the six regressions in #61. |
| [#7 Initialization/completeness](https://github.com/pinax-network/substreams-evm-extended/issues/7) | [Host ledger/spec](initialization-and-completeness.md), atomic application, exact checkpoint identity and undo; #58 adds a native BSC checkpoint replay. | Native sink publication/clock-chain verification and live BlockUndoSignal handling. Known observed holders remain distinct from global enumeration; protocol-global evaluation is tracked under #16. |
| [#8 Additional networks](https://github.com/pinax-network/substreams-evm-extended/issues/8) | Chain/asset/producer requirements and source research. | Replace the ERC-20 verifier's hardcoded BSC chain ID with explicit configuration, then independently bind and qualify Ethereum, Base, HyperEVM and Arc: Extended semantics, fork effects, runtime/package/getter controls, holders, clocks and native identities/aliases. BSC evidence does not transfer. |
| [#14 Compound v2](https://github.com/pinax-network/substreams-evm-extended/issues/14) | Share/cash/IRM extraction, compiler layouts, synthetic tests and #52's 2019 cUSDC/cETH layout and rate-model corrections. | Actual deployed runtime/source and dependency binding, real USDC implementation/activation, captured Ethereum replay and package/getter controls; initialized state, successor epochs and undo conformance. |
| [#15 Compound III](https://github.com/pinax-network/substreams-evm-extended/issues/15) | Principal/market extraction, compiler layouts, exact arithmetic and adversarial tests. | Replace implementation/activation placeholders; capture Ethereum inputs and packaged/getter controls. Stop old-epoch decoding after invalidation; exercise successor epochs and retained-state replay/undo. |
| [#16 Conformance](https://github.com/pinax-network/substreams-evm-extended/issues/16) | Five Rust model families, pinned OZ source execution, captured Aave evidence and BSC static-aToken conversion controls. | Independent oracles for remaining models and end-to-end initialized holder/global/clock/epoch evaluation, including idle blocks, invalidations and fork undo. Separate package qualification and cold/unsupported observations. |
| [#17 Native balances](https://github.com/pinax-network/substreams-evm-extended/issues/17) | One-map reducer, saved controls, producer/fork matrix and #55's bounded BSC package/getter checks. | Remaining lifecycle/reward/fork reasons, Base failed-deposit persistence and per-network native identity/alias controls under #8. BSC success does not satisfy the full matrix. |
| [#21 Roadmap](https://github.com/pinax-network/substreams-evm-extended/issues/21) | Packages/shared contracts exist; #11/#12/#13/#18/#19/#20/#22/#24 are closed. | Complete #14/#15/#16/#17/#23 and shared #7/#8 gates. Preserve extraction, evaluated balances and downstream interpretation as separate responsibilities. |
| [#23 stETH](https://github.com/pinax-network/substreams-evm-extended/issues/23) | Shares/global inputs, Aragon guards, exact internal-share conversion and #51's independent-review fixes. | Actual runtime/activation/dependency qualification, captured Ethereum rebase/share/fee/burn/external-share cases and package/getter parity. Post-invalidation decoding and successor-epoch/undo conformance remain offline follow-ups. |
| [#61 Six BSC exclusions](https://github.com/pinax-network/substreams-evm-extended/issues/61) | #60 preserves runtime/dependency mismatches and unknown-write refusals. | Review BNC4's beacon, sPro/swkeyDAO2 divisor changes and TAKE/RADR/TOPS refused writes. Preserve original refusal regressions; requalify separate replacements with fresh runtime, package, RPC and holder controls. |

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
