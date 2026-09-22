# Offline issue-audit remediation

This work implements the recommendations from the 17-open-issue audit of
`2ae1516f6493326915b069f0e49f1227481a6ded`. It preserves the one-map RPC-free
production boundary, canonical protobufs, historical fixtures/packages and
the hold on live Substreams, Firehose, RPC and sink qualification.

## Requirements and implementation evidence

| Recommendation | Required behavior | Evidence/status |
| --- | --- | --- |
| #7 atomic retention | A rejected balance/state block changes no entries, suspensions, clocks, counters or undo records; retry and undo preserve pre-error state. | Implemented in `common/retention`; regressions cover late invalid/duplicate/unsupported rows and epoch kinds. |
| #7 checkpoint identity | All checkpoint values bind the same explicit height/hash, and the first block extends that exact hash. | `seed_checkpoint` now requires the hash; both row APIs enforce it. |
| #7 deployment undo | Seeds bind the exact applied creation clock and belong to its undo journal. | `seed_deployment_zero` requires the full latest creation clock and retained journal; tests cover forks, rollback, replacement, prior entries and insufficient undo capacity. Creation qualification remains the caller's responsibility. |
| CI layout coverage | CI executes integration assertions in addition to library/binary tests. | The workflow now includes `--tests`; layout tests are no longer merely compiled by Clippy. |
| #16/#24 exact OZ arithmetic | Use full-precision multiplication with source-exact checked inputs, virtual totals and rounded results. | Checked uint256 operands, additions, exponentiation and quotient/ceil bounds with a 512-bit product. An independent pinned Solidity oracle exercises 204 vectors across six conversion/preview getters (1,224 calls), including arithmetic reverts. See [oracle provenance](../conformance/fixtures/oz-v5-oracle.md). |
| #24 underlying asset binding | Extract only source-qualified balance bits and invalidate underlying proxy implementation or code changes, including restored changes. | Explicit `uint256` or `fiat-token-v2_2-low255` model retains the raw word separately. The USDC fixture binds its actual ZeppelinOS implementation slot and source pin. Vault, Pool and underlying pointers now share per-write guards, including equal-value and restored writes, with continuity validation and individual evidence. Regression and compiler-layout assertions cover both balance models, flag-only writes, shared dependencies and reverted effects. |
| #23 Lido resolution | Bind and guard the Aragon Kernel/app resolution and its implementation dependencies. | Required source-bound Aragon configuration derives four guarded slots. Pointer writes (including equal-value and restored writes), implementation/dependency code and version excursions invalidate with evidence. Tests cover shared-Kernel attribution, reverts, system/block scope and deterministic output. |
| #18 receipt validation | Trace logs remain authoritative; ordered receipt identity/content validates them exactly, even with output disabled. | Implemented; tamper tests cover each log field, receipt ordering, nested/reverted frames and ambiguous ordinals. Real saved v4/v5 blocks pass. |
| #18/#20 producer versions | Reject explicitly configured unreviewed versions. | Executions, Aave actions and the sibling ERC-20 event extractor require a nonempty subset of 4/5. Tests reject 3, 6, 999 and mixed lists. |
| #4 bounded migration | Review a source-resolved subset, preserve the qualified baseline, add separate typed-path candidate profiles and negative tests, replay complete saved blocks with canonical controls. | Separate Token and CYS candidates narrow only the reviewed membership paths. Source/runtime binding checks and adversarial tests pass. All 1,024 saved blocks / 110,139 output rows match unchanged current and historical baselines. See [bounded replay](../erc20/balances/docs/role-path-candidates.md). |

## Host API changes

The retention crate is host-side consumer tooling. Its initialization APIs now
require explicit identities:

```rust,ignore
ledger.seed_checkpoint(key, "42", checkpoint_number, &checkpoint_hash, evidence)?;
ledger.seed_deployment_zero(&contract, &holders, &creation_clock)?;
```

Deployment seeds must be attached immediately after the exact creation block
is applied and before advancing to another block; a nonzero undo capacity is
required. Hashes are part of retained initialization provenance. No protobuf
wire contract or production map state is added.

Lido epoch parameters now require `aragon.kernel`, `aragon.app_id`,
`aragon.kernel_implementation` and `aragon.source_pin`. The old parameters
declared an implementation without binding how the Aragon proxy resolved it;
they are deliberately refused until that dependency path is explicit. See the
[source record](research/09-Lido-Aragon-resolution.json) and package README.

OZ epoch parameters now require `asset_balance_model` and `asset_source_pin`.
Upgradeable assets additionally bind the reviewed `asset_implementation_slot`
and `asset_implementation` pair. Old parameters without an explicit balance
model/source pin are refused. The USDC model masks the blacklist bit from
`totalAssets` while retaining the original storage word as evidence.

The bounded role migration preserves the qualified 431-profile fixture and
creates separate unqualified candidates. In BSC **[122288006, 122289030)**,
the replay initializes 33,591 observed holders, including 69 for the two
candidates. It finds 110,138 same-block and 4,012 retained canonical matches,
leaving 66,265 cold observations unknown. No actual role-membership write for
either candidate occurs in this interval; those shapes have source-bound
synthetic tests, not a captured producer control.

## Validation and qualification boundary

All five prioritized offline work groups are implemented. Final checks passed
with Rust **1.88**, the repository lockfile and Cargo offline mode:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo test --offline --locked --workspace --lib --bins --tests` | **624 passed**: 612 library/binary tests and 12 integration tests |
| `cargo clippy --offline --locked --workspace --all-targets -- -D warnings` | Passed |
| `cargo check --offline --locked --workspace --target wasm32-unknown-unknown` | Passed |
| `git diff --check` | Passed |

Logs are in `out/issue-remediation-20260921/`. The standalone Rust oracle
generator also passes rustfmt; its recorded source/compiler/runtime hashes and
generated runtime were independently checked. Separate reviewers inspected the
retention, arithmetic/oracle, ERC-4626 dependency and execution changes. The
saved replay source inventory still matches the final checkout. The canonical
protobufs, persistence rules, qualified baseline and historical packages are
unchanged. The three pre-existing uncommitted tests are preserved.

This completes the audit's five suggested implementation groups, not every
open issue's acceptance criteria. Remaining implementation scope includes the
other legacy profiles, static-aToken withdrawal-limit/liquidity extraction and
full retained global-state evaluation; those are distinct from live
qualification requirements.

This work does not promote candidate profiles, claim complete global-holder
coverage, or qualify a new package from historical evidence. Captured getters,
new runtime/activation bindings, actual packaged-stream comparison and live
sink/reorg qualification remain subject to the existing explicit hold.
