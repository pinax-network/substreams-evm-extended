---
name: storage-layout-verification
description: How to upgrade a balance-state fixture's inferred storage slots to compiler-verified layouts offline with solc --storage-layout (or an AST walk for solc 0.4.x), store the layout as evidence and pin fixture slots to it with a Rust test.
---

# Storage-layout verification with solc

Executed once on 2026-09-21 (see `docs/evidence/storage-layouts/README.md`);
follow these steps again when a pin changes. Practical notes from that run:
`solc-select` via `pipx run solc-select install <versions>` fetches the
macOS binaries (Rosetta on arm64); for OpenZeppelin use the repo remappings
but NOT `--include-path lib/...` (ambiguous imports); static-a-token-v3 needs
`solc 0.8.20` because `ECDSA.sol` requires `^0.8.20`; the 0.4.24 compact AST
prints one `======= path =======` block per unit, split on that; Lido's
`--allow-paths` must include the repository itself.

Prerequisite reading: `docs/storage-layout-provenance.md` (per-contract table,
pins, and what "verified" cannot mean without live access).

## Steps

1. Scratch directory per contract (outside the repo). Clone the pinned repo
   and check out the exact commit; `git submodule update --init --recursive`
   when the build uses `lib/` submodules (foundry projects).
2. Compiler: read the pragma and the project config (`foundry.toml`,
   `hardhat.config.*`). If the installed `solc --version` does not satisfy
   it, download the matching release from `https://binaries.soliditylang.org/`
   (`macosx-amd64` binaries run under Rosetta on Apple silicon) or use
   `solc-select`; record version and sha256 in the evidence file.
3. Compile the concrete contract with the project's remappings:
   ```sh
   solc --storage-layout --base-path . --include-path lib --include-path node_modules \
        @openzeppelin/=lib/openzeppelin-contracts/ ... path/To/Contract.sol > layout.json
   ```
   Abstract contracts print no layout: add a one-line harness
   `contract H is Target {}` in the scratch directory and compile that.
4. solc 0.4.x (Lido) has no `--storage-layout`: run `--ast-compact-json`, walk
   the `linearizedBaseContracts` from the most base to the most derived, count
   non-constant state variables with Solidity packing rules, and write the
   derived layout in the same JSON shape with `"method": "ast-derived"`.
5. Save the artifact under `docs/evidence/storage-layouts/<Contract>@<commit>.json`
   (raw solc output plus a header with repo, commit, solc version, sha256,
   command line, date).
6. Add a host-only Rust test in the package (`tests/layout.rs`, `#![cfg(not(target_arch = "wasm32"))]`)
   that parses the fixture and the layout JSON and asserts every fixture slot,
   mapping base and bit range (`offset*8`, width from type) against it.
7. Update the provenance table level to **compiler-verified** and the package
   README; leave code hash and activation block as live-gated.

## Contract list (from the provenance page)

Comet `CometWithExtendedAssetList` @ `f766f515…`; Compound `CErc20Delegator`,
`CErc20Immutable`/`CErc20`, `CEther`, `JumpRateModelV2` @ `a3214f67…`;
`StaticATokenLM` @ `101f5d97…` (submodules); `SavingsDai` @ `66587976…`;
`Pot` (makerdao/dss); OpenZeppelin `ERC4626Upgradeable` v5.0.0 via a harness;
Lido `Lido.sol` @ `2da0f48f…` with `@aragon/os@4.4.0` (AST method).
