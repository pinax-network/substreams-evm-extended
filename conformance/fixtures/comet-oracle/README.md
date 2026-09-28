# Comet compiled-source arithmetic oracle

This local runtime is compiled from Compound Comet source pinned at
[`f766f51583c23acc33b2a7824654ef2029a96804`](https://github.com/compound-finance/comet/tree/f766f51583c23acc33b2a7824654ef2029a96804).
The original [license and notice](LICENSE) accompany the derivative harness.
It is a host test harness, not a deployed runtime capture. Normal Rust tests
use the committed bytecode without a compiler, network access or chain calls.

The [generator](../../tools/build_comet_oracle.rs) inherits unchanged
`CometCore`, `CometStorage`, `CometMath` and `CometConfiguration`. It extracts
the entire pinned `getNowInternal`, `accruedInterestIndices`, `getSupplyRate`,
`getBorrowRate`, `getUtilization`, `mulFactor`, `balanceOf` and `borrowBalanceOf`
functions. Only `override` modifiers and the two
`userBasic[account].principal` reads are replaced; both reads use one explicit
`int104 boundPrincipal` input. Eight immutable rate fields become explicit
storage inputs with their original names and `uint` types. Test inputs are
limited to the model's uint64 rate domain. No constructor, immutable patching,
mapping lookup, collateral action or dependency call is executed.

The [generated harness](Oracle.sol) retains original market packing:
supply/borrow indices are uint64 in slot 0; supply/borrow totals are uint104
and last-accrual time is uint40 in slot 1. Original unused storage slots 2–7
remain present; rates occupy slots 8–15 and principal is in slot 16.
Tests check these assignments against the compiler's
[layout](storage-layout.json), check ABI [selectors](oracle.signatures), and
reject out-of-domain inputs rather than truncate them. The `indices()`
wrapper uses the same `getNowInternal() - lastAccrualTime` expression as the
getters. Conversion wrappers invoke the unchanged inherited functions.

Four differential tests make **1,515 model comparisons**:

- 96 supply/borrow rate comparisons below, at and above both kinks, with
  rounding and uint64/uint256 refusal boundaries.
- 1,292 stored-utilization, accrued-pair, supplied-balance and debt
  comparisons over six boundary markets and 32 deterministic generated
  markets, three clocks and five signed principal values including both
  int104 extremes. Stored zero indices and zero supply are explicit values.
- 37 source-order controls: exact uint40 timestamp, backwards-clock,
  supply-index and borrow-index failures before all principal-sign branches;
  same-clock evaluation at `2^40 - 1` skips rates and index growth.
- 90 inherited present/principal conversion comparisons, including rounding,
  zero divisors and uint104/uint256 boundaries. Two additional ABI calls
  verify that excess uint64/uint104 argument widths are rejected.

The comparisons require identical successful integer outputs or recognized
full custom-error/panic payloads. Rust exposes some failures more coarsely:
an out-of-range rate/principal can correspond to source checked intermediate
overflow or its later safe cast. These are both refusals, never successful
zero. Focused source-order cases assert the exact payload independently.
Malformed ABI output, missing storage, invalid jumps, unsupported opcodes and
resource exhaustion fail the harness rather than count as Solidity refusal.

The shared bounded VM retains its old zero-clock entrypoint for OZ and v2.
Comet supplies an explicit TIMESTAMP. SIGNEXTEND and SGT are the only other
added opcodes, with direct positive/negative/sign-boundary tests alongside
timestamp and unsupported-opcode controls. No general EVM support is claimed.

## Compiler and source provenance

[Provenance](provenance.txt) records all six source URLs and SHA-256 hashes,
the as-run Rust generator, generated harness, runtime, selectors, layout,
compiler arguments and official compiler manifest. The final capture is
`out/comet-oracle-20260928/source-03/`; `source-01/` and `source-02/` remain
preserved and produced identical runtime bytes. All builds target `petersburg`, enable
the optimizer, and disable metadata hashing. Runtime hex SHA-256 is
`09156302621c78548f41c63956c13bc2347a192163dce788c21221bf9ddc1e9a`.

The compiler is official macOS amd64 Solidity
`0.8.15+commit.e14f2714`, obtained from
[`ethereum/solc-bin@16a99b8c26ed33a91796e209ff6797ee7baf2b0d`](https://raw.githubusercontent.com/ethereum/solc-bin/16a99b8c26ed33a91796e209ff6797ee7baf2b0d/macosx-amd64/solc-macosx-amd64-v0.8.15+commit.e14f2714).
Its SHA-256 is
`00656dc73224e4c0702940df10310bdc024b60f4a7598e774d305bc3b94f7d79`;
the generator requires this hash and checks the exact release entry in the
pinned official manifest. The older storage-layout report's native compiler
hash is preserved as historical evidence, not reused as this build's identity.

Regenerate with the repository's pinned Rust toolchain and a fresh directory:

```sh
rustc --edition 2021 conformance/tools/build_comet_oracle.rs -o /tmp/build-comet-oracle
/tmp/build-comet-oracle /path/to/official-solc-0.8.15 out/comet-oracle-FRESH
cmp out/comet-oracle-FRESH/build/Oracle.bin-runtime conformance/fixtures/comet-oracle/oracle.bin-runtime
cargo test --offline --locked -p conformance comet::source_oracle
```

This bounded arithmetic evidence does not establish constructor configuration,
deployment reachability, runtime/proxy/immutable binding, an Ethereum getter
capture, initialized observed holders or package qualification. It does not
close #15 or #16. Retained projector integration remains separately labeled
synthetic evidence; no live hold is lifted by these tests.
