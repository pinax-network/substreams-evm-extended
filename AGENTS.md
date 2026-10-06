# Repository guidance

This repository contains EVM Substreams that consume Firehose **Extended**
blocks. The Rust workspace members are the shared protobuf crate, the shared
`common/persist` persisted-effect rules, the `common/epochs` schedule helper,
and the packages `erc20/balances`, `erc20/events`, `native/balances`,
`aave/balance-state`, `aave/actions`, `compound-v2/balance-state`,
`compound-v3/balance-state`, `lido/balance-state`, `erc4626/balance-state`,
`evm/executions`, `dex/pool-state` and `evm-balances`.

The repository holds Substreams package code only: map crates, the
`evm-balances` db_out crate, manifests, SQL, Makefiles, regression tests and
fixtures. Host-side diagnostic tools, reference models and harnesses were
removed on 2026-10-03. They remain in git history at `6dade89`, which the
dated evidence documents refer to. Do not add new tool crates or CLI harnesses.

## Production boundary

- Keep one RPC-free `map_events` module per map package and the shared
  `evm.balances.v1.Events` output for balance packages. Preserve the canonical
  balances protobuf field numbers and wire types; do not introduce a custom
  storage protobuf or map cache. Protocol balance-state packages emit the
  companion `evm.balance_state.v1.Events` instead, never a replacement for
  `Balance.amount`. `erc20/balances` keeps its embedded copy of the
  persistence rules; `common/persist` must not diverge from it.
- Require Extended block data. The protocol balance-state packages and
  `aave/actions` also require explicit, independently qualified layouts and
  bindings (markets, epochs, vaults, pools). Except in `erc20/balances` (next
  item), unknown writes and unreviewed runtime or dependency changes must fail
  closed.
- `erc20/balances` is inference only, at the owner's direction (2026-10-05):
  `map_events(block)` takes no params or layouts and infers transfer-guided
  balances for every contract from the block's persisted storage. It stays
  RPC-free with bounded per-block work. A block fails only when it is not a
  complete Extended block or the persistence rules cannot resolve it; inference
  never fails a block and drops a contract's rows for the block on any doubt.
  Undetected computed balances remain possible: its rows are measured, not
  qualified. The former layout path, with its tests, fixtures and qualification
  evidence, is in git history at `9b41c7f`. Its last packed build is the
  committed `spkg/erc20-balances-v0.1.0.spkg` (2026-09-23), which predates the
  typed-path and enumerable-set source at `9b41c7f`; the inference package is
  v0.2.0.
- `dex/pool-state` emits the existing `dex.pool_state.v1.BlockPoolState` through
  one `map_events(Block)`. Preserve pool-state protobuf names/field numbers,
  exact integers, canonical log order and explicit invalid markers. Require
  Extended blocks and successful transaction call traces; never fall back to
  receipt logs. Contract ABI sources and generated structs stay in
  `substreams-abis`, not this repository. At the owner's direction
  (2026-10-05) that pin moved with the substreams 0.8.0 bump from tag `v1.5.0`
  to tag `v2.0.0`, the owner's release of the port to substreams 0.8.0 and
  substreams-ethereum 0.12.0 (pinax-network/substreams-abis#55). Otherwise the
  pinned types are reused without a dependency version change.
  Read `dex/pool-state/README.md`; extraction is not pool or price admission.
- `evm-balances` is the only `db_out` (owner, 2026-10-05). It imports
  `native/balances` and `erc20/balances` as local manifests and writes the
  substreams-evm ClickHouse tables with the row format of upstream `db_out` at
  substreams-evm@`cb8607f`: tables, keys, columns and value encoding. It
  encodes them with `substreams-database-change` 5.0.0, where every field also
  carries `update_op = UPDATE_OP_SET`; upstream's 3.0.0 has no such field, and
  the SQL sink reads only a CREATE field's name and value. Its Makefile builds
  the imported packages one at a time, so their WASM and module hashes equal
  the standalone packages'. Its `clickhouse/schema.*.sql` are
  upstream's files byte for byte; its manifest's `sink:` section embeds their
  concatenation. It adds no map, store, params or RPC. Do not add other
  `db_out` or database-change modules, or custom sinks. Other packages' map
  outputs remain the contract for external native sinks; local sink state
  belongs under `out/`.
- `native/balances` is params-free at the owner's direction (2026-10-06):
  `map_events(block)` has no producer-version gate or manifest network allowlist.
  Keep its complete Extended-block, failed-transaction reason and persisted
  balance continuity checks. Accepting a block is not network qualification;
  network-specific persistence work remains separate. `evm-balances` imports
  both balance maps without params and defaults to `network: bsc` for routing.
- Host-side qualification can use RPC. Production balance processing cannot.

## Evidence

- Verify new work ad hoc (for example in clickhouse-local or with scratch
  queries), and report the method and results in the pull request. Keep
  regression tests in Rust inside the package crates.
- Preserve captured fixtures, source/runtime bindings, failed attempts and
  historical evidence: in the tree, or at a cited commit when the owner removes
  a package path (the `erc20/balances` layout path: `9b41c7f`; the per-package
  CLI native-sink ClickHouse paths: `cb62110`). Use fresh output
  directories for new live checks.
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

The workspace is on substreams 0.8.0, substreams-ethereum 0.12.0 and buffa
0.9.2 (2026-10-05). `proto/src/pb` is generated from `proto/v1` by `buf
generate` in `proto/` (`buf.gen.yaml`: the `buf.build/anthropics/buffa` plugin
v0.9.2, the version and options the SDK crates use); regenerate it rather than
editing it. Only `pb/mod.rs` is written by hand.

Package Makefiles build WASM with `--remap-path-prefix=<repository root>=.`,
so WASM and module hashes do not depend on the checkout directory; keep it in
every WASM build. Handlers decode their input with buffa's default limits
(32 MiB of repeated elements per block); see `docs/handoff.md` for the
measured headroom.
