# USDe and CONCILIUM storage qualification

USDe (rank 423, `0x5d3a1ff2b6bab83b63cd9ad0787074081a52ef34`) and
CONCILIUM (rank 440, `0x40af8fd127dcd302d7ffa6f37cf5a002e54ac68c`)
use direct unsigned balance mappings at roots 5 and 0. Ranks refer to
activity in the sampled RPC stream. Both complete source trees retain the
inherited balance getter without a caller, clock or external-call dependency.
Transfer fees and bridge restrictions affect writes, not the getter's result.

The source service's compiler artifacts reproduce each runtime at the parent,
sample and final blocks; the sources were not locally recompiled. USDe
reconstruction applies only the compiler-declared
immutable locations and the reviewed CBOR metadata suffix; all remaining
bytes must match. CONCILIUM matches without transformations. Independent
getter traces read one balance word and make no external calls.

USDe's reviewed non-balance fields include owner and bridge configuration,
peer and nested enforced-option mappings, allowances, and a four-word
`RateLimit` record at mapping root 11. The four terminal words are amount in
flight, last update, limit and window. CONCILIUM has allowances, pool and
fee-exemption mappings, and four explicit address-array slots 15–18. Unknown
scalar fields and unreviewed mappings still stop processing. USDe dynamic
option payloads are not qualified. The existing multiword-mapping rule does
not enforce an exact mapping depth; this bounded qualification does not claim
that every fabricated nested record is rejected.

Forty raw-word overrides cover zero, one, 123 and uint256 maximum at five
addresses per token, including the null address. Another 176 controls alter
reviewed metadata while independently checking the balance. Eighty field-getter
calls bind the storage interpretation, including 48 complete four-word rate
limit returns. All 296 exported requests match the exact sent overrides and
recorded responses.

All gates pass for blocks **122288006–122289029**, inclusive:

- Strict native replay completes all 1,024 blocks without errors.
- The actual packaged WASM emits 34 RPC-matching balances, including six zeros.
- A parent checkpoint initializes 24 observed holders with 48 balance/storage
  reads. Actual WASM events match 54 reference observations, including 20
  carried forward without a new event.
- Fresh final RPC checks match all 24 retained holders, including ten zeros.
- Cold replay has 20 unknown observations, seven of them nonzero.

The per-token `carry_forward_matches` field in the underlying coverage reports
counts cold, previously observed state. It is zero for both tokens. The
separate `initialized_carry_forward_matches` total of 20 counts checkpoint-
initialized state: six CONCILIUM and 14 USDe observations.

Reference observations never repair retained state, and processing makes no
balance RPC calls. These are observed-holder checks over a fixed window;
they do not establish global holder enumeration, complete cold-stream
coverage, deployment initialization or a new combined-profile checkpoint.

Two captured block fixtures contain seven independent RPC expectations. Ten
Rust tests cover these captures, exact raw and metadata controls, null-address
filtering, metadata-only writes, field getters, four-word record boundaries,
required preimages, explicit array slots and rejection of unreviewed dynamic
payloads. The initial test compilation failure is retained in the local
evidence; the corrected focused test run passes all ten tests.

Evidence:

- [Source and runtime review](evidence/bridge450-source-review.json)
- [Qualification](evidence/bridge450-qualified.json) and [RPC requests](evidence/bridge450-rpc-requests.json)
- [Packaged RPC audit](evidence/bridge450-rpc.json)
- [Checkpoint and cold coverage](evidence/bridge450-holder-coverage.json)
- [Actual-WASM holder checks](evidence/bridge450-wasm-holders.json)
- [Summary](evidence/bridge450-summary.json), [fixtures](../tests/fixtures/bridge450/cases.json) and [Rust tests](../src/bridge450_tests.rs)

The production package is unchanged: one RPC-free `map_events`, the shared
balance protobuf and default parameters `[]`. Both layouts are explicit test
configuration.
