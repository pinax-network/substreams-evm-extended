# Bounded legacy role-path migration

Issue [#4](https://github.com/pinax-network/substreams-evm-extended/issues/4)
now has two separate offline candidate profiles, covering Token
`0x255e746abb8d9acac00d6d023e5e63e3b8dfa7cd` and CYS
`0x0c69199c1562233640e0db5ce2c399a88eb507c7`. The published 431-profile
baseline remains byte-for-byte unchanged at SHA-256
`e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468`.

The [candidate fixture](../tests/fixtures/role-path-candidates/README.md)
replaces only each token's legacy role-root width-two rule with the exact
`[bytes32, address]` path, offset zero, width one. Token uses root 8; CYS uses
root 6. Balance roots, runtime hashes and every unrelated profile field are
preserved. These candidates do not migrate the remaining legacy profiles.

The complete saved source bundles identify a boolean membership mapping at
record offset zero and an admin role at offset one. Their `_setRoleAdmin`
functions have no source callsite. The source recheck verifies all 46
per-file hashes, original capture SHA-256 bindings and bound runtime hashes;
it reconstructs Token's runtime using its seven declared immutable
replacements and directly matches CYS's compiled bytes. This reuses saved
compiler evidence and does not claim a fresh compilation or live runtime
verification.

The host-only integration tests accept membership grant/revoke shapes and
refuse admin writes, adjacent words, malformed address padding, wrong
mapping depths and missing preimages. They also reproduce the adjacent-word
acceptance in the unchanged legacy profile and assert that reversing the
candidate's one rule change restores every original field.

## Full saved-window replay

The [report](evidence/role-path-candidates-20260921.json) and
[source inventory](evidence/role-path-candidates-20260921-source-inputs.json)
record the entire BSC interval **[122288006, 122289030)**. The reproducible
host binary is
[`replay_role_candidates`](../tools/src/bin/replay_role_candidates.rs).
Its as-run source, block hashes/digests, complete output, combined candidate
config and per-block comparisons remain in
`out/role-path-candidates-20260921/`.

- All 1,024 exact block identities and parent links match the saved clocks.
- All 110,139 emitted rows match both the unchanged current baseline and the
  preserved historical protobuf output, including every Events/Balance field
  and optional-contract presence; only row ordering is normalized.
- Canonical RPC capture comparisons include 110,138 same-block matches and
  4,012 matches retained from earlier emissions, with no mismatches. One
  emitted row has no canonical RPC capture row and is verified only against
  the historical output/current baseline.
- The ledger initializes **33,591 observed holders**, including 6,197 known
  zeros at the final block. It uses no checkpoints or deployment seeds and
  leaves **66,265 cold reference observations unknown**. These are observation
  counts, not a global holder enumeration.

The two replacement profiles have the following bounded coverage:

| Candidate | Emitted rows matching saved RPC | Initialized observed holders | Retained RPC matches | Cold RPC observations | Cold nonzero observations |
| --- | ---: | ---: | ---: | ---: | ---: |
| Token | 156 | 42 | 17 | 71 | 68 |
| CYS | 114 | 27 | 0 | 103 | 45 |

There are **zero persisted role-membership writes** for these two contracts
in the saved interval. Role-shape acceptance/refusal therefore rests on
source-bound synthetic tests; the replay establishes unchanged historical
balance behavior in that interval, not producer visibility of real role
mutations. The candidate initialized-holder set contains 69 holders, not
all holders of either token.

No Substreams, Firehose, RPC or sink operation ran. These artifacts do not
qualify a newly built SPKG, another runtime, a new interval or a production
profile replacement. Those gates remain separate; no historical evidence
was rewritten.
