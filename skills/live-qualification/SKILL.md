---
name: live-qualification
description: Qualify a packaged balance-state map against the live chain with bounded Substreams runs and same-block RPC getter parity, without leaking credentials.
---

# Live qualification

Use this once the owner has resumed live use for a network (BSC on
2026-09-22; see `docs/handoff.md` §1). The worked example is
`aave/balance-state` (#13): `aave-balance-state-tools live-parity` and the
evidence under `aave/balance-state/docs/evidence/live-*`.

## Credentials

- Keep `SUBSTREAMS_API_KEY` and the key-bearing `RPC_URL` in a `chmod 600`
  env file outside the repository and `source` it per command. Never echo
  them, never pass them to a tool that logs its arguments, never commit them.
- Before committing evidence, grep it and the run logs for the key and for
  the RPC host path. Reports name endpoints by host only.

## Bind the runtime epoch first

1. `eth_getStorageAt` the implementation pointer (EIP-1967 or the package's
   resolution slots) at a recent finalized block; it must equal the
   parameters.
2. Binary-search the first block holding that value, then sample the pointer
   at evenly spaced blocks up to head to rule out a switch away and back.
3. When several contracts bind the epoch, it starts at the **latest** install.
   Fetch that block with `firecore tools firehose-single-block-client`,
   decode the pointer write and set `activation_block` and
   `activation_ordinal = write ordinal + 1`. Old blocks may be producer
   version 4: list it.
4. Record previous implementations, the write (tx, call, ordinal) and the
   code hashes in an `epoch-binding-*.json`.

## Stream modestly

- Build and `substreams pack` into a fresh `out/<date>/` directory; hash the
  spkg and wasm. Rebuild and re-pack after any map change: evidence must be
  for the exact package.
- Find active blocks cheaply with `eth_getLogs` on the bound contracts (and
  on events of routine side paths such as `Approval`/`permit`), then stream
  each with `substreams run … -s N -t +1 -o jsonl --bytes-encoding hex`. Add
  one contiguous window (about 2,000 blocks) to exercise idle blocks,
  heartbeats and unfiltered side effects. Always pass a stop block.
- A refused block is a finding: decode the failing key against the compiled
  layout, record the failure in the evidence, fix, re-run into a fresh
  directory.

## Check every row at its block hash

For each emitted row call the getter with `{"blockHash": …,
"requireCanonical": true}`: the basis against the stored-basis getter, each
global word against the protocol getter, and the observable balance through
the `conformance` model against `balanceOf`. Check the clock against the
header. Write one `report.json` with the package hashes, the events digest,
the block list, per-check counts and mismatch examples.

## Claims

State the epoch, the blocks and the holders written in them. Same-block
parity does not initialize holders without a row, qualify other markets,
intervals or networks, or prove bytecode equality with the pinned source.
