# BNBTiger and COOKIE: guarded metadata candidates

These two **NOT-QUALIFIED** profiles are additions outside the historical 431-profile
fixture and qualified 425-profile package. Both remain byte-for-byte unchanged.
The selected direct getters return a single uint256 mapping word (BNBTiger root 7,
COOKIE root 1); their [compiled-source proof](bnbtiger-cookie-getter-proof.md) is a
prerequisite, not an authorization to ignore every declared storage word.

The new opt-in `metadata_semantics` modes name the exact captured source/runtime
contracts: `bnbtiger_solc_0_8_4` and `cookie_solc_0_6_12`. Each parser binding fixes
the token address, full runtime hash and balance root. The mode cannot coexist
with generic scalar/mapping permissions, enumerable/checkpoint/address-list
rules, proxies, creation, balance conversions or another dependency rule. This
does not support every contract compiled with either Solidity version.

## Source-bound permissions

Offsets below count bytes from the least-significant end of an EVM word. Every
record checks **both old and new** values, all fixed constants and zero unused
padding. The packed guard conservatively admits at most one runtime field change
per physical record. Repeated writes to one key need positive, strictly increasing
ordinals and exact value continuity. Distinct keys can commute or share ordinals.

| BNBTiger location | Permission and source boundary |
| --- | --- |
| Scalars 0,1,6,29 | Canonical low-160 addresses: owner, previous owner, team wallet, router. Router changes are source-reviewed external-context writes, not executed dependencies. |
| Scalar 2 | uint256 owner lock time. No timestamp execution claim. |
| **Scalar 5** | Decimals byte 0 fixed to **9**; marketing address bytes 1..20 writable; bytes 21..31 zero. |
| Scalars 13..24,26..28 | Full uint256 fee/share components and totals, transaction/wallet limits and swap threshold. No invented percentage cap or cross-word sum check. |
| **Scalar 30** | Pair address bytes 0..19; four canonical bools at bytes 20..23; bytes 24..31 zero. Pair/router and swap paths are source-reviewed, not executed. |
| Root 8 | Exactly `[address,address]`, one uint256 allowance word; both keys nonzero as required by `_approve`. |
| Roots 9..12 | Exactly `[address]`, one canonical bool word for fee/limit/pair flags. |
| Excluded | Constructor-only name/symbol at 3/4, supply at 25, and all string payload words. No deployment admission. |

| COOKIE location | Permission and source boundary |
| --- | --- |
| Scalars 0,10 | Canonical owner/router addresses; owner may become zero. |
| Scalar 3 | uint256 supply may remain equal or increase via the two reachable mint overloads. Internal burn helpers have no public caller in the complete selected source. |
| **Scalar 6** | Decimals byte 0 fixed to **18**; uint16 tax at 1..2 <=1000, burn rate at 3..4 <=100, max-transfer rate at 5..6 <=500; bytes 7..31 zero. Temporary tax zero/restore is allowed. |
| Scalars 8,9 | Canonical swap-enable bool; full uint256 liquidity threshold. |
| **Scalar 11** | Pair at bytes 0..19, bool lock at byte 20, constructor-only uint16 maxHoldingRate at 21..22 fixed to **500**, bytes 23..31 zero. A changed pair must be nonzero; an unchanged zero pair is allowed before configuration. |
| Scalar 13 | Canonical nonzero operator. Owner and operator are not equated. |
| Root 2 | Exactly `[address,address]` uint256 allowance; both keys nonzero. |
| Roots 7,12,14 | Exactly `[address]`: canonical exemption/blacklist bools, or delegate address. Zero delegate is permitted. |
| **Root 15** | Exactly `[address,uint32]`, two words: offset 0 is canonical uint32 fromBlock and every recorded store must equal the current uint32 block number; offset 1 is uint256 votes. This is not an array or Trace208. |
| Roots 16,17 | Exactly `[address]`: uint32 count or uint256 nonce. Changing writes increment modulo the declared width, preserving Solidity 0.6.12 wrapping semantics. Equal records remain valid domain witnesses. |
| Excluded | Constructor-only name/symbol at 4/5 and every string payload word. |

Metadata key preimages require exact Keccak hashes, canonical address/uint32 keys,
exact nesting and exact terminal offsets. Outer anchors, adjacent words and extra
depth do not match. Balance/metadata ambiguity refuses. The mode does not reconstruct
global checkpoint/nonce/authentication state or validate a complete transfer, router,
signature or governance transaction. Source writer conditions are not inferred from
layout alone, and the historical proof's invalid all-ones metadata poison controls
are negative admission cases wherever they violate these domains.

For these two modes, the existing persisted-effect collector's no-op hook retains
every persisted storage record. Known balance no-ops emit nothing. Known metadata
no-ops still check constants, padding, widths and continuity. Unknown and excluded
constructor records refuse even when equal. Other profiles preserve their original
unknown-no-op filtering. Failed/reverted effects remain excluded under the unchanged
persistence rules. Any malformed metadata alongside a valid balance change rejects
the complete projection; no partial balance output is accepted.

## Bindings and offline evidence

The preparer pins 29 complete Phase A artifacts: both complete captures, original
and regenerated compiler inputs/outputs/metadata/version records, primary sources,
writer inventories, notices/licenses, release manifest and five proof reports.
The old source inventory stays historical. Candidate tools record a separate fresh
source inventory and compare their compiled-in as-run sources to current files, so
a stale executable cannot relabel newer source as its own.

Eight COOKIE dependency bodies are exact primary matches; flattened BNBTiger and
three custom COOKIE bodies retain attribution gaps. BNBTiger's exact two DEAD
immutable replacements and CBOR replacement remain the only allowed transforms.
COOKIE's full compiler/capture programs remain identical. Creation-byte equality
is not constructor execution or creation admission; Phase A's zero-call-value
execution scope is unchanged. Original paths, line endings, match labels and
licenses remain intact in the linked proof fixture.

The dedicated replay uses the complete saved BSC interval
**[122288006,122289030)** and the unchanged full canonical reference. It runs three
projections per block: original431, selected2 and combined433. Original431 must
equal the complete historical Events payload, including all **110,139** rows.
Combined433 restricted to the original contracts must equal that same payload;
its remainder must equal independently projected selected2. Only row ordering is
normalized. The two appended objects are removed to reproduce every original
profile field and array position exactly. This is a separate output, not a rewrite
of the historical stream or package.

Every selected emitted row needs a same-block full canonical match. An empty
retention ledger learns holders only from native emissions; reference-only rows
are classified as earlier-native retained matches or unknown cold observations.
Zero remains unknown until observed. Reports separate rows, distinct initialized
holders, canonical holders, known-zero holders and cold/nonzero observations.
Metadata counters describe admitted persisted field bits, not setter execution;
equal packed words cannot identify the unchanged field's writer. Original COOKIE
supply/packed-slot refusals are reproduced before candidate projection.

The [final saved replay](evidence/bnbtiger-cookie-candidates-20260929.json) passed
after integration on `3408f2b2c3cb7180698fb451835c93e7e3768315`. Preparation and replay
share the exact [287-file source inventory](evidence/bnbtiger-cookie-candidates-20260929-source-inputs.json),
SHA-256 `660b1414df2b4911648b197152926191d780b5ec43674b0d8c268f081bfc83b0`.
The report SHA-256 is `9cbe19646f84f9770ef0cf2ae34945d8566b16e9362c3beba34a4dbe62b1952d`.
Its finite counts are:

| Candidate | Same-block rows | Initialized observed holders | Retained-only matches | Cold observations | Cold nonzero | Canonical rows / holders |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| BNBTiger | 22 | 8 | 0 | 6 | 3 | 28 / 9 |
| COOKIE | 24 | 6 | 0 | 1 | 0 | 25 / 7 |

Each candidate ended with one known-zero initialized holder. The combined output
had 110,185 rows: the unchanged 110,139-row baseline plus 46 selected rows. COOKIE
had 13 changing metadata records: eight supply, two swap-lock, two transfer-tax
and one allowance record. BNBTiger had no metadata records. Neither had an equal
metadata record. These are observed persisted field changes, not proof of every
admitted setter or metadata shape. No reference value seeded the ledger.

The original block manifest is independently pinned at SHA-256
`56397f18ea7371231b261baefb10122c1439e604d130be2cbc97fca598e38ba6`.
Every raw protobuf file must match its ordered block/hash/digest record before
projection; unchanged headers alone cannot admit modified trace bytes.

Fresh `prepare-02`/`replay-02` evidence and the preceding successful preliminary
attempt remain under `out/bnbtiger-cookie-candidate-20260929/` in the review
worktree. All selected events and metadata records reproduce the preliminary
output exactly. Pinned, locked offline workspace validation passed: **1,336 tests
across 130 suites (86 nonempty), zero failed or ignored**, all-target Clippy with
warnings denied, formatting and the WASM workspace check. The focused additions
cover 12 projector groups, four binding groups and seven replay controls. No
production change followed the frozen final replay.

## Reproduction and qualification limits

From the workspace root, use a fresh ignored output for each attempt and a unique
target directory:

```sh
export CARGO_TARGET_DIR="$PWD/out/bnbtiger-cookie-candidate-20260929/target-isolated"
cargo build --locked --offline --release -p erc20-balances-tools \
  --bin prepare_bnbtiger_cookie_candidates --bin replay_bnbtiger_cookie_candidates
"$CARGO_TARGET_DIR/release/prepare_bnbtiger_cookie_candidates" out/new-prepare
"$CARGO_TARGET_DIR/release/replay_bnbtiger_cookie_candidates" \
  /path/to/original/erc20/balances out/new-replay
```

The replay requires pinned original block/reference/report caches and frozen
candidate fixtures. A refusal, missing oracle row, mismatch, clock gap, source
drift or malformed record stops the run with an explicit failed prefix; it is
never replaced by an empty successful block. Preserve every failed attempt.

Fresh deployed runtime/producer, packaged WASM/getter/holder and live checks remain
separate and require explicit authorization. Neither sample equality nor the
finite initialized set establishes universal token/global-holder support. No RPC,
Firehose, Substreams, sink, VM, protobuf or dependency change is part of this work.
