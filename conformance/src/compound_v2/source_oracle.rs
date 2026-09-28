//! Execute compiler outputs from the pinned source; no chain calls or compiler
//! is needed during tests. Harness substitutions and their limits are recorded
//! in fixtures/compound-v2-oracle/README.md and the Rust generator.
use super::*;
use crate::erc4626::oz_evm_oracle::{decode, execute, word};

const CURRENT_MARKET: &str = include_str!("../../fixtures/compound-v2-oracle/current-market.bin-runtime");
const LEGACY_MARKET: &str = include_str!("../../fixtures/compound-v2-oracle/legacy-market.bin-runtime");
const CURRENT_RATE: &str = include_str!("../../fixtures/compound-v2-oracle/current-rate.bin-runtime");
const LEGACY_RATE: &str = include_str!("../../fixtures/compound-v2-oracle/legacy-rate.bin-runtime");
const LEGACY_JUMP: &str = include_str!("../../fixtures/compound-v2-oracle/legacy-jump-rate.bin-runtime");

fn max() -> BigUint {
    (u(1) << 256u32) - u(1)
}
fn call(runtime: &str, selector: &str, args: &[BigUint], storage: &[BigUint]) -> std::result::Result<Vec<BigUint>, Vec<u8>> {
    let mut calldata = decode(selector);
    for arg in args {
        calldata.extend(word(arg));
    }
    execute(&decode(runtime), &calldata, storage).map(|bytes| {
        assert_eq!(bytes.len() % 32, 0, "malformed oracle ABI output");
        bytes.chunks_exact(32).map(BigUint::from_bytes_be).collect()
    })
}

// A checked-math error return is distinct from an EVM revert. Both prohibit
// publishing a modeled value; neither becomes a successful zero balance.
fn compare(actual: Result<BigUint>, oracle: std::result::Result<Vec<BigUint>, Vec<u8>>, error_word: bool, label: &str) {
    match oracle {
        Ok(values) => {
            assert_eq!(values.len(), if error_word { 2 } else { 1 }, "{label}");
            if error_word && !values[0].is_zero() {
                assert!(actual.is_err(), "{label}: legacy error {} accepted", values[0]);
                assert!(values[1].is_zero(), "legacy source clears the failure value");
            } else {
                assert_eq!(actual, Ok(values[usize::from(error_word)].clone()), "{label}");
            }
        }
        Err(payload) => {
            assert!(actual.is_err(), "{label}: Solidity refused but model returned {actual:?}");
            assert!(
                payload.starts_with(&decode("4e487b71")) || payload.starts_with(&decode("08c379a0")),
                "unexpected revert {payload:?}: {label}"
            );
        }
    }
}

fn triples() -> Vec<[BigUint; 3]> {
    let mut cases = vec![
        [u(0), u(0), u(0)],
        [max(), u(0), max()],
        [u(0), u(1), u(0)],
        [u(200), u(800), u(0)],
        [u(199_999_999_999_999_999), u(800_000_000_000_000_001), u(0)],
        [u(0), u(2), u(1)],
        [u(0), u(1), u(1)],
        [u(0), u(1), u(2)],
        [max(), u(1), max()],
        [u(0), max() / u(EXP_SCALE), u(0)],
        [u(0), max() / u(EXP_SCALE) + u(1), u(0)],
    ];
    // Reproducible inputs stay independent of the implementation's helpers.
    let mut seed = 0xc070_2026_0928u64;
    for _ in 0..24 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        cases.push([u(seed), u(seed.rotate_left(19)), u(seed % 100)]);
    }
    cases
}

#[test]
fn current_and_legacy_jump_source_executions_match_checked_rate_boundaries() {
    let models = [
        JumpRateModelV2 {
            base_rate_per_block: u(0),
            multiplier_per_block: u(23_782_343_987),
            jump_multiplier_per_block: u(518_455_098_934),
            kink: u(800_000_000_000_000_000),
        },
        JumpRateModelV2 {
            base_rate_per_block: u(0),
            multiplier_per_block: max() / u(EXP_SCALE) + u(1),
            jump_multiplier_per_block: u(0),
            kink: u(EXP_SCALE),
        },
        JumpRateModelV2 {
            base_rate_per_block: max(),
            multiplier_per_block: u(1),
            jump_multiplier_per_block: u(1),
            kink: u(EXP_SCALE),
        },
        JumpRateModelV2 {
            base_rate_per_block: u(0),
            multiplier_per_block: u(0),
            jump_multiplier_per_block: max() / u(EXP_SCALE) + u(1),
            kink: u(0),
        },
        JumpRateModelV2 {
            base_rate_per_block: max(),
            multiplier_per_block: u(0),
            jump_multiplier_per_block: u(1),
            kink: u(0),
        },
        JumpRateModelV2 {
            base_rate_per_block: max() / u(EXP_SCALE),
            multiplier_per_block: u(0),
            jump_multiplier_per_block: u(0),
            kink: u(0),
        },
    ];
    let mut comparisons = 0;
    for (runtime, legacy) in [(CURRENT_RATE, false), (LEGACY_JUMP, true)] {
        for model in &models {
            let storage = [
                u(0),
                model.multiplier_per_block.clone(),
                model.base_rate_per_block.clone(),
                model.jump_multiplier_per_block.clone(),
                model.kink.clone(),
            ];
            for args in triples() {
                compare(
                    JumpRateModelV2::utilization(&args[0], &args[1], &args[2]),
                    call(runtime, "6e71e2d8", &args, &storage),
                    false,
                    "utilization",
                );
                compare(
                    model.borrow_rate(&args[0], &args[1], &args[2]),
                    call(runtime, "15f24053", &args, &storage),
                    legacy,
                    "borrow rate",
                );
                comparisons += 2;
                for reserve_factor in [u(0), u(EXP_SCALE), u(EXP_SCALE) + u(1)] {
                    let mut supply_args = args.to_vec();
                    supply_args.push(reserve_factor.clone());
                    compare(
                        model.supply_rate(&args[0], &args[1], &args[2], &reserve_factor),
                        call(runtime, "b8168816", &supply_args, &storage),
                        false,
                        "supply rate",
                    );
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 2_100);
}

#[test]
fn jump_parameter_scaling_matches_both_compiled_source_revisions() {
    let cases = [
        [u(0), u(40_000_000_000_000_000), u(1_090_000_000_000_000_000), u(800_000_000_000_000_000)],
        [max(), max() / u(EXP_SCALE), max(), max() / u(BLOCKS_PER_YEAR)],
        [u(0), max() / u(EXP_SCALE) + u(1), u(0), u(EXP_SCALE)],
        [u(0), u(0), u(0), max() / u(BLOCKS_PER_YEAR) + u(1)],
        [u(0), u(0), u(0), u(0)],
    ];
    for runtime in [CURRENT_RATE, LEGACY_JUMP] {
        for args in &cases {
            let expected = call(runtime, "b26255ba", args, &[u(0), u(0), u(0), u(0), u(0)]);
            let actual = JumpRateModelV2::from_per_year(&args[0], &args[1], &args[2], &args[3]);
            match expected {
                Ok(values) => {
                    let m = actual.unwrap();
                    assert_eq!(values, [m.base_rate_per_block, m.multiplier_per_block, m.jump_multiplier_per_block, m.kink]);
                }
                Err(payload) => {
                    assert!(actual.is_err());
                    assert!(!payload.is_empty());
                }
            }
        }
    }
}

#[test]
fn white_paper_source_errors_are_not_successful_zero_rates() {
    let models = [
        WhitePaper2019 {
            base_rate_per_year: u(0),
            multiplier_per_year: u(200_000_000_000_000_000),
        },
        WhitePaper2019 {
            base_rate_per_year: u(1_892_160),
            multiplier_per_year: u(200_000_000_000_000_000),
        },
        WhitePaper2019 {
            base_rate_per_year: max(),
            multiplier_per_year: u(1),
        },
        WhitePaper2019 {
            base_rate_per_year: u(0),
            multiplier_per_year: max(),
        },
    ];
    for model in models {
        let storage = [model.multiplier_per_year.clone(), model.base_rate_per_year.clone()];
        for args in triples() {
            compare(
                WhitePaper2019::utilization(&args[0], &args[1]),
                call(LEGACY_RATE, "e51446e0", &args[..2], &storage),
                true,
                "WhitePaper utilization",
            );
            compare(
                model.borrow_rate(&args[0], &args[1]),
                call(LEGACY_RATE, "15f24053", &args, &storage),
                true,
                "WhitePaper borrow",
            );
        }
    }
}

fn market(cash: BigUint, borrows: BigUint, reserves: BigUint, supply: BigUint) -> Market {
    Market {
        total_cash: cash,
        total_borrows: borrows,
        total_reserves: reserves,
        total_supply: supply,
        borrow_index: u(EXP_SCALE),
        accrual_block_number: 1,
        reserve_factor_mantissa: u(EXP_SCALE / 10),
        initial_exchange_rate_mantissa: u(EXP_SCALE),
    }
}
fn storage(m: &Market, rate: BigUint, current: u64, rate_error: BigUint) -> Vec<BigUint> {
    vec![
        m.total_cash.clone(),
        m.total_borrows.clone(),
        m.total_reserves.clone(),
        m.total_supply.clone(),
        m.borrow_index.clone(),
        u(m.accrual_block_number),
        m.reserve_factor_mantissa.clone(),
        m.initial_exchange_rate_mantissa.clone(),
        u(current),
        rate,
        rate_error,
    ]
}

#[test]
fn ctoken_exchange_and_underlying_products_match_both_compiled_sources() {
    for (runtime, legacy) in [(CURRENT_MARKET, false), (LEGACY_MARKET, true)] {
        for args in triples() {
            for supply in [u(0), u(1), max()] {
                let m = market(args[0].clone(), args[1].clone(), args[2].clone(), supply);
                compare(
                    m.exchange_rate_stored(),
                    call(runtime, "d2f7265a", &[], &storage(&m, u(0), 1, u(0))),
                    legacy,
                    "exchange",
                );
            }
        }
        // Overflowing scaled numerator followed by a large denominator.
        let m = market(max() / u(EXP_SCALE) + u(1), u(0), u(0), max());
        compare(
            m.exchange_rate_stored(),
            call(runtime, "d2f7265a", &[], &storage(&m, u(0), 1, u(0))),
            legacy,
            "exchange final-fit",
        );
        for rate in [u(0), u(1), u(EXP_SCALE), max() / u(EXP_SCALE) + u(1), max()] {
            for shares in [u(0), u(1), u(2), u(EXP_SCALE), max()] {
                let m = Market {
                    initial_exchange_rate_mantissa: rate.clone(),
                    ..market(u(0), u(0), u(0), u(0))
                };
                let model = JumpRateModelV2 {
                    base_rate_per_block: u(0),
                    multiplier_per_block: u(0),
                    jump_multiplier_per_block: u(0),
                    kink: u(EXP_SCALE),
                };
                compare(
                    underlying_balance_with(
                        &shares,
                        &m,
                        &RateModel::Jump(model),
                        if legacy { CTokenRevision::Legacy2019 } else { CTokenRevision::Current },
                        1,
                    ),
                    call(runtime, "07a8b552", &[rate.clone(), shares], &[]),
                    legacy,
                    "underlying scalar",
                );
            }
        }
    }
}

#[test]
fn ctoken_controlled_rate_accrual_preserves_errors_and_unchanged_failed_state() {
    let base = market(u(10), u(1), u(0), u(1));
    let cases = [
        (market(u(10), u(EXP_SCALE), u(10), u(1)), u(1_000_000_000_000), 11, u(0)),
        (base.clone(), u(0), 2, u(0)),
        (base.clone(), u(1), 2, u(0)),
        (base.clone(), u(LEGACY_2019_BORROW_RATE_MAX_MANTISSA + 1), 1, u(0)),
        (base.clone(), u(1), 1, u(7)),
        (base.clone(), u(1), 0, u(0)),
        (
            Market {
                total_borrows: max() / u(EXP_SCALE),
                ..base.clone()
            },
            u(2),
            1 + EXP_SCALE,
            u(0),
        ),
        (
            Market {
                total_borrows: max(),
                ..base.clone()
            },
            u(1),
            2,
            u(0),
        ),
        (
            Market {
                total_borrows: u(EXP_SCALE),
                reserve_factor_mantissa: max(),
                ..base.clone()
            },
            u(2),
            2,
            u(0),
        ),
        (
            Market {
                total_borrows: u(EXP_SCALE),
                total_reserves: max(),
                reserve_factor_mantissa: u(EXP_SCALE),
                ..base.clone()
            },
            u(1),
            2,
            u(0),
        ),
        (
            Market {
                borrow_index: max(),
                ..base.clone()
            },
            u(2),
            2,
            u(0),
        ),
        (Market { borrow_index: max(), ..base }, u(1), 2, u(0)),
    ];
    for (runtime, revision) in [(CURRENT_MARKET, CTokenRevision::Current), (LEGACY_MARKET, CTokenRevision::Legacy2019)] {
        for (m, rate, current, rate_error) in &cases {
            // The mock's error tuple is legacy-only. Current rate mocks return
            // one uint. Full rate-model execution is covered separately above.
            let expected = call(runtime, "341f141d", &[], &storage(m, rate.clone(), *current, rate_error.clone()));
            let actual = if revision == CTokenRevision::Current && *current == m.accrual_block_number {
                Ok(m.clone())
            } else if rate > &revision.borrow_rate_max_mantissa()
                || *current < m.accrual_block_number
                || (revision == CTokenRevision::Legacy2019 && !rate_error.is_zero())
            {
                Err(Unknown::Invalid("source precondition"))
            } else {
                accrue_at_rate(m, rate, *current)
            };
            match expected {
                Ok(values) => {
                    assert_eq!(values.len(), 5);
                    if values[0].is_zero() {
                        if *current == 11 {
                            assert_eq!(
                                values[1..],
                                [u(1_000_010_000_000_000_000), u(1_000_000_000_010), u(1_000_010_000_000_000_000), u(11)],
                                "ordinary successful accrual writes all projected words"
                            );
                        }
                        let a = actual.unwrap();
                        assert_eq!(values[1..], [a.total_borrows, a.total_reserves, a.borrow_index, u(a.accrual_block_number)]);
                    } else {
                        assert_eq!(revision, CTokenRevision::Legacy2019);
                        assert_eq!(
                            values[0],
                            if rate_error.is_zero() { u(9) } else { u(5) },
                            "legacy MATH_ERROR versus INTEREST_RATE_MODEL_ERROR"
                        );
                        assert!(actual.is_err());
                        assert_eq!(
                            values[1..],
                            [
                                m.total_borrows.clone(),
                                m.total_reserves.clone(),
                                m.borrow_index.clone(),
                                u(m.accrual_block_number)
                            ],
                            "legacy failure leaves market unchanged"
                        );
                    }
                }
                Err(payload) => {
                    assert!(actual.is_err());
                    if revision == CTokenRevision::Current {
                        assert!(!payload.is_empty());
                    }
                    // A backwards legacy block uses assert/INVALID with no
                    // returndata; a cap failure uses Error(string).
                    if payload.is_empty() {
                        assert_eq!(revision, CTokenRevision::Legacy2019);
                        assert!(*current < m.accrual_block_number);
                    }
                }
            }
        }
    }
}
