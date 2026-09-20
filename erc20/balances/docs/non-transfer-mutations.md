# Non-Transfer ERC-20 balance mutations

Issue [#22](https://github.com/pinax-network/substreams-evm-extended/issues/22)
carries forward the balance edge cases that the event-driven RPC modules in
`substreams-evm` handled as *discovery signals*: token-specific events whose
participants were queried with `balanceOf`. The Extended storage mapper does
not discover accounts from events; it emits the holder of every persisted
write to a qualified balance mapping. This page records, per token family,
which events are evidence of a balance mutation, which are only a hint that an
account is worth querying, and what saved data covers.

Reference inventory: [old candidate logic](https://github.com/pinax-network/substreams-evm/blob/970a665e15619de8ad7f686bd89412a1030d46dc/erc20/balances/src/lib.rs),
[token events](https://github.com/pinax-network/substreams-evm/blob/970a665e15619de8ad7f686bd89412a1030d46dc/erc20/tokens/src/lib.rs),
ABIs in `substreams-abis` v1.5.0 (`abi/tokens/erc20/{weth,usdc,usdt,WBTC,sai,stETH}`).
An `Approval`, a caller or a transaction sender is a discovery hint, never
evidence of a changed or initialized balance.

## Matrix

Columns: **event** (what the contract logs), **balance mutation** (which
`balanceOf` values change), **hint only** (events that identify an account but
change no balance), **storage coverage** (what the mapper does today),
**saved evidence**.

| Token / version | Event | Balance mutation | Hint only | Storage coverage | Saved evidence |
| --- | --- | --- | --- | --- | --- |
| WETH9 / WBNB (`0xbb4c…095c`, BSC) | `Deposit(dst, wad)` | `balanceOf[dst] += wad`; no `Transfer` | — | Qualified direct mapping (slot 3); emitted from the persisted write and verified preimage | 5 captured transactions ([`wbnb-mutations/cases.json`](../tests/fixtures/wbnb-mutations/cases.json)); 76 deposit and 93 withdrawal transactions without any WBNB `Transfer` in the saved 1,024-block window |
| | `Withdrawal(src, wad)` | `balanceOf[src] -= wad`; the contract's **native** balance falls by `wad` | — | same | same |
| | `Transfer`, `Approval` | ordinary | `Approval` | same; allowance writes are ignored mapping slot 4 | block 122260950, 18 rows |
| Binance-Peg BEP20 USDT (`0x55d3…7955`) / USDC (`0x8ac7…580d`) on BSC | `Transfer(0x0, to)` on `mint`, `Transfer(from, 0x0)` on `burn` | standard | `OwnershipTransferred` | Qualified profiles ([`bsc-reviewed-layouts.json`](../tests/fixtures/bsc-reviewed-layouts.json)); USDC behind a pinned proxy | block 122260950 and the refined450 window |
| Circle FiatTokenV2_2 (Ethereum `0xa0b8…eb48`, Base `0x8335…2913`) | `Mint(minter, to, amount)` + `Transfer(0x0, to)` | `balanceOf[to] += amount` | `MinterConfigured`, `MinterRemoved`, `MasterMinterChanged`, `BlacklisterChanged`, `PauserChanged`, `RescuerChanged`, `OwnershipTransferred`, `Pause`/`Unpause`, `AuthorizationUsed`/`AuthorizationCanceled` | **Not supported**: V2.2 packs the blacklist flag into the high bit of the balance word (`balanceAndBlacklistStates`); `balanceOf` masks it. `Blacklisted`/`UnBlacklisted` write the word without changing the balance, and `balance_bits` only accepts whole-byte widths, so a reviewed 255-bit rule is required before qualification | none: no saved Ethereum or Base Extended block |
| | `Burn(burner, amount)` + `Transfer(burner, 0x0)` | `balanceOf[burner] -= amount` | | same | none |
| Tether v0.4.18 (Ethereum `0xdac1…1ec7`) | `Issue(amount)` | `balances[owner] += amount`; no `Transfer`; owner from storage, not from topics | `Params` (fee rate), `Pause`/`Unpause`, `AddedBlackList`/`RemovedBlackList` (flag mapping) | Direct mapping in principle, but `Deprecate(newAddress)` redirects `balanceOf` to `upgradedAddress`, and transfer fees credit `owner` with a `Transfer`; unsupported until the runtime and the deprecation flag are bound | none |
| | `Redeem(amount)` | `balances[owner] -= amount`; no `Transfer` | | same | none |
| | `DestroyedBlackFunds(user, balance)` | `balances[user] = 0`; no `Transfer` | | same | none |
| Tether v0.8.4 (other chains) | `Mint(destination, amount)`, `Redeem(amount)`, `DestroyedBlockedFunds(user, balance)` | as above | `BlockPlaced`/`BlockReleased`, `NewPrivilegedContract`/`RemovedPrivilegedContract`, `OwnershipTransferred` | unsupported until bound per chain | none |
| Anyswap/Multichain swap asset (bridged USDT/USDC) | `LogSwapin(txhash, account, amount)`, `LogSwapout(account, bindaddr, amount)` | mint/burn of `account` | `LogChangeDCRMOwner` | unsupported until bound | none |
| WBTC (Ethereum) | `Mint(to, amount)` + `Transfer(0x0, to)`, `Burn(burner, value)` + `Transfer(burner, 0x0)` | standard | `MintFinished`, `Pause`/`Unpause`, `OwnershipTransferred`/`OwnershipRenounced` | direct mapping in principle; unsupported until bound | none |
| SAI (DSToken) | `Mint(guy, wad)`, `Burn(guy, wad)` | `balanceOf[guy]` | `LogNote`, `LogSetAuthority`, `LogSetOwner` | direct mapping in principle; unsupported until bound | none |
| stETH | `Submitted`, `TransferShares`, `SharesBurnt`, `ExternalSharesMinted`/`Burnt`, `TokenRebased` | share writes plus **global** rebases that change every holder's `balanceOf` without any holder write | | Not a direct mapping; owned by the [stETH balance-state adapter](https://github.com/pinax-network/substreams-evm-extended/issues/23) | none |

Every row for a token without saved Extended blocks records event semantics
from source and ABI review only. It is not qualification; each layout still
needs its own runtime binding, saved-data replay and, after live resumption,
RPC controls under [#8](https://github.com/pinax-network/substreams-evm-extended/issues/8).
The BSC reflection and calculated-balance candidates stay under
[#5](https://github.com/pinax-network/substreams-evm-extended/issues/5).

## What the saved WBNB fixtures establish

Five unmodified transaction traces from the retained BSC window, each with the
block header and identity ([provenance](../tests/fixtures/wbnb-mutations/cases.json)),
are replayed by `src/wbnb_mutation_tests.rs` and, for the native side, by the
`wrapper_backing` tests in `native/balances/tools`:

| Case | Shows |
| --- | --- |
| 122288015 tx 18 | `deposit()` by a contract five frames deep; `Deposit` only; the emitted holder is the storage owner, not the sender; the wrapper's native BNB rises by the same `wad` while the holder's own native balance nets to zero |
| 122288032 tx 47 | deposit then full withdrawal by a router; the holder ends at a **known zero** and is emitted as `0`; the wrapper's backing returns to its start |
| 122288035 tx 9 | `withdraw()` by a contract that is not the sender; `Withdrawal` only; the wrapper's native BNB falls by `wad` |
| 122288021 tx 35 | reverted transaction with a `Deposit` event, a `Transfer` event and balance writes: nothing persists, no ERC-20 row, no wrapper native row |
| 122288007 tx 22 | succeeded transaction whose reverted child repeats writes later persisted; a WBNB call made through Permit2 debits a holder that is neither sender nor caller; had the reverted writes been applied, continuity would have failed |

Wrapped-token units are ERC-20 balances of the holder. The BNB backing them
is the wrapper contract's native account balance, emitted by
`native/balances` with an absent contract. Neither the holder's WBNB nor its
redemption claim is a second native wallet balance, and the two packages
never emit the same fact twice.

## Provenance and cold observations

Storage-derived rows carry the block clock and the write's ordinal; the
mapper never turns an absent write into zero. RPC-only "cold" observations
from the historical candidate discovery remain separate evidence
([holder coverage](holder-coverage.md), [#7](https://github.com/pinax-network/substreams-evm-extended/issues/7)).
Unknown storage or absent holder evidence is reported as unknown; tests that
need a value are written against captured data, not adjusted to pass.

Standard `Transfer`/`Approval` event extraction is [#19](https://github.com/pinax-network/substreams-evm-extended/issues/19);
this page is the balance-correctness corpus, not an event decoder.
