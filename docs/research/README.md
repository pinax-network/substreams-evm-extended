# Pinned-source research notes

JSON notes produced offline (WebFetch of raw GitHub files at pinned commits and
official documentation; no RPC, no explorers as sole source) while scoping the
roadmap packages. Each file has `topic`, `facts` (with `key`, `value`/
`claimed_value`, `source_url`, `source_pin`, `note`, and where a second pass
checked the claim a `verdict`/`correction`) and `open_questions`. They are
evidence of what was read and when, not qualification of any deployment.

| File | Topic | Used by |
| --- | --- | --- |
| `00-Compound_v2__Ethereum_mainnet__cToken_ma.json`, `08-…` | Compound v2 cToken markets: addresses, proxy pattern, storage layout, exchange-rate and accrual formulas, JumpRateModelV2, IRM addresses (08 is the verified second pass) | `compound-v2/balance-state`, `conformance::compound_v2` |
| `01-Aave_V3_deployments__BNB_Smart_Chain__Et.json` | Aave V3 deployments (BNB, Ethereum, Base), address book pins, aToken/Pool layout facts | `aave/balance-state`, `aave/actions`, `erc4626` (static aToken) |
| `02-Compound_III__Comet__markets_for_the_fir.json` | Comet markets, packed storage, index math, rate immutables, deployments JSON | `compound-v3/balance-state`, `conformance::comet` |
| `03-Firehose_Ethereum_Extended_block_model__.json` | `sf.ethereum.type.v2.Block` Extended model, DetailLevel, `Block.ver`, BalanceChange reasons, ordinals | `common/persist`, `native/balances`, `evm/executions` |
| `04-Standards_and_event_signatures_for_mappi.json` | ERC-20/721/4626/7702 event signatures and standards facts | `erc20/events`, `evm/executions`, coverage doc |
| `05-ERC_4626_vault_selection_for_a_first_sou.json` | EIP-4626 semantics, sDAI/Pot, OpenZeppelin virtual offset, Venus, Aave StataToken candidates | `erc4626/balance-state`, `conformance::erc4626` |
| `06-Lido_stETH_on_Ethereum_mainnet__addresse.json` | Lido v4.0.1: addresses, storage positions, share math, TokenRebased, CL reporting | `lido/balance-state`, `conformance::lido` |
| `07-Chain_identity_and_finality_for_BNB_Smar.json` | Chain ids and finality for BSC, Ethereum, Base, HyperEVM, Arc | `docs/extraction-coverage.md` |
| `09-BSC_exclusion_candidates.json` | Saved TAKE/TOPS source/runtime reconstruction, TAKE maintainer pins and one unqualified guard-slot candidate; unresolved RADR/BNC4/sPro/swkeyDAO2 evidence | `erc20/balances`, issue #61 |
| `10-Compound_v2_checked_arithmetic.json` | Checked uint256 operation order and compiled pinned-source comparisons for current/2019 CToken, WhitePaper and legacy/current jump-rate arithmetic | `conformance::compound_v2`, issues #14/#16 |
| `11-Known_zero_reference_inputs.json` | Known-zero Aave index and SavingsDai chi branches, floor/ceil division failures and retained missing-state boundaries | `conformance::aave`, `conformance::erc4626`, issues #7/#16 |
| `12-BurnMint_role_candidate.json` | Complete saved/primary source binding with one import relocation, reachable role membership and unqualified exact-path candidate | `erc20/balances`, issue #4 |
| `13-Comet_source_oracle.json` | Official-compiler execution of pinned rates, utilization, indices, supplied/debt getters and conversions, with timestamp and signed-width controls | `conformance::comet`, issues #15/#16 |
| `14-Lido_source_oracle.json` | Pinned getter execution, unchecked product wrapping, checked total addition and raw-input preservation under extraction specification revision 4 | `conformance::lido`, `lido/balance-state`, issues #16/#23 |
| `15-SavingsDai_source_oracle.json` | Pinned rpow/divup and vault conversion execution, full uint256 Pot timestamp retention and explicit revert/invalid-opcode controls | `conformance::erc4626`, `conformance::retained`, issues #7/#16 |
| `16-Point_Bedrock_role_candidates.json` | Complete saved source/runtime binding, exact public source matches and Point's remaining token-source gap; separate membership-path candidates and bounded saved replay | `erc20/balances`, issue #4 |
| `17-Aave_static_source_oracles.json` | Pinned current/v3.4 Aave and static-token execution, stored uint128 index narrowing and precise division failure order, with explicit harness/dependency bounds | `conformance::aave`, `conformance::erc4626`, issue #16 |
| `18-FHE_B2_role_candidates.json` | Exact primary source/runtime bindings and independently recomputed immutable substitutions; narrow membership candidates preserve unrelated metadata and both historical cohorts | `erc20/balances`, issue #4 |

Claims marked with a correction in a file supersede the corresponding claim.
