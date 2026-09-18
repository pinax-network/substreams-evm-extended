# Migration from substreams-evm

This repository carries the Extended-block ERC-20 balance work from
[substreams-evm PR #262](https://github.com/pinax-network/substreams-evm/pull/262).
The source baseline is commit
`9f723e4b28a3abad755384ff1329950ea1287938`, plus the completed local five-token
qualification, MUSD/OLY role corrections and dense-stream clock verification.
The migration includes the current working files, not only the previously
pushed commit.

## Workspace boundary

The new workspace contains three crates: `proto`, `erc20/balances-storage`
and its Rust diagnostic tools. The original directory depth is retained so
captured fixtures, report links and audit commands keep their paths.
Unrelated protocol modules, `common`, database-change dependencies, `db_out`
modules and custom database sinks are not part of this workspace.

The canonical RPC reference is retained as an immutable standalone SPKG; its
dependencies are embedded. Its source modules do not need to be copied here.
The native ClickHouse sink consumes the unchanged Events protobuf directly;
see [setup and validation](../erc20/balances-storage/clickhouse/README.md).

The old PR's history and discussions remain available at their original URL.
The source repository retains its shared balance protobuf and RPC-based balance
module. A separate cleanup PR removes its previously merged Extended-only
storage module after the new home is available.

## Exact preserved artifacts

| Artifact | SHA-256 |
| --- | --- |
| `proto/v1/balances.proto` | `1d8dd199cda6185d4a159abbc4e9afc404d8e698d7428266ea559b2d8d24ec6a` |
| `proto/src/pb/evm.balances.v1.rs` | `c297dedec83a70f0cca7753c13395e9bd06307466b48fc90b3414be90bb013bd` |
| Canonical RPC `spkg/erc20-balances-v0.3.4.spkg` | `8aaa03b551b9d67ce1ea9aa0dae4310eda1807f2bc161bd53b17f82de9d92543` |
| Historical `spkg/reference/erc20-balances-storage-v0.1.0-before-migration.spkg` | `f1d57bdff549947cd69e47d117ad8cd74933c1e065b687f958fea4de39004d81` |
| Rebuilt storage WASM at the migration baseline | `36f5c502ec6546fc842ffaf8cfc692140d10e7f8cb58ff731c259a1b6d48c117` |

The schema and generated Rust types are byte-identical to the source tree.
Only the protobuf module wrapper was reduced to the balance namespace. The
build at migration commit `bffd1660c7c773db0c4e670637a96de339ee604d`
also produces byte-identical production WASM. Both historical and
migrated packages report module hash
`d94199efaedeed37d58d1be9780b46138caf5576`, with one `map_events`, default `[]`,
the Extended Ethereum block input and `evm.balances.v1.Events` output.

The migrated SPKG has different package metadata and embedded documentation.
Its digest is recorded separately in [migration evidence](evidence/migration.json).
Historical reports keep the digest of the package they actually tested; those
reports are not rewritten to imply that their RPC calls used a later package.

Subsequent [typed mapping-path source changes](../erc20/balances-storage/docs/typed-mapping-paths.md)
are validated separately using offline Rust checks and captured data. They are
not embedded in these preserved SPKGs and do not inherit live qualification from
the migration baseline. Live chain testing remains paused at the user's request.

## Validation and retained evidence

The minimal workspace passes 362 Rust library/binary tests. These are the
storage module and its tools' tests; unrelated source-repository tests are
outside this workspace. The release WASM build and package generation pass.
Formatting, Clippy, WASM checking and native-sink results are recorded with
the final migration evidence.

The final pre-migration [combined capture](../erc20/balances-storage/docs/evidence/refined450-combined.json)
checks 431 profiles across 1,024 consecutive BSC blocks, including all streamed
block identities and 110,139 previously RPC-verified balance rows. MUSD/OLY
revalidation is counted separately. The [coverage report](../erc20/balances-storage/docs/refined450-coverage.md)
retains cold unknowns, observed-holder limits and remaining candidates.

Captured fixtures and compact evidence are versioned. The original raw audit
outputs and archived Rust investigation helpers are also preserved locally
under the ignored `erc20/balances-storage/out` directory for continued work;
offline builds and tests do not depend on that local cache. Fresh native-sink
checks use the package built in this repository and separate output directories.

Repository extraction does not broaden token or network qualification. The
remaining role-storage, calculated-balance, initialization and cross-network
work remains explicitly tracked by the migrated reports.
