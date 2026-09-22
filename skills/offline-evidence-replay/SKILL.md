---
name: offline-evidence-replay
description: How to produce and record evidence for substreams-evm-extended packages without RPC, Firehose or sinks: the saved BSC Extended block inventory, trimming single-transaction fixtures, per-package replay tools with continuity and oracle checks, evidence JSON conventions and the wording rules for coverage claims.
---

# Offline evidence and replay

## What data exists

Saved `sf.ethereum.type.v2.Block` files (`<height>.pb`, DetailLevel
EXTENDED) under `erc20/balances/out/*` on the original machine: 41 directories,
6,093 blocks, all BSC; the contiguous window is
`top50-full-holder-blocks` = [122288006, 122289030) plus two extensions to
122289030; 71 blocks are `Block.ver` 4. No Ethereum, Base or other chain blocks
are cached. `docs/handoff.md` lists every directory with its height range.
Nothing outside those windows can be replayed; say so in evidence.

## Finding activity and cutting fixtures

Build the scratch scanners once:

```sh
cargo build --release --manifest-path tools/scratch-scan/Cargo.toml
B=tools/scratch-scan/target/release
$B/aave erc20/balances/out/top50-full-holder-blocks        # writes/slots/preimage bases per contract
$B/stata erc20/balances/out/top50-full-holder-blocks       # any activity of a target address set
$B/scan  erc20/balances/out/top50-full-holder-blocks trim 122288242 32 <pkg>/tests/fixtures/122288242-tx32-<what>.pb
```

Write a new scanner by copying `tools/scratch-scan/src/stata.rs` (target set,
counts of calls/writes/logs/code changes, examples). A trimmed fixture keeps
the header and identity and one transaction; block-level balance/code/system
records are dropped, so tests that need them must use full blocks.

## Replay tools (per package, host-only crates under `<pkg>/tools`)

Replay tools exist for `native/balances`, `aave/balance-state` and
`evm/executions` (the last also tabulates what each producer version records;
see its README's capability table before assuming an absent fact is absent on
chain). Pattern from `native/balances/tools` and `aave/balance-state/tools`:

1. Read every `<height>.pb` in the given directories in height order.
2. Refuse unqualified `Block.ver` and count those blocks separately.
3. Run the package `project()` and keep a **continuity ledger** keyed by
   `(contract, holder)`/`(contract, slot)`: each row's `previous_value` must
   equal the retained value from the last block; reset the run at gaps and
   refusals; count checks and mismatches.
4. Check clocks link (`parent_hash == previous hash`, number + 1).
5. Where the protocol allows it, compute an **oracle** from the pinned formula
   and compare with the observed write (Aave: `rayMul(linearInterest(prev
   rate, prev ts, now), prev index)`); where a historical RPC snapshot exists
   as a fixture (native BSC 122260950), compare end-of-block values.
6. Write one evidence JSON per run under `<pkg>/docs/evidence/` with: mode
   (`offline_saved_data_only`), directories, block range, counts (blocks,
   rows, checks, mismatches, refused), first/last hash, status, package
   version and the command line; never overwrite an earlier run.

## Claim wording

- State the tested interval and the initialized observed-holder set; matching
  samples never establish universal token or global-holder support.
- A native test or saved replay does not qualify a later SPKG or a live
  stream; say "not a new package qualification".
- Distinguish "observed in saved blocks", "consistent with pinned source",
  "inferred from declaration order" and "placeholder".
- Report emitted rows, initialized observed holders, cold unknowns and any
  independently enumerated set separately (`docs/initialization-and-completeness.md`).
- Never print RPC endpoint URLs or credentials into evidence; use environment
  variables when live use resumes.

## Consumer-side retention

`common/retention` (`evm-retention`) is the host-side ledger that applies
`evm.balances.v1` or `evm.balance_state.v1` rows with origins (checkpoint,
deployment zero, observed), suspensions, gap/fork refusal and bounded undo.
Use it in new replay tools instead of ad-hoc maps so reports use the same
vocabulary.
