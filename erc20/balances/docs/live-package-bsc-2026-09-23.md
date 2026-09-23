# Current package on live BSC (2026-09-23)

The first build of the renamed package, `spkg/erc20-balances-v0.1.0.spkg`,
was run on live BSC final blocks **123,561,000–123,562,023** (1,024 blocks)
under #6. The mapper source is unchanged since the offline typed-path and
enumerable-set work; only host tools were added for this run. Historical
packages and their evidence are unchanged, and none of their checks apply to
these bytes.

## Package identity

| Artifact | SHA-256 / hash |
| --- | --- |
| Source commit | `0004720c85fa0d1345f3c5d259dcb59368a7b95e` (mapper unchanged by this PR) |
| Release WASM `erc20_balances.wasm` | `005a2d3d22d10c5c61fa36b00adea1c0b5e9c55536a553c14b9f423d09546099` |
| `spkg/erc20-balances-v0.1.0.spkg` | `532b571f03f66931b4f1dd222ec8ff4da64d0054c81f1b4ce7213a41abc9f0cd` |
| `map_events` module hash | `4a64d86d11ebe2e92430d414d4b7ba484849b5d1` |
| Embedded `evm.balances.v1` descriptor | `1b5d5f374489f084a21374a896a83d5907a415e973635b8f609a20dd9b8e1fda` |

`erc20-balances-tools inspect-package` decodes the SPKG
([report](evidence/live-2026-09-23/package.json)): exactly one module,
`map_events`, a map with inputs `params: []` and
`source: sf.ethereum.type.v2.Block`, output `proto:evm.balances.v1.Events`.
Its one WASM binary equals the release build and imports only
`logger.println`, `env.output`, `env.register_panic` and
`env.skip_empty_output`: no RPC host function. The canonical reference's
binaries import `rpc.eth_call`. The embedded `evm.balances.v1` file
descriptor is byte-identical to the one in the canonical RPC reference
`erc20-balances-v0.3.4.spkg` (`8aaa03b5…`).

## Which profiles ran

The input was the 431 qualified profiles in
[`bsc-refined450-layouts.json`](../tests/fixtures/bsc-refined450-layouts.json).
Two independent gates reduced them to **425**, each exclusion named with its
reason. No profile was changed or loosened.

**Runtime bindings** (`runtime-status`, [report](evidence/live-2026-09-23/runtime-status.json)):
each profile's runtime, proxy and dependency bindings were rechecked before
the range (123,560,999) and at its last block (123,562,023). 428 still match.

| Token | Label | Reason |
| --- | --- | --- |
| `0x7c8d5502b544ddaf8852fc46d1174e34876d545c` | BNC4 | beacon implementation slot changed |
| `0xdfe1308fb3ef1dc2f87b4ffaef5ebdf80be1e4ea` | sPro | balance divisor value changed |
| `0x009b797edf9acc666a36020c4509c6095de51408` | swkeyDAO2 | balance divisor value changed |

The two divisor profiles pin the divisor's value
([sPro](ranks101-150-coverage.md), [swkeyDAO2](ranks201-250-coverage.md)); a
changed divisor moves every balance without a storage write per holder, so
these profiles are only valid while it holds.

**Unreviewed writes** (`refusal-scan`, [report](evidence/live-2026-09-23/refusal-scan.json)):
the 1,024 Extended blocks were fetched from `bsc.firehose.pinax.network` and
replayed through the native mapper. The package halts the whole stream on
its first refusal, and the first packaged run did exactly that: with the 428
profiles it stopped at block 123,561,001
([failed attempt](evidence/live-2026-09-23/compare-attempt1-refused.json)).
The scan projects each profile alone to attribute a refusal, and aborts
instead if the block is refused with no profile configured. Three profiles
refused, all with `unresolved storage … refusing incomplete events`:

| Token | Label | First block | Refused slot | Origin in the block |
| --- | --- | ---: | --- | --- |
| `0xe747e54783ba3f77a8e5251a3cba19ebe9c0e197` | TAKE | 123,561,001 | `0x9b779b17…becc55f00` | no preimage; the OpenZeppelin 5 `ReentrancyGuard` ERC-7201 slot, written 1→2→1 in one call |
| `0xf08d1886e2a69dfabd22a45eed8e1407b338b97e` | RADR | 123,561,119 | `0x9ab217ff…cb512112d` | mapping base slot 21, key `0x1c0ebd`, written 0→1 |
| `0xcdf52c0b13c24f32f1d8d4ec6356203a1ef0826a` | TOPS | 123,561,227 | `0xe7e16eda…caeb6441` | no preimage in the block; written from `0x40942f68a8ba0ef0` to 0 |

These are real production findings: with all 431 profiles, the current
package cannot pass block 123,561,001. Each needs its own source review
before any slot is added; nothing here reviews them.

## Results

All runs use the 425-profile file and the same 1,024 blocks. Every capture
binds all delivered clocks to consecutive canonical RPC headers, including
blocks without output.

| Check | Report | Result |
| --- | --- | --- |
| Native replay equals the package | [recount](evidence/live-2026-09-23/refusal-scan-kept.json) | 253,503 rows, 0 refusals, the same count the package delivered |
| Package vs canonical RPC reference | [compare](evidence/live-2026-09-23/compare.json) | 253,503 package rows, every one also a reference row at the same block; **0 value differences** in 33,928,529 snapshot comparisons; 0 package-only keys or updates; 20/20 independent `balanceOf` samples at the last block |
| Every emitted balance | [audit](evidence/live-2026-09-23/audit.json) | `rpc_parity`: all **253,503** emitted balances equal `balanceOf` at their block hash (`requireCanonical`), 0 mismatches, 41,982 zeros |
| Initialized holders | [holders](evidence/live-2026-09-23/holders.json) | 88,534 observed holders checkpointed at 123,560,999 (177,068 reads, each storage projection equal to `balanceOf`); all 376,418 configured reference rows equal the seeded state, 0 unknown, 0 mismatch |
| Final state | [holders](evidence/live-2026-09-23/holders.json) | all 88,534 holders equal `balanceOf` at 123,562,023 (`0x3dc58d8b…`), 36,259 of them zero |
| Determinism | compare and audit captures | three separate package streams (the comparison, the failed first audit and the audit rerun) produced byte-identical normalized output (`79ccea19…`); both reference streams likewise (`0b9c4949…`) |
| Native ClickHouse sink | [smoke](../clickhouse/README.md#current-package-2026-09-23) | `native_clickhouse_parity` on the fixed 144-block sample: resume and identical replay deduplicate |

The reference emits 538,758 rows for 3,428 tokens; the comparison therefore
reports `coverage_gap`. Its 285,255 reference-only rows split into 162,340
rows for 3,104 tokens that are not configured, and 122,915 rows for configured
tokens where no holder storage changed at that block (the holder report counts
all 376,418 configured reference rows, 253,503 of them emitted). Each of those
configured rows equals the checkpoint-seeded state. Without the checkpoint,
113,498 of the 376,418 configured rows would be unknown, 38,997 of them
nonzero; the cold state never assumes zero and has no value mismatch. Of the 425 profiles, 324 have
reference rows in the window and 320 emit; the holder report lists the 101
profiles unobserved in this interval, which this run does not test.

The first audit attempt stopped with an RPC transport failure after 60 blocks
and 12,034 matching checks ([failed attempt](evidence/live-2026-09-23/audit-attempt1-transport.json)).
The tools' HTTP client now retries a transport failure or a 429/502/503/504
three times before failing; other statuses still fail at once.

## Tools added

- `inspect-package`: modules, inputs, output, WASM host imports and the
  embedded schema, compared with the canonical reference.
- `runtime-status`: splits a layout file by whether each profile's runtime
  and dependency bindings still hold for a later range. A transport failure
  aborts; a mismatch must repeat on a second check before exclusion.
- `refusal-scan`: native replay over captured Extended blocks, attributing
  each refusal to a profile and recording the refused slot's writes and
  preimage chain.
- `holder-coverage` now also checks every known holder's final state against
  `balanceOf` at the last replayed block.

## Scope and limits

The interval is 123,561,000–123,562,023 on BSC, the configured set is the
425 profiles in the kept layout file (`f8f0fa8f…`), and holder claims cover
only holders observed in that interval. Matching samples do not establish
universal token support or a global holder set. The six excluded profiles
are neither requalified nor removed from the historical fixture.
