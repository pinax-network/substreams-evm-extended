# BNBTiger and COOKIE: bounded getter proof

This host-only proof reconstructs the complete saved compiler and captured programs,
then executes `balanceOf(address)` in explicit synthetic storage. It establishes
neither production admission nor a deployed-state/producer qualification. There is
no new candidate, parser, protobuf, persistence, dependency or VM change.

| Target | Saved BSC address | Balance root | Compiler / settings | Saved runtime |
| --- | --- | --- | --- | --- |
| BNBTiger | `0xac68931b666e086e9de380cfdb0fb5704a35dc2d` | 7 | 0.8.4+c7e474f2; optimizer enabled, 200 runs; original default EVM target | 10,594 bytes; `18f0619e94b94d884d917414ff4248c11e327998aef6a3e7089b9977e6ecef1c` |
| COOKIE | `0x3505bee89d3b4e351dbd4849241a6b0716ea407f` | 1 | 0.6.12+27d51765; optimizer disabled, 200 runs; Istanbul | 23,518 bytes; `258d1c6c6dcac1bfb46886ed147f9445408e9e5420a2c7a9434cd1b60bcf200f` |

The original raw captures are pinned before parsing. All source bodies, original
paths, complete settings, source IDs, ABI, metadata, storage layout, runtime and
creation bytecode/source maps remain bound. Compiler input adds only output
selection. Official compiler binaries are bound by SHA-256 and Keccak to the
immutable release manifest. Complete fresh compiler outputs are separately pinned.
Both fresh raw metadata strings equal their captured counterparts. BNBTiger's
fresh generated Yul source is retained with its compiler output; it is not
invented Solidity attribution.

BNBTiger retains its original `match` labels. Its runtime reconstruction applies
only two exact 32-byte zero-placeholder replacements (offsets 1,347 and 3,534)
with the declaration's padded `0xdead`, plus the exact 53-byte CBOR replacement
at offset 10,541. Creation has only that CBOR replacement at offset 12,923.
COOKIE retains `exact_match`; both complete programs have no replacements,
links, immutables or appended constructor arguments. Complete reconstructed
creation bytes are compared to saved captures, but constructors are not executed
and this proof does not authorize creation admission. The cause of BNBTiger's
metadata digest substitution remains unestablished.

COOKIE's eight full dependency files byte-match immutable OpenZeppelin 3.4.2,
Uniswap v2-core and v2-periphery bodies. The committed primary manifest includes
exact URLs, bodies and hashes and all three upstream LICENSE files. This does not
identify a unique historical import commit. The three custom COOKIE files
(`CookieToken.sol`, `BEP20.sol`, `IBEP20.sol`) and flattened BNBTiger origin remain
unestablished. COOKIE's original Windows-style absolute paths and all original
line endings are preserved. BNBTiger's original `Unlicensed` notice is retained;
no explorer UI label or similar dependency is used to relicense it.

## Getter and metadata controls

Both the unpatched compiler runtime and captured runtime execute each case. The
proof requires identical complete traces, effects and outcomes and checks that no
executed opcode/immediate or code-copy operation consumes a substituted region.
For a valid getter it requires exactly one `SLOAD` and one exact Keccak preimage
`pad(holder) || pad(balance_root)`, returning that exact uint256 word. There can be
no additional storage reads, writes, logs, caller/context dependence or external
call. Full synthetic storage remains unchanged, including explicit known-zero
cells. The one storage-read PC is attributed to the literal balance getter in
the full selected source.

The balance domain covers 0, 1, 123 and uint256 maximum for zero, dead, self and
ordinary holders, with two callers (synthetic owner and non-owner). Independent
controls alter every declared metadata shape at selected keys/words, including
packed members, nested allowances, both checkpoint words and hypothetical long
string head/data words. Narrow fields use their exact bit width and offset while
preserving neighboring bits. Some controls deliberately create invalid metadata
or inconsistent supply; they establish only raw getter independence, not
reachable state or valid writes. Other-holder balances are also varied.

Malformed calldata controls cover every short prefix of a canonical call, an
unknown selector, trailing calldata and dirty high address bits. Solidity 0.8.4
rejects BNBTiger's dirty address; COOKIE's 0.6.12 decoder masks it. Empty calldata
executes the selected receive path and returns empty bytes. These are measured
source-specific outcomes. All calls have **zero call value**, the unchanged VM's
only value context. Nonzero-value behavior is not tested. REVERT, INVALID and
HarnessFailure are distinct; an unexpected or unsupported exit fails the proof,
rather than counting as an equivalent source failure.

## Field-level writer review

The complete writer-review artifact covers every storage declaration with exact
slot/byte offset, source hash, literal writer anchors and reachability notes. It
is a manual whole-source review, not an inferred permission from layout alone.

| Target | Field group | Selected-source behavior and boundary |
| --- | --- | --- |
| BNBTiger | owner / previous owner / lock time | Ownership functions and time-dependent lock/unlock; only source-reviewed here. |
| BNBTiger | name, symbol, decimals, total supply | Declaration/constructor-only; no runtime setter or mint/burn. Short literal strings do not establish long-string admission. |
| BNBTiger | marketing wallet at slot 5 byte 1, team wallet | Owner setters; slot 5 also contains constructor-only decimals and padding. |
| BNBTiger | fee/limit/exemption/pair maps and scalars | Exact owner setters plus constructor writes; buy/sell/distribution totals are recomputed. |
| BNBTiger | router, pair, swap flags | Router/factory operations need external context; swap entry/exit updates packed flags. No successful router path is executed. |
| COOKIE | name, symbol, decimals, max holding rate | Constructor/declaration-only. Decimals share slot 6 with tax rates; max holding rate shares slot 11 with pair and swap flag. |
| COOKIE | balances, allowances, supply | Inherited and derived mint overloads are reachable; only derived `mint(address,uint256)` updates delegates. Internal `_burn`/`_burnFrom` have no public caller in the complete selected source; a burn-address transfer does not reduce supply. |
| COOKIE | tax/limit rates, exemptions, blacklist, operator | Operator setters with source bounds; tax-free modifier temporarily clears/restores tax. Blacklist inclusion reads raw balance and holding limit. |
| COOKIE | router, pair, swap flag | Operator router update performs external reads; swap modifier toggles the flag. Those success paths are excluded. |
| COOKIE | delegates / checkpoints / counts / nonces | Delegate/signature/mint paths; checkpoint root 15 has `[address,uint32]` keys and two words (`uint32 fromBlock`, `uint256 votes`). `safe32` checks block conversion; the uint32 count increment wraps under 0.6.12. Block/signature context is not executed. |

A future candidate needs a separate review of the exact changed bits for runtime
fields sharing words with constructor-only fields or padding. Getter independence
is not permission to admit the whole scalar word. Neither dynamic strings nor
arbitrary mapping depths/keys become allowed by this proof.

## Evidence and limits

The frozen [compiler report](evidence/bnbtiger-cookie-getter-proof-20260929-compiler.json)
and [execution report](evidence/bnbtiger-cookie-getter-proof-20260929.json) passed
with the same [269-input source inventory](evidence/bnbtiger-cookie-getter-proof-20260929-source-inputs.json).
The [complete case manifest](evidence/bnbtiger-cookie-getter-proof-20260929-cases.json)
binds every raw/annotated trace; [compact transcripts](evidence/bnbtiger-cookie-getter-proof-20260929-transcripts.json)
preserve calldata, synthetic prestate, expected output and exact read/Keccak witnesses.

| Program | Calls | Returns | Empty reverts | Exact compiler/capture pairs |
| --- | ---: | ---: | ---: | ---: |
| BNBTiger | 816 | 668 | 148 | 408 |
| COOKIE | 624 | 480 | 144 | 312 |
| Total | 1,440 | 1,148 | 292 | 720 |

The matrix records 176,752 executed instructions, 1,140 balance reads and 1,140
Keccaks, with all 2,280 effect source spans resolved and zero stores/logs. It
contains 128 balance-domain calls, 992 selected metadata-word controls, eight
other-holder controls and 312 ABI controls. These are synthetic program calls,
not chain observations. The 36 BNBTiger and 23 COOKIE declarations yield 37 and
25 selected metadata cells respectively, after excluding the balance root and
including the second checkpoint word and hypothetical string payload words.

Source-05 and operations-01 are the final frozen attempts. Earlier source-01
(unavailable older dependency path), source-02 (literal writer-anchor spacing),
source-03/04 (preparatory binding runs), build/focused-test/lint failures and their
as-run bytes remain under `out/bnbtiger-cookie-getter-proof-20260929/`.
The original parent-RPC transport failure and successful saved follow-up stay
historical; no new chain call was made.

Fourteen focused Rust tests pass. On base
`374dce16919c5a2c8fbefdf44473534dbb64450e`, pinned locked offline workspace
library/binary/integration tests pass: **1,284 tests in 122 suites (80 nonempty),
zero failures or ignored tests**. Formatting, all-target Clippy with warnings
denied and the WASM workspace check also pass. The isolated target and complete
logs remain with the proof attempts. These gates are separate from
runtime/package qualification.

This work has no canonical replay or observed-holder coverage claim. Transfer,
router/factory, signature, block/time context and nonzero call value success paths
are excluded. A separate Phase B must review explicit metadata permissions and
runtime/deployment guards, then perform a complete canonical-bound saved replay.
Fresh package/producer/live qualification remains a separate gate.
