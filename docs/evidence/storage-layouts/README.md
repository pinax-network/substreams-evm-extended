# Compiler storage layouts of the bound contracts

Raw `solc --combined-json storage-layout` output for every pinned source the
balance-state fixtures bind, produced offline on 2026-09-21 with the exact
compiler each pragma requires (see the `solc` field of each file; binaries
from binaries.soliditylang.org via solc-select). Each file records repository,
commit, compiler version and sha256, and the command line; contracts without
regular storage are kept with empty tables. Lido (`solc 0.4.24`, no
`--storage-layout`) is derived from the compact AST and says so in `method`.

| File | Contract(s) | Pinned to | Checked by |
| --- | --- | --- | --- |
| `comet@f766f515.json` | `CometWithExtendedAssetList` (via `CometStorage`) | compound-finance/comet `f766f515…` | `compound-v3/balance-state/tests/storage_layout.rs` |
| `compound-v2@a3214f67.json` | `CErc20`, `CErc20Immutable`, `CErc20Delegator`, `CEther`, `JumpRateModelV2` | compound-finance/compound-protocol `a3214f67…` | `compound-v2/balance-state/tests/storage_layout.rs` |
| `sdai@66587976.json` | `SavingsDai` | sky-ecosystem/sdai `66587976…` | `erc4626/balance-state/tests/storage_layout.rs` |
| `dss-pot@….json` | `Pot` | makerdao/dss master at capture | same |
| `static-a-token-v3@101f5d97.json` | `StaticATokenLM` | bgd-labs/static-a-token-v3 `101f5d97…` (solc 0.8.20 because `ECDSA.sol` requires `^0.8.20`) | same |
| `openzeppelin-upgradeable@v5.0.0.json` | `ERC4626Harness is ERC4626Upgradeable` + the ERC-7201 namespace constants | OpenZeppelin/openzeppelin-contracts-upgradeable v5.0.0 | same |
| `lido-core@2da0f48f.json` | `Lido` (AST-derived) | lidofinance/core `2da0f48f…` with `@aragon/os@4.4.0`, `openzeppelin-solidity@2.0.0` | `lido/balance-state/tests/storage_layout.rs` |
| `fiat-token@v2.2.0.json` | `FiatTokenV2_2` (USDC implementation family; `balanceAndBlacklistStates` at slot 9, blacklist flag in bit 255) | circlefin/stablecoin-evm v2.2.0 (`405efc10…`) with `@openzeppelin/contracts` 3.4.2 | `compound-v2/balance-state/tests/storage_layout.rs` |

What a matching layout proves: the fixture's slot numbers and bit ranges are
those the pinned source compiles to. What it does not prove: that the bytecode
deployed at the fixture addresses was built from that source (code-hash
binding) or the block at which an epoch became active; those stay live-gated
(`docs/storage-layout-provenance.md`).
