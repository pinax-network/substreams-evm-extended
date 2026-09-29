# evm-epochs

Dependency-free schedule mechanics shared by the five balance-state extractors.
`Position` orders block number and execution ordinal lexicographically. `schedule`
normalizes each market by activation, requires positive strictly increasing epoch
IDs and unique starts, and returns half-open intervals in the original input
order. Different markets may share starts; nonconsecutive IDs are valid.

`Interval` exposes membership, block intersection and overlap without incrementing
ordinals or inventing a maximum sentinel. A scheduled end inside a block marks a
predecessor prefix, which cannot be advertised as END_OF_BLOCK state. `successor`
identifies later entries whose extractors must emit reset-only carryover flags.

The helper does not decode storage, attest layouts/runtime or validate deployment
activation. Packages validate each physical write stream before interval splitting
and keep model-specific pointer/dependency, invalidation and decoding rules. Raw
parameter hashes retain their existing stream-identity meaning.
