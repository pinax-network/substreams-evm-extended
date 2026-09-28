//! StaticATokenLM source functions with controlled config/liquidity/holder
//! reads and a separately pinned Aave normalized-income dependency.
use super::*;
use crate::aave::source_oracle::{call, compare, counted, max, n, storage, Contract};

const GETTERS: [&str; 6] = [
    "convertToAssets(uint256)",
    "previewRedeem(uint256)",
    "convertToShares(uint256)",
    "previewDeposit(uint256)",
    "previewMint(uint256)",
    "previewWithdraw(uint256)",
];
fn model(vault: &StataTokenLm, getter: &str, amount: &BigUint, now: u64) -> Result<BigUint> {
    match getter {
        "convertToAssets(uint256)" | "previewRedeem(uint256)" => vault.convert_to_assets(amount, now),
        "convertToShares(uint256)" | "previewDeposit(uint256)" => vault.convert_to_shares(amount, now),
        "previewMint(uint256)" => vault.preview_mint(amount, now),
        "previewWithdraw(uint256)" => vault.preview_withdraw(amount, now),
        _ => unreachable!(),
    }
}

#[test]
fn source_static_helpers_preserve_zero_shortcuts_and_checked_operation_order() {
    counted(424, || {
        let values = [
            n(0),
            n(1),
            ray() / n(2) - n(1),
            ray() / n(2),
            ray() / n(2) + n(1),
            ray() - n(1),
            ray(),
            ray() + n(1),
            (n(1) << 128u32) - n(1),
            max(),
        ];
        let mut pairs: Vec<_> = values.iter().flat_map(|a| values.iter().map(move |b| (a.clone(), b.clone()))).collect();
        pairs.extend([
            (max() - ray(), n(1)), // +RAY fits; -1 happens afterwards.
            (max() - ray() + n(1), n(1)),
            (max() / ray(), n(1)),
            (max() / ray() + n(1), n(1)),
            (n(1), max() - ray()),
            (n(1), max() - ray() + n(1)),
        ]);
        for (a, b) in pairs {
            for (getter, evaluate) in [
                ("mulDown(uint256,uint256)", ray_mul_round_down as fn(&BigUint, &BigUint) -> Result<BigUint>),
                ("mulUp(uint256,uint256)", ray_mul_round_up),
                ("divDown(uint256,uint256)", ray_div_round_down),
                ("divUp(uint256,uint256)", ray_div_round_up),
            ] {
                compare(evaluate(&a, &b), call(Contract::Static, getter, &[a.clone(), b.clone()], &[], 0));
            }
        }
    });
}

#[test]
fn source_static_getters_compose_rate_before_conversion_even_for_zero_amounts() {
    counted(2250, || {
        let max128 = (n(1) << 128u32) - n(1);
        for index in [n(0), n(1), ray(), ray() + ray() / n(2), max128.clone()] {
            for rate in [n(0), ray(), max128.clone()] {
                for now in [0, 1, 2, aave::SECONDS_PER_YEAR + 1, u64::MAX] {
                    let vault = StataTokenLm {
                        reserve: aave::Reserve {
                            liquidity_index: index.clone(),
                            current_liquidity_rate: rate.clone(),
                            last_update_timestamp: 1,
                        },
                        reserve_active_and_unpaused: Some(true),
                    };
                    let values = storage(Contract::Static, &vault.reserve, &n(0), &n(0), true, false);
                    for amount in [n(0), n(1), ray(), max() / ray(), max()] {
                        for getter in GETTERS {
                            compare(
                                model(&vault, getter, &amount, now),
                                call(Contract::Static, getter, &[amount.clone()], &values, now),
                            );
                        }
                    }
                }
            }
        }
    });
}

#[test]
fn source_static_limits_keep_config_shortcuts_and_required_conversions() {
    counted(192, || {
        let max128 = (n(1) << 128u32) - n(1);
        let reserves = [
            (
                aave::Reserve {
                    liquidity_index: ray() + ray() / n(10),
                    current_liquidity_rate: n(0),
                    last_update_timestamp: 1,
                },
                2,
            ),
            (
                aave::Reserve {
                    liquidity_index: n(0),
                    current_liquidity_rate: n(0),
                    last_update_timestamp: 1,
                },
                1,
            ),
            (
                aave::Reserve {
                    liquidity_index: ray(),
                    current_liquidity_rate: ray(),
                    last_update_timestamp: 1,
                },
                0,
            ),
            (
                aave::Reserve {
                    liquidity_index: max128.clone(),
                    current_liquidity_rate: max128,
                    last_update_timestamp: 1,
                },
                u64::MAX,
            ),
        ];
        for (reserve, now) in reserves {
            for (active, paused) in [(false, false), (false, true), (true, true), (true, false)] {
                let vault = StataTokenLm {
                    reserve: reserve.clone(),
                    reserve_active_and_unpaused: Some(active && !paused),
                };
                for shares in [n(0), n(7)] {
                    for liquidity in [n(0), n(10), max()] {
                        let values = storage(Contract::Static, &reserve, &shares, &liquidity, active, paused);
                        compare(
                            vault.max_redeem(&shares, Some(&liquidity), now),
                            call(Contract::Static, "maxRedeem(address)", &[n(1)], &values, now),
                        );
                        compare(
                            vault.max_withdraw(&shares, Some(&liquidity), now),
                            call(Contract::Static, "maxWithdraw(address)", &[n(1)], &values, now),
                        );
                    }
                }
            }
        }
    });
}

#[test]
fn missing_static_facts_are_unknown_instead_of_synthetic_zero_source_inputs() {
    let mut vault = StataTokenLm {
        reserve: aave::Reserve {
            liquidity_index: ray(),
            current_liquidity_rate: n(0),
            last_update_timestamp: 1,
        },
        reserve_active_and_unpaused: None,
    };
    assert_eq!(vault.max_redeem(&n(0), Some(&n(0)), 1), Err(Unknown::MissingInput("reserve configuration")));
    vault.reserve_active_and_unpaused = Some(true);
    assert_eq!(vault.max_redeem(&n(0), None, 1), Err(Unknown::MissingInput("aToken underlying balance")));
    vault.reserve_active_and_unpaused = Some(false);
    assert_eq!(vault.max_redeem(&n(0), None, 1), Ok(n(0)));
    // Missing inputs are a host contract, not an EVM failure-code equivalence.
}
