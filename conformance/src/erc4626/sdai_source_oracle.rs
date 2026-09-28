//! Official-solc execution of whole pinned SavingsDai arithmetic functions.
//! Pot values and one holder value are controlled inputs; no external calls,
//! constructors, compiler invocation or downloads occur during these tests.
use super::oz_evm_oracle::{decode, execute_modern_at, word};
use super::*;
use std::sync::OnceLock;

const RUNTIME: &str = include_str!("../../fixtures/sdai-oracle/oracle.bin-runtime");
const ASSETS: &str = "07a2d13a";
const SHARES: &str = "c6e6f592";
const DEPOSIT: &str = "ef8b30f7";
const MINT: &str = "b3d7f6b9";
const WITHDRAW: &str = "0a28a477";
const REDEEM: &str = "4cdad506";
const MAX_WITHDRAW: &str = "ce96cb77";
const RPOW: &str = "8f907195";
const DIVUP: &str = "df98da5e";
const GETTERS: [&str; 7] = [ASSETS, SHARES, DEPOSIT, MINT, WITHDRAW, REDEEM, MAX_WITHDRAW];

fn n(value: u64) -> BigUint {
    BigUint::from(value)
}
fn model(chi: BigUint, dsr: BigUint) -> SavingsDai {
    SavingsDai { chi, rho: n(100), dsr }
}
fn storage(model: &SavingsDai, shares: &BigUint) -> Vec<BigUint> {
    let words = vec![model.chi.clone(), model.rho.clone(), model.dsr.clone(), shares.clone()];
    assert!(words.iter().all(|value| value.bits() <= 256), "oracle inputs must fit their Solidity domain");
    words
}
fn call(selector: &str, args: &[BigUint], storage: &[BigUint], timestamp: u64) -> std::result::Result<BigUint, Vec<u8>> {
    // No malformed selector or truncated calldata can be accepted as an
    // expected empty assembly revert by the comparison helper.
    let argc = match selector {
        RPOW | DIVUP => 2,
        ASSETS | SHARES | DEPOSIT | MINT | WITHDRAW | REDEEM | MAX_WITHDRAW => 1,
        _ => panic!("unrecognized source selector"),
    };
    assert_eq!(args.len(), argc);
    static CODE: OnceLock<Vec<u8>> = OnceLock::new();
    let mut calldata = decode(selector);
    for arg in args {
        calldata.extend(word(arg));
    }
    execute_modern_at(CODE.get_or_init(|| decode(RUNTIME)), &calldata, storage, &n(timestamp)).map(|bytes| {
        assert_eq!(bytes.len(), 32, "malformed source return data");
        BigUint::from_bytes_be(&bytes)
    })
}
fn panic_payload(code: u64) -> Vec<u8> {
    let mut payload = decode("4e487b71");
    payload.extend(word(&n(code)));
    payload
}
#[derive(Default)]
struct Counts {
    success: usize,
    refused: usize,
}
fn compare(model: Result<BigUint>, source: std::result::Result<BigUint, Vec<u8>>, counts: &mut Counts, label: &str) {
    match (model, source) {
        (Ok(model), Ok(source)) => {
            assert_eq!(model, source, "{label}");
            counts.success += 1;
        }
        (Err(Unknown::Invalid("uint256 overflow")), Err(payload)) => {
            // _rpow uses explicit empty REVERT; checked Solidity products
            // use Panic(0x11). Modern INVALID is a harness panic, never here.
            assert!(
                payload.is_empty() || payload == panic_payload(0x11),
                "{label}: unexpected overflow payload {payload:?}"
            );
            counts.refused += 1;
        }
        (Err(Unknown::Invalid("division by zero")), Err(payload)) => {
            assert_eq!(payload, panic_payload(0x12), "{label}");
            counts.refused += 1;
        }
        (model, source) => panic!("{label}: Rust={model:?}, source={source:?}"),
    }
}
fn evaluate(model: &SavingsDai, selector: &str, amount: &BigUint, timestamp: u64) -> Result<BigUint> {
    match selector {
        ASSETS | REDEEM => model.convert_to_assets(amount, timestamp),
        SHARES | DEPOSIT => model.convert_to_shares(amount, timestamp),
        MINT => model.preview_mint(amount, timestamp),
        WITHDRAW => model.preview_withdraw(amount, timestamp),
        MAX_WITHDRAW => model.max_withdraw(amount, timestamp),
        _ => unreachable!(),
    }
}
fn getter(model: &SavingsDai, selector: &str, amount: &BigUint, timestamp: u64) -> std::result::Result<BigUint, Vec<u8>> {
    let argument = if selector == MAX_WITHDRAW { n(1) } else { amount.clone() };
    call(selector, &[argument], &storage(model, amount), timestamp)
}

#[test]
fn compiled_sdai_rpow_covers_zero_rounding_exponent_parity_and_overflow() {
    let mut cases = Vec::new();
    for x in [
        n(0),
        n(1),
        ray() / n(2),
        ray() - n(1),
        ray(),
        ray() + n(1),
        ray() * n(2),
        (n(1) << 128u32) - n(1),
        n(1) << 128u32,
        max_uint256(),
    ] {
        for exponent in [0, 1, 2, 3, 4, 7, 16, 63, u64::MAX] {
            cases.push((x.clone(), exponent));
        }
    }
    let mut seed = 0x5da1_2026_0928u64;
    for i in 0..32 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        cases.push((ray() + n(seed), seed % 65));
        cases.push(((n(seed) << 64u32) + n(seed.rotate_left(29)), i));
    }
    let mut counts = Counts::default();
    for (x, exponent) in cases {
        compare(rpow(&x, exponent), call(RPOW, &[x, n(exponent)], &[], 0), &mut counts, "rpow");
    }
    assert_eq!(counts.success + counts.refused, 154);
    assert!(counts.success > 50 && counts.refused > 10);
}

#[test]
fn compiled_sdai_divup_covers_zero_numerator_and_maximum_words() {
    let mut counts = Counts::default();
    for x in [n(0), n(1), n(2), ray() - n(1), ray(), max_uint256()] {
        for y in [n(0), n(1), n(2), ray() - n(1), ray(), max_uint256()] {
            compare(div_up(&x, &y), call(DIVUP, &[x.clone(), y], &[], 0), &mut counts, "divup");
        }
    }
    assert_eq!((counts.success, counts.refused), (31, 5));
}

#[test]
fn compiled_sdai_conversions_match_explicit_and_generated_pot_inputs() {
    let mut models = vec![
        model(ray(), ray()),
        model(n(0), ray()),
        model(ray(), n(0)),
        model(n(0), max_uint256()),
        model(max_uint256(), ray() * n(2)),
        model(ray() + ray() / n(3), ray() + n(1)),
        SavingsDai {
            rho: n(1) << 64u32,
            ..model(ray(), max_uint256())
        },
        SavingsDai {
            rho: max_uint256(),
            ..model(max_uint256(), max_uint256())
        },
    ];
    let mut seed = 0x5da1_cafe_2026u64;
    for _ in 0..8 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        models.push(model(ray() + n(seed), ray() + n(seed % 1000)));
    }
    let mut counts = Counts::default();
    for model in models {
        for timestamp in [99, 100, 101, 102, u64::MAX] {
            for amount in [n(0), n(1), n(2), ray(), max_uint256() / ray(), max_uint256() / ray() + n(1), max_uint256()] {
                for selector in GETTERS {
                    compare(
                        evaluate(&model, selector, &amount, timestamp),
                        getter(&model, selector, &amount, timestamp),
                        &mut counts,
                        "conversion/preview",
                    );
                }
            }
        }
    }
    assert_eq!(counts.success + counts.refused, 3_920);
    assert!(counts.success > 1_000 && counts.refused > 500);
}

#[test]
fn compiled_sdai_source_order_keeps_projection_and_failure_kinds_exact() {
    let mut counts = Counts::default();
    for (model, timestamp, expected) in [
        (model(n(0), max_uint256()), 102, Err(vec![])),
        (model(max_uint256(), ray() * n(2)), 101, Err(panic_payload(0x11))),
    ] {
        for selector in GETTERS {
            let source = getter(&model, selector, &n(0), timestamp);
            assert_eq!(source, expected, "projection must execute before a zero amount");
            compare(evaluate(&model, selector, &n(0), timestamp), source, &mut counts, "zero-amount projection");
        }
    }
    let zero = model(n(0), ray());
    for selector in [SHARES, DEPOSIT, WITHDRAW] {
        let source = getter(&zero, selector, &max_uint256(), 100);
        assert_eq!(source, Err(panic_payload(0x11)), "multiply before zero denominator");
        compare(evaluate(&zero, selector, &max_uint256(), 100), source, &mut counts, "product precedes division");
    }
    for (model, timestamp) in [(zero, 100), (model(n(0), max_uint256()), 101)] {
        for selector in GETTERS {
            let source = getter(&model, selector, &n(0), timestamp);
            assert_eq!(
                source,
                if [SHARES, DEPOSIT].contains(&selector) {
                    Err(panic_payload(0x12))
                } else {
                    Ok(n(0))
                }
            );
            compare(evaluate(&model, selector, &n(0), timestamp), source, &mut counts, "known zero and exponent one");
        }
    }
    let stored = model(ray(), max_uint256());
    for timestamp in [99, 100] {
        for selector in GETTERS {
            let source = getter(&stored, selector, &n(1), timestamp);
            assert_eq!(source, Ok(n(1)), "before/equal rho skips rpow");
            compare(evaluate(&stored, selector, &n(1), timestamp), source, &mut counts, "stored chi branch");
        }
    }
    assert_eq!(counts.success + counts.refused, 45);
}

#[test]
fn sdai_input_domain_rejects_oversized_rho_without_silently_narrowing() {
    let high = SavingsDai {
        rho: n(1) << 64u32,
        ..model(ray(), max_uint256())
    };
    assert_eq!(high.chi_at(u64::MAX), Ok(ray()));
    let invalid = SavingsDai { rho: n(1) << 256u32, ..high };
    for selector in GETTERS {
        assert_eq!(evaluate(&invalid, selector, &n(0), 100), Err(Unknown::Invalid("uint256 overflow")));
    }
    // Above-ABI-domain values cannot become a different successful source
    // call by truncation; these controls are not counted as parity evidence.
    assert!(std::panic::catch_unwind(|| storage(&invalid, &n(0))).is_err());
    assert!(std::panic::catch_unwind(|| call(RPOW, &[n(1) << 256u32, n(0)], &[], 0)).is_err());
    assert!(std::panic::catch_unwind(|| call(RPOW, &[n(0)], &[], 0)).is_err());
    assert!(std::panic::catch_unwind(|| call("ffffffff", &[], &[], 0)).is_err());
}

#[test]
fn sdai_oracle_inputs_match_compiler_selectors_and_storage_layout() {
    use tiny_keccak::{Hasher, Keccak};
    let signatures = include_str!("../../fixtures/sdai-oracle/oracle.signatures");
    for (selector, signature) in [
        (ASSETS, "convertToAssets(uint256)"),
        (SHARES, "convertToShares(uint256)"),
        (DEPOSIT, "previewDeposit(uint256)"),
        (MINT, "previewMint(uint256)"),
        (WITHDRAW, "previewWithdraw(uint256)"),
        (REDEEM, "previewRedeem(uint256)"),
        (MAX_WITHDRAW, "maxWithdraw(address)"),
        (RPOW, "rpow(uint256,uint256)"),
        (DIVUP, "divup(uint256,uint256)"),
    ] {
        let mut hasher = Keccak::v256();
        hasher.update(signature.as_bytes());
        let mut hash = [0; 32];
        hasher.finalize(&mut hash);
        assert_eq!(&hash[..4], decode(selector));
        assert!(signatures.contains(&format!("{selector}: {signature}\n")));
    }
    let layout: serde_json::Value = serde_json::from_str(include_str!("../../fixtures/sdai-oracle/storage-layout.json")).unwrap();
    let fields = layout["storage"].as_array().unwrap();
    assert_eq!(fields.len(), 4);
    for (i, label) in ["boundChi", "boundRho", "boundDsr", "boundShares"].iter().enumerate() {
        assert_eq!(fields[i]["label"], *label);
        assert_eq!(fields[i]["slot"], i.to_string());
        assert_eq!(fields[i]["offset"], 0);
        assert_eq!(fields[i]["type"], "t_uint256");
    }
    assert_eq!(decode(RUNTIME).len(), 1_369);
}
