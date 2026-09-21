---
name: pinned-source-research
description: How to bind a contract to a pinned source offline for substreams-evm-extended: which files to fetch, what to extract (declaration order, packing, constants, keccak-named and ERC-7201 slots, formulas, rounding, events), how to write source_pin strings and where to store the notes.
---

# Pinned-source research (offline)

## Rules

- Sources are raw GitHub files at an exact commit or tag, official protocol
  documentation, and official address books / deployments JSON. Block
  explorers are secondary and never the sole source. No RPC.
- Record every claim with its URL and pin in a JSON note under
  `docs/research/` (see its README for the schema), and put a compact
  `source_pin` string in the fixture: `owner/repo@<40-hex-commit> path/A.sol,
  path/B.sol; <address book or deployments file>`.
- The deployed runtime may differ from the pinned source; say so. Binding the
  code hash and the activation block is a live-gated step.

## What to extract per contract

1. **Inheritance chain** in linearization order; for each base, every
   state-variable declaration in order. Constants and immutables take no
   slot; consecutive small types pack into one slot from the low end; a
   mapping or dynamic array takes one slot; a struct value in a mapping spans
   `ceil(size/32)` consecutive words at `keccak(key . base) + i`.
2. **Unstructured storage**: every `bytes32 constant X = keccak256("...")` or
   literal position with its label; the map derives the slot by hashing the
   label at parse time (`other_slot_names`) and a test asserts the committed
   hex. Aragon apps (`aragonOS.*`), Lido (`lido.*`), OpenZeppelin ERC-7201
   (`keccak256(abi.encode(uint256(keccak256(id)) - 1)) & ~0xff`), EIP-1967
   (`keccak256("eip1967.proxy.implementation") - 1`).
3. **Packed words**: for each field its bit offset (byte offset x 8, low end
   first) and width; helper libraries (`UnstructuredStorageExt`
   low/high uint128) define the exact halves.
4. **Getter formulas**: the exact integer expression, operation order,
   rounding helpers (`rayMul` half-up vs `rayMulRoundDown`, `mulDiv` floor vs
   ceil, `_divup`, `rpow` half-up), checked casts and their revert
   conditions, argument guards (`< 2^128`), zero-supply branches.
5. **Writers**: which functions write each slot and under which authorization
   (`_auth`, `onlyOwner`); which slots are written on every call (guards,
   nonces, stake limits) so routine blocks do not fail closed.
6. **Events** used as evidence: full signature, `topic0`
   (`keccak256("Name(type,...)")`), indexed vs data fields, emitter.
7. **Upgrade mechanics**: proxy kind (EIP-1967, Aragon Kernel app, beacon),
   version flags (`contractVersion`), migration functions that move or zero
   slots between versions.

## Recording the outcome

- Fixture: slots and constants with a `source_pin`; placeholders explicit.
- README: a table mapping each emitted field to the pinned declaration.
- `docs/storage-layout-provenance.md`: the verification level of each slot.
- A Rust test that hashes every named slot and topic0 from its label.
