# Enumerable role-member sets

`enumerable_address_sets` is an explicit, source-bound metadata rule for the
reviewed OpenZeppelin 3.4.2 address-set operations used by DSG. It does not
change balances or the protobuf, add another map/store, enumerate holders, or
discover a layout automatically. The field defaults to an empty array and is
omitted when serializing an unchanged legacy layout.

Example configuration fragment, to be combined with an independently verified
token layout:

```json
{
  "enumerable_address_sets": [{
    "root": "0x0000000000000000000000000000000000000000000000000000000000000008",
    "key_types": ["bytes32"],
    "semantics": "oz_3_4_2"
  }]
}
```

This is a shape example, not a qualified DSG profile. Root 8 is not built into
the mapper. The parser requires the explicit semantics and exactly one bytes32
role key. It rejects duplicate roots and conflicts with configured balance,
scalar, mapping, array, checkpoint or token-account dependency roots. Typed
mapping paths cannot share an enumerable root.

## State and runtime prerequisites

The caller must bind the runtime and verify both this storage shape and the
exact write order. A library version string alone does not qualify an arbitrary
compiler build. The initial set must already obey its source invariants:
members are unique canonical address words, their one-based index entries agree
with array positions, absent members have index zero, and unused/removed tail
words are zero. The stateless mapper does not reconstruct untouched members or
prove those historical invariants from a sampled block.

For role `r` and configured root `S`, let `R = keccak256(r || S)`. The array
length lives at `R`, element `i` at `keccak256(R) + i`, and a member's index at
`keccak256(padded_address || (R + 1))`. Index zero means absence, including for
the zero-address member. The mapping anchor `R + 1` and role admin `R + 2` are
not mutable metadata under this rule. Runtimes that write the admin field need
separate support.

Full verified preimages bind the role and address-index paths. Lengths and
indexes use all 256 bits; physical storage positions wrap modulo 2^256, but a
logical append cannot overflow the length and an empty set cannot be popped.
The rule never treats an arbitrary storage range as an allowed array.

## Observed-operation checks

The [saved runtime evidence](evidence/dsg-enumerable/) binds two ordered
templates:

| Operation | Ordered writes |
| --- | --- |
| Append an absent member | Increment length, initialize the new tail, create the member index |
| Remove a present member | Copy the tail into the removed position, update its index, clear the tail, decrement length, clear the removed index |

Every changing stage must be observed, including writes that change a word to
zero. An unchanged stage may be present or absent only when equality follows
from the complete operation: a zero-address append, a tail self-swap/index
self-assignment, or clearing an already-zero tail. Duplicate adds and absent
removals do not authorize standalone writes. Identical optional zero writes
may have equivalent alignments, but must describe the same complete operation
and consume the same observed events.

Validation uses structural transaction/call positions, execution boundaries
and positive unique storage ordinals. It rejects incomplete, reordered,
interleaved or cross-call operations, ambiguous call ownership, reused
witnesses, unmatched writes to recognized role fields, and discontinuous
observed or source-implied state. Failed/reverted execution cannot authorize
persisted writes. Generic metadata rules cannot override a failed enumerable
operation. Successful validation permits individual observed
`(contract, key, ordinal)` events; inferred keys grant no permission.

For Extended version 3 only, a zero begin ordinal on the first transaction
call is replaced by the transaction's begin ordinal, as prescribed by the
Ethereum protobuf model. Other missing call boundaries are not normalized.
Groups with a selected operation require coherent complete call frames and
persisted storage ordinals. Other groups cannot supply witnesses: known positive
boundaries and valid intervals block overlaps, without claiming to reconstruct
an unrelated malformed frame whose bounds are absent.

Runtime and balance-dependency guards remain active. The only temporary state
is per-block validation state; no persistent set cache is introduced.
Known balance-leaf and configured protected-slot collisions with metadata
operations reject, including source-implied unchanged stages. The rule cannot
identify every untouched historical mapping leaf without its key evidence.

Unknown unchanged writes outside recognized role namespaces and operation keys
retain the mapper's ordinary no-op filtering policy. This rule does not prove
that every unobserved opcode or unrelated no-op has been identified.

## Qualification limits

The synthetic runtime cases contain 11 calls and 39 SSTORE instructions, nine
of which do not change the value. They are evidence of the reviewed runtime's
behavior, not a full-EVM conformance result or an observation of Firehose's
unchanged-write visibility and ordinal policy. The ordinary saved transfer
window does not establish role-operation visibility.

Thirty-one independent Rust regressions exercise the 11 saved cases and all 26
presence/omission combinations, exact event permissions, invalid and restored
writes, structural boundaries, full-word arithmetic, protected aliases and
unchanged balance behavior. Known holder hints protect balance leaves even
without a preimage, including observed and omitted unchanged metadata writes.
The full workspace passes 406 offline tests.

Two [saved-data replays](typed450-offline-review.md#replay-with-the-enumerable-rule-enabled)
at source `3eae9c839e06fec260e8717845547f21ccafa355` each cover 1,024 blocks.
With DSG's rule explicitly enabled, all 25 APD/DSG emitted balances match saved
RPC output; 33 reference observations remain cold unknowns. The unchanged
431-profile baseline produces the same 110,139 historical balances with no
protobuf differences. These are native Rust compatibility checks over saved
inputs, not role-mutation or producer-visibility qualification.

The observation model therefore permits the specific source-implied unchanged
stages above to be absent. It never assumes that a producer can omit changing
writes. Independently check producer behavior, runtime continuity, actual
packaged WASM/RPC parity and initialized-holder state before promoting any
profile. DSG and the legacy enumerable profiles remain unqualified for this
new rule. Historical SPKGs predate it, and live testing remains paused.

Follow [issue #2](https://github.com/pinax-network/substreams-evm-extended/issues/2)
for implementation and producer validation and
[issue #3](https://github.com/pinax-network/substreams-evm-extended/issues/3)
for APD/DSG qualification.
