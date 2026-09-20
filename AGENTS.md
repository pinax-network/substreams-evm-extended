# Repository guidance

This repository contains EVM Substreams that consume Firehose **Extended**
blocks. The Rust workspace members are the shared protobuf crate, the shared
`common/persist` persisted-effect rules, the host-only `conformance` reference
models, and the packages `erc20/balances`, `erc20/events`, `native/balances`,
`aave/balance-state` and `evm/executions`, each with its native Rust
diagnostic tools.

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
- Do not add `db_out`, database-change modules or custom sinks. External native
  sinks consume the existing protobuf; local sink state belongs under `out/`.
- Host-side qualification can use RPC. Production balance processing cannot.

## Diagnostics and evidence

- Keep diagnostic code, regression tests and scripts in Rust. Host tools and
  their dependencies must remain excluded from the WASM ingestion path.
- Preserve captured fixtures, source/runtime bindings, failed attempts and
  historical evidence. Use fresh output directories for new live checks.
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
