---
name: roadmap-issue-workflow
description: How roadmap issues in pinax-network/substreams-evm-extended move from acceptance list to merged PR and status comment: reading the issue, doing the offline part, commit and PR conventions, CI constraints, polling checks and merging without auto-merge, and when an issue may be closed.
---

# Roadmap issue workflow

1. **Read the issue** (`gh issue view N`) and split its acceptance list into
   offline items (Rust code, tests, saved-block replays, pinned-source
   research, docs) and live-gated items (RPC, Firehose, SPKG, sink). Every
   roadmap issue says live use is paused; do not start live items.
2. **Branch from a fresh main** (`git checkout main && git pull`) with a short
   kebab-case branch name. Another agent commits on `codex/*` branches; pull
   before branching and never rebase their work.
3. **Implement** following `skills/balance-state-package` (or the package's own
   README for other kinds). Keep diagnostics in Rust; keep host tools out of
   the WASM path (`#[cfg(target_arch = "wasm32")] mod handler` for maps,
   `#![cfg(not(target_arch = "wasm32"))]` for host crates).
4. **Validate** exactly what CI runs:
   `cargo fmt --all -- --check`, `cargo test --workspace --lib --bins`,
   `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo check --locked --workspace --target wasm32-unknown-unknown`.
   Clippy includes rustdoc lints (for example "link reference defined in list
   item" for `* [`Name`]:` in module docs). New dependencies need one unlocked
   `cargo test` so `Cargo.lock` is updated and committed.
5. **Commit** with a descriptive message and the attribution trailers the
   session requires (`Co-Authored-By` and `Claude-Session` lines for Claude
   sessions).
6. **Open the PR** with a Summary, Limits (what is synthetic/inferred/
   placeholder) and Validation section, ending with the generator footer.
7. **Merge**: auto-merge is disabled. Poll
   `gh pr checks N` (tab-separated; `awk -F'\t' '{print $2}'`) until the
   single "Rust checks" job passes, then `gh pr merge N --merge`. A background
   loop with a 30 s sleep works.
8. **Comment on the issue** with two lists: done offline (with the PR number)
   and still open / live-gated. Close the issue only when nothing live-gated
   remains; otherwise leave it open. Keep `docs/follow-up.md` and the roadmap
   issue #21 table current.
9. **Record** anything not derivable from the code in `docs/handoff.md`
   (facts learned, data locations, open findings).

Shell caveats on the original machine: zsh does not word-split unquoted
variables (`${=VAR}`); avoid leading `=` in echo arguments; foreground sleeps
are blocked in the assistant harness (use background commands).
