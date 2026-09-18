# Remaining role-storage boundaries

The MUSD and OLY [configuration correction](role-width-coverage.md) prompted
a wider review of `other_mapping_words`. The review inventories 51 profiles
from the 426-profile baseline, excluding MUSD, OLY and NXT. Complete source
evidence binds 45 profiles across 26 effective runtimes; six profiles retain
their previous bytecode evidence and remain unresolved by this source audit.

The audit reproduces two distinct guard limitations:

- **33 boolean-role profiles:** the recursive two-word rule accepts one word
  past a nested membership boolean. Twelve have no source call to the admin
  setter, 19 set known roles during initialization, one writes during
  construction, and one exposes an owner-authorized arbitrary-role setter.
  Removing the width rule from all of them would reject legitimate admin
  writes in some contracts.
- **Eight enumerable-role profiles:** valid array and offset-one index writes
  currently reject, while a fabricated mapping under the array-length field
  is accepted. These need explicit array and index recognition.

Four reviewed profiles use terminal scalar records and do not need the same
role-specific narrowing. The 344 synthetic checks record exact keys,
preimages, accepted writes and errors. They describe mapper behavior; they
do not establish that fabricated writes are reachable or allege historical
balance mismatches. No RPC calls or configuration changes were made by this
audit.

The audit proposed recognizing an exact sequence of mapping keys and allowed
field offsets. For example, a role admin lives one word after a single
`bytes32` mapping key, while membership is one boolean word after a second,
address-shaped key. A terminal four-word rate record has a different shape.
Each rule must require correct preimages and canonical key widths, without
applying a record's width to every nested leaf. Existing width rules should
be migrated per reviewed profile, preserving legitimate scalar records.

The subsequent [typed mapping-path implementation](typed-mapping-paths.md)
adds that exact-depth configuration to the Rust source, with offline regression
tests. It does not migrate these published profiles or cover enumerable-role
arrays and offset-one index mappings. Profile replacement still needs fresh runtime checks, full native replay, packaged
RPC comparisons and initialized-holder/final-state checks. The current
historical parity evidence remains bounded to its recorded intervals.

- [Source-bound inventory and findings](evidence/role-shape-audit.json)
- [All synthetic checks](evidence/role-shape-checks.json)
- [Per-profile recommendations and proposed matching rules](evidence/role-shape-recommendations.json)

The original audit and Rust helper files remain under
`out/role-width-audit` and `out/next-candidate-rust-scripts/role-width-audit`.
