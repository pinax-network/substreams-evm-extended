# Producer behavior on live role operations (BSC, 2026-09-23)

Issue #2 asks how the Extended producer records actual role grants and
revocations: whether unchanged stages appear, and in what order changing
writes and their ordinals arrive. This check observes the producer on real
BSC operations, including DSG's own. It does not qualify DSG, APD or any
profile.

## Sample

`eth_getLogs` selected every `RoleRevoked` log in the 200,000 blocks before
123,568,578 (42 logs, 33 blocks, 34 contracts) and every `RoleGranted` or
`RoleRevoked` log in the last 20,000 (151 logs). The 79 blocks holding them
were fetched from `bsc.firehose.pinax.network` as Extended blocks (producer
version 5). `erc20-balances-tools role-operations` then classified every
storage write that each emitting frame made to the emitting contract
([report](evidence/role-operations-2026-09-23/report.json)).

A write is classified against candidate roots `R = keccak256(role || S)`:
`R` itself, `R+1`, `R+2`, a mapping entry `keccak256(account || R)` or
`keccak256(account || R+1)`, or an array element `keccak256(R) + i`. Roots
come from the block's Keccak preimages when recorded. Otherwise they are
computed for slots 0–255 and the two OpenZeppelin 5 `AccessControl`
namespaces; each write records which (`root_source`).

## Results

236 operations: 194 grants and 42 revocations, all in persisted frames.

| Shape of the role writes, in record order | Operations |
| --- | ---: |
| grant: membership flag | 122 |
| grant: membership flag, admin at `R+1` | 36 |
| grant: membership flag, then set length, new element, member index | 19 |
| revoke: membership flag | 32 |
| revoke: membership flag, then element swap, moved member's index, tail clear, length, removed index | 5 |
| revoke: the word at `R` decremented 2→1, then membership flag (a custom layout) | 1 |

In **every frame the ordinals strictly increase in record order**, and **no
write keeps its value** (0 unchanged writes across all 236 frames). The 24
enumerable operations come from five contracts, with set roots under
slot 151 (OpenZeppelin 4.x upgradeable), slot 1 (4.x) and the OpenZeppelin 5
`AccessControlEnumerable` namespace
([operations](evidence/role-operations-2026-09-23/enumerable-operations.json)).
Their writes follow the two templates the rule describes:

- an append increments the length, initializes the new tail element and
  creates the member index, all changing;
- a non-tail removal copies the tail into the removed position, updates the
  moved member's index, clears the tail, decrements the length and clears
  the removed index.

In these OpenZeppelin 4.x/5.x contracts the `RoleGranted`/`RoleRevoked` log
ordinal falls between the plain membership write and the set writes, which
matches their source order (`super._grantRole` emits, then the set changes).

Of the 379 role writes, 58 resolve only through computed roots: the block has
no preimage for `keccak256(role || S)`. With
a constant role the compiler can fold that hash, so no `KECCAK256` executes.
A rule that requires full preimages for the role path cannot apply to such a
runtime and must fail closed on it.

## DSG's own role history (OpenZeppelin 3.4.2)

DSG (`0x3a090ac70c4f453838c34490e3b1cf925c03fc71`) has had code since block
68,813,972. `eth_getLogs` over 68,813,972–123,568,578 finds 411 role logs in
eight blocks: 409 grants and 2 revocations, all of `DEFAULT_ADMIN_ROLE`, at
69,774,272 (an admin revoking the second of two members) and 69,774,501 (the
remaining admin renouncing, which empties the set). Replaying the grant and
revoke order shows **both revocations remove the tail member**. The eight
blocks are producer version 4 ([report](evidence/role-operations-2026-09-23/dsg-report.json),
[operations](evidence/role-operations-2026-09-23/dsg-operations.json)).

| Shape, in record order | Operations |
| --- | ---: |
| grant: set length, new element, member index | 407 |
| revoke of the tail member: tail clear, length, removed index | 2 |

(Two further grants share a frame with another grant of the same role.)
All 1,239 role writes resolve through block preimages, every frame's ordinals
strictly increase, each log follows its set writes as in the 3.4.2 source, and
there are no unchanged writes. For a tail removal the 3.4.2 source executes
`_values[toDelete] = last` and `_indexes[last] = toDelete + 1` with unchanged
values before popping; **the producer records neither**. This is the
omitted-equality path the rule already accepts, and it is consistent with
`evm/executions`: 0 equal-value storage changes in 2,093,149 saved and 523,805
live writes.

The rule-enabled DSG layout from the saved candidate replay
([layout](evidence/role-operations-2026-09-23/dsg-candidate-layout-NOT-QUALIFIED.json),
not qualified) keeps its runtime binding at 68,813,978 and 69,774,501
([runtime](evidence/role-operations-2026-09-23/dsg-runtime-status.json)). The
candidate excludes deployment, so the creation block is left out. Over the
other seven blocks, which hold 408 of the operations including both
revocations:

- `refusal-scan --allow-gaps` with the rule: no refusal and no balance row
  ([scan](evidence/role-operations-2026-09-23/dsg-refusal-scan.json));
- the same layout without the rule refuses at 68,813,979 on an unresolved
  role key ([control](evidence/role-operations-2026-09-23/dsg-refusal-scan-without-rule.json));
- the packaged WASM (`532b571f…`) run one block at a time agrees: all seven
  succeed with empty output with the rule, and 68,813,979 panics on the same
  key without it ([runs](evidence/role-operations-2026-09-23/dsg-package-runs.json)).

## What this does not establish

- No zero-address member was observed.
- The DSG blocks are a sparse sample with no balance rows; they check that the
  rule admits real operations, not DSG balance parity or holder state. DSG
  remains unqualified (#3), and the blocks between samples were not streamed.
- Other runtimes, compiler builds or producer versions. The OpenZeppelin
  4.x/5.x contracts above use a different layout from DSG's and are not
  covered by the `oz_3_4_2` rule.
