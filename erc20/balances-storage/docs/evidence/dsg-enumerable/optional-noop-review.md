# Review of optional equal-write omission

An observation policy that accepts either presence or omission of **specific source-derived same-value stores** can be defensible. It must be an explicit limited observation model, not a claim that the producer always omits or always includes them. All changing writes remain mandatory, including writes whose **new value is zero**.

The allowed omissions for this runtime are narrow:

- Zero append: the new tail `0 -> 0`; length increment and zero member's index creation remain mandatory.
- Tail/singleton removal: self-copy `W(a) -> W(a)` and self-index `N -> N`; tail clear is still mandatory when `a != 0`.
- Zero tail clear: `0 -> 0`; it can occur when removing a zero tail or moving zero from the tail into an earlier position. The correlated changing length/index/destination writes remain mandatory.
- Duplicate add and absent removal execute no stores. They must not become templates that admit arbitrary standalone no-op role writes.

Operation variables must come from required changing records and verified source-derived key paths: role/record, length, member identity, one-based indexes and destination/tail positions. A source-derived omitted event can constrain a candidate operation, but its inferred key cannot authorize another observed event. At acceptance, **every visible role-field write must be consumed once**, including visible equal writes, and every required changing stage must exist. Reject extra no-ops, changing substitutions for optional stages, wrong ordering, wrong old values, broken cross-event continuity and cross-call completions.

A material ambiguity exists for zero singleton removal. The runtime sequence is:

1. `A: 0 -> 0` self-copy at pc7257.
2. `I(0): 1 -> 1` self-index at pc7277.
3. `A: 0 -> 0` tail clear at pc7308.
4. `R: 1 -> 0` at pc7310.
5. `I(0): 1 -> 0` at pc7335.

If the producer exposes one of the two identical `A` events and omits the other plus the index no-op, that visible event can align with either optional PC. Requiring a unique assignment to those indistinguishable optional PCs rejects a valid allowed-omission case. Acceptance may coalesce alignments **only if they agree on all operation variables, mandatory stages, consumed event identities and resulting state**. An ambiguity that changes the operation/member/role or leaves a visible event unconsumed must still reject. Execution ordinals cannot identify a missing opcode's PC by themselves.

The opposite implementation risk is a greedy matcher that skips an equal event and then accepts a later operation using the same key. It can strand a malicious intermediate no-op or merge operations. Search optional alignments within the single scoped operation, preserve complete event consumption, and reject unmatched records at the end. If acceptance is finally expressed as an account/key allowlist, first prove that every persisted event on every accepted key was consumed; one valid event must not hide an invalid second event.

For zero singleton removal with every equal stage omitted, the changing length and member-index deletion still pin `N=p=1` and member zero. They justify only the operation, not a general array range. For a nonzero tail, the clear `W(a) -> 0` is changing and cannot be omitted. For a middle removal that moves zero, destination and moved-index updates are changing and cannot be omitted even though destination becomes zero.

Required negative tests should remove each mandatory stage individually; replace optional equality with a changed value; inject an extra same-key event before/between/after otherwise valid operations; corrupt old values; splice records across calls; duplicate ordinals; and add an unrelated inferred-only array/index key. Positive tests should cover all presence/omission subsets of the9 equal stores across the compact cases, plus the identical-event ambiguity above and repeated successful operations with restoration.

This review supports a fail-closed policy over the declared observation model. It does not establish producer visibility, global set consistency, arbitrary compiler versions or qualification of DSG.
