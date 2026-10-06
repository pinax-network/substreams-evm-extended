# Handoff: current state

Where the repository stands on 2026-10-06, for whoever picks it up next. Keep
this page short and current: when a package's version, hashes or
qualification change, update its row here, and put the dated evidence in the
pull request and the package's evidence files. The rules are in
[`AGENTS.md`](../AGENTS.md), which wins where the two disagree; the procedures
are in [`skills/`](../skills/README.md).

## Live use

Each live Substreams, Firehose, RPC or sink step needs the owner's explicit OK
for that step. The rule, with the credential handling and the separate
`dex/pool-state` hold, is in [`AGENTS.md` (Evidence)](../AGENTS.md#evidence);
nothing on this page authorizes a live step.

## Balance packages

The committed spkgs are CI's canonical Linux builds (Rust 1.99.0, substreams
CLI v1.22.0), taken from the "Package hashes" job's `packages` artifact.
Identify a build by its module hash, never by the spkg sha256. None is
live-qualified.

| Package | Version and spkg | Module | Module hash | WASM sha256 |
| --- | --- | --- | --- | --- |
| [`native/balances`](../native/balances/README.md) | v0.2.1, `spkg/native-balances-v0.2.1.spkg` | `map_events` | `72453949f9227b31dad20f80fabc09774d740d1b` | `d3b6a5497b230d2cd07ae981a41cd6ab802ce2663502b9b635afed4acdd1b790` |
| [`erc20/balances`](../erc20/balances/README.md) | v0.4.0, `spkg/erc20-balances-v0.4.0.spkg` | `map_events` | `d8a9db86240ca6466270f140798fae0b8db05f7b` | `17a847aa77346058a160553c1fe3f79ab42cc4c0c5de1e0f3997e3a7b28e05d1` |
| [`evm-balances`](../evm-balances/README.md) | v0.6.0, `spkg/evm-balances-v0.6.0.spkg` | `db_out` | `2710d961971add90e77e9693690a9abba03b1546` | `b8bb73e4c9a01869c9a63657c3d90ba0f193eb15819cf8f86316cab563d4d8d2` |

- **`native/balances`** takes only the block, with no params. Not qualified.
  The last qualified build is historical: the prost build was live-qualified
  on BSC on 2026-09-23 (WASM `48d89d28…`, module `5a2a2e0c…`), for the
  accounts it emitted in the stated windows
  ([evidence](../native/balances/README.md#live-qualification-bsc-2026-09-23)).
  No build since then is qualified.
- **`erc20/balances`** is inference only. Not qualified, and it has not run in
  a Substreams engine. Offline, on five captured BSC intervals scored against
  every row the RPC reference package emitted for them, it reproduces
  58.5–62.8% of those rows with 99.92–100% value precision
  ([measured](../erc20/balances/README.md#measured) gives each interval and
  its scored holders). The earlier layout package v0.1.0, another source, was
  checked on live BSC on 2026-09-23
  ([report at `9b41c7f`](https://github.com/pinax-network/substreams-evm-extended/blob/9b41c7f/erc20/balances/docs/live-package-bsc-2026-09-23.md)).
- **`evm-balances`** imports both maps with the module hashes above and
  embeds the schema `ed1c3bff…`. Not qualified; it has not run in a
  Substreams engine or a sink.

Older versions under `spkg/` (native v0.1.0 and v0.2.0, erc20 v0.1.0 and
v0.2.0, evm-balances v0.4.0 and v0.5.0) are historical builds that the package
READMEs describe. `spkg/erc20-balances-v0.3.4.spkg` is the immutable RPC
reference ([migration](migration.md)); versions skip v0.3.x so that no build
can overwrite it.

## Other packages

None has a committed spkg or a recorded module hash for its current build,
and none is qualified at that build. Every package moved to substreams 0.8.0
on 2026-10-05, so the dated live evidence below covers earlier builds only;
each README states what its evidence covers.

| Package | Version | Default network | Evidence |
| --- | --- | --- | --- |
| `aave/balance-state` | v0.2.0 | `bsc` | [live, 2026-09-22](../aave/balance-state/README.md#live-qualification-bsc-2026-09-22) |
| `aave/actions` | v0.1.0 | `bsc` | [live, 2026-09-23](../aave/actions/README.md#evidence) |
| `erc4626/balance-state` | v0.2.0 | `bsc` | [live static aToken, 2026-09-23](../erc4626/balance-state/README.md#live-qualification-bsc-static-atoken-2026-09-23) |
| `evm/executions` | v0.1.0 | `bsc` | [saved and live blocks, 2026-09-23](../evm/executions/README.md#producer-capabilities-saved-bsc-data) |
| `erc20/events` | v0.1.0 | `bsc` | [captured block, offline](../erc20/events/README.md#evidence) |
| `compound-v2/balance-state` | v0.2.0 | `mainnet` | synthetic tests and compiled layouts ([README](../compound-v2/balance-state/README.md)) |
| `compound-v3/balance-state` | v0.2.0 | `mainnet` | synthetic tests and compiled layouts ([README](../compound-v3/balance-state/README.md)) |
| `lido/balance-state` | v0.2.0 | `mainnet` | synthetic tests and compiled layouts ([README](../lido/balance-state/README.md)) |
| `dex/pool-state` | v0.2.0 | `mainnet` | offline only, under the owner's live hold ([README](../dex/pool-state/README.md#offline-validation-and-packaging)) |

## Validation

[`AGENTS.md` (Validation)](../AGENTS.md#validation) describes the offline
checks, the toolchain pin and the "Package hashes" job. Run the commands of
the "Rust checks" job in [`ci.yml`](../.github/workflows/ci.yml) before a pull
request. A change to a balance package's WASM bumps its version and commits
the spkg from the job's `packages` artifact in the same pull request.
`substreams pack` also embeds the three balance packages' READMEs, so a README
edit changes their spkgs but not their module hashes.

## History

- [`history/handoff-2026-09-21-to-2026-10-06.md`](history/handoff-2026-09-21-to-2026-10-06.md):
  this page's previous content, verbatim: the 2026-09-21 plan, the review and
  qualification record, and every dated section through 2026-10-06, including
  the measured buffa decode headroom. Its
  [section 3](history/handoff-2026-09-21-to-2026-10-06.md#3-where-the-artifacts-live)
  lists where the captured Extended blocks were kept; they are not in git.
- [`history/review-findings-2026-09-21.md`](history/review-findings-2026-09-21.md)
  and [`history/audit-remediation-2026-09-21.md`](history/audit-remediation-2026-09-21.md):
  the 2026-09-21 hardening review and the offline issue-audit remediation.
- [`6dade89`](https://github.com/pinax-network/substreams-evm-extended/tree/6dade8957887c0c278cfa8da6bef61b9cc22f534):
  the host tools, reference models and harnesses removed on 2026-10-03; the
  dated evidence refers to them.
- [`9b41c7f`](https://github.com/pinax-network/substreams-evm-extended/tree/9b41c7f/erc20/balances):
  the `erc20/balances` layout path with its tests, fixtures and qualification
  evidence, removed on 2026-10-05.
- [`cb62110`](https://github.com/pinax-network/substreams-evm-extended/tree/cb6211007f1f9d0cb5852a6b666da475a8fa6542):
  the per-package CLI native-sink ClickHouse paths, replaced by `evm-balances`
  on 2026-10-05.

## Open work

Open work is tracked in
[GitHub issues](https://github.com/pinax-network/substreams-evm-extended/issues),
under the roadmap
[#21](https://github.com/pinax-network/substreams-evm-extended/issues/21). On
the path to production:

- [#124](https://github.com/pinax-network/substreams-evm-extended/issues/124):
  qualify the balance packages on BSC, then cut Token API over to
  `evm-balances`. Every live step needs the owner's OK.
- [#116](https://github.com/pinax-network/substreams-evm-extended/issues/116):
  BSC producer drift under firehose-tracer 5.5.0. The erc20 read policy
  landed in PR #137; the infra facts are open.
- [#120](https://github.com/pinax-network/substreams-evm-extended/issues/120):
  the sink fixes are released in
  [substreams-sink-sql v4.13.0](https://github.com/pinax-network/substreams-sink-sql/releases/tag/v4.13.0)
  (commit `933a187`), the sink `evm-balances` supports
  ([README](../evm-balances/README.md#build-and-deploy)).
- [#7](https://github.com/pinax-network/substreams-evm-extended/issues/7):
  the [cutover contract](initialization-and-completeness.md) (RPC-era
  backfill, stale pairs, replay duplicates, sink completeness) and its checks
  are written; the cold-start decision (#8) and the live checks (#124 B2)
  remain.
- [#122](https://github.com/pinax-network/substreams-evm-extended/issues/122):
  the release flow (PR #140).
  [`native-balances-v0.2.1`](https://github.com/pinax-network/substreams-evm-extended/releases/tag/native-balances-v0.2.1)
  and
  [`erc20-balances-v0.4.0`](https://github.com/pinax-network/substreams-evm-extended/releases/tag/erc20-balances-v0.4.0)
  are released (2026-10-06); `evm-balances-v0.6.0` follows the #120 sink
  checks. Releases are not live qualification.
- [#123](https://github.com/pinax-network/substreams-evm-extended/issues/123):
  how Token API treats wrong computed balances and stale RPC-era pairs.
- [#8](https://github.com/pinax-network/substreams-evm-extended/issues/8):
  the networks epic, with one issue per network.

Upstream proposals for a deterministic `db_out` composite-key order:
[anthropics/buffa#574](https://github.com/anthropics/buffa/issues/574) and
[streamingfast/substreams-sink-database-changes#14](https://github.com/streamingfast/substreams-sink-database-changes/issues/14).
