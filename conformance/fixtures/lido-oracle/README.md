# Lido v4 source-execution oracle

This is an independent arithmetic oracle for the selected stETH model, not
deployed getter or package qualification. The Rust tests execute committed
Solidity 0.4.24 runtime bytes in the bounded host interpreter; no compiler,
network or chain service runs during tests.

## Sources and compilation

- Lido `StETH.sol`, `Lido.sol` and `UnstructuredStorageExt.sol` at
  [`lidofinance/core@2da0f48f1a2a103a394dcf8760810fe9165697fb`](https://github.com/lidofinance/core/tree/2da0f48f1a2a103a394dcf8760810fe9165697fb).
- Complete `SafeMath.sol` and `UnstructuredStorage.sol` at
  [`aragon/aragonOS@f3ae59b00f73984e562df00129c925339cd069ff`](https://github.com/aragon/aragonOS/tree/f3ae59b00f73984e562df00129c925339cd069ff),
  the dependency of the existing Lido AST evidence.
- Official `solc 0.4.24+commit.e67f0147`, macOS amd64, SHA-256
  `7034c4048bc713d5c14cdd6681953c736e2adbdb9174f8bfbfb6a097109ffaaa`.
  The generator verifies it against the immutable
  [solc-bin manifest](https://raw.githubusercontent.com/ethereum/solc-bin/16a99b8c26ed33a91796e209ff6797ee7baf2b0d/macosx-amd64/list.json).
- Exact compiler arguments: `--bin-runtime --hashes --optimize --optimize-runs 200
  --evm-version byzantium -o build LidoOracle.sol`.

[provenance.txt](provenance.txt) records all primary URLs, original source and
license hashes, the as-run Rust generator hash, compiler hash/version/arguments,
every substitution, and harness/runtime/selector hashes. Full original sources,
compiler stdout/stderr, manifest, generated inputs and earlier attempts remain
under `out/lido-oracle-20260928/`. The two dependency sources and license texts
are retained here; [LidoOracle.sol](LidoOracle.sol) is the generated harness.

The generator extracts **12 complete, unchanged function bodies**: StETH's
`balanceOf`, `getPooledEthByShares`, `getSharesByPooledEth`,
`getTotalPooledEther`; Lido's `_getInternalEther`, `_getExternalEther`,
`_getTotalPooledEther`, both share-rate getters and three packed-word accessors.
It also extracts the unchanged `getLowAndHighUint128` function and its mask
into `PackedOracle`, using the original unstructured read implementation.
It does not restate the model's arithmetic as new Solidity formulas.

Harness substitutions are deliberately bounded:

| Source location | Controlled harness location |
| --- | --- |
| total/external shares hashed word | slot 0 |
| buffered/deposited ether hashed word | slot 1 |
| CL validators/pending balances hashed word | slot 2 |
| `_sharesOf(address)` mapping lookup | supplied holder value at slot 3 |

Only the three constants are relocated; the source's total-share alias is
unchanged. The holder accessor is a stub and ignores its address argument.
Three new public wrappers expose internal getters. Storage packing still
runs through the original read/mask/shift function. Tests reject values wider
than either packed uint128 lane or the uint256 holder before VM execution.
No constructor, Aragon routing, holder mapping, external call or state
transition is executed. The existing AST layout proof remains separate.

## Compared behavior and domain

The tests make **989 EVM executions**: 936 source getter calls across 72 pools
(including 64 deterministic generated full-width packed states), plus 53
counterexample, argument/domain, actual-projector and zero-denominator controls.
Two malformed harness inputs are rejected before execution and are not counted.
Returned ABI words are compared exactly; a missing input remains Unknown in
the host.

Typed VM outcomes distinguish returned data, `REVERT` payloads and `INVALID`.
The strict getter-argument and SafeMath failures must carry their exact
`Error(string)` payloads. Division by zero must execute `INVALID`, even when
the numerator is zero; an empty revert cannot satisfy that expectation.
A wrong-selector control produces a distinct empty revert. Unsupported
opcodes and VM resource failures remain hard test failures. Positive internal
shares with zero internal ether return zero for forward/total conversions,
while inverse conversion fails; positive ether with zero internal shares
has the opposite inverse/forward distinction.

Let `N=2^128`, `M=N-1`, buffered/deposited ether each `M`, other ether fields
zero. These states pass the model's individual field checks:

| Inputs | Pinned source result |
| --- | --- |
| total `M`, external `0`, holder `M-1` | holder balance `N-6` after wrapping; previous Rust gave `2N-4` |
| total `M`, external `M-2` | external ether `N²/2-4N+3`; multiplication wraps before division even though the unbounded final quotient fits uint256 |
| total `N/2+1`, external `N/2` | external product fits, but total getter reverts with `MATH_ADD_OVERFLOW` |

The source's internal sum cannot overflow uint256 for four valid uint128
fields, but can exceed uint128. Its forward/external products are plain
wrapping multiplication; the total addition uses SafeMath. The inverse
product fits uint256 when its argument is below `M` and external shares do
not exceed total shares. `M` itself is rejected as a getter argument.

The host conservatively rejects external shares above total shares, even
though the source's raw subtraction would wrap. Tests expose that difference
instead of broadening the supported domain. No aggregate `internalEther <= M`
invariant is assumed, and no counterexample is claimed reachable on-chain.

The mapper now emits spec revision 4. Three actual-projector cases compare
the optional derived total against compiled source and assert all six stored
inputs remain. The retained bridge also checks the wrapped holder result and
a valid holder balance when the total getter fails, including raw shares,
origin clocks, an idle block and undo. Runtime attestations in that test are
explicitly synthetic.

## Reproduction

From the repository root, with the official compiler available:

```sh
rustc --edition=2021 conformance/tools/build_lido_oracle.rs -o /tmp/build-lido-oracle
/tmp/build-lido-oracle /absolute/path/to/solc-0.4.24 out/lido-oracle-FRESH
cargo test --offline --locked -p conformance --lib lido::source_oracle
```

The generator fetches only immutable public source/compiler metadata URLs.
It refuses an existing output directory and preserves compiler diagnostics.
Fresh runtime/code-hash and dependency/epoch binding, initialized checkpoints,
same-block-hash getters and actual packaged output remain live-gated under
issues #16/#23. No historical evidence qualifies a newly built package.
