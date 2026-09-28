# Compound v2 checked-arithmetic source oracle

The host model rejects overflow at each Solidity uint256 addition or
multiplication, before a later subtraction or division can make the final
number fit. It retains zero-supply, zero-borrow and revision-specific
same-block branches. A checked-math error is unknown output, never a zero.

These five checked-in hex runtimes are local test harnesses compiled from
three pinned Compound source trees. They are not deployed runtime captures.

| Source tree | Compiler | Exercised code |
| --- | --- | --- |
| `compound-finance/compound-protocol@a3214f67b73310d547e00fc578e8355911c9d376` | `0.8.10+commit.fc410830` | `CToken`, `ExponentialNoError`, `BaseJumpRateModelV2` |
| `compound-finance/compound-protocol@f385d71983ae5c5799faae9b2dfea43e5cf75262` | `0.5.8+commit.23d335f2` | `CToken`, `Exponential`, `CarefulMath`, `WhitePaperInterestRateModel`, `ErrorReporter` |
| `compound-finance/compound-protocol@4caf72a1f88335adc9cc06acf6f372241369ed01` | `0.5.17+commit.d19bba13` | `LegacyJumpRateModelV2`, `BaseJumpRateModelV2`, `SafeMath` |

The Rust generator extracts the entire pinned `exchangeRateStoredInternal`
and `accrueInterest` function bodies. It removes current-source override
modifiers and substitutes only the external interest-rate invocation with
a controlled rate (or legacy error/rate tuple). Cash, market storage and
the evaluation block are harness inputs. Original arithmetic, guards,
error handling, writes and events remain intact. The underlying scalar
wrapper calls the original inherited `mul_ScalarTruncate` or
`mulScalarTruncate`. Rate-model harnesses inherit their original functions
and expose the internal parameter setter without its owner wrapper.

Five Rust source-execution tests make **2,676 calls**: 2,100 utilization,
borrow and supply comparisons; 10 parameter-scaling comparisons; 280
WhitePaper comparisons; 262 exchange/scalar comparisons; and 24 controlled
accrual comparisons. Eight focused regressions cover the original bugs.
The controlled accrual stage deliberately isolates returned-rate arithmetic
from the separately tested rate models: for example, MAX borrows plus
positive interest fails its addition, although the supported rate models
would reject MAX borrows earlier while scaling utilization. The public
u64 block delta and capped rate cannot overflow their own product.

Tests distinguish successful return values, legacy error codes with unchanged
market state, modern panic/Error reverts, and the legacy INVALID assertion
for a backwards block. Unsupported opcodes, missing storage and invalid jumps
fail the test harness. The shared bounded interpreter now supports isolated
storage writes, event stack consumption, CODECOPY and legacy INVALID, with
direct sanity checks. Events are consumed for execution; their contents are
not an event-conformance claim. No constructor is executed.

[Provenance](provenance.txt) records all source URLs and SHA-256 digests,
generated harnesses, compiler hashes, selectors and runtime artifacts from
`out/compound-checked-20260928/source-04/`. All three compilers are official
macOS amd64 binaries from `ethereum/solc-bin` at
`16a99b8c26ed33a91796e209ff6797ee7baf2b0d`:

| Compiler | SHA-256 |
| --- | --- |
| [0.8.10](https://raw.githubusercontent.com/ethereum/solc-bin/16a99b8c26ed33a91796e209ff6797ee7baf2b0d/macosx-amd64/solc-macosx-amd64-v0.8.10+commit.fc410830) | `a79fff23aeb35be856e446827c44a9cfa4c382f29babd2f6a405ef73d1e2a4cc` |
| [0.5.8](https://raw.githubusercontent.com/ethereum/solc-bin/16a99b8c26ed33a91796e209ff6797ee7baf2b0d/macosx-amd64/solc-macosx-amd64-v0.5.8+commit.23d335f2) | `9f383896c8506cd96ac5ef9c97d7ab1c385fb5f4772a5f3d2cf8715cae793554` |
| [0.5.17](https://raw.githubusercontent.com/ethereum/solc-bin/16a99b8c26ed33a91796e209ff6797ee7baf2b0d/macosx-amd64/solc-macosx-amd64-v0.5.17+commit.d19bba13) | `35be28f68488b71f1de66148f915b91d64e062900e4fc175bf2eb8fb948810bd` |

All builds enable the optimizer and target `petersburg`; current metadata
hashing is disabled, while legacy compiler metadata is retained. The earlier
native 0.8.10 build in `source-03/` produced identical current runtime bytes;
the official compiler regeneration is the final provenance. Earlier red
tests and build/interpreter failures remain under the same output parent.

Regenerate using local compiler paths and a fresh output directory:

```sh
rustc --edition 2021 conformance/tools/build_compound_oracle.rs -o /tmp/build-compound-oracle
/tmp/build-compound-oracle /path/to/solc-0.8.10 /path/to/solc-0.5.8 /path/to/solc-0.5.17 out/compound-oracle-FRESH
cargo test --offline --locked -p conformance compound_v2
```

The generator only downloads pinned GitHub sources. Normal tests use only
committed bytecode and existing host dependencies. No runtime dependency,
production mapper, layout or package changes. Ethereum deployment binding,
historical getters, initialized holders and package qualification remain
separate gates; issues #14 and #16 are not closed by this arithmetic evidence.
