# tools/scratch-scan

Ad-hoc Rust scanners that were written in a session scratchpad while building
the balance-state packages and are preserved here so the next person does not
have to rewrite them. They read saved Firehose Extended blocks (`<height>.pb`,
`sf.ethereum.type.v2.Block`) from the directories under `erc20/balances/out/`
and print JSON summaries. They are **not** workspace members (see the root
`Cargo.toml` `exclude`), are not linted to the workspace bar, and must never be
depended on by a map crate.

```sh
cargo build --release --manifest-path tools/scratch-scan/Cargo.toml
B=tools/scratch-scan/target/release
```

| Binary | Usage | What it does |
| --- | --- | --- |
| `scan` | `$B/scan <blocks-dir>` | WBNB non-Transfer mutation survey: Deposit/Withdrawal without Transfer logs, reverted or delegatecall WBNB writes, nested depositors, zero final writes (result: `docs/evidence/scans/wbnb-scan.json`). |
| `scan` (trim) | `$B/scan <blocks-dir> trim <height> <tx-index> <out.pb>` | Writes a single-transaction fixture block: header and identity kept, only the given transaction retained, block-level balance/code/system-call records dropped. This is how every `tests/fixtures/*.pb` of the aave, erc20/balances (wbnb-mutations) and evm/executions packages was produced. |
| `probe` | `$B/probe <fixture.pb>...` | Prints, per call, the WBNB logs, storage writes and native balance changes of the first transaction of each fixture with decoded values. |
| `aave` | `$B/aave <blocks-dir>` | Counts writes on the Aave V3 BNB Pool, aBnbUSDT and aBnbUSDC, the most written slots and the mapping bases seen in Keccak preimages for USDT keys (result: `docs/evidence/scans/aave-scan.json`; this is how Pool `_reserves` slot 52 and aToken slots 0x34/0x35/0x36 were observed). |
| `aavelogs` | `$B/aavelogs <blocks-dir>...` | Counts Pool Supply/Withdraw/Borrow/Repay/LiquidationCall/FlashLoan logs and lists examples, used to pick the aave/actions fixtures. |
| `stata` | `$B/stata <blocks-dir>...` | Looks for any call, write, log or code change touching the seven Aave BNB legacy static aTokens (result: `docs/evidence/scans/stata-scan.json`: none in 6,093 saved blocks). |

Block directories are listed in [`docs/handoff.md`](../../docs/handoff.md).
