//! Bounded execution of the pinned Comet source compiled by official solc.
//! No compiler, downloads or chain calls run during these tests. Controlled
//! inputs and substitutions are documented in fixtures/comet-oracle/README.md.
use super::*;
use crate::erc4626::oz_evm_oracle::{decode, execute_at, word};
use std::sync::OnceLock;

const RUNTIME: &str = include_str!("../../fixtures/comet-oracle/oracle.bin-runtime");
const SUPPLY_RATE: &str = "d955759d";
const BORROW_RATE: &str = "9fa83b5a";
const UTILIZATION: &str = "7eb71131";
const INDICES: &str = "2b7eafd2";
const BALANCE: &str = "70a08231";
const DEBT: &str = "374c49b4";

fn market() -> Market {
    Market {
        base_supply_index: 1_058_123_456_789_012,
        base_borrow_index: 1_074_987_654_321_098,
        total_supply_base: u(1_234_567_890_123_456),
        total_borrow_base: u(987_654_321_098_765),
        last_accrual_time: 100,
        rates: RateModel::from_per_year(
            (800_000_000_000_000_000, 800_000_000_000_000_000),
            (32_500_000_000_000_000, 400_000_000_000_000_000, 0),
            (35_000_000_000_000_000, 250_000_000_000_000_000, 15_000_000_000_000_000),
        ),
    }
}

fn max(bits: u32) -> BigUint {
    (u(1) << bits) - u(1)
}

fn storage(m: &Market, principal: &BigInt) -> Vec<BigUint> {
    // Never mask an invalid model input into a different valid Solidity input.
    assert!(m.total_supply_base.bits() <= 104 && m.total_borrow_base.bits() <= 104);
    assert!(m.last_accrual_time < (1 << 40));
    assert!(fits_int104(principal));
    let mut words = vec![u(0); 17];
    words[0] = u(m.base_supply_index) | (u(m.base_borrow_index) << 64u32);
    words[1] = &m.total_supply_base | (&m.total_borrow_base << 104u32) | (u(m.last_accrual_time) << 208u32);
    let r = &m.rates;
    for (slot, value) in [
        r.supply_kink,
        r.supply_slope_low,
        r.supply_slope_high,
        r.supply_base,
        r.borrow_kink,
        r.borrow_slope_low,
        r.borrow_slope_high,
        r.borrow_base,
    ]
    .into_iter()
    .enumerate()
    {
        words[8 + slot] = u(value);
    }
    words[16] = if principal.sign() == Sign::Minus {
        (u(1) << 104u32) - principal.magnitude()
    } else {
        principal.magnitude().clone()
    };
    words
}

fn call(selector: &str, args: &[BigUint], words: &[BigUint], now: u64) -> std::result::Result<Vec<BigUint>, Vec<u8>> {
    static CODE: OnceLock<Vec<u8>> = OnceLock::new();
    let mut calldata = decode(selector);
    for arg in args {
        calldata.extend(word(arg));
    }
    execute_at(CODE.get_or_init(|| decode(RUNTIME)), &calldata, words, &u(now)).map(|bytes| {
        assert_eq!(bytes.len() % 32, 0, "malformed oracle ABI output");
        bytes.chunks_exact(32).map(BigUint::from_bytes_be).collect()
    })
}

fn panic_payload(code: u64) -> Vec<u8> {
    let mut payload = decode("4e487b71");
    payload.extend(word(&u(code)));
    payload
}

#[derive(Default)]
struct Counts {
    success: usize,
    refused: usize,
}

fn compare(actual: Result<Vec<BigUint>>, expected: std::result::Result<Vec<BigUint>, Vec<u8>>, counts: &mut Counts, label: &str) {
    match (actual, expected) {
        (Ok(actual), Ok(expected)) => {
            assert_eq!(actual, expected, "{label}");
            counts.success += 1;
        }
        (Err(Unknown::Invalid(reason)), Err(payload)) => {
            // Exact custom-error selectors / panic words only. Empty ABI
            // refusal, unknown opcodes and arbitrary reverts cannot pass.
            let accepted = match reason {
                "Comet timestamp exceeds uint40" => payload == decode("3d32ffdb"),
                "evaluation clock precedes last accrual" | "index exceeds uint64" | "int104 negation overflow" => payload == panic_payload(0x11),
                "index delta exceeds uint64" => payload == decode("e54396a2"),
                "rate exceeds uint64" => payload == decode("e54396a2") || payload == panic_payload(0x11),
                "principal exceeds uint104" => payload == decode("1b8f24aa") || payload == panic_payload(0x11),
                "zero supply index" => payload == panic_payload(0x12),
                "zero borrow index" => payload == panic_payload(0x11) || payload == panic_payload(0x12),
                _ => false,
            };
            assert!(accepted, "{label}: Rust={reason}, unexpected source revert={payload:?}");
            counts.refused += 1;
        }
        (actual, expected) => panic!("{label}: Rust={actual:?}, source={expected:?}"),
    }
}

fn single(value: Result<BigUint>) -> Result<Vec<BigUint>> {
    value.map(|value| vec![value])
}

#[test]
fn oracle_inputs_match_compiler_selectors_and_packed_storage_layout() {
    use tiny_keccak::{Hasher, Keccak};
    let signatures = include_str!("../../fixtures/comet-oracle/oracle.signatures");
    for (selector, signature) in [
        (SUPPLY_RATE, "getSupplyRate(uint256)"),
        (BORROW_RATE, "getBorrowRate(uint256)"),
        (UTILIZATION, "getUtilization()"),
        (INDICES, "indices()"),
        (BALANCE, "balanceOf(address)"),
        (DEBT, "borrowBalanceOf(address)"),
        ("b2fc9add", "presentSupply(uint64,uint104)"),
        ("9a835005", "presentBorrow(uint64,uint104)"),
        ("6b42349c", "principalSupply(uint64,uint256)"),
        ("02a53999", "principalBorrow(uint64,uint256)"),
    ] {
        let mut hasher = Keccak::v256();
        hasher.update(signature.as_bytes());
        let mut hash = [0; 32];
        hasher.finalize(&mut hash);
        assert_eq!(&hash[..4], decode(selector));
        assert!(signatures.contains(&format!("{selector}: {signature}\n")));
    }
    let layout: serde_json::Value = serde_json::from_str(include_str!("../../fixtures/comet-oracle/storage-layout.json")).unwrap();
    for (label, slot, offset, kind) in [
        ("baseSupplyIndex", "0", 0, "t_uint64"),
        ("baseBorrowIndex", "0", 8, "t_uint64"),
        ("totalSupplyBase", "1", 0, "t_uint104"),
        ("totalBorrowBase", "1", 13, "t_uint104"),
        ("lastAccrualTime", "1", 26, "t_uint40"),
        ("supplyKink", "8", 0, "t_uint256"),
        ("supplyPerSecondInterestRateSlopeLow", "9", 0, "t_uint256"),
        ("supplyPerSecondInterestRateSlopeHigh", "10", 0, "t_uint256"),
        ("supplyPerSecondInterestRateBase", "11", 0, "t_uint256"),
        ("borrowKink", "12", 0, "t_uint256"),
        ("borrowPerSecondInterestRateSlopeLow", "13", 0, "t_uint256"),
        ("borrowPerSecondInterestRateSlopeHigh", "14", 0, "t_uint256"),
        ("borrowPerSecondInterestRateBase", "15", 0, "t_uint256"),
        ("boundPrincipal", "16", 0, "t_int104"),
    ] {
        let fields: Vec<_> = layout["storage"].as_array().unwrap().iter().filter(|field| field["label"] == label).collect();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0]["slot"], slot);
        assert_eq!(fields[0]["offset"], offset);
        assert_eq!(fields[0]["type"], kind);
    }
    assert_eq!(decode(RUNTIME).len(), 2_841);
}

#[test]
fn compiled_rates_cover_kinks_floors_and_checked_casts() {
    let mut rates = vec![market().rates];
    for base in [0, u64::MAX] {
        rates.push(RateModel {
            supply_kink: FACTOR_SCALE / 2,
            supply_slope_low: u64::MAX,
            supply_slope_high: u64::MAX,
            supply_base: base,
            borrow_kink: FACTOR_SCALE,
            borrow_slope_low: 1,
            borrow_slope_high: u64::MAX,
            borrow_base: base,
        });
    }
    rates.push(RateModel {
        supply_kink: 0,
        supply_slope_low: 0,
        supply_slope_high: 0,
        supply_base: 0,
        borrow_kink: 0,
        borrow_slope_low: 0,
        borrow_slope_high: 0,
        borrow_base: 0,
    });
    let mut counts = Counts::default();
    for rate in rates {
        let m = Market { rates: rate, ..market() };
        let mut inputs = vec![u(0), u(1), u(FACTOR_SCALE), max(64), max(128), max(256)];
        for kink in [m.rates.supply_kink, m.rates.borrow_kink] {
            inputs.extend([u(kink.saturating_sub(1)), u(kink), u(kink) + u(1)]);
        }
        for utilization in inputs {
            for (selector, result) in [
                (SUPPLY_RATE, m.rates.supply_rate(&utilization)),
                (BORROW_RATE, m.rates.borrow_rate(&utilization)),
            ] {
                compare(
                    single(result.map(u)),
                    call(selector, std::slice::from_ref(&utilization), &storage(&m, &BigInt::zero()), 100),
                    &mut counts,
                    "rate",
                );
            }
        }
    }
    assert_eq!(counts.success + counts.refused, 96);
    assert!(counts.success > 40 && counts.refused > 10);
}

#[test]
fn compiled_market_paths_match_packed_boundaries_and_deterministic_inputs() {
    let base = market();
    let mut cases = vec![
        base.clone(),
        Market {
            base_supply_index: 0,
            ..base.clone()
        },
        Market {
            base_borrow_index: 0,
            ..base.clone()
        },
        Market {
            total_supply_base: u(0),
            total_borrow_base: max(104),
            ..base.clone()
        },
        Market {
            total_supply_base: u(1),
            total_borrow_base: max(104),
            base_supply_index: BASE_INDEX_SCALE,
            base_borrow_index: u64::MAX,
            ..base.clone()
        },
        Market {
            total_supply_base: max(104),
            total_borrow_base: max(104),
            base_supply_index: u64::MAX,
            base_borrow_index: u64::MAX,
            ..base.clone()
        },
    ];
    let mut seed = 0xc03e_2026_0928u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for i in 0..32 {
        cases.push(Market {
            base_supply_index: next(),
            base_borrow_index: next(),
            total_supply_base: (u(next()) << (i % 40)) + u(1),
            total_borrow_base: u(next()) << (i % 40),
            last_accrual_time: next() % 1_000_000,
            rates: base.rates.clone(),
        });
    }
    let principals = [
        -(BigInt::one() << 103u32),
        BigInt::from(-1),
        BigInt::zero(),
        BigInt::one(),
        (BigInt::one() << 103u32) - BigInt::one(),
    ];
    let mut counts = Counts::default();
    for m in cases {
        compare(
            Ok(vec![m.utilization()]),
            call(UTILIZATION, &[], &storage(&m, &BigInt::zero()), m.last_accrual_time),
            &mut counts,
            "stored utilization",
        );
        for now in [m.last_accrual_time, m.last_accrual_time + 1, m.last_accrual_time + 86_400] {
            compare(
                m.accrued_indices(now).map(|(s, b)| vec![u(s), u(b)]),
                call(INDICES, &[], &storage(&m, &BigInt::zero()), now),
                &mut counts,
                "both indices",
            );
            for principal in &principals {
                let words = storage(&m, principal);
                compare(
                    single(balance_of(principal, &m, now)),
                    call(BALANCE, &[u(1)], &words, now),
                    &mut counts,
                    "supplied balance",
                );
                compare(
                    single(borrow_balance_of(principal, &m, now)),
                    call(DEBT, &[u(1)], &words, now),
                    &mut counts,
                    "debt",
                );
            }
        }
    }
    assert_eq!(counts.success + counts.refused, 1_292);
    assert!(counts.success > 500 && counts.refused > 100);
}

#[test]
fn source_getter_order_checks_clock_and_both_indices_before_principal_sign() {
    let base = market();
    let cases = [
        (
            Market {
                last_accrual_time: (1 << 40) - 1,
                ..base.clone()
            },
            1 << 40,
            decode("3d32ffdb"),
        ),
        (base.clone(), base.last_accrual_time - 1, panic_payload(0x11)),
        (
            Market {
                base_supply_index: u64::MAX,
                ..base.clone()
            },
            101,
            panic_payload(0x11),
        ),
        (
            Market {
                base_borrow_index: u64::MAX,
                ..base.clone()
            },
            101,
            panic_payload(0x11),
        ),
        (
            Market {
                rates: RateModel {
                    supply_base: u64::MAX,
                    supply_slope_low: 0,
                    supply_slope_high: 0,
                    ..base.rates.clone()
                },
                ..base.clone()
            },
            (1 << 40) - 1,
            decode("e54396a2"),
        ),
        (
            Market {
                rates: RateModel {
                    borrow_base: u64::MAX,
                    borrow_slope_low: 0,
                    borrow_slope_high: 0,
                    ..base.rates.clone()
                },
                ..base.clone()
            },
            (1 << 40) - 1,
            decode("e54396a2"),
        ),
    ];
    let mut counts = Counts::default();
    for (m, now, payload) in cases {
        for principal in [-1, 0, 1] {
            let principal = BigInt::from(principal);
            for (selector, result) in [(BALANCE, balance_of(&principal, &m, now)), (DEBT, borrow_balance_of(&principal, &m, now))] {
                let expected = call(selector, &[u(1)], &storage(&m, &principal), now);
                assert_eq!(expected, Err(payload.clone()), "exact pre-branch source failure");
                compare(single(result), expected, &mut counts, "getter pre-branch");
            }
        }
    }
    // Equal time skips rates and index addition, but timestamp must still fit.
    let equal = Market {
        base_supply_index: u64::MAX,
        base_borrow_index: u64::MAX,
        last_accrual_time: (1 << 40) - 1,
        rates: RateModel {
            supply_base: u64::MAX,
            borrow_base: u64::MAX,
            ..base.rates
        },
        ..base
    };
    compare(
        equal.accrued_indices(equal.last_accrual_time).map(|(s, b)| vec![u(s), u(b)]),
        call(INDICES, &[], &storage(&equal, &BigInt::zero()), equal.last_accrual_time),
        &mut counts,
        "same clock skips invalid rates",
    );
    assert_eq!((counts.success, counts.refused), (1, 36));
}

#[test]
fn compiled_core_conversions_cover_width_rounding_and_refusal_boundaries() {
    let mut counts = Counts::default();
    for index in [0, 1, BASE_INDEX_SCALE, BASE_INDEX_SCALE + 1, u64::MAX] {
        for principal in [u(0), u(1), u(1_000_001), max(104)] {
            for (selector, value) in [
                ("b2fc9add", present_value_supply(index, &principal)),
                ("9a835005", present_value_borrow(index, &principal)),
            ] {
                compare(
                    Ok(vec![value]),
                    call(selector, &[u(index), principal.clone()], &[], 0),
                    &mut counts,
                    "present value",
                );
            }
        }
        for present in [u(0), u(1), u(1_000_001), max(104), max(256)] {
            for (selector, value) in [
                ("6b42349c", principal_value_supply(index, &present)),
                ("02a53999", principal_value_borrow(index, &present)),
            ] {
                // Source multiplies before division; zero index plus MAX input
                // overflows multiplication first. Keep that error-order case
                // explicit rather than accepting arbitrary panic payloads.
                let expected = call(selector, &[u(index), present.clone()], &[], 0);
                if index == 0 && present == max(256) {
                    assert!(value.is_err());
                    assert_eq!(expected, Err(panic_payload(0x11)));
                    counts.refused += 1;
                } else {
                    compare(single(value), expected, &mut counts, "principal value");
                }
            }
        }
    }
    assert_eq!(counts.success + counts.refused, 90);
    assert!(counts.success > 50 && counts.refused > 10);
    // Invalid ABI widths are a harness-input refusal, not a model comparison.
    assert_eq!(call("b2fc9add", &[u(1) << 64u32, u(0)], &[], 0), Err(vec![]));
    assert_eq!(call("b2fc9add", &[u(1), u(1) << 104u32], &[], 0), Err(vec![]));
    assert!(std::panic::catch_unwind(|| storage(
        &Market {
            total_supply_base: u(1) << 104u32,
            ..market()
        },
        &BigInt::zero()
    ))
    .is_err());
    assert!(std::panic::catch_unwind(|| storage(&market(), &(BigInt::one() << 103u32))).is_err());
}
