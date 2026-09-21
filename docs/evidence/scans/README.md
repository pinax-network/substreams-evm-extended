# Saved-block scans

Outputs of the ad-hoc scanners in [`tools/scratch-scan`](../../../tools/scratch-scan/README.md)
over the locally cached BSC Extended blocks (`erc20/balances/out/*`).

| File | Scanner | Blocks | Result |
| --- | --- | --- | --- |
| `wbnb-scan.json` | `scan` | `top50-full-holder-blocks` [122288006, 122289030) | WBNB Deposit/Withdrawal transactions without Transfer logs, reverted and delegatecall WBNB writes; basis for the `erc20/balances` wbnb-mutations fixtures (issue #22) |
| `aave-scan.json` | `aave` | same window | Writes on the Aave V3 BNB Pool and aBnbUSDT/aBnbUSDC, most written slots, mapping bases seen for USDT keys; basis for the `aave/balance-state` layout (Pool `_reserves` slot 52, aToken `_userState` 0x34, allowances 0x35, `_totalSupply` 0x36) |
| `stata-scan.json` | `stata` | all 41 directories, 6,093 files | No call, write, log or code change touches any of the seven Aave BNB legacy static aTokens, so `erc4626/balance-state` has no saved-block fixture |
