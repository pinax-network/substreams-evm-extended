# TOPS original-runtime context inputs

These files support a host-only execution proof for the exact captured TOPS runtime. They do not admit a layout or qualify a deployment.

The complete source, compiler output, creation/runtime bytecode, immutable reconstruction, AST, Yul, source maps, metadata and dependency records remain in `../tops-operation-proof/`. The two copied historical compiler records here retain their original hashes and old source inventory. A fresh preparation verifies those artifact pins and snapshots the current executable sources; it does not claim to rerun the historical compiler.

The three Python files are unchanged primary EVM instruction specifications from `ethereum/execution-specs` commit `3e170675290673a68eeb4652501fb2ae74fea0cb`, under `src/ethereum/forks/cancun/vm/instructions/`. `LICENSE-execution-specs` is the exact upstream **CC0 1.0 Universal** license. File digests and immutable URLs are enforced by `tops_runtime_proof::SPECS` and included in preparation reports. They establish operand, memory and return-buffer semantics; the bounded executor supplies synthetic GAS words and does not implement gas accounting or execute external contracts.

The original runtime still has nine capture-bound sources, eight exact OpenZeppelin dependency pins and an explicit custom-token primary source gap. Neither the supplied pair responses nor the supplied USDT response verifies those contracts or their state. `ORIGIN` and timestamp are declared inputs. `CALLVALUE` is fixed at zero.

The final `source-02`/`operations-02` run under `out/tops-runtime-cleanup-proof-20260929/` uses an identical 291-file current-source inventory and records 400 original-runtime calls (370 returns, 21 exact reverts, nine explicit unsupported boundaries). Its five immutable evidence copies are linked from [the focused proof document](../../../docs/tops-runtime-cleanup-proof.md). The fresh preparation is an artifact verification and executor snapshot; the original compiler report remains historical.
