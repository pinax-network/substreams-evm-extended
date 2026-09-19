# substreams-evm-extended

EVM Substreams that require Firehose Extended blocks, maintained separately
from the modules in [substreams-evm](https://github.com/pinax-network/substreams-evm).

The first module is [ERC-20 storage balances](erc20/balances/README.md).
One RPC-free `map_events` reads persisted storage changes and emits
`evm.balances.v1.Events`, with the exact protobuf used by the RPC balance
implementation. Layouts are explicitly configured and verified; the default
is `[]`. Blocks without the required Extended data are rejected.

[Native balances](native/balances/README.md) is a second one-map package with
the same protobuf: `Balance.contract` is absent and `amount` is the final
persisted native balance of every account changed in the block. It ports the
historical RPC-free native reducer and replays saved BSC blocks offline; no
SPKG is committed and live qualification is pending.

The [native ClickHouse sink supplied by the Substreams CLI](erc20/balances/clickhouse/README.md)
consumes the protobuf directly. This workspace contains no `db_out` module,
custom ClickHouse/PostgreSQL sink, or database-change dependency.

The shared schema stays in the repository-root `proto/` crate:

- `proto/v1/balances.proto`: canonical balance schema.
- `proto/src/pb/`: shared generated Rust types.
- `common/persist/`: shared persisted-effect rules for Extended blocks.
- `erc20/balances/`: Extended-block ERC-20 balance module and native audit tools.
- `native/balances/`: Extended-block native balance module and offline replay tool.

Keeping the schema separate from the module gives future Extended modules the
same protobuf contract without copying generated types.

## Build and test

The pinned Rust toolchain includes the WASM target. Building packages also
requires the Substreams CLI.

```sh
cargo test --workspace --lib --bins --locked
make -C erc20/balances pack
```

All executable diagnostics and regression tests are Rust. The
[audit tools](erc20/balances/README.md#compare-and-audit)
compare actual packaged output with hash-pinned RPC results and separately
check initialized holders. RPC is used for qualification, not inside the map.

## Coverage

The migrated BSC evidence covers 431 explicitly qualified profiles among the
first 450 candidates ranked by activity in the sampled RPC stream. The
[latest coverage report](erc20/balances/docs/refined450-coverage.md)
preserves emitted-balance, initialized-holder, cold-start and remaining-gap
results separately. Historical parity does not establish universal ERC-20
support or global holder enumeration.

Some role-storage boundaries and calculated/reflection balances remain under
investigation. Ethereum, Base, HyperEVM and Arc require independent qualification
after the BSC work; the repository name is not a claim of verified coverage on
every EVM network.

The current source adds [exact typed mapping paths](erc20/balances/docs/typed-mapping-paths.md)
and opt-in [enumerable role-set checks](erc20/balances/docs/enumerable-role-sets.md),
with offline checks against saved data. These changes are not included in the
preserved SPKGs. Live chain testing is paused; new token and package
qualification remains pending.

Outstanding implementation, holder coverage, packaging and network work is
tracked in [GitHub follow-up issues](docs/follow-up.md). The requested chains,
asset scopes, protocol deployments and action-evidence mapping for the
extraction roadmap are recorded in [extraction coverage](docs/extraction-coverage.md).

See [migration provenance](docs/migration.md) for the original PR, preserved
schema/package digests and the distinction between historical qualification
and the new repository's build and sink checks.
