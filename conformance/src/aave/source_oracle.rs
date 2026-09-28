//! Compiled unchanged Aave function bodies with explicit state/holder bindings.
//! Reserve lanes are legal uint128/uint40 inputs. Pure helper operands are
//! uint256; both pinned aToken holder reads are bounded to uint120.
use super::*;
use crate::erc4626::oz_evm_oracle::{decode, execute_modern_at, word};
use std::{cell::Cell, sync::OnceLock};
use tiny_keccak::{Hasher, Keccak};

#[derive(Clone, Copy)]
pub(crate) enum Contract {
    Current,
    HalfUp,
    Static,
}
thread_local! { static CALLS: Cell<usize> = const { Cell::new(0) }; }
pub(crate) fn counted(count: usize, test: impl FnOnce()) {
    CALLS.set(0);
    test();
    assert_eq!(CALLS.get(), count, "actual EVM executions");
}
pub(crate) fn n(v: u128) -> BigUint {
    BigUint::from(v)
}
pub(crate) fn max() -> BigUint {
    (n(1) << 256u32) - n(1)
}
pub(crate) fn selector(signature: &str) -> [u8; 4] {
    let mut hash = [0u8; 32];
    let mut keccak = Keccak::v256();
    keccak.update(signature.as_bytes());
    keccak.finalize(&mut hash);
    hash[..4].try_into().unwrap()
}
pub(crate) fn panic_payload(code: u128) -> Vec<u8> {
    let mut payload = selector("Panic(uint256)").to_vec();
    payload.extend(word(&n(code)));
    payload
}
fn cast_payload(value: &BigUint) -> Vec<u8> {
    let mut payload = selector("SafeCastOverflowedUintDowncast(uint8,uint256)").to_vec();
    payload.extend(word(&n(128)));
    payload.extend(word(value));
    payload
}

pub(crate) fn compare(model: Result<BigUint>, source: std::result::Result<BigUint, Vec<u8>>) {
    match (model, source) {
        (Ok(model), Ok(source)) => assert_eq!(model, source),
        (Err(Unknown::Invalid(reason)), Err(payload)) => {
            let expected = match reason {
                "rayMul overflow" | "rayMulFloor overflow" | "rayMulCeil overflow" | "rayDiv by zero" | "rayDiv overflow" => Vec::new(),
                "linear interest overflow" | "evaluation clock precedes last update" | "uint256 overflow" | "uint256 underflow" => panic_payload(0x11),
                "division by zero" => panic_payload(0x12),
                _ => panic!("unclassified source error: {reason}"),
            };
            assert_eq!(payload, expected, "source-specific error for {reason}");
        }
        (model, source) => panic!("model={model:?}, source={source:?}"),
    }
}
pub(crate) fn storage(contract: Contract, reserve: &Reserve, shares: &BigUint, liquid: &BigUint, active: bool, paused: bool) -> Vec<BigUint> {
    assert!(
        reserve.liquidity_index.bits() <= 128 && reserve.current_liquidity_rate.bits() <= 128,
        "reserve lane exceeds uint128"
    );
    assert!(reserve.last_update_timestamp < 1u64 << 40, "reserve timestamp exceeds uint40");
    let width = match contract {
        Contract::Current | Contract::HalfUp => 120,
        Contract::Static => 256,
    };
    assert!(shares.bits() <= width, "bound holder exceeds source storage width");
    assert!(liquid.bits() <= 256, "liquidity exceeds uint256");
    let mut values = vec![n(0); 13];
    values[1] = &reserve.liquidity_index + (&reserve.current_liquidity_rate << 128u32);
    values[3] = n(reserve.last_update_timestamp.into()) << 128u32;
    values[10] = shares.clone();
    values[11] = liquid.clone();
    values[12] = n(u128::from(active)) + (n(u128::from(paused)) << 8u32);
    values
}
pub(crate) fn call(contract: Contract, signature: &str, args: &[BigUint], storage: &[BigUint], timestamp: u64) -> std::result::Result<BigUint, Vec<u8>> {
    static CURRENT: OnceLock<Vec<u8>> = OnceLock::new();
    static OLD: OnceLock<Vec<u8>> = OnceLock::new();
    static STATIC: OnceLock<Vec<u8>> = OnceLock::new();
    let code = match contract {
        Contract::Current => CURRENT.get_or_init(|| decode(include_str!("../../fixtures/aave-static-oracle/AaveOracle.bin-runtime"))),
        Contract::HalfUp => OLD.get_or_init(|| decode(include_str!("../../fixtures/aave-static-oracle/HalfUpOracle.bin-runtime"))),
        Contract::Static => STATIC.get_or_init(|| decode(include_str!("../../fixtures/aave-static-oracle/StaticOracle.bin-runtime"))),
    };
    let mut calldata = selector(signature).to_vec();
    for arg in args {
        calldata.extend(word(arg));
    }
    assert!(storage.iter().all(|value| value.bits() <= 256));
    CALLS.set(CALLS.get() + 1);
    execute_modern_at(code, &calldata, storage, &n(timestamp.into())).map(|output| {
        assert_eq!(output.len(), 32, "malformed ABI return");
        BigUint::from_bytes_be(&output)
    })
}

#[test]
fn source_stored_index_cast_rejects_wide_projection() {
    counted(2, || {
        let limit = n(1) << 128u32;
        let reserve = Reserve {
            liquidity_index: &limit - n(1),
            current_liquidity_rate: ray(),
            last_update_timestamp: 0,
        };
        let values = storage(Contract::Current, &reserve, &n(0), &n(0), false, false);
        let projected = &limit * n(2) - n(2);
        assert_eq!(
            call(Contract::Current, "normalizedIncome()", &[], &values, SECONDS_PER_YEAR),
            Ok(projected.clone())
        );
        assert_eq!(
            call(Contract::Current, "updatedIndex()", &[], &values, SECONDS_PER_YEAR),
            Err(cast_payload(&projected))
        );
        assert_eq!(
            next_liquidity_index(&reserve, SECONDS_PER_YEAR),
            Err(Unknown::Invalid("liquidity index uint128 overflow"))
        );
    });
}

#[test]
fn source_static_divide_overflow_precedes_zero_division() {
    counted(2, || {
        for (signature, model) in [
            (
                "divDown(uint256,uint256)",
                crate::erc4626::ray_div_round_down as fn(&BigUint, &BigUint) -> Result<BigUint>,
            ),
            ("divUp(uint256,uint256)", crate::erc4626::ray_div_round_up),
        ] {
            assert_eq!(call(Contract::Static, signature, &[max(), n(0)], &[], 0), Err(panic_payload(0x11)));
            assert_eq!(model(&max(), &n(0)), Err(Unknown::Invalid("uint256 overflow")));
        }
    });
}

#[test]
fn source_static_round_up_subtraction_precedes_zero_division() {
    counted(1, || {
        assert_eq!(
            call(Contract::Static, "divUp(uint256,uint256)", &[n(0), n(0)], &[], 0),
            Err(panic_payload(0x11))
        );
        assert_eq!(crate::erc4626::ray_div_round_up(&n(0), &n(0)), Err(Unknown::Invalid("uint256 underflow")));
    });
}

#[test]
fn source_ray_helpers_cover_ties_dust_and_checked_intermediates() {
    counted(636, || {
        let values = [
            n(0),
            n(1),
            half_ray() - n(1),
            half_ray(),
            half_ray() + n(1),
            ray() - n(1),
            ray(),
            ray() + n(1),
            (n(1) << 128u32) - n(1),
            max(),
        ];
        let mut pairs: Vec<_> = values.iter().flat_map(|a| values.iter().map(move |b| (a.clone(), b.clone()))).collect();
        pairs.extend([
            ((max() - half_ray()) / n(2), n(2)),
            ((max() - half_ray()) / n(2) + n(1), n(2)),
            (max() / n(2), n(2)),
            (max() / n(2) + n(1), n(2)),
            (max() / ray(), ray()),
            (max() / ray() + n(1), ray()),
        ]);
        for (a, b) in pairs {
            for contract in [Contract::Current, Contract::HalfUp] {
                compare(ray_mul(&a, &b), call(contract, "mulHalf(uint256,uint256)", &[a.clone(), b.clone()], &[], 0));
                compare(ray_div(&a, &b), call(contract, "divHalf(uint256,uint256)", &[a.clone(), b.clone()], &[], 0));
            }
            compare(
                ray_mul_floor(&a, &b),
                call(Contract::Current, "mulFloor(uint256,uint256)", &[a.clone(), b.clone()], &[], 0),
            );
            compare(ray_mul_ceil(&a, &b), call(Contract::Current, "mulCeil(uint256,uint256)", &[a, b], &[], 0));
        }
    });
}

#[test]
fn source_linear_interest_checks_clock_and_product_in_both_pins() {
    counted(70, || {
        for rate in [n(0), n(1), ray(), (n(1) << 128u32) - n(1), max()] {
            for (last, now) in [(0, 0), (0, 1), (0, SECONDS_PER_YEAR), (5, 4), (5, 5), (5, 10), ((1u64 << 40) - 1, u64::MAX)] {
                for contract in [Contract::Current, Contract::HalfUp] {
                    compare(
                        linear_interest(&rate, last, now),
                        call(contract, "linear(uint256,uint40)", &[rate.clone(), n(last.into())], &[], now),
                    );
                }
            }
        }
    });
}

#[test]
fn source_reserve_getters_and_liquidity_updates_preserve_width_and_clock_order() {
    counted(1512, || {
        let limit = n(1) << 128u32;
        for index in [n(0), n(1), ray(), &limit / n(2) - n(1), &limit / n(2), &limit - n(1)] {
            for rate in [n(0), n(1), ray(), &limit - n(1)] {
                for (last, now) in [(0, 0), (0, 1), (0, SECONDS_PER_YEAR), (8, 7), (8, 8), (8, 100), ((1u64 << 40) - 1, u64::MAX)] {
                    let reserve = Reserve {
                        liquidity_index: index.clone(),
                        current_liquidity_rate: rate.clone(),
                        last_update_timestamp: last,
                    };
                    for contract in [Contract::Current, Contract::HalfUp] {
                        let values = storage(contract, &reserve, &n(0), &n(0), false, false);
                        compare(reserve.normalized_income(now), call(contract, "normalizedIncome()", &[], &values, now));
                        for shares in [n(0), n(1), (n(1) << 120u32) - n(1)] {
                            let values = storage(contract, &reserve, &shares, &n(0), false, false);
                            let era = match contract {
                                Contract::Current => Era::Floor,
                                Contract::HalfUp => Era::HalfUp,
                                Contract::Static => unreachable!(),
                            };
                            compare(
                                balance_of(&shares, &reserve, now, era),
                                call(contract, "balanceOf(address)", &[n(1)], &values, now),
                            );
                        }
                    }
                    let values = storage(Contract::Current, &reserve, &n(0), &n(0), false, false);
                    let source = call(Contract::Current, "updatedIndex()", &[], &values, now);
                    let model = next_liquidity_index(&reserve, now);
                    if model == Err(Unknown::Invalid("liquidity index uint128 overflow")) {
                        let projected = reserve.normalized_income(now).unwrap();
                        assert!(projected.bits() > 128);
                        assert_eq!(source, Err(cast_payload(&projected)));
                    } else {
                        compare(model, source);
                    }
                }
            }
        }
    });
}

#[test]
fn source_pinned_era_getters_cover_half_ray_ties_and_uint120_holder_boundary() {
    counted(14, || {
        for index in [ray() + half_ray() - n(1), ray() + half_ray(), ray() + half_ray() + n(1)] {
            let reserve = Reserve {
                liquidity_index: index,
                current_liquidity_rate: n(0),
                last_update_timestamp: 1,
            };
            for (contract, era) in [(Contract::Current, Era::Floor), (Contract::HalfUp, Era::HalfUp)] {
                for shares in [n(0), n(1)] {
                    let values = storage(contract, &reserve, &shares, &n(0), false, false);
                    compare(balance_of(&shares, &reserve, 1, era), call(contract, "balanceOf(address)", &[n(1)], &values, 1));
                }
            }
        }
        let shares = (n(1) << 120u32) - n(1);
        for index in [ray(), ray() + half_ray()] {
            let reserve = Reserve {
                liquidity_index: index,
                current_liquidity_rate: n(0),
                last_update_timestamp: 1,
            };
            let values = storage(Contract::HalfUp, &reserve, &shares, &n(0), false, false);
            compare(
                balance_of(&shares, &reserve, 1, Era::HalfUp),
                call(Contract::HalfUp, "balanceOf(address)", &[n(1)], &values, 1),
            );
        }
    });
}

#[test]
fn source_harness_selector_and_packed_layout_controls() {
    for (signatures, layout, is_static) in [
        (
            include_str!("../../fixtures/aave-static-oracle/AaveOracle.signatures"),
            include_str!("../../fixtures/aave-static-oracle/AaveOracle_storage.json"),
            false,
        ),
        (
            include_str!("../../fixtures/aave-static-oracle/HalfUpOracle.signatures"),
            include_str!("../../fixtures/aave-static-oracle/HalfUpOracle_storage.json"),
            false,
        ),
        (
            include_str!("../../fixtures/aave-static-oracle/StaticOracle.signatures"),
            include_str!("../../fixtures/aave-static-oracle/StaticOracle_storage.json"),
            true,
        ),
    ] {
        for line in signatures.lines().filter(|line| line.len() > 10 && line.as_bytes()[8] == b':') {
            let (hex, signature) = line.split_once(": ").unwrap();
            assert_eq!(selector(signature).as_slice(), decode(hex));
        }
        let layout: serde_json::Value = serde_json::from_str(layout).unwrap();
        let fields = layout["storage"].as_array().unwrap();
        assert_eq!(fields[0]["label"], "boundReserve");
        assert_eq!(fields[0]["slot"], "0");
        assert_eq!(fields[1]["label"], "boundShares");
        assert_eq!(fields[1]["slot"], "10");
        let members = layout["types"][fields[0]["type"].as_str().unwrap()]["members"].as_array().unwrap();
        for (label, slot, offset, kind) in [
            ("liquidityIndex", "1", 0, "t_uint128"),
            ("currentLiquidityRate", "1", 16, "t_uint128"),
            ("lastUpdateTimestamp", "3", 16, "t_uint40"),
        ] {
            let field = members.iter().find(|field| field["label"] == label).unwrap();
            assert_eq!(field["slot"], slot);
            assert_eq!(field["offset"], offset);
            assert_eq!(field["type"], kind);
        }
        if is_static {
            for (position, label, slot, offset) in [(2, "boundLiquidity", "11", 0), (3, "boundActive", "12", 0), (4, "boundPaused", "12", 1)] {
                assert_eq!(fields[position]["label"], label);
                assert_eq!(fields[position]["slot"], slot);
                assert_eq!(fields[position]["offset"], offset);
            }
        }
    }
}

#[test]
fn malformed_harness_words_refuse_before_evm_execution() {
    counted(0, || {
        let reserve = Reserve {
            liquidity_index: ray(),
            current_liquidity_rate: ray(),
            last_update_timestamp: 1,
        };
        let bad_reserves = [
            Reserve {
                liquidity_index: n(1) << 128u32,
                ..reserve.clone()
            },
            Reserve {
                current_liquidity_rate: n(1) << 128u32,
                ..reserve.clone()
            },
            Reserve {
                last_update_timestamp: 1u64 << 40,
                ..reserve.clone()
            },
        ];
        for bad in bad_reserves {
            assert!(std::panic::catch_unwind(|| storage(Contract::Current, &bad, &n(0), &n(0), false, false)).is_err());
        }
        for (contract, width) in [(Contract::Current, 120), (Contract::HalfUp, 120), (Contract::Static, 256)] {
            assert!(std::panic::catch_unwind(|| storage(contract, &reserve, &(n(1) << width), &n(0), false, false)).is_err());
        }
        assert!(std::panic::catch_unwind(|| storage(Contract::Static, &reserve, &n(0), &(n(1) << 256u32), false, false)).is_err());
    });
}
