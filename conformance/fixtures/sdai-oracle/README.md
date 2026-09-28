# SavingsDai compiled-source arithmetic oracle

This host fixture executes selected functions from
[`sky-ecosystem/sdai@665879762f8b5df5d234463f45d1d6a49bd4fbeb`](https://github.com/sky-ecosystem/sdai/tree/665879762f8b5df5d234463f45d1d6a49bd4fbeb),
compiled with official Solidity 0.8.17. It is a controlled local harness,
not deployed runtime evidence. The original [AGPL license](LICENSE) and
source notices accompany the [derivative harness](Oracle.sol).

The [Rust generator](../../tools/build_sdai_oracle.rs) extracts complete
`_rpow`, `_divup`, `convertToAssets`, `convertToShares`, `previewDeposit`,
`previewMint`, `previewWithdraw`, `previewRedeem` and `maxWithdraw` functions,
plus the original `RAY` constant. Its only substitutions are four `pot.rho()`
calls, four `pot.dsr()` calls, eight `pot.chi()` calls and one
`balanceOf[owner]` read. Controlled uint256 inputs occupy four sequential
slots: chi, rho, dsr and holder shares. `block.timestamp`, arithmetic,
assembly, checked products, the unchecked division helper and branch order
stay intact. Two wrappers expose the original internal arithmetic helpers.
No constructor, Pot call, holder mapping, mutation or permit executes.

## Compared inputs and outcomes

Four differential tests make **4,155 source calls**:

- 154 `_rpow` comparisons cover zero, below/equal/above RAY, odd/even/zero/one
  exponents, u64 maximum exponent, rounding to zero and multiplication limits.
- 36 `_divup` comparisons cover zero numerator, zero denominator, rounded
  quotients and uint256 maximum words.
- 3,920 conversion/preview/max-withdraw comparisons combine eight explicit
  and eight deterministic generated Pot states, five clocks and seven
  amounts. Explicit states include stored/projected zero chi, extreme dsr,
  rho `2^64` and rho `2^256 - 1`.
- 45 exact source-order controls distinguish empty assembly reverts from
  `Panic(0x11)` multiplication overflow and `Panic(0x12)` division by zero.
  Projection executes before a zero amount; scaling precedes division;
  `_divup(0,0)` returns zero; before/equal rho bypasses projection; exponent
  one bypasses squaring.

Successful source return words must equal Rust exactly. Rust groups assembly
and checked-product failures under `uint256 overflow`; general comparisons
accept only those corresponding source payloads. Focused ordering cases
independently assert the precise payload. Call construction requires a
recognized selector, complete arguments and uint256 words, so invalid
selectors or truncated calldata cannot count as arithmetic refusal.

Two further tests bind [selectors](oracle.signatures) and
[input layout](storage-layout.json) and check domains. The shared bounded VM
adds `OracleExit` to distinguish RETURN, REVERT and INVALID. Its strict
modern entrypoint accepts empty REVERT but treats INVALID as a harness
failure; old entrypoints preserve legacy behavior. Direct controls include
empty successful returns and unsupported instructions. No new opcode or
dependency is needed. Missing storage, malformed output and resource
exhaustion also fail the harness rather than count as source refusal.

Compared scalar and amount inputs fit uint256. Evaluation clocks and direct
`_rpow` exponents fit the host API's u64 domain. This is not parity for
arbitrary-width BigUint inputs or Solidity exponents above u64. Source Pot
rho is uint256: the host `SavingsDai.rho` is now BigUint, validated before
either branch. A positive elapsed duration still fits u64 because it cannot
exceed the u64 evaluation clock. A stored rho above that clock selects stored
chi without narrowing or subtraction.

The separate [retained regression](../../tests/retained.rs) initializes one
synthetic observed holder and explicit Pot facts at block 10, advances an
idle clock at block 11 and undoes the block. Rho `2^64` remains exact in
consumed provenance and evaluates stored chi; absent rho stays unknown.
Checkpoint ingestion rejects rho above uint256. These are synthetic boundary
values, not claims that they occur in a qualified Pot deployment.

## Provenance and reproduction

[Provenance](provenance.txt) records upstream source/license, compiler and
official manifest, as-run generator, generated harness, runtime, selectors
and layout hashes. The final capture is
`out/sdai-oracle-20260928/source-01/`. Runtime hex SHA-256:
`d13c9dd21d7f69e2efa674a5678238549e35fb4a802f6694636ec5d4c55e3155`.
The build enables optimization, targets `petersburg`, and disables metadata
hashing. Tests use committed bytecode without compiler or network calls.

Official macOS amd64 compiler `0.8.17+commit.8df45f5f` comes from
[`ethereum/solc-bin@16a99b8c26ed33a91796e209ff6797ee7baf2b0d`](https://raw.githubusercontent.com/ethereum/solc-bin/16a99b8c26ed33a91796e209ff6797ee7baf2b0d/macosx-amd64/solc-macosx-amd64-v0.8.17+commit.8df45f5f),
SHA-256 `e40eef83c24d4c42b47f461b01748a6ca89f1e09e778995b71debfa0de99e12a`.
The generator verifies the exact release entry in the pinned manifest. The
older storage-layout report's different native compiler hash is preserved
as historical evidence, not reused as this build's identity.

```sh
rustc --edition 2021 conformance/tools/build_sdai_oracle.rs -o /tmp/build-sdai-oracle
/tmp/build-sdai-oracle /path/to/official-solc-0.8.17 out/sdai-oracle-FRESH
cmp out/sdai-oracle-FRESH/build/Oracle.bin-runtime conformance/fixtures/sdai-oracle/oracle.bin-runtime
cargo test --offline --locked -p conformance sdai
```

The pre-fix high-rho rejection remains in
`out/sdai-oracle-20260928/validation/high-rho-red.log`. An initial test API
compile error and an incorrect oversized-fixture expectation are preserved
separately; the corrected control asserts the earlier ingestion rejection.
No protobuf or production projection changes. Constructors, dependencies,
deployed runtime, chain intervals, initialized global holders, Ethereum
getters and package qualification remain separate #7/#16 gates. This
arithmetic evidence does not authorize live checks.
