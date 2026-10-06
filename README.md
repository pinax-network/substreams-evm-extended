# substreams-evm-extended

EVM Substreams that require Firehose Extended blocks, maintained separately
from the modules in [substreams-evm](https://github.com/pinax-network/substreams-evm).

The first module is [ERC-20 balances](erc20/balances/README.md). One RPC-free
`map_events(block)` infers each block's ERC-20 balances from `Transfer` flows
and persisted storage changes, with no params or layouts, and emits
`evm.balances.v1.Events`, with the exact protobuf used by the RPC balance
implementation. Blocks without the required Extended data are rejected.

[Native balances](native/balances/README.md) is a second one-map package with
the same protobuf: `Balance.contract` is absent and `amount` is the final
persisted native balance of every account changed in the block. It ports the
historical RPC-free native reducer. It was live-qualified on BSC on 2026-09-23
(WASM `48d89d28…`, module `5a2a2e0c…`); no later build is.

[`evm-balances`](evm-balances/README.md) imports both packages and writes the
substreams-evm ClickHouse tables (`blocks`, `erc20_balances`,
`native_balances` and the historical OHLC views) through one `db_out`, with
upstream's SQL byte for byte embedded in its spkg. It is the workspace's only
`db_out` and database-change dependency. The earlier per-package CLI
native-sink paths are at
[`cb62110`](https://github.com/pinax-network/substreams-evm-extended/tree/cb6211007f1f9d0cb5852a6b666da475a8fa6542).

The committed packages are CI's canonical Linux builds (Rust 1.99.0,
substreams CLI v1.22.0; identify a build by these hashes, never by spkg
sha256). None is live-qualified.

| Package | Module | Module hash | WASM sha256 |
| --- | --- | --- | --- |
| `spkg/native-balances-v0.2.1.spkg` | `map_events` | `72453949f9227b31dad20f80fabc09774d740d1b` | `d3b6a5497b230d2c…` |
| `spkg/erc20-balances-v0.4.0.spkg` | `map_events` | `d8a9db86240ca6466270f140798fae0b8db05f7b` | `17a847aa77346058…` |
| `spkg/evm-balances-v0.6.0.spkg` | `db_out` | `2710d961971add90e77e9693690a9abba03b1546` | `b8bb73e4c9a01869…` |

`evm-balances` imports the two maps with the same module hashes, and embeds
the schema `ed1c3bff…`.

Picking this up? Start with [`docs/handoff.md`](docs/handoff.md) (state,
evidence, open findings, next steps) and the procedures in [`skills/`](skills/README.md).

The shared schema stays in the repository-root `proto/` crate:

- `proto/v1/balances.proto`: canonical balance schema.
- `proto/v1/balance_state.proto`: versioned holder/global balance-state
  companion schema for protocol packages ([contract](docs/balance-state-contract.md)).
- `proto/v1/executions.proto`: execution-fact schema (`evm/executions`).
- `proto/v1/erc20_events.proto`: ERC-20 event evidence schema (`erc20/events`).
- `proto/v1/aave_actions.proto`: Aave lending-action evidence schema (`aave/actions`).
- `proto/v1/dex-pool-state.proto` and `proto/v1/dex/`: wire-compatible V2/V3 closing-state messages (`dex/pool-state`), without unrelated ABI event projections.
- `proto/src/pb/`: shared generated Rust types (buffa, from `buf generate` in `proto/`).
- `common/persist/`: shared persisted-effect rules for Extended blocks.
- `erc20/balances/`: Extended-block ERC-20 balance module.
- `erc20/events/`: standard ERC-20 Transfer and Approval log evidence.
- `native/balances/`: Extended-block native balance module.
- [`evm-balances/`](evm-balances/README.md): one `db_out` from `native/balances` and `erc20/balances` into the substreams-evm ClickHouse schema, which its spkg embeds.
- `aave/balance-state/`: Aave V3 aToken holder basis and reserve state module.
- `compound-v2/balance-state/`: Compound v2 cToken shares, market words, cash and rate-model dependency module.
- `erc4626/balance-state/`: ERC-4626 vault shares, total supply and source-bound conversion inputs (Aave static aToken, Savings DAI, OpenZeppelin).
- `lido/balance-state/`: Lido stETH holder shares, packed global words, derived pooled ether and report evidence module.
- `compound-v3/balance-state/`: Compound III (Comet) signed principal and market index module.
- `aave/actions/`: Aave V3 Pool lending-action evidence.
- `evm/executions/`: call trees, logs, code changes and SetCode authorizations.
- [`dex/pool-state/`](dex/pool-state/README.md): one RPC-free `map_events` for complete Extended-block V2 reserves and ordered V3 changes; no price or pool-admission policy.

Keeping the schema separate from the module gives future Extended modules the
same protobuf contract without copying generated types.

Generic pool-state extraction is maintained in `dex/pool-state`; contract ABI
sources and generated event structs remain in `substreams-abis`. With the
substreams 0.8.0 bump the owner moved that pin from tag `v1.5.0` to tag
`v2.0.0`, the release of its port to substreams 0.8.0
(pinax-network/substreams-abis#55); the package's outputs on the captured
blocks are unchanged. Its new Extended-only package is checked offline;
historical live evidence does not qualify the new input boundary or artifact
digest. The owner's hold on
IVSpikes-associated source and RPC checks remains active until explicit
reauthorization.

## Build and test

The pinned Rust toolchain includes the WASM target. The packages use
substreams 0.8.0, substreams-ethereum 0.12.0 and buffa 0.9.2. Building
packages also requires the Substreams CLI.

```sh
cargo test --workspace --lib --bins --tests --locked
make -C erc20/balances pack
```

The repository holds package code only; regression tests are Rust, inside the
package crates. The host tools that produced the qualification evidence
(packaged-output comparison with hash-pinned RPC results, initialized-holder
checks, offline replays, reference models) were removed on 2026-10-03. The
dated evidence documents refer to them as they were at
[`6dade89`](https://github.com/pinax-network/substreams-evm-extended/tree/6dade8957887c0c278cfa8da6bef61b9cc22f534).
RPC is used for qualification, not inside the maps.

## Releases

`native/balances`, `erc20/balances` and `evm-balances` are released by tag,
`<package>-v<version>` (for example `native-balances-v0.2.1`), equal to the
manifest's version. One version names one module hash: a change to a
package's WASM bumps its version and commits the spkg that CI's "Package
hashes" job built. The tag then runs
[`release.yml`](.github/workflows/release.yml), which:

- builds and packs the package on Linux, CI's canonical builder;
- writes `HASHES.txt`: commit, rustc, substreams CLI, module hashes, WASM
  sha256 and, for evm-balances, the schema sha256;
- requires `substreams registry verify` to pass with no warning, and the
  module hashes of the committed spkg;
- publishes a GitHub release with the spkg and `HASHES.txt`.

Identify a release by its module hashes, never by the spkg sha256:
`substreams pack` is not deterministic. A release is not live qualification;
its notes say so unless dated evidence names its module hashes.

## Coverage

ERC-20 balances are inferred, not configured. On five captured BSC intervals,
scored against every row the RPC reference package emitted for them, the
package reproduces 58.5–62.8% of the reference rows with 99.92–100% value
precision; the [measured results](erc20/balances/README.md#measured) give each
interval, its scored holder set and the limits. Matching intervals do not
establish universal ERC-20 support or global holder enumeration. The former
layout path covered 431 explicitly qualified profiles among the first 450 BSC
candidates ranked by activity; its code, coverage reports and evidence remain at
[`9b41c7f`](https://github.com/pinax-network/substreams-evm-extended/tree/9b41c7f/erc20/balances).

Computed balances (reward, reflection, rebasing, scaled) remain open. Ethereum,
Base, HyperEVM and Arc require independent qualification after the BSC work; the
repository name is not a claim of verified coverage on every EVM network.

The inference package is packed and committed (`spkg/erc20-balances-v0.4.0.spkg`,
above) but has only been checked offline against saved data: it has not run in
a Substreams engine. Its BSC qualification is
[#124](https://github.com/pinax-network/substreams-evm-extended/issues/124), and
each live step needs the owner's OK ([live use](AGENTS.md#evidence)).

Outstanding implementation, holder coverage, packaging and network work is
tracked in [GitHub follow-up issues](docs/follow-up.md). The requested chains,
asset scopes, protocol deployments and action-evidence mapping for the
extraction roadmap are recorded in [extraction coverage](docs/extraction-coverage.md).

See [migration provenance](docs/migration.md) for the original PR, preserved
schema/package digests and the distinction between historical qualification
and the new repository's build and sink checks.
