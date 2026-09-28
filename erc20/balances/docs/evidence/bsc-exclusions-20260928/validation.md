# Offline validation

Final workspace-validation base: `cd2a245e522624625bf17ca747d1448b6cd2de14`.
The final replay was recorded immediately before that merge on
`d78eb42441d5972e19cfbae1a50349cd82dd9ede`. The merge leaves the recorded
ERC20 projector/diagnostic hashes and retention's `Balances` path unchanged;
its retention change concerns `BalanceState` epoch resumption.
All final Cargo commands used the fresh, worktree-local target directory
`target/issue61-isolated-20260928`; earlier shared-target results are invalidated
in [attempts.md](attempts.md). Logs remain under ignored
`out/bsc-exclusions-20260928/`.

| Command | Result | Log and SHA-256 |
| --- | --- | --- |
| `cargo fmt --all -- --check` | Passed | No output |
| `cargo test --locked --offline --workspace --lib --bins --tests` | **735 passed**, zero failed, 39 suites | `workspace-tests-current-main.log`: `a68d57e326806fe7644518e820f3fc1ccb5a42e39c66f11cf5cad9f0b990e190` |
| `cargo clippy --locked --offline --workspace --all-targets -- -D warnings` | Passed | `clippy-current-main.log`: `35a7ec26b25f8da070cd8dd15570078267e139586e71aed5b0196ed8f9ee85b4` |
| `cargo check --locked --offline --workspace --target wasm32-unknown-unknown` | Passed | `wasm-check-current-main.log`: `307038621d86462ae8b96fd01a7fb11534577dfa2b8be999942df3eb6c6efb47` |
| `git diff --check` | Passed | No output |
| `cargo run --locked --offline -p erc20-balances-tools --bin review_bsc_exclusions -- <original erc20/balances> out/bsc-exclusions-20260928/run-6` | 1,024 blocks; 8,436 exact same-block saved-reference matches; no refusals or missing comparisons | `replay-isolated-final.log`: `b9e47c1715f3c16552869c828978f9f8ea6a750dab61473dbf4d758f074e49fd` |

The test count sums each anchored `test result: ok. N passed` line in the single
isolated workspace log, not prior targeted runs. Relative to the 723-test base,
this adds nine candidate/width integration regressions and three host diagnostic
unit tests. Source hashes of the tested implementation:

```text
erc20/balances/src/lib.rs
fbb985f16c159800f15e394fdb2be949848ab80b37c43b7c7aa56f9dbfc6ee0e
erc20/balances/tests/bsc_exclusion_candidates.rs
4441484f789e516b196633f47a57f1b019ac43cc046e4b3d65fab3b9c24e2649
erc20/balances/tools/src/bin/review_bsc_exclusions.rs
4db46cd80205ad4a7bb88a5341f16077716e5ac895a1f6fff458225bff26f892
```

The separately recorded [run-6 replay report](report.json) binds the same
implementation and all source/candidate/capture inputs. Neither host tests,
Clippy, a WASM compile check, nor saved-data replay qualifies a newly packaged
WASM artifact or closes the remaining live gates.

Final `report.json` SHA-256:
`a002971c733999bb2c5152a17d455c15774051a4955d32896335b3df9e928c5f`.
