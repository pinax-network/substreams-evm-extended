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
