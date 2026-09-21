# Repository guidance

This repository contains EVM Substreams that consume Firehose **Extended**
blocks. The Rust workspace members are the shared protobuf crate, the shared
`common/persist` persisted-effect rules, the host-only `common/retention`
consumer ledger and `conformance` reference models, and the packages
`erc20/balances`, `erc20/events`, `native/balances`, `aave/balance-state`,
`aave/actions`, `compound-v2/balance-state`, `compound-v3/balance-state`,
`lido/balance-state`, `erc4626/balance-state`, `evm/executions` and
`dex/pool-state`, each with
its native Rust diagnostic tools where they exist.

## Production boundary

- Keep one RPC-free `map_events` module per package and the shared
  `evm.balances.v1.Events` output for balance packages. Preserve the canonical
  balances protobuf field numbers and wire types; do not introduce a custom
  storage protobuf or map cache. Protocol balance-state packages emit the
  companion `evm.balance_state.v1.Events` instead, never a replacement for
  `Balance.amount`. `erc20/balances` keeps its embedded copy of the
  persistence rules; `common/persist` must not diverge from it.
- Require Extended block data and explicit, independently qualified layouts.
  Unknown writes and unreviewed runtime or dependency changes must fail closed.
- `dex/pool-state` emits the existing `dex.pool_state.v1.BlockPoolState` through
  one `map_events(Block)`. Preserve pool-state protobuf names/field numbers,
  exact integers, canonical log order and explicit invalid markers. Require
  Extended blocks and successful transaction call traces; never fall back to
  receipt logs. Contract ABI sources and generated structs stay in
  `substreams-abis`, not this repository; the existing pinned types are reused
  without a dependency version change.
  Read `dex/pool-state/README.md`; extraction is not pool or price admission.
- Do not add `db_out`, database-change modules or custom sinks. External native
  sinks consume the existing protobuf; local sink state belongs under `out/`.
- Host-side qualification can use RPC. Production balance processing cannot.

## Diagnostics and evidence

- Keep diagnostic code, regression tests and scripts in Rust. Host tools and
  their dependencies must remain excluded from the WASM ingestion path.
- Preserve captured fixtures, source/runtime bindings, failed attempts and
  historical evidence. Use fresh output directories for new live checks.
- The IVSpikes-associated pool-state migration is offline only. Its new
  Extended-only package/digest is not qualified by historical live evidence.
  The owner's existing hold on its Substreams/Firehose/RPC checks remains in
  force until explicit reauthorization; do not start a source or RPC smoke test.
- Compare with the immutable canonical RPC reference package. Distinguish newly
  built packages from historical package digests rather than rewriting evidence.
- Coverage claims must state the tested interval and initialized observed-holder
  set. Matching samples do not establish universal token or global-holder support.
- Keep RPC credentials in environment variables and never print access-bearing
  endpoint URLs or secrets into logs or evidence.

## Validation

Use the pinned Rust toolchain and lockfile. Offline validation consists of
formatting, workspace library/binary tests, Clippy for all targets, and a WASM
workspace check. Live RPC, stream and holder checks are separate from offline CI.
See `erc20/balances/README.md` for commands and qualification limits.
