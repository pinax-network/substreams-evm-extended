# sink-first and minimal (evm.balance_state.v1)

## Rationale
## Validation already performed

The file is a validated draft at `/private/tmp/claude-501/-Users-denis-Github-substreams-evm-extended/df7da1af-462f-4031-9cb8-e8fa8422da54/scratchpad/design/proto/v1/balance_state.proto`; its intended home is `/Users/denis/Github/substreams-evm-extended/proto/v1/balance_state.proto`.

- `protoc 34.1` compiles it.
- `buf 1.42.0 lint` (STANDARD) reports **zero** findings other than `PACKAGE_DIRECTORY_MATCH` / `PACKAGE_SAME_DIRECTORY`, which the existing `proto/v1/balances.proto` trips identically because the repo uses a flat `proto/v1` directory with `importPaths: [../../proto/v1]`. Naming, enum-zero-value and field-naming rules all pass.
- A fixture exercising all five tables (Aave holder + 3 reserve metrics + 2 binding rows + 2 Arc alias rows + header) encodes to 1,753 bytes, decodes, and **re-encodes byte-identically** (sha256 `82b9e8e46051dda3987d22f338d417f384d29086b65cc126a4a367bd8d720867`).
- A second fixture covering Comet negative principal, a Comet immutable, Compound v2 cross-contract cash, three Lido metrics including a log-evidenced snapshot, sDAI Pot `chi`, and a Lido invalidation encodes and decodes (1,599 bytes).
- Structural audit of the file: 6 messages, 12 enums, 5 top-level repeated fields, **0** nested repeated fields, **0** `oneof`, **0** float/double, **0** proto3 `optional`.

One real defect was found by compiling rather than by reading: `ModelBinding` permitted `EVIDENCE_LOG` (a proxy `Upgraded` or Lido `ContractVersionSet` is exactly how an invalidation is observed) but had no `log_index`. Added at tag 23, `ordinal` moved to 24, `checkpoint_id` to 25.

## Why five tables and not more or fewer

The native sink makes one table per top-level repeated message, so the table count *is* the message count. Five is the minimum that keeps every row flat:

| Table | Exists because |
| --- | --- |
| `BlockHeader` | The only place canonical hash/parent/timestamp can enter the database — the sink adds `_block_number_` and an ingestion timestamp but no block hash. Emitting it on **every** block is what makes the clock complete without trusting cursor progress. |
| `HolderBasis` | Per-holder state. Distinct identity (has a `holder`) and distinct semantics from shared state. |
| `GlobalState` | Shared state. Merging it into `HolderBasis` would force a null holder on every shared row and mix two unrelated descriptor enums in one column. |
| `ModelBinding` | Epoch, implementation, dependency and invalidation facts. These have no value/unit/scale and would be entirely null columns in a value table. |
| `AssetAlias` | Alias identity is a chain-level declaration with its own shape (representation + precision + canonicality), not a value at a block. |

Everything else that could have become a table is instead a **typed descriptor column**: `GlobalMetric` (29 values) replaces what would otherwise be per-protocol messages (`AaveReserveState`, `CometMarketState`, `CTokenState`, `LidoRebaseState`, `VaultConversionState`) — five tables collapsed into one. `PositionSide` halves the metric list by folding supply/borrow variants of index, rate, kink, slope and totals into one value each. `DependencyKind` fans a variable-length dependency list into flat rows instead of a nested repeated field.

## Field-by-field justification

**Subject identity (tags 1–6, identical across `HolderBasis`, `GlobalState`, `ModelBinding`).** `chain_id` because the Extended block carries none and the manifest binds the network; `market` / `token` / `underlying` as three separate addresses because Aave splits them (state on the Pool keyed by underlying, basis on the aToken) while Comet, cToken, stETH and vaults collapse them. Carrying all three on global rows too is what lets holder rows and global rows join on one key.

**Value encoding.** `value` is an exact decimal string, signed, never rescaled — matching `evm.balances.v1.Balance.amount`. `unit` + `scale_pow10` are the unit/scale the acceptance demands: `UNIT_SHARE`/18 for an Aave scaled balance, `UNIT_RATIO`/27 for a RAY index, `UNIT_RATIO`/15 for a Comet index, `UNIT_UNIX_SECONDS`/0 for an accrual clock, `UNIT_COUNT`/0 for a literal scale constant. `previous_value` supports the same cross-block continuity check the native replay tool already performs; `""` means "not recorded" and is distinguishable from `"0"` because strings preserve that, which is precisely what the existing `optional bytes contract` cannot do.

**No proto3 `optional` anywhere.** The sink README states optional `bytes` becomes `String` with no NULL, so absent-versus-empty is already lost for `Balance.contract`. Rather than repeat that trap, presence is carried by enums: `Scope` says whether `tx_hash`/`tx_index`/`call_index` are meaningful (a block-scope row has `tx_index = 0` that means nothing, not transaction 0), and `Evidence` says whether `log_index` and `storage_slot` are meaningful. This is the single most consequential sink-driven decision in the schema.

**`Boundary`.** Directly answers "state whether an output represents a change boundary or end-of-block state". Always set, never inferred. `BOUNDARY_END_OF_BLOCK` (default, matching existing repo semantics) carries `change_count` and a `previous_value` taken before the block's first write; `BOUNDARY_CHANGE` puts `ordinal` into the row identity.

**`model_id` + `epoch_block`.** The runtime-epoch reference. `epoch_block` is the binding's activation block, which is deterministic and available to a **stateless** map from qualified parameters — a counter would not be, since a Substreams map holds no cross-block state.

## How each acceptance item is met

1. **Holder basis updates** — `HolderBasis`: `chain_id`, (`market`,`token`,`underlying`), `holder`, `basis_type` (raw / scaled / share / signed-principal / scaled-debt), `value` + `unit` + `scale_pow10`, `model_id` + `epoch_block`. Negative principal survives as a signed decimal string under `BASIS_TYPE_SIGNED_PRINCIPAL` + `POSITION_SIDE_SIGNED_NET`; debt can never be read as an unsigned ERC-20 balance because `basis_type` and `side` are mandatory descriptors and `BASIS_TYPE_RAW_BALANCE` is the only value that means "this already is `balanceOf`".
2. **Protocol-specific global state** — `GlobalState` with `GlobalMetric` covering indices, rates, accrual clocks (timestamp and block), totals, reserves, cash, reserve factor, initial and current conversion rates, scale constants, kinked rate-model inputs, Lido ether components and external shares, and OZ virtual offsets. Cross-contract inputs are explicit: `source_contract` is the contract actually read and differs from `market`. `STATE_KIND_OBSERVED_DELTA` vs `STATE_KIND_VERIFIED_SNAPSHOT` vs `STATE_KIND_DERIVED` are separate values, and the comment on `OBSERVED_DELTA` states in the schema itself that it asserts nothing about any other metric — a partial delta cannot be labelled a complete snapshot.
3. **Canonical clock** — `BlockHeader` on every delivered block including empty ones, carrying number, hash, parent hash, timestamp and producer version. No finality field exists; the comment names `BlockUndoSignal.last_valid_block` / `last_valid_cursor` and `final_blocks_only` as the fork contract.
4. **Identity and ordering** — `scope` + `tx_hash` + `tx_index` + `call_index` + `log_index` + `ordinal`, with `SCOPE_SYSTEM_CALL` and `SCOPE_BLOCK` covering changes with no transaction hash and `SCOPE_QUALIFICATION` covering configuration. Ordering is by `ordinal` within a block and by the stream clock across blocks; the comment records that this needs producer version 4 or 5.
5. **One shared update, many holders** — enforced structurally: a shared change is a `GlobalState` row and there is no mechanism to expand it into holder rows. The `HolderBasis` comment states a holder row appears only when that holder's own storage was written, and that the consumer retains the basis and re-evaluates against a selected canonical clock. `STATE_KIND_DERIVED` is only legal when every input was observed in the same block, which structurally prevents the map from faking retained state.
6. **The five distinctions** — missing/uninitialized = no row (stated in the `HolderBasis` comment); known zero = `STATE_KIND_KNOWN_ZERO`; unsupported model = `BINDING_STATUS_UNSUPPORTED` on a `ModelBinding` row, which names the subject while emitting no data rows for it; invalidated dependency = `BINDING_STATUS_INVALIDATED` + `InvalidationReason` + `invalidated_from_block`; reverted = `STATE_KIND_REVERTED_ATTEMPT`, normally never emitted, existing so attempted writes can never be smuggled in under `OBSERVED_DELTA`. Checkpoints are referenced through `checkpoint_id`, not re-implemented.
7. **Boundary compliance** — new package namespace, `evm.balances.v1` byte-for-byte untouched, no field renumbered, no cache, no RPC, no extra map in `erc20/balances`, no `db_out`, no custom sink.
8. **Sink mapping and cross-output identities** — see the sink mapping section, including the explicit non-promises.

## Protocol family mapping, one concrete row each

**#13 Aave V3** — BSC chain 56, Pool `0x6807dc92…e0cB`, aToken USDT `0xa9251ca9…f1B1`, underlying `0x55d39832…7955` (18 decimals on BSC, not 6). Holder supplies:

`HolderBasis{chain_id:56, model_id:"aave.v3.atoken", epoch_block:<rev-5 activation>, market:0x6807dc92…, token:0xa9251ca9…, underlying:0x55d39832…, holder:0x1111…, basis_type:SCALED_BALANCE, side:SUPPLY, value:"4925103874102938471029", unit:SHARE, scale_pow10:18, previous_value:"0", state_kind:OBSERVED_DELTA, boundary:END_OF_BLOCK, change_count:1, evidence:STORAGE_CHANGE, scope:TRANSACTION, tx_hash:0x2222…, tx_index:42, call_index:3, ordinal:1187, source_contract:0xa9251ca9…, storage_slot:keccak(holder‖_userState)}`

Three `GlobalState` rows follow on the same Pool slot group: `ACCRUAL_INDEX`/`SUPPLY` `"1023456789012345678901234567"` RATIO/27 (`liquidityIndex`), `INTEREST_RATE`/`SUPPLY` `"24500000000000000000000000"` RATE_PER_YEAR/27 (`currentLiquidityRate`, **same slot and same ordinal** as the index — the `metric` enum is what disambiguates them), and `ACCRUAL_TIMESTAMP` `"1758153600"` UNIX_SECONDS/0. The rounding era rides on the binding: `revision:5`, `rounding_profile:"aave.v3.5.norm_half_up_then_floor"`, because `getNormalizedIncome` uses half-up `rayMul` and `TokenMath.getATokenBalance` then applies `rayMulFloor` — a chain a single enum cannot express. A revision ≤3 epoch is a different binding with `rounding_profile:"aave.v3.3.norm_half_up_then_half_up"`. The low-120-bit masking of `_userState.balance` from v3.4 onward is a `DEPENDENCY_KIND_STORAGE_LAYOUT` row.

**#14 Compound v2, donation-only cash change** — cUSDC `0x39AA39c0…7563`, underlying USDC `0xA0b86991…eB48`. Someone transfers USDC straight to the market. No cToken storage is written, no event is emitted by cUSDC, no `accrueInterest` runs — yet `exchangeRateStored` changes. Exactly one row:

`GlobalState{chain_id:1, model_id:"compound.v2.ctoken", epoch_block:7710760, market:0x39AA39c0…, token:0x39AA39c0…, underlying:0xA0b86991…, metric:CASH, side:NOT_APPLICABLE, value:"412903847561", unit:TOKEN, scale_pow10:6, previous_value:"412803847561", state_kind:OBSERVED_DELTA, boundary:END_OF_BLOCK, change_count:1, evidence:STORAGE_CHANGE, scope:TRANSACTION, ordinal:431, source_contract:0xA0b86991…, storage_slot:keccak(cUSDC‖USDC.balances)}`

`source_contract` is the **underlying**, not the market: this is the cross-contract dependency made visible. No derived `CONVERSION_RATE` row is emitted, because `totalBorrows`, `totalReserves` and `totalSupply` were not written in this block and a map has no store — the consumer recomputes `(cash + totalBorrows − totalReserves) × 1e18 / totalSupply` from retained state, or falls back to `INITIAL_CONVERSION_RATE` (`2e14` for cUSDC) when the basis total is zero. This single row is the clearest case for why a set of rows is never a market snapshot. cDAI differs only in its binding: `DEPENDENCY_KIND_CASH_SOURCE` → `0x197E90f9…` (Maker Pot) while a DSR delegate is active, plus `DEPENDENCY_KIND_RATE_MODEL` for the governance-mutable `interestRateModel`, and `RATE_PERIODS_PER_YEAR` `"2102400"` for the block-based accrual.

**#15 Compound III, global accrual with no holder change** — Ethereum cUSDCv3 `0xc3d688B6…cdc3`. Any interaction triggers `accrueInternal`; a tracked holder's `UserBasic` is untouched. Emitted: `ACCRUAL_INDEX`/`SUPPLY` `"1015283746591234"` RATIO/15 and `ACCRUAL_INDEX`/`BORROW`, `ACCRUAL_TIMESTAMP` (`lastAccrualTime`, uint40), and `TOTAL_BASIS`/`SUPPLY` + `/BORROW` when the totals slot was written. **Zero `HolderBasis` rows.** That one index row changes the present value of every initialized holder; consumers compute `principal × baseSupplyIndex′ / 1e15` themselves. Per-implementation immutables are separate rows at the epoch block: `RATE_KINK`/`SUPPLY` `"800000000000000000"` RATIO/18, the per-second slopes and base, `INDEX_SCALE` `"1000000000000000"`, `BASE_SCALE` `"1000000"`, each `state_kind:VERIFIED_SNAPSHOT`, `evidence:QUALIFIED_CONSTANT`, `scope:QUALIFICATION` — configuration, never presented as an observation. Because every governance rate change is a new implementation, that is a new `epoch_block` binding, and the old one becomes `INVALIDATED` with `INVALIDATION_REASON_IMPLEMENTATION_UPGRADED`. Negative principal: `value:"-12500000000"` with `BASIS_TYPE_SIGNED_PRINCIPAL` / `POSITION_SIDE_SIGNED_NET`.

**#23 Lido rebase** — stETH proxy `0xae7ab965…fE84`. `Accounting.handleOracleReport` writes the packed CL slot, the buffered-ether slot and the packed shares slot. Rows: `CL_VALIDATORS_BALANCE` `"9876543210000000000000000"` NATIVE/18 from slot `0x096e4653…8112`, `CL_PENDING_BALANCE` from the **same** slot, `BUFFERED_ETHER` and `DEPOSITED_POST_REPORT` from `0x81a11fa1…`, `TOTAL_BASIS`/`SUPPLY` (`totalShares`, low 128 bits of `0x6038150a…59e6`) and `EXTERNAL_SHARES` (high 128 bits of the same slot), plus

`GlobalState{…, metric:TOTAL_UNDERLYING, side:SUPPLY, value:"9654321098765432109876543", unit:NATIVE, scale_pow10:18, state_kind:VERIFIED_SNAPSHOT, evidence:LOG, scope:TRANSACTION, log_index:137, ordinal:2208}`

taken from `TokenRebased.postTotalEther` — a log that restates a complete total, which is why `VERIFIED_SNAPSHOT` + `EVIDENCE_LOG` exist as an orthogonal pair. `HolderBasis` rows appear only for accounts whose `shares` mapping actually moved (fee recipients); the rebase itself moves no holder shares. Every passive balance is piecewise-constant between reports, which the consumer gets for free by retaining shares and re-evaluating. The V3→V4 storage migration is the invalidation example: old binding `revision:3`, `BINDING_STATUS_INVALIDATED`, `INVALIDATION_REASON_STORAGE_LAYOUT_CHANGED`, `dependency_kind:STORAGE_LAYOUT`, `dependency_value:"lido.StETH.totalShares"`, `invalidated_from_block:<finalizeUpgrade_v4>` — because the pre-V3 slot is zeroed at migration, retained state keyed to it must be rebuilt.

**#24 ERC-4626 conversion-input change** — sDAI `0x83f20f44…beea`, underlying DAI, conversion depends only on Pot `0x197E90f9…`. A `drip()` writes `chi` and `rho`:

`GlobalState{chain_id:1, model_id:"erc4626.sdai", epoch_block:18100000, market:0x83f20f44…, token:0x83f20f44…, underlying:0x6B175474…, metric:CONVERSION_RATE, side:NOT_APPLICABLE, value:"1052368914372019283746501928", unit:RATIO, scale_pow10:27, state_kind:OBSERVED_DELTA, evidence:STORAGE_CHANGE, scope:TRANSACTION, ordinal:77, source_contract:0x197E90f9…, storage_slot:<Pot.chi>}`

plus `ACCRUAL_TIMESTAMP` (`rho`) and, on a governance change, `CONVERSION_RATE_PER_SECOND` (`dsr`). No holder rows. `rounding_profile` names the `rpow` half-up-per-step chain. The BSC-first Aave StataTokenV2 `0x0471D185…3da6` reuses the *same* metric set through a different binding: `DEPENDENCY_KIND_CONVERSION_SOURCE` → the Aave Pool, and its conversion inputs are literally the three Aave reserve rows above — the generic metric enum paying off across families. An OZ-derived vault such as Venus adds `VIRTUAL_ASSET_OFFSET` `"1"` and `VIRTUAL_SHARE_OFFSET` `"10^(18−assetDecimals)"`, which is how the schema records that `convertToAssets ≠ shares × totalAssets / totalSupply` for that implementation.

**Arc alias** — chain 5042, two rows sharing `alias_id:"arc.usdc"`: `{representation:NATIVE, contract:<empty>, decimals:18, canonical:true, log_emitter:0xffff…fffe}` and `{representation:ERC20_INTERFACE, contract:0x36000000…0000, decimals:6, canonical:false, log_emitter:0x36000000…0000}`, both `state_kind:VERIFIED_SNAPSHOT`, `evidence:QUALIFIED_CONSTANT`, `scope:QUALIFICATION`. One balance, two precisions, never two assets, never summed. `canonical:true` marks the representation that carries the full 18-decimal precision, so the documented fact that amounts below 10⁻⁶ USDC exist only natively is machine-readable as a 12-place truncation. `log_emitter` records that plain native sends log from the system emitter while `0x3600…` logs only for ERC-20-interface activity, which is what stops the same transfer being counted twice. `erc20/balances` and `native/balances` are untouched and emit none of this; a chain-scoped companion package does.

## Sink mapping
## One ClickHouse table per top-level repeated message

Five repeated fields in `Events`, therefore five tables. The scalar-free `Events` wrapper does not become a table. Every table additionally receives the native sink's own columns: `_block_number_`, an ingestion timestamp, `_version_`, `_deleted_` and a deterministic per-block `_row_id_`, with a `ReplacingMergeTree` key of `(_block_number_, _row_id_)`.

Column types assume `--bytes-encoding 0xhex`: `bytes` → `String` (hex), `string` → `String`, `uint32` → `UInt32`, `uint64` → `UInt64`, `bool` → `Bool`. Enum columns land as the CLI's enum representation (integer or name) — **this must be read off the generated schema before anyone relies on it**; live sink usage is paused, so it is stated as unverified rather than asserted (see risks).

### `BlockHeader` — from `Events.block_headers`
`chain_id UInt64, number UInt64, hash String, parent_hash String, timestamp_seconds UInt64, producer_version UInt32, holder_basis_rows UInt32, global_state_rows UInt32, model_binding_rows UInt32, asset_alias_rows UInt32, emitter_package String, emitter_version String`
- Exactly one row per delivered block, including empty blocks.
- Logical key: `(chain_id, number)`. Dedupe: `FINAL`, then one row per `_block_number_`.
- This is the only table carrying a block hash. Audits bind every other row to an independently captured clock through it.

### `HolderBasis` — from `Events.holder_basis`
`chain_id UInt64, model_id String, epoch_block UInt64, market String, token String, underlying String, holder String, basis_type Enum, side Enum, value String, unit Enum, scale_pow10 UInt32, previous_value String, state_kind Enum, boundary Enum, change_count UInt32, evidence Enum, scope Enum, tx_hash String, tx_index UInt32, call_index UInt32, log_index UInt32, ordinal UInt64, source_contract String, storage_slot String, checkpoint_id String`
- Logical key per block: `(chain_id, model_id, market, token, underlying, holder, basis_type, side)`; add `ordinal` when `boundary = BOUNDARY_CHANGE`.
- Latest observed basis: deduplicate with `FINAL`, then take the greatest `_block_number_` per logical key. Never filter zeros first — `_version_` is ingestion time, not chain order, and dropping `STATE_KIND_KNOWN_ZERO` rows resurrects a stale nonzero value.

### `GlobalState` — from `Events.global_state`
Same shape with `metric Enum` in place of `holder`/`basis_type`.
- Logical key per block: `(chain_id, model_id, market, token, underlying, metric, side)`; add `ordinal` under `BOUNDARY_CHANGE`.
- **`storage_slot` is not part of the key.** Packed slots carry several metrics at one slot and one ordinal (Aave `liquidityIndex` + `currentLiquidityRate`; Lido `clValidatorsBalance` + `clPendingBalance`; Lido `totalShares` + `externalShares`; Comet's four indices). `metric` is the discriminator.

### `ModelBinding` — from `Events.model_bindings`
`chain_id, model_id, epoch_block, market, token, underlying, status Enum, implementation String, code_hash String, revision UInt64, source_pin String, rounding_profile String, dependency_kind Enum, dependency_contract String, dependency_value String, invalidation_reason Enum, invalidated_from_block UInt64, evidence Enum, scope Enum, tx_hash String, tx_index UInt32, call_index UInt32, log_index UInt32, ordinal UInt64, checkpoint_id String`
- Logical key: `(chain_id, model_id, market, token, underlying, epoch_block, dependency_kind, dependency_contract)`.
- One self row (`DEPENDENCY_KIND_SELF`) plus one row per dependency edge. Binding-level columns are repeated on every edge row so each row is independently interpretable.
- Heartbeat re-emission produces identical logical rows at different blocks; a consumer wanting current bindings takes the greatest `_block_number_` per logical key.

### `AssetAlias` — from `Events.asset_aliases`
`chain_id UInt64, alias_id String, representation Enum, contract String, decimals UInt32, canonical Bool, log_emitter String, model_id String, epoch_block UInt64, state_kind Enum, evidence Enum, scope Enum, checkpoint_id String`
- Logical key: `(chain_id, alias_id, representation, contract)`.
- Invariant a consumer should assert: exactly one `canonical = true` row per `(chain_id, alias_id)`.

## Cross-output consumption identities

- **Any table → `BlockHeader`**: join on `_block_number_` to obtain hash, parent hash, canonical timestamp and producer version. This is how a row acquires a block hash, since the sink stores none.
- **`HolderBasis` ↔ `GlobalState`**: `(chain_id, model_id, market, token, underlying)`. Evaluating a holder balance at block N means taking the holder's latest basis at or before N and the latest value of each required metric at or before N, then applying the model's formula and rounding chain.
- **Either → `ModelBinding`**: `(chain_id, model_id, market, token, underlying, epoch_block)`. Before applying a retained value, check that its `epoch_block` binding is not `BINDING_STATUS_INVALIDATED` with `invalidated_from_block` at or below the evaluation block.
- **`AssetAlias` → `erc20/balances` / `native/balances`**: `alias_id` groups a native `Balance` row (absent contract) with an ERC-20 `Balance` row at `contract = log_emitter`. This binding is **operator configuration, not a database join**: the existing `Balance` table has no `chain_id`, and its optional `contract` is stored as `String`, so an absent native contract and an empty ERC-20 contract are indistinguishable in SQL. Bind the database to one chain and one package before treating `contract = ''` as native.
- **→ `#18` execution facts**: `(tx_hash, call_index, ordinal)` under `SCOPE_TRANSACTION`; `(scope, ordinal)` for system-call and block scopes, which have no transaction hash.

## What the sink does not promise

- `_blocks_` markers and cursor progress are **not** a complete-block publication manifest. Clock completeness comes from the `BlockHeader` table being emitted on every delivered block, or from the stream cursor — not from the sink's markers. A cursor can point at a block earlier than the requested stop.
- ClickHouse writes are **not** multi-table transactions. A block can land partially. `BlockHeader.*_rows` lets a consumer detect that; it is a check to run, not a guarantee to assume.
- **One database (or one table set) per package.** Two balance-state packages sinking into the same database write the same five table names, and their deterministic `_row_id_` values collide under `ReplacingMergeTree` on `(_block_number_, _row_id_)` — silently dropping rows. Use a separate database and state directory per package, chain, parameter set and bytes encoding, as the existing sink README already instructs.
- Nothing here initializes holders without an observed write, and nothing here implements rollback. Both belong to #7.

## Versioning
## Namespace and file

- One new versioned namespace: `evm.balance_state.v1`, in `proto/v1/balance_state.proto`, alongside the untouched `proto/v1/balances.proto`. Companion manifests add it as `protobuf: { files: [balance_state.proto], importPaths: [../../proto/v1] }`, matching the existing layout.
- `evm.balances.v1` is not edited, not renumbered, not reinterpreted. `erc20/balances` and `native/balances` keep `evm.balances.v1.Events` as their output and gain no module.

## Compatible changes (stay in v1)

- Add a new field with the next free tag. Never reuse a tag or a name; deleted fields go into `reserved` for both.
- Add a new enum value with the next free number. Enums are open; the zero value is always `*_UNSPECIFIED` and never a real state.
- Add a new `model_id`, `rounding_profile`, `source_pin` or `checkpoint_id` value. These are registry strings documented in the package README, so registering a new protocol model is **not** a proto change.
- Add a new top-level repeated field to `Events` only when a genuinely new row shape appears. Every such addition is a new ClickHouse table and must be justified against the five-table budget.

## Breaking changes (require `evm.balance_state.v2`)

- Changing a field's type, tag, cardinality or meaning; repurposing an enum value; changing what a `StateKind` or `Boundary` asserts; making `BlockHeader` no longer one-per-block.
- A v2 lives in a new file and package. Both may be produced side by side during migration; a consumer selects by package name.

## Consumer rules

- Read enum fields as raw integers. An unknown value means "produced by a newer package" and the row must be refused, never coerced to the zero value. With prost, `field()` returns the default for an unknown integer, so read the underlying `i32` field.
- Treat an empty `bytes` or empty `string` as "not applicable for this row kind", decided by `scope` / `evidence` / `basis_type`. It is never zero.
- Adding a field changes the sink-derived DDL. The native sink generates the schema, so a schema change means a fresh database or an operator-run `ALTER`; the existing README rule (new database when package, parameters, chain or bytes encoding change) applies to proto changes as well.

## Rust generation

- `buf generate` with `buf.build/community/neoeinstein-prost`, proto3, output committed under `proto/src/pb/evm.balance_state.v1.rs`; `proto/src/pb/mod.rs` gains a `balance_state::v1` module beside `balances::v1`. Generated code is committed and reviewed, matching current practice.
- Commit a descriptor set at `proto/descriptors/evm.balance_state.v1.binpb` (`buf build -o …`). It is the input to the structural CI test below.

## Rust encode/decode fixtures and tests

All Rust, all offline, no network, under `proto/tests/`:

1. **Golden wire fixtures** — `proto/tests/fixtures/balance_state/*.binpb`, one per scenario: `aave-atoken-supply`, `comet-accrual-no-holder`, `compound-v2-donation-cash`, `lido-rebase`, `erc4626-pot-chi`, `arc-alias`, `empty-block-header`, `comet-negative-principal`, `lido-binding-invalidated`. Two are already produced and verified in this design pass (the combined Aave/Arc fixture is 1,753 bytes, sha256 `82b9e8e46051dda3987d22f338d417f384d29086b65cc126a4a367bd8d720867`; the multi-protocol fixture is 1,599 bytes).
2. **Round-trip test** — for each fixture: decode into the generated types, assert the typed field values, re-encode, and assert the bytes are **identical** to the committed fixture. Byte identity is what pins tag numbers and field ordering, so any accidental renumbering fails CI.
3. **Digest test** — assert each fixture's sha256 against a committed manifest, so a fixture cannot be quietly regenerated to match broken code.
4. **Structural invariant test** — parse the committed descriptor set with `prost-types::FileDescriptorSet` and assert, for `evm.balance_state.v1`: every enum's value 0 ends in `_UNSPECIFIED`; no message except `Events` has a repeated field; no `oneof`; no `float`/`double`; no proto3 optional (no `proto3_optional`); every top-level `Events` field is repeated and of message type. This turns the hard constraints into an executable CI check rather than a review convention.
5. **Non-regression test for the canonical schema** — encode a fixed `evm.balances.v1.Balance` and `Events` and assert byte equality with a committed golden, proving the additive change did not disturb the canonical protobuf.
6. **Semantic guard tests** — a negative `value` is rejected unless `basis_type` is `SIGNED_PRINCIPAL` (or a future signed type); `tx_hash` is empty whenever `scope != SCOPE_TRANSACTION`; `log_index` is zero unless `evidence == EVIDENCE_LOG`; `previous_value` `""` and `"0"` survive a round trip as distinct values; `BlockHeader.*_rows` equal the actual row counts of the same `Events`.

## Risks
- Enum-to-ClickHouse column mapping is unverified. The sink README documents bytes/string/number mapping but says nothing about proto enums, and live sink usage is paused, so whether `state_kind` lands as Int32 or as a name string is unknown. Every enum column in the sink-mapping section is therefore stated as to-be-verified. Generate the schema once live work resumes and record it as evidence before any consumer SQL depends on it.
- One `BlockHeader` row per delivered block is roughly 192k rows/day on BSC at 450 ms cadence, and the `_blocks_` marker table grows with it. That is the price of a complete clock without trusting cursor progress. The parameter that disables headers on empty blocks removes the cost and the guarantee together; there is no third option.
- Two balance-state packages sinking into one database silently lose rows: identical table names plus deterministic per-block `_row_id_` values collide under `ReplacingMergeTree` on `(_block_number_, _row_id_)`. The mitigation is operational (one database per package), not structural, and a careless operator will not get an error.
- `STATE_KIND_REVERTED_ATTEMPT` is normally never emitted. It is defended as a guard against attempted writes being smuggled in under OBSERVED_DELTA and is emittable only behind an off-by-default diagnostic parameter, but a reviewer may reasonably call it dead weight in the enum.
- `model_id`, `rounding_profile`, `source_pin`, `checkpoint_id` and `dependency_value` are free strings governed by a README registry, not by the type system. A typo produces a silently unjoinable row. A closed enum was rejected because a rounding chain such as Aave's half-up normalization followed by a floor cannot be expressed as one enum value, and because registering a new protocol model must not require a proto change.
- `epoch_block` values depend on qualification facts the repository does not yet have: the activation blocks of Aave aToken revisions 4 and 5 on BSC and Ethereum, the Lido V3/V4 upgrade blocks, the Comet storage slot indices behind the transparent proxy, and the live cDAI implementation. The schema can express all of them; none of the values is known today (extraction-coverage open questions 6-9).
- Storage-slot layouts feeding `storage_slot` are inferred from Solidity declaration order in the research notes, not from a compiler storage-layout dump, for Aave `ReserveData`, Comet `CometStorage`, the legacy 2019 cUSDC runtime and the Lido `shares` mapping slot. A wrong base slot produces confidently wrong rows that the schema cannot catch.
- Lido `TokenRebased` is not confirmed to be emitted on every report. A `VERIFIED_SNAPSHOT` row evidenced by that log must therefore be treated as corroborating, never as the sole source of post-report totals; the storage-derived component rows remain primary.
- The `GlobalMetric` enum will churn as protocols are added. New values are wire-compatible and add no columns, but consumers with stale mappings will see unknown integers and must refuse those rows rather than coerce them.
- Cross-output joins to `erc20/balances` and `native/balances` are weaker than the joins inside this package: those tables carry no `chain_id`, and their optional `contract` is a `String` that cannot distinguish absent from empty. The Arc alias binding therefore rests on operator configuration (one chain, one package per database), which is exactly the ambiguity the existing README already warns about.
- Correctness now rests heavily on the #16 Rust conformance models. Because a stateless map can only emit `STATE_KIND_DERIVED` when every input appears in the same block, consumers must implement the composition and rounding themselves for the common case. The schema guarantees the inputs are present and typed; it cannot guarantee anyone combines them correctly.
- Comet rate parameters are implementation immutables, so a missed upgrade means stale `QUALIFIED_CONSTANT` rows that look authoritative. The defence is the code-hash binding and fail-closed qualification, both of which depend on the operator actually binding a code hash; `code_hash` is allowed to be empty.
- Five tables plus a per-block header row is more schema surface than the two-table status quo. It is justified per table, but it is a real increase in what an operator must provision, migrate and audit.