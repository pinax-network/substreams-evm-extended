# WKEYDAO and GOT: bounded host operation proof

This host-only Phase A binds two different complete runtimes: WKEYDAO
`0x194b302a4b0a79795fb68e2adf1b8c9ec5ff8d1f` and GOT
`0x701add4311e85c1f9c1549319fe2c476bc8a1b8b` on chain 56. It adds no production
validator, candidate, schema, dependency or VM opcode. All operation inputs are
synthetic local state. No RPC, Firehose, Substreams or sink is called.

The original complete captures and regenerated compiler artifacts are in
[the fixture directory](../tests/fixtures/wkey-got-operation-proof/README.md).
WKEYDAO has five source files and GOT has seven. Four shared dependencies, plus
GOT's ECDSA and IERC20 files, match
[OpenZeppelin 3.4.2 at its exact commit](https://github.com/OpenZeppelin/openzeppelin-contracts/tree/8e0296096449d9b1cd7c5631e917330635244c37)
byte for byte. Original MIT notices and license are retained. Both distinct
custom `contracts/ERC20.sol` bodies retain their AGPL-3.0-or-later notices;
neither has an independently established maintainer revision in the reviewed
material. Complete captured source and compiler equality do not remove that gap.

Compilation uses official `0.7.5+commit.eb77ed08`, with original optimizer
settings enabled / 200 runs, empty libraries and omitted EVM version. Metadata
identifies default Istanbul. Only outputSelection is extended. The binary is
bound to the immutable
[official compiler manifest](https://github.com/ethereum/solc-bin/blob/16a99b8c26ed33a91796e209ff6797ee7baf2b0d/macosx-amd64/list.json)
by version, release association, SHA-256 and Keccak. Complete fresh outputs,
raw metadata, ABI, layout, source IDs, bytecode, maps and empty generated-source
lists are checked separately for each target.

| Binding | WKEYDAO | GOT |
| --- | --- | --- |
| Runtime bytes | 7,991 | 8,440 |
| Runtime Keccak | `84d1cbfc7b7c569181930ce930f0dbe6edb8e8df5631b0a066bd0197d109b9f3` | `8f10d493bbd10ba2062c25efaa1cfe1035b392f339a485fce3faed0c0768dc9c` |
| Creation object bytes | 9,180 | 9,572 |
| Constructor argument append | 96 bytes | 96 bytes |
| Combined role root | 9 | 8 |
| Full compiler output SHA-256 | `73a1801e4de6ec8a6c6d22c649de3782996da063b104edbccde843718b12633c` | `d6a4e5151228a4e89545f1193e349c60ad9e1f1e7c628abda7b6cbf2fae76552` |

There is no runtime transformation, metadata stripping, immutable replacement or
library linking. The complete creation object plus each independently re-encoded
`constructor(address,address,uint256)` append must equal the saved creation
bytes. Original match labels and deployment records stay unchanged. These
records are historical captures, not fresh deployment verification.

## State and operation scope

Both tokens use balances root 0, allowances root 1, supply slot 2, name 3,
symbol 4, decimals 5, MaxSupply 6 and nonce root 7. WKEYDAO stores its domain
separator at 8 and roles at 9; GOT computes its domain inside permit and stores
roles at 8. Their pair, two receiver and two ratio slots therefore differ too.
Each target keeps its complete schema and independent synthetic prestate.

For a role at `R = keccak(role || root)`, the combined old set stores length at
`R`, elements at `keccak(R)+i`, one-based positions under `R+1`, and admin at
`R+2`. Membership is nonzero position. The source has no `_setRoleAdmin` callsite.
This proof does not authorize admin writes or enable the existing DSG template
for either runtime.

The bounded matrix covers role grants, revokes, self-renunciation, authorization,
zero members, self-swaps, equality stores, absent operations, sequential state,
getters and malformed synthetic set states. Exact ordered stores/logs, full
committed state, preimages, read continuity, source PCs and rollback are checked.
Named Solidity 0.7.5 INVALID controls remain distinct from successful coherent
operations, exact source reverts and explicit unsupported VM boundaries.

Token controls preserve source-specific cap behavior: mint checks MaxSupply and
logs the contract as sender, while burn reduces MaxSupply. WKEYDAO's `_burnFrom`
is public; GOT's is internal. Fee paths use separate buy and sell receivers and
some unchecked arithmetic. Neither compiler similarity nor a matching role
sequence makes their complete balance behavior interchangeable with ERC20TokenX.

The final measured run passed **1,263 calls**: 1,000 returns, 252 exact expected
reverts, six explicitly expected source INVALID controls and five unsupported
boundaries. WKEYDAO contributed 630 calls (501 / 123 / 3 / 3); GOT contributed
633 (499 / 129 / 3 / 2). Every expected source INVALID and unsupported outcome
has a named case, exact boundary and rollback assertion. There were no
unexpected INVALID or harness-failure outcomes. These synthetic measurements
do not establish deployment qualification.

WKEYDAO construction stops at unsupported CHAINID PC 222 after five attempted
stores (fee ratio, name, symbol, decimals and cap), with no logs and full rollback.
GOT's original constructor returns the exact 8,440-byte runtime after 17 stores
and three role logs. Its zero receiver / zero buy receiver / excessive ratio
controls revert after eight attempted stores and one role log; all roll back.
Zero/maximum ratio and fee-receiver/deployer alias variants also match their
independent expectations. Synthetic construction does not establish actual
deployed initial-set coherence.

The two runtimes independently execute append stores at PCs 6367/6383/6402
(WKEYDAO) and 6748/6764/6783 (GOT). Removal executes destination, moved index,
tail clear, length and removed index at 7130/7150/7181/7183/7208 and
7511/7531/7562/7564/7589 respectively. Tail self-swaps and zero-to-zero stores
are retained; only complete source-implied operations are measured.

The fee controls stop at foreign EXTCODESIZE PCs 5134 / 5057 after two stores
and two logs. Permit stops at unsupported TIMESTAMP PCs 3390 / 3234 before any
store/log, using the legacy context-free VM entrypoint. These are explicit
harness limitations with exact attempted prefixes and full rollback, never
source reverts or successful callbacks. No external call is executed or mocked.

## Evidence and remaining gates

Final compiler generation is saved in
`out/wkeydao-got-operation-proof-20260929/source-02/`, with the matching
operation run in `operations-02/`. Both tools refuse output-directory reuse and
snapshot their as-run sources. The earlier `source-01` / `operations-01` runs
remain preserved. Every raw execution is saved before assertions and source
annotation; malformed-input and corrupted store/log/exit tests exercise failure
retention. Root's first independent trace-checker attempt is also preserved
separately from the successful corrected review.

The committed [operation report](evidence/wkey-got-operation-proof-20260929.json),
[compiler report](evidence/wkey-got-operation-proof-20260929-compiler.json),
[source inventory](evidence/wkey-got-operation-proof-20260929-source-inputs.json),
[case inventory](evidence/wkey-got-operation-proof-20260929-cases.json) and
[compact transcripts](evidence/wkey-got-operation-proof-20260929-transcripts.json)
are byte-identical copies of those final runs. They bind 1,263 raw and 1,263
annotated traces, 445 compact transcripts, 218,417 executed opcode records and
5,535 source-mapped effects. Both runs use source inventory SHA-256
`4782430d017a36991bbefe3577cf72380b311025a701a632cb503d8aff7a2ca9`.
The operation report SHA-256 is
`0dfc95a2b5c949ec5eab362fb7a638256a86de527b1ce94cb06966426aa86e08`;
the compiler report is
`bd61d288b2d02fc6820c89c7f88e2907c11551fae3271c2bf429634bdcb555c0`.

The isolated, locked offline workspace run passed 1,163 tests in 99 suites
(67 nonempty), including the 17 new binding and operation tests. Formatting,
all-target Clippy with warnings denied and the WASM workspace check also passed.
Logs and the final validation record are retained under the same output root.

```sh
CARGO_TARGET_DIR=out/wkeydao-got-operation-proof-20260929/target-isolated \
  cargo run --locked --offline -p erc20-balances-tools --bin build_wkey_got_proof -- \
  erc20/balances/tests/fixtures/wkey-got-operation-proof /path/to/official-solc-0.7.5 NEW_SOURCE
CARGO_TARGET_DIR=out/wkeydao-got-operation-proof-20260929/target-isolated \
  cargo run --locked --offline -p erc20-balances-tools --bin execute_wkey_got_proof -- \
  NEW_SOURCE NEW_OPERATIONS
```

The builder retrieves immutable public dependency/license/compiler-manifest
files only. The operation executor performs no network requests. A future
candidate requires separate review, adversarial projector tests and saved
canonical replay. GOT would need both broad root-8 permissions removed; WKEYDAO
would need its broad root-9 permission removed. All unrelated profile fields and
historical baselines must remain intact. Custom source provenance, current
runtime/dependency continuity, qualified initial sets, real producer framing and
preimage/equal-write visibility, and separately authorized package/getter/holder
checks remain unresolved. No synthetic sample establishes universal coverage.
