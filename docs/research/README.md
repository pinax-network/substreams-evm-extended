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

Claims marked with a correction in a file supersede the corresponding claim.
