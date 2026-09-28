# BSC exclusion review: offline preparation, 2026-09-28

Issue [#61](https://github.com/pinax-network/substreams-evm-extended/issues/61)
remains open. This review prepares one **unqualified TAKE candidate** and
preserves all six exclusions from the September 23 package report. It changes
neither the historical 431-profile baseline nor the qualified 425-profile
package, package digest, reference, or original failed reports.

The only production change is a storage-width validation fix: persisted keys,
old values, and new values wider than 32 bytes now refuse for configured tokens
and their beacons before metadata is ignored. This includes no-op records.
Existing persistence rules still discard failed and reverted effects. Compact
uint256 encodings remain supported. No protobuf, module, layout grammar, or
shared/embedded persistence rule changed.

## Evidence boundary

The host-only Rust `review_bsc_exclusions` tool reads saved Extended blocks and
the saved immutable canonical RPC reference; it makes no network requests.
The new [report](evidence/bsc-exclusions-20260928/report.json) binds its inputs,
implementation files, candidate, source bundles, and output inventory by SHA-256.
The committed [transaction fixtures](../tests/fixtures/bsc-exclusions-20260928/README.md)
retain three whole decoded original transactions and their original block headers; they
are trimmed fixtures, not complete historical blocks. Regression tests reproduce
each original refusal before applying the TAKE candidate. Synthetic mutations
are confined to tests and are not claimed as observed chain behavior.

On the **1,024 saved blocks [123561000, 123562024)**, every Extended block hash
matches the saved canonical clock and consecutive parent hashes link. The TAKE
candidate emits **8,436 rows**, including **3,198 zero rows**; all 8,436 match
the saved same-block canonical reference, with zero missing comparisons and zero
refusals. An emitted-only consumer ledger initializes **207 observed holders**
with no checkpoint or reference seeding; it has 107 known-zero holders at the
end. There are 8,466 known reference observations that match retained state and
6,586 cold observations that remain unknown, including 990 nonzero observations.
These are observation counts, not distinct additional holders. The result does
not establish complete global holders, current runtime identity, a newly built
WASM package, or fresh live qualification.

## TAKE: one getter-independent guard slot

Contract `0xe747e54783ba3f77a8e5251a3cba19ebe9c0e197` refused at block
123561001, transaction index 66. Delegate call 30 executes implementation
`0xa9709c4caf6e3fb5fd62b11b53b3f8b393c8927d` while writes retain the proxy's
storage address. The captured mint writes the guard 1→2, total supply, recipient
balance 0→100000000000000000000, then guard 2→1. The single new permission is:

```text
0x9b779b17422d0df92223018b32b4d1fa46e071723d6817e2486d003becc55f00
```

This is OpenZeppelin's ERC-7201 `openzeppelin.storage.ReentrancyGuard` slot.
The [maintainer's TokenImpl source](https://github.com/overtake-dev/take-token-repo/blob/7cfcaf7a2429a83612b92dc5fa2f42b551b600e9/bsc/src/TokenImpl.sol)
inherits the pinned OpenZeppelin implementation and does not override
`balanceOf`. Its inherited getter reads only the ERC20 `_balances[account]`
mapping. The guard word is read by the reentrancy modifier and its internal
status getter; neither is reachable from `balanceOf`. `mint` and `burn` use
`nonReentrant`; role, pause, and transfer-mode checks control whether execution
succeeds. They do not replace the persisted `_balances` value returned by the
getter. This review adds no other administration or role permissions.

The independent pinned TokenImpl, ERC20, and ReentrancyGuard files have exactly
the same SHA-256 values as the saved complete 23-file compiler bundle. The saved
solc 0.8.29 compiler output reconstructs the profile's implementation runtime
hash `0x5f5b08388e6393a54c2f3666232213d298f6bee046ccf2c3631b99416bbe2f49`
after **only** the three declared 32-byte immutable replacements at offsets
6776, 6861, and 7308. Each replacement is the implementation address padded to
32 bytes. The complete reviewed capture SHA-256 is pinned before parsing any
fields, preventing an empty source set or altered immutable schema from being
accepted as the reviewed artifact. Captured chain id 56 and contract/implementation address, source
hashes, allowed transformation kind, declared offsets/lengths, runtime bytes,
and runtime hash are checked offline. This is verification of a
saved compilation artifact, not a fresh compiler invocation. Full pins and hashes
are recorded in the [research note](../../../docs/research/09-BSC_exclusion_candidates.json).

The candidate preserves the existing proxy hash
`0x2b5c23057264b2b56cb20607c6f7e9abf8b7d659de7b4f42094ff993d421a54b`,
implementation address/hash, balance root, and every other original field.
`other_slots` asserts getter independence, not valid guard transitions. Valid
uint256 values other than 1/2 remain metadata and cannot fabricate balances.
Tests cover adjacent unknown slots, oversized key/old/new/no-op values, failed
and reverted calls, exact delegate storage context, implementation code changes,
and a changed-then-restored implementation pointer. Runtime identity before the
interval still requires the host qualification gate; the stateless mapper cannot
prove a preexisting runtime from a block with no code change.

## Five cases still excluded

| Contract | Saved evidence and conclusion | Remaining work |
| --- | --- | --- |
| TOPS `0xcdf52c0b13c24f32f1d8d4ec6356203a1ef0826a` | Block 123561227, tx 63: the refused key is element **4**, field **0** (`lpAmount`) of the three-word `LPInfo[]` in `lpInfos[0x28ebbb1c3003a30f9e627647862dd85aa4c165ce]`, mapping root **31**. Captured source/runtime reconstruction and Keccak preimages establish this exact path. | A reviewed mapping-to-dynamic-struct-array rule must validate index/length, three-word bounds, append, shift/pop, aliasing, malformed and reverted effects; a one-off computed-slot allowlist is insufficient. An independent maintainer source pin is also not established. No candidate is added. |
| RADR `0xf08d1886e2a69dfabd22a45eed8e1407b338b97e` | Block 123561119, tx 53: selector `0x30a0b5b3` writes mapping root **21**, key **0x1c0ebd**, 0→1. Saved source capture reports no match. Existing sampled balance getter evidence for root 0 does not prove that root 21 is harmless on all reachable getter paths. | Obtain and bind source, then review getter invariance and write reachability. Public searches did not identify a maintainer source repository. No new mapping permission is added. |
| BNC4 `0x7c8d5502b544ddaf8852fc46d1174e34876d545c` | September 23 runtime gate rejected the beacon implementation slot. Old evidence reads slot 1 of beacon `0x453f0bfbb8c46bfe5ae029099d587bc46883ed27` and returns the old implementation `0x2781ba79f733ceda7467d2dd21688823d89ad53d`. Its saved source capture is unverified. | Identify and qualify the replacement implementation, source/runtime binding, getter and storage semantics at explicit boundaries. The old getter evidence cannot identify the newer implementation. No repin. |
| sPro `0xdfe1308fb3ef1dc2f87b4ffaef5ebdf80be1e4ea` | September 23 runtime gate rejected divisor slot **11**. Old getter evidence reads balance mapping root **12** and divisor slot 11. Saved source is unverified. | Requalify the dependency and account for all initialized holders whose amount changes without their mapping being written. A changed divisor cannot be treated as metadata or silently repinned. |
| swkeyDAO2 `0x009b797edf9acc666a36020c4509c6095de51408` | September 23 gate rejected divisor slot **9**; balances use root **10** through implementation `0x7589b8d477961e9a0995688c18a4974a254dd08e`. Saved source verifies the transparent proxy only. Historical controls show integer division, including rounding and zero-divisor reverts. | Bind implementation semantics and qualify dependency changes with an explicit holder-state rebuild or supported companion state model. Proxy source does not establish implementation source; no repin. |

TOPS's captured getter is simply `_balances[account]` at root 5. Its miner-only
`createLPInfo` appends a record for a nonzero user/amount;
`_processExpiredLPInfo` is called by transfers, shifts surviving records, pops
expired records, and adds their amounts to `lpAmount` root 32. These facts explain
the observed metadata clear without implementing the required array rule. The
bound captured runtime hash is
`0xf4dc8abcac62d4707f5a839731c0e2b3dde17c643accf43299541c421ba0050b`.

The [unresolved evidence](evidence/bsc-exclusions-20260928/unresolved.json)
retains original profiles, source-capture hashes/status and old getter reports
for RADR/BNC4/sPro/swkeyDAO2. Historical runtime exclusions remain in the
unchanged [September 23 report](evidence/live-2026-09-23/runtime-status.json).
These observations are historical; this work did not query current state.

## Reproduction and remaining qualification

From the workspace root, with the ignored original cache available:

```sh
CARGO_TARGET_DIR="$PWD/target/issue61-validation" \
  cargo test --locked --offline -p erc20-balances --test bsc_exclusion_candidates
CARGO_TARGET_DIR="$PWD/target/issue61-validation" \
  cargo run --locked --offline -p erc20-balances-tools --bin review_bsc_exclusions -- \
  /path/to/original/erc20/balances out/bsc-exclusion-review-new
```

The output directory must be fresh. The diagnostic requires the original
September 23 `firehose-blocks` and `ranking` directories and the source/getter
captures named in the committed evidence. It never invokes a network client.
The narrow committed transaction/source regression suite does not require this
ignored cache. Earlier local runs and the initial malformed-word regression
failure are recorded in [attempts](evidence/bsc-exclusions-20260928/attempts.md).
Use a worktree-local Cargo target directory: earlier shared-target validation
was invalidated after another worktree's test binaries were reused. The final
report comes from a fresh isolated build. During replay, a refusal, mismatch or
missing emitted-row reference stops processing, preserves partial output/report
and returns failure; a refusal never becomes an empty successful block.
Final formatting, 735 workspace tests, Clippy and WASM checks are recorded in
[validation](evidence/bsc-exclusions-20260928/validation.md).

Before TAKE can join a separately qualified profile set, explicit live
authorization is still needed for fresh boundary runtime/dependency checks,
refusal scan, canonical comparison, direct audit, holder coverage, and packaged
WASM validation over an agreed interval. Retained-holder initialization and
bootstrap completeness must be stated separately. This offline candidate and
parser fix do not close issue #61 or authorize any live check.
