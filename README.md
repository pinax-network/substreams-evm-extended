# substreams-evm-extended

EVM Substreams that require Firehose Extended blocks, maintained separately
from the modules in [substreams-evm](https://github.com/pinax-network/substreams-evm).

The first module is [ERC-20 storage balances](erc20/balances-storage/README.md).
One RPC-free `map_events` reads persisted storage changes and emits
`evm.balances.v1.Events`, with the exact protobuf used by the RPC balance
implementation. Layouts are explicitly configured and verified; the default
is `[]`. Blocks without the required Extended data are rejected.

The [native ClickHouse sink supplied by the Substreams CLI](erc20/balances-storage/clickhouse/README.md)
consumes the protobuf directly. This workspace contains no `db_out` module,
custom ClickHouse/PostgreSQL sink, or database-change dependency.

## Build and test

The pinned Rust toolchain includes the WASM target. Building packages also
requires the Substreams CLI.

```sh
cargo test --workspace --lib --bins --locked
make -C erc20/balances-storage pack
```

All executable diagnostics and regression tests are Rust. The
[audit tools](erc20/balances-storage/README.md#compare-and-audit)
compare actual packaged output with hash-pinned RPC results and separately
check initialized holders. RPC is used for qualification, not inside the map.

## Coverage

The migrated BSC evidence covers 431 explicitly qualified profiles among the
first 450 candidates ranked by activity in the sampled RPC stream. The
[latest coverage report](erc20/balances-storage/docs/refined450-coverage.md)
preserves emitted-balance, initialized-holder, cold-start and remaining-gap
results separately. Historical parity does not establish universal ERC-20
support or global holder enumeration.

Some role-storage boundaries and calculated/reflection balances remain under
investigation. Ethereum, Base, HyperEVM and Arc require independent qualification
after the BSC work; the repository name is not a claim of verified coverage on
every EVM network.

See [migration provenance](docs/migration.md) for the original PR, preserved
schema/package digests and the distinction between historical qualification
and the new repository's build and sink checks.
