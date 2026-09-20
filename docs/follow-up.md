# Follow-up work

The completed migration, exact typed mapping-path source and opt-in enumerable
role-set validator are on `main`.
The historical SPKGs and qualification evidence retain their original scope;
they do not establish parity for the newer source. No new token was qualified
by the latest offline APD/DSG replay.

| Work | GitHub issue | Effort |
| --- | --- | --- |
| Source-bound enumerable role-set support, including DSG | [#2](https://github.com/pinax-network/substreams-evm-extended/issues/2) | High |
| APD/DSG controls, packaged parity and initialized holders | [#3](https://github.com/pinax-network/substreams-evm-extended/issues/3) | Medium |
| Migration of legacy role profiles to exact paths | [#4](https://github.com/pinax-network/substreams-evm-extended/issues/4) | High |
| Remaining 19 sampled BSC candidates | [#5](https://github.com/pinax-network/substreams-evm-extended/issues/5) | High |
| New versioned package and native sink qualification | [#6](https://github.com/pinax-network/substreams-evm-extended/issues/6) | Medium |
| Cold-start initialization and holder completeness | [#7](https://github.com/pinax-network/substreams-evm-extended/issues/7) | High |
| Ethereum, Base, HyperEVM and Arc | [#8](https://github.com/pinax-network/substreams-evm-extended/issues/8) | High |

The [extraction roadmap](https://github.com/pinax-network/substreams-evm-extended/issues/21)
adds focused balance-state and execution-fact packages. Its requirements are
recorded in [extraction coverage](extraction-coverage.md).

| Work | GitHub issue | Effort |
| --- | --- | --- |
| Chain, asset and action coverage requirements | [#11](https://github.com/pinax-network/substreams-evm-extended/issues/11) | Low |
| Versioned holder/global balance-state contract ([contract](balance-state-contract.md)) | [#12](https://github.com/pinax-network/substreams-evm-extended/issues/12) | Medium |
| Aave aToken basis and reserve inputs ([`aave/balance-state`](../aave/balance-state/README.md)) | [#13](https://github.com/pinax-network/substreams-evm-extended/issues/13) | High |
| Compound v2 shares and exchange-rate dependencies | [#14](https://github.com/pinax-network/substreams-evm-extended/issues/14) | High |
| Compound III principal and market inputs | [#15](https://github.com/pinax-network/substreams-evm-extended/issues/15) | High |
| Rust conformance models for time-dependent balances | [#16](https://github.com/pinax-network/substreams-evm-extended/issues/16) | High |
| Native balances package (`native/balances`) | [#17](https://github.com/pinax-network/substreams-evm-extended/issues/17) | High |
| EVM call trees and persisted execution facts ([`evm/executions`](../evm/executions/README.md)) | [#18](https://github.com/pinax-network/substreams-evm-extended/issues/18) | High |
| Standard ERC-20 transfer and approval evidence ([`erc20/events`](../erc20/events/README.md)) | [#19](https://github.com/pinax-network/substreams-evm-extended/issues/19) | Medium |
| Aave lending-action evidence adapter ([`aave/actions`](../aave/actions/README.md)) | [#20](https://github.com/pinax-network/substreams-evm-extended/issues/20) | Medium |
| Non-Transfer ERC-20 balance regression coverage | [#22](https://github.com/pinax-network/substreams-evm-extended/issues/22) | Medium |
| stETH shares and global rebase state | [#23](https://github.com/pinax-network/substreams-evm-extended/issues/23) | High |
| ERC-4626 shares and conversion state | [#24](https://github.com/pinax-network/substreams-evm-extended/issues/24) | High |

The matching GitHub labels estimate the full remaining scope, not urgency or
readiness. Low is reserved for small localized changes; none of these current
issues fits that category. Medium work uses established validation paths;
High work includes substantial implementation, uncertain semantics or broad
qualification. Dependencies and the live-testing pause are separate constraints.

Live Substreams, Firehose, RPC and native sink testing is paused until explicitly
resumed. Offline source review, Rust tests and saved-data analysis remain
available. Creating these issues or merging the source does not resume live
tests or deploy an ingestion service.

The [APD/DSG review](../erc20/balances/docs/typed450-offline-review.md)
records 25 emitted balances matching saved canonical RPC output and 33 cold
unknown observations across all 1,024 cached blocks. The
[DSG runtime investigation](../erc20/balances/docs/evidence/dsg-enumerable/)
preserves synthetic ordering evidence. The opt-in
[enumerable rule](../erc20/balances/docs/enumerable-role-sets.md) implements
source-level operation checks; this evidence is not producer-visibility or
token qualification, and issue #2 remains open for those checks.
