# Follow-up work

The completed migration and exact typed mapping-path source are on `main`.
The historical SPKGs and qualification evidence retain their original scope;
they do not establish parity for the newer source. No new token was qualified
by the latest offline APD/DSG replay.

| Work | GitHub issue |
| --- | --- |
| Source-bound enumerable role-set support, including DSG | [#2](https://github.com/pinax-network/substreams-evm-extended/issues/2) |
| APD/DSG controls, packaged parity and initialized holders | [#3](https://github.com/pinax-network/substreams-evm-extended/issues/3) |
| Migration of legacy role profiles to exact paths | [#4](https://github.com/pinax-network/substreams-evm-extended/issues/4) |
| Remaining 19 sampled BSC candidates | [#5](https://github.com/pinax-network/substreams-evm-extended/issues/5) |
| New versioned package and native sink qualification | [#6](https://github.com/pinax-network/substreams-evm-extended/issues/6) |
| Cold-start initialization and holder completeness | [#7](https://github.com/pinax-network/substreams-evm-extended/issues/7) |
| Ethereum, Base, HyperEVM and Arc | [#8](https://github.com/pinax-network/substreams-evm-extended/issues/8) |

Live Substreams, Firehose, RPC and native sink testing is paused until explicitly
resumed. Offline source review, Rust tests and saved-data analysis remain
available. Creating these issues or merging the source does not resume live
tests or deploy an ingestion service.

The [APD/DSG review](../erc20/balances-storage/docs/typed450-offline-review.md)
records 25 emitted balances matching saved canonical RPC output and 33 cold
unknown observations across all 1,024 cached blocks. The
[DSG runtime investigation](../erc20/balances-storage/docs/evidence/dsg-enumerable/)
preserves synthetic ordering evidence for unfinished enumerable support; it is
not a production implementation or producer-visibility qualification.
