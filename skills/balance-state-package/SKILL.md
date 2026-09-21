---
name: balance-state-package
description: How to add or change a protocol balance-state package (one RPC-free map_events emitting evm.balance_state.v1) in substreams-evm-extended, with the fail-closed rules, fixture and test conventions, README template and review checklist used by the existing aave, compound-v2, compound-v3, lido and erc4626 packages.
---

# Balance-state package

## When this applies

A protocol whose observable `balanceOf` depends on shared state (an index, an
exchange rate, packed totals) or whose ERC-20 amount is not the stored basis.
The map emits the **inputs** (holder basis + global words + bindings); the
consumer evaluates with the `conformance` crate at a canonical clock. Never
emit a computed balance and never replace `evm.balances.v1` `Balance.amount`.

## Layout of a package (copy `compound-v3/balance-state` or `lido/balance-state`)

```
<protocol>/balance-state/
  Cargo.toml          # deps: substreams, substreams-ethereum, proto, evm-persist, hex, serde, serde_json, sha2, tiny-keccak
  substreams.yaml     # module map_events, output proto:evm.balance_state.v1.Events, default params bind nothing
  Makefile, .gitignore
  src/lib.rs          # parse(), validate_block(), project(); handler gated #[cfg(target_arch = "wasm32")]
  src/tests.rs        # synthetic tests (see checklist)
  tests/fixtures/<chain>-<market>-epoch(s).json
  README.md
```

Register the crate in the root `Cargo.toml` members, run `cargo test` once
unlocked so `Cargo.lock` updates, then keep `--locked` green.

## Code pattern (all five packages share it)

1. **Parameters** with `#[serde(deny_unknown_fields)]`: `chain_id`,
   `producer_versions`, `heartbeat_blocks`, and a list of markets/epochs with
   `epoch`, `model_id`, `source_pin`, `activation_block`, implementation
   pointer (slot + address) when the contract is a proxy, the balance mapping
   slot, the scalar/packed word slots, `other_slots`, `other_mapping_slots`
   and, where the source uses unstructured storage, `other_slot_names` hashed
   at parse time. Reject overlapping slots, missing pins, duplicate markets,
   and (to do, see handoff) producer versions other than 4 and 5.
2. **Persisted effects** through `evm_persist::collect_block` into a `Sink`
   that collects storage writes (and native balance changes or code changes
   when needed). Never read `Call.storage_changes` directly.
3. **Reduce** per `(address, key)` in ordinal order: strictly increasing
   ordinals (`ambiguous` otherwise), `old == previous new` (`discontinuous`
   otherwise), keep first old / last new, `first_ordinal`, `ordinal`,
   `change_count`, last write's transaction/call provenance. Include address,
   key and ordinals in the error messages.
4. **Preimages**: collect `keccak_preimages` from every call whose address is
   the contract *or* that writes the contract's storage (delegatecall frames
   have `Call.address == implementation`); verify `keccak(value) == key`; a
   holder key is a 64-byte preimage with 12 zero bytes and the balance slot as
   base; use `mapping_has_base` for nested mappings and struct-word offsets.
5. **Rows**: `HolderBasis` for every reduced write to a verified holder key;
   `GlobalState` for every decoded field of a written word (the proto says one
   row per decoded field; do not filter unchanged fields without a documented
   decision); `ModelEpoch` `BOUND` at `activation_block` and `REAFFIRMED` on
   the heartbeat with dependencies and qualified constants; `INVALIDATED` rows
   with evidence for pointer writes and code changes; exactly one `BlockClock`.
6. **Fail closed** on: non-Extended blocks, unlisted `Block.ver`, malformed
   persisted records, invalid preimages, and any persisted write to the
   contract that is not a configured word, a verified holder key, a pointer or
   a reviewed slot / mapping member. Dependencies' unrelated storage is ignored.
7. **Sort** every output list deterministically and fill the clock counts.

## Slots written on every call (do not forget)

Reentrancy guards and version flags are written even when balances are not:
Comet `keccak256("comet.reentrancy.guard")`, cToken `_notEntered` (slot 0),
Aragon `aragonOS.reentrancyGuard.mutex`. `evm-persist` drops writes whose old
and new values are equal, but 0→1 then 1→0 are two writes that reduce to
`old == new` and must be reviewed. Read the pinned source for every
`nonReentrant`, `whenNotPaused`, nonce and counter write.

## Fixture and README conventions

- Fixture JSON is a complete parameter document; addresses from an official
  address book or deployments file named in `source_pin`; slots as 32-byte hex;
  placeholders (implementation, `activation_block: 1`) must be called out in
  the README as unverified.
- README sections: what is emitted (table), the getter formula in words with
  the pinned form, parameters and their provenance level (see
  `docs/storage-layout-provenance.md`), fail-closed rules (table), validation
  commands, what the tests cover and what remains live-gated, issue link.
- Update the root `README.md` package list, `docs/follow-up.md`, and the
  `conformance/README.md` table when a model is added.

## Test checklist (synthetic; every item below has been missed at least once)

- holder write: positive/zero/negative or dust values, first-time holder (old
  word all zeros and `old_value: vec![]`), full withdrawal to `"0"`
- same-block repeated writes: `previous_value`, `value`, `change_count`,
  `first_ordinal`, `ordinal`, `transaction_hash` of the last write
- tie (`ambiguous`) and discontinuity; ordinal 0
- packed words: each field at its bit range; 128-bit extremes; unchanged halves
- reviewed scalar slot (guard 0→1→0 in the same frame as a holder write) and
  reviewed nested mapping members (struct word +1)
- unresolved key fails closed; pointer write and code change on the contract,
  implementation and every dependency
- reverted frame inside a succeeded tx; FAILED and REVERTED transactions;
  `status: 0` refused; system-call scope
- delegatecall frame shape: `Call.address == implementation`, writes on the proxy
- every `validate_block` refusal (detail level, header, hashes, number,
  timestamp, tx status)
- pre-activation block emits only the clock; multi-market block attributes rows
  by `storage_contract`; heartbeat `REAFFIRMED`
- determinism: project the same block with reversed transaction and write
  order and compare encoded bytes
- parameters: unknown field, overlap, missing pin, model/dependency mismatch,
  constant cross-checks
- conformance: exact values on both sides of a kink, long elapsed time, checked
  cast/overflow error arms, revert-on-negation extremes, missing input is an
  error never zero

## Validation

```sh
cargo fmt --all -- --check
cargo test --workspace --lib --bins
cargo clippy --workspace --all-targets -- -D warnings   # rustdoc lints count
cargo check --locked --workspace --target wasm32-unknown-unknown
```

## Review checklist (the four dimensions used on 2026-09-21)

1. Pinned-source conformance: slots and declaration order incl. packed and
   constant members, bit offsets, formulas, rounding, widths, revert paths,
   event ABIs; fetch the pinned files, not the README.
2. Fail-closed and persisted-effect semantics: persist rules, reduce, preimage
   verification, delegatecall context, silently ignored vs failing writes,
   routine-block writes (list them), ordering, error messages, WASM gating.
3. Contract conformance: `""` vs `"0"`, previous values, scales, enums, key
   conventions, storage_contract vs market, ModelEpoch/Dependency fields,
   clock counts, sort order, README tables vs code.
4. Test adequacy against the issue acceptance list (the checklist above).
Verify each finding adversarially (correctness, reproduction, intent) before
fixing; record results in `docs/review-findings-<date>.md`.
