# Extended extraction coverage: chains, assets, actions

This document records the first supported extraction surface for the
[roadmap](https://github.com/pinax-network/substreams-evm-extended/issues/21)
(issue [#11](https://github.com/pinax-network/substreams-evm-extended/issues/11)).
It fixes requirements and evidence for the focused Extended-block packages; it
does not implement a wallet service, prices, portfolio calculations, an RPC
fallback or a universal labeling claim. The consumer owns its database,
wallet-relative interpretation and business rules.

Every address, chain id and formula below was read from the cited official
source on 2026-09-18 at the stated pin. Addresses are deployment identities,
not runtime qualification: binding a runtime code hash, storage layout and
activation block is part of each implementation issue and of network
qualification under [#8](https://github.com/pinax-network/substreams-evm-extended/issues/8),
and requires RPC or Firehose access that stays paused until explicitly
resumed. Facts that could not be confirmed from an official source are listed
as open questions rather than filled in.

## 1. Chains

| Network | Chain id | Native asset (raw unit) | Finality model | Block cadence | Extended data |
| --- | --- | --- | --- | --- | --- |
| BNB Smart Chain | 56 | BNB, 18 decimals | Parlia PoSA with BEP-126 fast finality: a block is final when it and its child are justified by ≥2/3 validator votes, "within two blocks in most cases"; `finalized`/`safe` tags plus `eth_getFinalizedHeader`. Fast finality is meaningful from the Plato fork (block 30,720,096). | 450 ms since Fermi (2026-01-14); 3 s → 1.5 s (Lorentz, 2025-04-29) → 750 ms (Maxwell, 2025-06-30) before that | Saved captures at producer versions 4 and 5; `bsc.substreams.pinax.network:443`, `bsc.firehose.pinax.network:443` documented |
| Ethereum mainnet | 1 | ETH, wei | Gasper: 12 s slots, 32-slot epochs, finality after two justified epochs (≈12.8 min); `finalized`/`safe` tags per execution-apis | 12 s slots | No saved capture; `eth.substreams.pinax.network:443`, `eth.firehose.pinax.network:443` documented |
| Base mainnet | 8453 | ETH, wei (bridged; minted by deposit transactions) | OP Stack: unsafe (sequencer, ~2 s), safe (derived from L1), finalized (derived from finalized L1, ≈20 min); an L1 reorg does not by itself reorg L2 | 2 s | No saved capture; `base.substreams.pinax.network:443`, `base.firehose.pinax.network:443` documented |
| HyperEVM mainnet | 999 | HYPE, 18 decimals | HyperBFT (HotStuff-derived), one-block finality inherited from consensus, commit usually within 2 blocks; no `safe`/`finalized` tag semantics documented | Dual blocks: small every 1 s (3M gas), large every 60 s (30M gas), one interleaved block-number sequence | No saved capture; `hyperevm.substreams.pinax.network:443`, `hyperevm.firehose.pinax.network:443` in The Graph Networks Registry (`firstStreamableBlock` 0) |
| Arc mainnet | 5042 (testnet 5042002) | USDC, 18-decimal native representation; the same balance is exposed by the ERC-20 interface `0x3600000000000000000000000000000000000000` at 6 decimals | Malachite BFT with a permissioned validator set; finality on inclusion, sub-second, "no reorganization risk"; `safe`/`finalized` tag semantics not documented | No fixed interval; one-second timestamp granularity, non-decreasing, so blocks may share a timestamp | No saved capture; `arc.substreams.pinax.network:443`, `arc.firehose.pinax.network:443` in the registry (`firstStreamableBlock` 0); mainnet launched 2026-09-16 |

Sources: BNB Chain docs and `bnb-chain/bsc@c5533ab5b724` (`params/config.go`, `consensus/parlia/parlia.go`, `core/state_transition.go`), BEP-126; ethereum.org PoS docs and `ethereum/execution-apis@465d1b98d43e`; Optimism specs and Base docs, `base/base@d8a2a42398cc`; Hyperliquid docs (HyperEVM, dual-block architecture, HyperCore↔HyperEVM transfers); Arc docs (`connect-to-arc`, `stablecoin-native-model`, `consensus-layer`, `evm-differences`); The Graph Networks Registry v0.7.122; Substreams chains-and-endpoints page. Endpoint hostnames are recorded from documentation only; none was contacted.

The requested list is these five networks. [#8](https://github.com/pinax-network/substreams-evm-extended/issues/8)
names four; BSC is the network with saved evidence. Additional chains need
explicit confirmation before expanding that task.

### Historical depth

Historical depth is a per-endpoint property, not a chain property. The
registry states `firstStreamableBlock` 0 for HyperEVM and Arc; the BSC,
Ethereum and Base Extended depth available from the documented endpoints is
recorded as an open question for [#8](https://github.com/pinax-network/substreams-evm-extended/issues/8).
Two saved BSC facts bound what offline work can claim: the earliest saved
Extended block is 51,995,162 (producer version 4) and the densest saved window
is [122288006, 122289030) (version 5).

### Execution-layer specifics that affect balance semantics

- **BSC**: transaction gas fees and, since Cancun, blob fees are credited to
  the system address `0xffff…fffe`; at block end Parlia resets that balance to
  zero and redistributes it through system transactions to `0x…1000`
  (ValidatorSet) and `0x…1002` (SystemReward), with BEP-95 burning a share to
  `0x…dead`. Block 122260950 shows the reset as a block-level record at
  ordinal 3244 after the last transaction credit at 3237. Blob-fee rewards
  appear as `REASON_REWARD_BLOB_FEE` (17). Genesis/system contracts occupy
  `0x…1000`–`0x…1008`, `0x…2000`–`0x…2006` and `0x…3000`.
- **Ethereum**: EIP-4895 withdrawals are system-level balance increases in
  Gwei, processed after all transactions, not calls; EIP-1559 base fees and
  EIP-4844 blob fees are burned and never appear as a recipient credit;
  EIP-6780 limits SELFDESTRUCT deletion to same-transaction creations; EIP-4788
  and EIP-7002 system calls run from `0xffff…fffe` outside transactions;
  EIP-7702 (Pectra, block 22,431,084) applies valid authorizations even when
  the transaction fails. Consensus-layer validator balances are a separate
  source domain.
- **Base**: type `0x7E` deposit transactions increase the `from` balance by
  `mint` unconditionally, including on failed execution; base, priority, L1
  data and Isthmus operator fees are paid into predeploy vaults
  (`0x42…0019`, `0x42…0011`, `0x42…001a`, `0x42…001B`) rather than burned, and
  "fee payments are not registered as internal EVM calls". The BSC failed-
  transaction gas-only rule is therefore not transferable without Base
  producer fixtures ([native/balances persisted-effect matrix](../native/balances/docs/persisted-effects.md)).
- **HyperEVM**: base and priority fees are burned, the priority fee being
  credited to the zero address; HYPE and linked tokens cross the
  HyperCore↔HyperEVM boundary through system addresses (`0x2222…2222` for
  HYPE, `0x20…` plus token index otherwise) via system transactions. EVM
  balances do not describe HyperCore spot, perpetual or staked positions.
- **Arc**: native and ERC-20 USDC are one balance at two precisions; amounts
  below 10⁻⁶ USDC exist only natively. Plain native sends emit ERC-20-style
  `Transfer` logs from the system emitter `0xffff…fffe`, while `0x3600…0000`
  emits only for ERC-20-interface activity. Transfers to the zero address and
  burning are rejected at runtime, the base fee is paid to the beneficiary
  rather than burned, and EIP-4895 withdrawals are always empty. Extraction
  must preserve both raw observations with an explicit alias identity, not two
  assets ([#12](https://github.com/pinax-network/substreams-evm-extended/issues/12)).

### Required Extended producer fields

From `sf.ethereum.type.v2` at `streamingfast/firehose-ethereum@9485efe2e6290e525fd4978b50462516ec752672`:

- `Block.detail_level == DETAILLEVEL_EXTENDED` (the zero value; BASE blocks
  leave every field below empty), `Block.ver`, `Block.hash`, `Block.number`,
  `header.parent_hash`, `header.timestamp`, `header.state_root`.
- Per transaction: `status` (1 succeeded, 2 failed, 3 reverted), `index`,
  `hash`, `from`, `to`, `nonce`, `type`, `set_code_authorizations`,
  `begin_ordinal`/`end_ordinal`, and `calls[]` with `index`, `parent_index`,
  `depth`, `call_type`, `caller`, `address`, `address_delegates_to`, `value`,
  `input`, `return_data`, `status_failed`/`status_reverted`/`failure_reason`,
  `state_reverted`, `begin_ordinal`/`end_ordinal`, `storage_changes`,
  `balance_changes`, `nonce_changes`, `code_changes`, `logs` and
  `keccak_preimages` (preimages of 256 bytes or less only).
- Block scope: `system_calls[]`, `Block.balance_changes`, `Block.code_changes`.
  `Block.withdrawals` is marked experimental in the proto and is not relied on.
- Persistence rule (proto documentation): a succeeded transaction persists
  every call with `state_reverted == false`; a failed or reverted transaction
  persists only root-call balance changes with reasons `GAS_BUY`,
  `GAS_REFUND`, `REWARD_TRANSACTION_FEE` and the sender nonce, plus accepted
  EIP-7702 authorizations. Implemented in [`common/persist`](../common/persist).
- Producer versions: version 3 recorded system-call ordinals on a different
  scale from transaction ordinals and zeroed root-call `begin_ordinal`
  (tracer comments, CHANGELOG v2.10.0); cross-scope ordinal reduction requires
  version 4 or 5. Version 5 additionally drops no-op state changes and gas
  changes; it is what the saved BSC captures carry, while its semantics are
  documented only in the Rust tracer releases (`evm-firehose-tracer-rs`
  v5.x) and the unreleased Go tracer line, not yet on the Substreams data
  model page. Each package lists the versions it accepts explicitly; the
  block carries no chain id, so the manifest binds the network.
- Reason enum through value 20: `REWARD_BLOB_FEE` 17 (BNB), `INCREASE_MINT`
  18 and `REVERT` 19 (Optimism family), `MONAD_TX_POST_STATE` 20. The pinned
  `substreams-ethereum` 0.11.1 bindings name values 0–16 only.

### Stream contract

Substreams delivers `BlockScopedData` with a `clock` (number, hash,
timestamp) and an opaque `cursor`; a fork produces `BlockUndoSignal` with
`last_valid_block` and `last_valid_cursor`, and every output above that block
must be discarded. `final_blocks_only` suppresses undo signals by streaming
only irreversible blocks. Outputs carry no block hash; the clock is the block
identity, and a block with empty output is a delivered block, not a gap.
Consumers persist cursors; sink cursor progress is not a complete-block
publication manifest.

## 2. Asset and balance scopes

"Wallet balances" is not one scope. The table records each family as
**required** (named by the roadmap and owned by an issue), **deferred**
(inventoried, not confirmed as requested) or **unsupported** (outside
Extended EVM extraction).

| Family | Status | Owner | Boundary |
| --- | --- | --- | --- |
| Native account balances | Required | [#17](https://github.com/pinax-network/substreams-evm-extended/issues/17) (`native/balances`) | Final persisted balance per changed account; absence is not zero; chain semantics under #8 |
| ERC-20 `balanceOf` from storage | Required | `erc20/balances`, [#4](https://github.com/pinax-network/substreams-evm-extended/issues/4)–[#7](https://github.com/pinax-network/substreams-evm-extended/issues/7) | Explicitly qualified layouts only; reflection/calculated tokens under [#5](https://github.com/pinax-network/substreams-evm-extended/issues/5) |
| Wrapped native and nonstandard ERC-20 mutations (WETH/WBNB, USDC, USDT, WBTC, SAI) | Required (regression corpus) | [#22](https://github.com/pinax-network/substreams-evm-extended/issues/22) | Holder units stay ERC-20 balances; wrapper backing is the wrapper's native balance, not a second wallet asset |
| Aave V3 aToken scaled balance + reserve index/rate/timestamp | Required | [#13](https://github.com/pinax-network/substreams-evm-extended/issues/13) | Selected epochs only (§3); variable-debt tokens need their own semantics |
| Compound v2 cToken shares + exchange-rate inputs | Required | [#14](https://github.com/pinax-network/substreams-evm-extended/issues/14) | Ethereum only; cash dependency is cross-contract (§3) |
| Compound III signed principal + market indices | Required | [#15](https://github.com/pinax-network/substreams-evm-extended/issues/15) | Ethereum and Base; not on BSC; supplied balance only, collateral positions separate |
| stETH shares + global rebase state | Required | [#23](https://github.com/pinax-network/substreams-evm-extended/issues/23) | Ethereum; one Lido contract-version epoch at a time (§3) |
| ERC-4626 shares + source-bound conversion | Required (selected vault) | [#24](https://github.com/pinax-network/substreams-evm-extended/issues/24) | Per-implementation conversion; interface conformance is not a formula (§3) |
| ERC-721 ownership and holder counts | Deferred | none yet | Transfer history is not a verified ownership snapshot; constructor and consecutive mints bypass events. Open an ownership issue only after confirmation |
| ERC-1155 quantities | Deferred | none yet | Key `(contract, id, holder)`; batch events, self-transfers and zero values; event deltas do not initialize balances |
| Debt balances (Aave variable debt, Comet negative principal, Compound v2 borrow snapshots) | Deferred | #13/#14/#15 boundaries | Signed or scaled debt is never an unsigned ERC-20 amount; needs explicit request and its own rounding rules |
| Collateral positions, LP underlying claims, withdrawal-queue NFTs, staked/escrowed amounts | Deferred | none yet | Protocol positions need protocol state; an ERC-20/NFT holding is not a DeFi position |
| Underlying-equivalent claims (`balanceOfUnderlying`, `convertToAssets`, `getPooledEthByShares`) | Required as **separate metrics**, never substituted for `balanceOf` | #12, #16 | Stored vs projected vs withdrawable are distinct results |
| Native/ERC-20 aliases (Arc USDC) | Required identity linkage | #12, #8, #17 | One balance, two precisions; never two assets |
| Consensus-layer validator balances, HyperCore spot/perp/staked balances | Unsupported | — | Different source domains; recorded as exclusions |

## 3. Initial protocol markets, implementations and epochs

A protocol brand is not a runtime identifier. Each selection below names the
deployment identity read from an official registry at a pinned commit and the
source revision whose semantics the reference models will follow. The
deployed runtime code hash and the activation block of each implementation
must still be bound from chain data before qualification.

### Aave V3 (issue #13, actions #20)

Address book `bgd-labs/aave-address-book@4e13aa197ca74e84c7e878bc752e519c260d6f30` (v4.68.1, 2026-09-16); source `aave-dao/aave-v3-origin@8305565ae342f1773c42cd2e4593f175fe5968a0` (v3.7 codebase, `ATOKEN_REVISION = 5`, `POOL_REVISION = 11`).

| Network | Pool proxy | Pool impl (snapshot) | Default aToken impl (snapshot) | First aTokens | Underlying decimals |
| --- | --- | --- | --- | --- | --- |
| BSC | `0x6807dc923806fE8Fd134338EABCA509979a7e0cB` | `0x5e2B0FcC5b9734C7Ec0A03401ee9e6805F783B6d` | `0x7e199Fc666368d95B9EaEfA7D2d8081AcAb74134` | USDT `0xa9251ca9DE909CB71783723713B21E4233fbf1B1` (underlying `0x55d398326f99059fF775485246999027B3197955`), USDC `0x00901a076785e0906d1028c7d6372d247bec7d61` (underlying `0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d`) | 18 and 18 on BSC |
| Ethereum (Core market) | `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2` | `0x728a138A4823392C2EFA55e028d434F526fE03CF` | `0xadC45Df3cf1584624C97338BEF33363BF5b97AdA` | USDC `0x98C23E9d8f34FEFb1B7BD6a91B7FF122F4e16F5c` (underlying `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48`), USDT `0x23878914EFE38d27C4D67Ab83ed1b93A74D4086a` (underlying `0xdAC17F958D2ee523a2206206994597C13D831ec7`) | 6 and 6 |

- **First epoch**: BSC USDT and USDC aTokens under the v3.7 implementation
  (aToken revision 5), because BSC is the only network with saved Extended
  evidence. Ethereum Core follows under #8. Lido and EtherFi Ethereum markets
  are separate Pools and are not selected.
- **Getter**: `balanceOf(user) = scaledBalance.rayMulFloor(POOL.getReserveNormalizedIncome(asset))`
  since v3.5 (`TokenMath.getATokenBalance`); `getNormalizedIncome` returns the
  stored `liquidityIndex` when `lastUpdateTimestamp == block.timestamp`, else
  `calculateLinearInterest(currentLiquidityRate, lastUpdateTimestamp).rayMul(liquidityIndex)`
  with half-up `rayMul`. Original V3 (revisions ≤3) rounds half-up; v3.5
  (revision 4) introduced floor for `balanceOf`/`totalSupply`, floor on mint,
  ceil on burn and transfer scaled amounts. The reference model must carry
  the revision.
- **Holder storage**: `_userState[user]` packs `uint120 balance`, 8-bit
  `DelegationMode`, `uint128 additionalData` (last index) from v3.4; ≤ v3.3
  used `uint128 balance`. The low 120 bits are the scaled balance in both
  eras for non-AAVE aTokens.
- **Reserve storage** (`DataTypes.ReserveData`): `liquidityIndex` and
  `currentLiquidityRate` share one slot; `lastUpdateTimestamp` (uint40) is
  packed with `deficit`, `id` and `liquidationGracePeriodUntil`. Slot offsets
  are inferred from declaration order and must be confirmed against a
  compiler layout dump.
- **Actions (#20)**: `Supply`, `Withdraw`, `Borrow`, `Repay`, `LiquidationCall`,
  `FlashLoan` on the Pool proxy (topic0 values in §4); `user`/`initiator`/
  `liquidator` are unindexed in several events and must be decoded from data.
  `InterestRateMode` 1 (stable) is deprecated since v3.2.

### Compound v2 (issue #14)

Ethereum mainnet only: `compound-finance/compound-protocol@a3214f67b73310d547e00fc578e8355911c9d376` `networks/mainnet.json`, confirmed against `compound-config` and `compound-js`. There is no Compound v2 on BSC; Venus is a separate BNB Chain protocol with a mirrored contract structure.

| Market | Address | Shape | Cash source | Notes |
| --- | --- | --- | --- | --- |
| cUSDC | `0x39AA39c021dfbaE8faC545936693aC917d5E7563` | direct legacy `CErc20` (2019 compiler revision, deployed block 7,710,760) | `USDC.balanceOf(cUSDC)` | **First market**: simplest cash dependency; storage layout must be checked against the deployed 2019 source, not the pinned `^0.8.10` tree |
| cDAI | `0x5d3a536E4D6DbD6114cc1Ead35777bAB948E3643` | `CErc20Delegator` → implementation (snapshot `cDaiDelegate` `0xbB8bE4772fAA655C255309afc3c5207aA7b896Fd`; deploy-time implementation was `0x99ee778b9a6205657dd03b2b91415c8646d521ec`) | Maker Pot: `pot.chi() * pot.pie(cDAI) / RAY` while a DSR delegate is active | Second market; requires Pot state and the implementation in force at the epoch |
| cETH | `0x4Ddc2D193948926D02f9B1fE9e1daa0718270ED5` | direct `CEther` | `address(this).balance - msg.value` | Native cash scoped separately if requested |

Comptroller/Unitroller `0x3d9819210A31b4961b30EF54bE2aeD79B9c9Cd3B`; Timelock `0x6d903f6003cca6255D85CcA4D3B5E5146dC33925`.

- `exchangeRateStored = (totalCash + totalBorrows − totalReserves) × 1e18 / totalSupply`, or `initialExchangeRateMantissa` when supply is zero (cUSDC 2e14).
- `accrueInterest` is block-number based: `borrowRate` from the market's `interestRateModel` (governance-mutable; JumpRateModelV2 with `blocksPerYear = 2102400`), `simpleInterestFactor = borrowRate × blockDelta`, truncating `Exp` arithmetic; the per-market model must be read from storage or `NewMarketInterestRateModel` events at the epoch, not from the deployment records.
- Metrics: cToken share balance (`accountTokens`), stored conversion (`exchangeRateStored`), projected conversion after simulated accrual (`exchangeRateCurrent`). Borrow snapshots are debt and stay deferred.

### Compound III / Comet (issue #15)

`compound-finance/comet@f766f51583c23acc33b2a7824654ef2029a96804`. `contracts/Comet.sol` no longer exists at this pin; `contracts/CometWithExtendedAssetList.sol` is the implementation reference (identical `balanceOf`/index math to the removed `Comet.sol` at parent `d5a30b0aaeff7755f1431e87f818990902237b03`). Deployments at the pin: arbitrum, base, linea, mainnet, mantle, optimism, polygon, ronin, scroll, unichain — **not BSC**.

| Market | Proxy | Base token | Base scale |
| --- | --- | --- | --- |
| Ethereum cUSDCv3 (**first market**) | `0xc3d688B66703497DAA19211EEdff47f25384cdc3` | USDC `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` | 1e6 |
| Ethereum cWETHv3 | `0xA17581A9E3356d9A858b789D68B4d866e593aE94` | WETH `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2` | 1e18 |
| Base cUSDCv3 | `0xb125E6687d4313864e53df431d5425969c15Eb2F` | USDC `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913` | 1e6 |
| Base cUSDbCv3 | `0x9c4ec768c28520B50860ea7a15bd7213a9fF58bf` | USDbC `0xd9aAEc86B65D86f6A7B5B1b0c42FFA531710b6CA` | 1e6 |

- `balanceOf(account) = principal > 0 ? principal × baseSupplyIndex′ / 1e15 : 0`
  where `baseSupplyIndex′` is the stored index advanced by
  `mulFactor(index, supplyRate × timeElapsed)` with `timeElapsed = now − lastAccrualTime`;
  utilization uses the stored (un-accrued) indices; rates are per-second
  immutables baked into each implementation (every governance rate change is
  a new implementation and proxy upgrade). Storage: `UserBasic {int104 principal; uint64 baseTrackingIndex; uint64 baseTrackingAccrued; uint16 assetsIn; uint8 _reserved}` in one slot; market state `baseSupplyIndex/baseBorrowIndex/trackingSupplyIndex/trackingBorrowIndex` (one slot) and `totalSupplyBase/totalBorrowBase/lastAccrualTime/pauseFlags` (one slot). Constants `BASE_INDEX_SCALE = 1e15`, `FACTOR_SCALE = 1e18`, `SECONDS_PER_YEAR = 31,536,000`, `MAX_BASE_DECIMALS = 18`.
- Negative principal is borrow debt (`borrowBalanceOf`, ceil rounding on principal) and stays deferred.

### Lido stETH (issue #23)

Ethereum mainnet, `docs.lido.fi/deployed-contracts` (protocol version v4.0.1) and `lidofinance/core` tag v4.0.1 (`2da0f48f1a2a103a394dcf8760810fe9165697fb`).

- stETH/Lido proxy `0xae7ab96520DE3A18E5e111B5EaAb095312D7fE84`; implementation listed `0x028271E30a695c0527A0C50cA30603feD004cDb0`; wstETH `0x7f39C581F595B53c5cb19bD0b3f8dA6c935E2Ca0`; WithdrawalQueueERC721 proxy `0x889edC2eDab5f40e902b864aD4d7AdE8E412F9B1` (deploy record implementation `0xE42C659Dc09109566720EA8b2De186c2Be7D94D9`); Accounting proxy `0x23ED611be0e1a820978875C0122F92260804cdDf`; VaultHub proxy `0x1d201BE093d847f6446530Efb0E8Fb426d176709`.
- **Epoch**: Lido contract version 4 (V3 stVaults with external shares are live; `ContractVersionSet(uint256)` logs mark upgrade boundaries). `balanceOf = shares[holder] × totalPooledEther / totalShares`; since V3 `totalShares` occupies the low 128 bits of `keccak256("lido.StETH.totalAndExternalShares")` with external shares in the high 128 bits, and the pre-V3 slot `keccak256("lido.StETH.totalShares")` is zeroed at migration. `totalPooledEther = internalEther + externalShares × internalEther / internalShares`, so the share rate is unchanged by external shares. CL balances reach the execution layer only through `Accounting.handleOracleReport` (`CLBalancesUpdated`, `TokenRebased`), so every passive balance is piecewise-constant between reports.
- wstETH conversion, withdrawal-request NFTs and validator balances are separate models.

### ERC-4626 (issue #24)

- **First BSC candidate**: the legacy Aave static aToken for BNB USDT
  `0x0471D185cc7Be61E154277cAB2396cD397663da6` (`stataBnbUSDT`, address-book
  key `USDT_STATIC_A_TOKEN`, created by `LEGACY_STATIC_A_TOKEN_FACTORY`
  `0x326aB0868bD279382Be2DF5E228Cb8AF38649AB4`; the address book registers no
  `StataTokenV2` for any BNB reserve). Its implementation is
  `StaticATokenLM.sol` from the archived `bgd-labs/static-a-token-v3`
  (`101f5d977889254ca2d2711b9582b45f832d10a0`), an upgradeable proxy under
  Aave governance: `convertToAssets = previewRedeem = floor(shares × POOL.getReserveNormalizedIncome(underlying) / 1e27)`,
  `maxWithdraw` applies the same conversion to `maxRedeem`, which is 0 while
  the reserve is inactive or paused, and `totalAssets = aToken.balanceOf(vault)`.
  The current `StataTokenV2` (`ERC4626StataTokenUpgradeable`, aave-v3-origin
  pin above) uses the numerically identical conversion but is a different
  runtime; the adapter binds to the deployed implementation.
- **Cross-chain reference**: Savings DAI `0x83f20f44975d03b1b09e64809b757c47f942beea` (Ethereum; `sky-ecosystem/sdai`, deployed commit `665879762f8b5df5d234463f45d1d6a49bd4fbeb`): `convertToAssets = shares × chi′ / RAY` with `chi′ = rpow(dsr, now − rho) × chi / RAY` from `MCD_POT` `0x197E90f9FAD81970bA7976f33CbD77088E5D7cf7`; `previewRedeem == convertToAssets`; `previewWithdraw` rounds up.
- OpenZeppelin ERC4626 (v5.0.0) derived vaults use the virtual-offset formula
  `shares × (totalAssets + 1) / (totalSupply + 10^offset)`; Venus ERC4626 on
  BSC (factory `0xC2f7924809830886EB04c6b40725Fd68F1891fA2`) layers that on
  `vToken.exchangeRateStored`, and no individual Venus vault address is
  confirmed in official sources.
- EIP-4626 requires `convertTo*` to round down and allows them to be inexact
  or time-weighted; `preview*` must include fees; `max*` must include limits.
  The adapter binds to one implementation, never to the interface.

## 4. Requested actions and their evidence

Facts are exposed; wallet-relative direction, grouping, intent and business
rules stay with the consumer. One transaction can carry many actions and
participants. Topic hashes are Keccak-256 of the canonical signatures read
from the pinned sources (ethereum/ERCs `5fc191d6d4da12ee224813871f94ff16e541e3f7`,
OpenZeppelin v5.1.0, Uniswap/permit2 `cc56ad0f3439c502c246fc5cfcc3db92bb8b7219`,
gnosis/canonical-weth `0dd1ea3e295eef916d0c6223ec63141137d22d67`,
aave-v3-origin v3.7.0 `IPool.sol`) and were recomputed locally while writing
this document.

| Requested interpretation | On-chain evidence | Package | Limits |
| --- | --- | --- | --- |
| authorize / approve | ERC-20 `Approval(address,address,uint256)` `0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925` (3 topics; also emitted by EIP-2612 `permit`, whose sender is a relayer); ERC-721 `Approval` shares the topic with 4 topics; `ApprovalForAll(address,address,bool)` `0x17307eab39ab6107e8899845ad3d59bd9653f200f220920489ca2b5937696c31` for 721 and 1155; Permit2 (`0x000000000022D473030F116dDEE9F6B43aC78BA3` on Ethereum) `Permit` `0xc6a377bfc4eb120024a8ac08eef205be16b817020812c73223e81d1bdb9708ec`, `Approval(address,address,address,uint160,uint48)` `0xda9fa7c1b00402c17d0161b249b1ab8bbec047c5a52207b9c112deffd817036b`, `Lockdown` `0x89b1add15eff56b3dfe299ad94e01f2b52fbcb80ae1a3baea6ae8c04cb2b98a4`, `NonceInvalidation` `0x55eb90d810e1700b35a8e7e25395ff7f2b2259abd7415ca2284dfb1c246418f3`; EIP-7702 delegation = `CodeChange` on the authority to `0xef0100‖address` (zero address clears) | #19 events, #18 code changes and calls | Off-chain signatures that were never executed are not facts; a consumed Permit2 `SignatureTransfer` leaves only the token `Transfer` plus the call trace; per-chain Permit2 addresses unconfirmed |
| borrow / repay, deposit / withdraw (lending) | Aave V3 Pool `Supply` `0x2b627736bca15cd5381dcf80b0bf11fd197d01a037c52b927a881a10fb73ba61`, `Withdraw` `0x3115d1449a7b732c986cba18244e897a450f61e1bb8d589cd2e69e6c8924f9f7`, `Borrow` `0xb3d084820fb1a9decffb176436bd02558d15fac9b0ddfed8c465bc7359d7dce0`, `Repay` `0xa534c8dbe71f871f9f3530e97a74601fea17b426cae02e1c5aee42c96c784051`, `LiquidationCall` `0xe413a321e8681d831f4dbccbca790d2952b56f977908e45be37335533e005286`, `FlashLoan` `0xefefaba5e921573100900a3ad9cf29f222d995fb3b6045797eaea7521bd8d6f0` | #20 for one Aave version; raw calls/logs in #18 | Other protocols retain raw evidence until named |
| wrap / unwrap | WETH9 `Deposit(address,uint256)` `0xe1fffcc4923d04b559f4d29a8bfc6cda04eb5b0d3c460751c2402c5c5cc9109c`, `Withdrawal(address,uint256)` `0x7fcf532c15f0a6db0bd6d0e038bea71d30d808c7d98cb3bf7268a95bf5081b65` from the known WETH address only; no zero-address `Transfer` is emitted | #22 corpus, #19 | Generic signatures; require the emitter identity |
| send / receive | Native: `BalanceChange` reduction (#17) and value-bearing calls (#18); tokens: ERC-20 `Transfer` `0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef` (3 topics; 4 topics means ERC-721), ERC-1155 `TransferSingle` `0xc3d58168c5ae7397731d063d5bbf3d657854427343f4c083240f7aacaa2d0f62` and `TransferBatch` `0x4a39dc06d4c0dbc64b70af90fd698a233a518aa5d07e595d983b8c0526c8f7fb` | #17, #19, #18 | Balance changes and transfers are different facts; fee-on-transfer and rebasing tokens are outside event-only accounting |
| claim / mint, stake / unstake, trade | Zero-address `Transfer` events, protocol logs and calls | #19, #18 | No universal classifier; protocols must be named before decoded adapters exist |
| deployment | `CallType::CREATE` calls, persisted `CodeChange` records, EIP-6780 rules for later `SELFDESTRUCT` (balance sweep only unless same-transaction creation) | #18 | Creation attempts differ from persisted code |
| cancel | **Protocol cancellation** is a fact when a named protocol emits it (Permit2 `Lockdown`/`NonceInvalidation`/`UnorderedNonceInvalidation` `0x3704902f963766a4e561bbaab6e6cdc1b1dd12f6e9e99648da8843b3f46b918d`; order-book cancellations need named adapters). **Mempool replacement** is not: a replaced or dropped transaction leaves no trace, and a "cancel" is any transaction consuming the nonce (client-local `--txpool.pricebump` policy, default 10%) | #18 for the included transaction only | Canonical blocks cannot establish replacement history |
| execute / execution | An *execution fact* is one node of the persisted call tree (`index`, `parent_index`, `depth`, `call_type`, `caller`, `address`, `address_delegates_to`, `value`, `input` selector, `return_data`, failure flags, `state_reverted`, ordinals); "executes" is the parent/child relation. A wallet-level "execute" label (Safe `execTransaction`, EIP-7702 delegated execution, routers, multicalls) is consumer interpretation over that tree | #18 | A selector match is not an economic outcome; reverted frames are attempted, not completed |

## 5. Output identity, ordering and consumption

- **Identity**: balance rows are end-of-block state keyed by `(block clock, contract-or-absent, address)`; fact rows (#18, #19, #20) carry `(block hash, transaction index and hash, call index, log index or ordinal)` and, for block and system scopes, an explicit scope without a transaction hash. The [balance-state contract](https://github.com/pinax-network/substreams-evm-extended/issues/12) encodes these identities for the protocol packages.
- **Ordering**: within a block by execution ordinal (versions 4/5 only); across blocks by the stream clock. Rows inside `Events` are sorted deterministically.
- **Clock advancement**: every delivered block advances the clock, including blocks with empty output. Consumers must not infer completeness from nonempty rows or sink markers.
- **Unknown states**: *missing/uninitialized* (no observation since the consumer's checkpoint), *known zero* (an observed zero value), *unsupported model* (layout or version not qualified; the map fails closed), *invalidated dependency* (a pinned implementation, slot or rate model changed; retained state must be rebuilt) and *reverted* (attempted, never persisted) are distinct and are never collapsed into zero. Initialization and checkpoints are shared under [#7](https://github.com/pinax-network/substreams-evm-extended/issues/7).
- **Native ClickHouse**: the Substreams CLI derives one table per repeated message (`Balance`), keyed `(_block_number_, _row_id_)` in a `ReplacingMergeTree`; optional `contract` is stored as `String`, so native (absent) and ERC-20 (present) rows must be separated by package or table; `_blocks_` lists nonempty-output blocks only; writes are not multi-table transactions and the schema keeps no block hash, so audits bind the sink's rows to independently captured clocks. No `db_out`, no custom sink.

## 6. Open questions

Unresolved items are recorded here rather than assumed. Each names the issue
that will close it.

1. Customer confirmation of scope: are ERC-721 ownership, ERC-1155 quantities,
   debt balances, collateral and LP claims requested? Which staking, trading
   and claim protocols should receive decoded adapters? (#11 follow-up;
   currently deferred.)
2. Extended historical depth available from the documented BSC, Ethereum and
   Base endpoints, and which producer version each serves at which heights
   (#8). Version 5 rollout is documented only in tracer changelogs.
3. HyperEVM: mainnet genesis date/height, `safe`/`finalized` tag semantics,
   the exact on-chain form of Core→EVM HYPE credits, and the chain-id registry
   conflict (ethereum-lists still maps 999 to Wanchain Testnet) (#8).
4. Arc: `safe`/`finalized` tag behaviour, genesis identity, whether the
   20 Gwei minimum base fee applies to mainnet, the fee recipient identity,
   and whether the system emitter also logs fee debits (#8, #17).
5. Base: whether a nonzero operator fee is configured since Isthmus, Jovian
   semantics, and producer fixtures showing where a failed deposit's mint is
   recorded (#8, #17).
6. Aave: activation blocks and code hashes of aToken revisions 4 and 5 and of
   the Pool implementation on BSC and Ethereum; compiler storage-layout dump
   for `ReserveData`; the live implementation behind the legacy BNB static
   aToken proxy at the chosen epoch (#13, #24).
7. Compound v2: implementation and interest-rate model in force for cDAI and
   cUSDC at the chosen epoch; slot-level layout of the 2019 cUSDC runtime
   (#14).
8. Comet: storage slot indices behind the transparent proxy; which proxies
   run legacy `Comet` versus `CometWithExtendedAssetList`; current
   immutables (#15).
9. Lido: mainnet enactment blocks of contract versions 2, 3 and 4; the
   `shares` mapping slot against a compiled layout; whether every report
   emits `TokenRebased` (#23).
10. Permit2 and WETH addresses on BSC, Base, HyperEVM and Arc; whether
    `SignatureTransfer` emits any Permit2-level log (#19).
11. Whether `REASON_REWARD_BLOB_FEE` can appear inside a failed BSC blob
    transaction's root call; the native reducer currently fails such blocks
    (#17).
12. EIP-7702 `SetCodeAuthorization.address` backfill status on the endpoints
    used (#18).
13. Per-chain reorg policy (final-blocks-only versus cursor plus undo) and
    assumed depth; Base Flashblocks partial-block delivery; whether HyperEVM
    Core→EVM system transactions and Arc system-emitter logs appear as
    ordinary transactions and logs in Extended blocks (#8, #17, #18).
14. Aave V3 Base market addresses, Aave V4 scope, bridged wstETH on Base and
    BSC, Compound v2 CToken event signatures, and any lending or staking
    candidate on HyperEVM and Arc are not yet inventoried (#11 follow-up).

## 7. Boundary

Extended-block Substreams extraction only, one RPC-free map per package,
native Substreams sinks, all tests and diagnostics in Rust. No Token API,
prices, APY, health factors, portfolio aggregation, RPC fallback service or
universal labeling. Live Substreams, Firehose, RPC and native sink usage
remains paused until explicitly resumed; this document was produced from
official documentation, pinned source and saved data only.
