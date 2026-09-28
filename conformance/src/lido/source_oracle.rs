//! Execute pinned solc0.4.24 output; never compile or access a chain in tests.
use super::*;
use crate::erc4626::oz_evm_oracle::{decode, execute_outcome_at, word, OracleExit};

const RUNTIME: &str = include_str!("../../fixtures/lido-oracle/lido.bin-runtime");
fn n(value: u128) -> BigUint {
    value.into()
}
fn pool(total: u128, external: u128, buffered: u128, deposited: u128, cl: u128, pending: u128) -> Pool {
    Pool {
        total_shares: n(total),
        external_shares: n(external),
        buffered_ether: n(buffered),
        deposited_post_report: n(deposited),
        cl_validators_balance: n(cl),
        cl_pending_balance: n(pending),
    }
}
fn call(p: &Pool, selector: &str, args: &[BigUint], holder: &BigUint) -> OracleExit {
    let packed = |low: &BigUint, high: &BigUint| {
        assert!(low.bits() <= 128 && high.bits() <= 128, "oracle input would spill out of a packed uint128 lane");
        low + (high << 128u32)
    };
    assert!(holder.bits() <= 256, "oracle holder input exceeds uint256");
    let storage = [
        packed(&p.total_shares, &p.external_shares),
        packed(&p.buffered_ether, &p.deposited_post_report),
        packed(&p.cl_validators_balance, &p.cl_pending_balance),
        holder.clone(),
    ];
    let mut calldata = decode(selector);
    for arg in args {
        calldata.extend(word(arg));
    }
    execute_outcome_at(&decode(RUNTIME), &calldata, &storage, &n(0))
}
fn error(reason: &str) -> Vec<u8> {
    let mut payload = decode("08c379a0");
    payload.extend(word(&n(32)));
    payload.extend(word(&n(reason.len() as u128)));
    payload.extend(reason.as_bytes());
    payload.resize(4 + 64 + reason.len().div_ceil(32) * 32, 0);
    payload
}
fn returned(result: OracleExit) -> BigUint {
    let OracleExit::Return(bytes) = result else {
        panic!("source getter must return, not revert/INVALID: {result:?}");
    };
    assert_eq!(bytes.len(), 32, "source ABI return width");
    BigUint::from_bytes_be(&bytes)
}

#[test]
fn source_holder_product_wraps_before_division() {
    let p = pool(u128::MAX, 0, u128::MAX, u128::MAX, 0, 0);
    let shares = n(u128::MAX - 1);
    let from_shares = returned(call(&p, "7a28fb88", std::slice::from_ref(&shares), &shares));
    let from_holder = returned(call(&p, "70a08231", &[n(1)], &shares));
    assert_eq!(from_shares, (n(1) << 128u32) - n(6));
    assert_eq!(from_holder, from_shares);
    assert_eq!(p.pooled_eth_by_shares(&shares), Ok(from_shares));
    assert_eq!(balance_of(Some(&shares), &p), Ok(from_holder));
}

#[test]
fn source_external_product_wraps_before_division() {
    let p = pool(u128::MAX, u128::MAX - 2, u128::MAX, u128::MAX, 0, 0);
    let external = returned(call(&p, "d87db880", &[], &n(0)));
    let total = returned(call(&p, "37cfdaca", &[], &n(0)));
    assert_eq!(external, (n(1) << 255u32) - (n(1) << 130u32) + n(3));
    assert_eq!(p.external_ether(), Ok(external));
    assert_eq!(p.total_pooled_ether(), Ok(total));
}

#[test]
fn source_total_addition_uses_checked_math_after_unchecked_product() {
    let half = 1u128 << 127;
    let p = pool(half + 1, half, u128::MAX, u128::MAX, 0, 0);
    assert_eq!(returned(call(&p, "d87db880", &[], &n(0))), (n(1) << 256u32) - (n(1) << 128u32));
    assert_eq!(call(&p, "37cfdaca", &[], &n(0)), OracleExit::Revert(error("MATH_ADD_OVERFLOW")));
    assert_eq!(p.total_pooled_ether(), Err(Unknown::Invalid("MATH_ADD_OVERFLOW")));
    // balanceOf uses the internal rate directly: it never calls the failing
    // total-pooled getter and still has a valid result for this same state.
    assert_eq!(balance_of(Some(&n(1)), &p), Ok(returned(call(&p, "70a08231", &[n(1)], &n(1)))));
}

fn compare(actual: Result<BigUint>, source: OracleExit) {
    match source {
        OracleExit::Return(bytes) => {
            assert_eq!(bytes.len(), 32);
            assert_eq!(actual, Ok(BigUint::from_bytes_be(&bytes)));
        }
        OracleExit::Revert(payload) => {
            // These well-formed, positive-denominator cases may only fail at
            // the checked total addition. Never accept an empty error as proof.
            assert_eq!(payload, error("MATH_ADD_OVERFLOW"));
            assert_eq!(actual, Err(Unknown::Invalid("MATH_ADD_OVERFLOW")));
        }
        OracleExit::Invalid => panic!("unexpected INVALID for a positive-denominator source input"),
    }
}

#[test]
fn all_getters_match_source_for_dust_extrema_and_generated_packed_inputs() {
    let mut pools = vec![
        pool(1000, 200, 100, 20, 50, 30),
        pool(3, 0, 2, 0, 0, 0),
        pool(5, 2, 2, 0, 0, 0),
        pool(1000, 0, 1, 0, 0, 0),
        pool(1000, 0, 1500, 0, 1500, 0),
        pool(u128::MAX, 0, u128::MAX, u128::MAX, u128::MAX, u128::MAX),
        pool(u128::MAX, u128::MAX - 1, u128::MAX, u128::MAX, u128::MAX, u128::MAX),
        pool((1u128 << 127) + 1, 1u128 << 127, u128::MAX, u128::MAX, 0, 0),
    ];
    let mut seed = 0x11d0_2026_0928_u128;
    let mut next = || {
        seed ^= seed << 23;
        seed ^= seed >> 17;
        seed ^= seed << 26;
        seed
    };
    for _ in 0..64 {
        let total = next().max(1);
        pools.push(pool(total, next() % total, next(), next(), next(), next()));
    }
    let mut calls = 0;
    for p in pools {
        assert!(!p.internal_ether().unwrap().is_zero());
        for (selector, expected) in [
            ("e25100a6", p.internal_ether()),
            ("599faac2", p.internal_shares()),
            ("d87db880", p.external_ether()),
            ("37cfdaca", p.total_pooled_ether()),
        ] {
            compare(expected, call(&p, selector, &[], &n(0)));
            calls += 1;
        }
        for amount in [n(0), n(1), n(u128::MAX - 1)] {
            compare(p.pooled_eth_by_shares(&amount), call(&p, "7a28fb88", std::slice::from_ref(&amount), &n(0)));
            compare(p.shares_by_pooled_eth(&amount), call(&p, "19208451", std::slice::from_ref(&amount), &n(0)));
            // The controlled holder is never above the modeled total shares.
            let holder = amount.min(p.total_shares.clone());
            compare(balance_of(Some(&holder), &p), call(&p, "70a08231", &[n(1)], &holder));
            calls += 3;
        }
    }
    assert_eq!(calls, 936);
}

#[test]
fn source_argument_limits_run_before_zero_denominators() {
    let p = pool(0, 0, 0, 0, 0, 0);
    for amount in [n(u128::MAX), n(1) << 128u32, (n(1) << 256u32) - n(1)] {
        assert_eq!(
            call(&p, "7a28fb88", std::slice::from_ref(&amount), &n(0)),
            OracleExit::Revert(error("SHARES_TOO_LARGE"))
        );
        assert_eq!(p.pooled_eth_by_shares(&amount), Err(Unknown::Invalid("SHARES_TOO_LARGE")));
        assert_eq!(
            call(&p, "19208451", std::slice::from_ref(&amount), &n(0)),
            OracleExit::Revert(error("ETH_TOO_LARGE"))
        );
        assert_eq!(p.shares_by_pooled_eth(&amount), Err(Unknown::Invalid("ETH_TOO_LARGE")));
        assert_eq!(call(&p, "70a08231", &[n(1)], &amount), OracleExit::Revert(error("SHARES_TOO_LARGE")));
        assert_eq!(balance_of(Some(&amount), &p), Err(Unknown::Invalid("SHARES_TOO_LARGE")));
    }
}

#[test]
fn source_underflow_does_not_weaken_the_conservative_external_share_guard() {
    let p = pool(5, 6, 1, 0, 0, 0);
    // Solidity has an unchecked subtraction; this deliberately unsupported
    // state remains refused by the model instead of being called qualified.
    assert_eq!(returned(call(&p, "599faac2", &[], &n(0))), (n(1) << 256u32) - n(1));
    assert_eq!(p.internal_shares(), Err(Unknown::Invalid("external shares exceed total shares")));
    assert_eq!(balance_of(None, &p), Err(Unknown::MissingInput("holder shares")));
}

#[test]
fn malformed_harness_inputs_fail_before_execution_or_lane_spill() {
    let mut p = pool(1, 0, 1, 0, 0, 0);
    p.buffered_ether = n(1) << 128u32;
    assert!(std::panic::catch_unwind(|| call(&p, "e25100a6", &[], &n(0))).is_err());
    let p = pool(1, 0, 1, 0, 0, 0);
    assert!(std::panic::catch_unwind(|| call(&p, "70a08231", &[n(1)], &(n(1) << 256u32))).is_err());
}

#[test]
fn actual_projector_derived_totals_match_source_and_keep_all_six_stored_inputs() {
    use proto::pb::evm::balance_state::v1 as pb;
    use substreams_ethereum::pb::eth::v2 as eth;
    let mut params: serde_json::Value = serde_json::from_str(include_str!("../../../lido/balance-state/tests/fixtures/mainnet-steth-v4-epoch.json")).unwrap();
    params["epochs"][0]["activation_block"] = 10.into();
    let config = lido_balance_state::parse(&params.to_string()).unwrap();
    let epoch = &config.epochs[0];
    let half = 1u128 << 127;
    for p in [
        pool(u128::MAX, 0, u128::MAX, u128::MAX, 0, 0),
        pool(u128::MAX, u128::MAX - 2, u128::MAX, u128::MAX, 0, 0),
        pool(half + 1, half, u128::MAX, u128::MAX, 0, 0),
    ] {
        let packed = |low: &BigUint, high: &BigUint| word(&(low + (high << 128u32))).to_vec();
        let input = eth::Block {
            ver: 5,
            number: 10,
            hash: vec![10; 32],
            detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
            header: Some(eth::BlockHeader {
                number: 10,
                parent_hash: vec![9; 32],
                state_root: vec![4; 32],
                timestamp: Some(prost_types::Timestamp { seconds: 100, nanos: 0 }),
                ..Default::default()
            }),
            transaction_traces: vec![eth::TransactionTrace {
                status: eth::TransactionTraceStatus::Succeeded as i32,
                calls: vec![eth::Call {
                    address: epoch.steth.clone(),
                    storage_changes: [
                        (epoch.total_and_external_shares_slot, packed(&p.total_shares, &p.external_shares)),
                        (epoch.buffered_slot, packed(&p.buffered_ether, &p.deposited_post_report)),
                        (epoch.cl_slot, packed(&p.cl_validators_balance, &p.cl_pending_balance)),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(index, (key, new_value))| eth::StorageChange {
                        address: epoch.steth.clone(),
                        key: key.to_vec(),
                        old_value: word(&n(1)).to_vec(),
                        new_value,
                        ordinal: 10 + index as u64,
                    })
                    .collect(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let output = lido_balance_state::project(&input, &config).unwrap();
        assert_eq!(output.clocks[0].spec_revision, 4);
        let rows: Vec<_> = output
            .global_state
            .iter()
            .filter(|g| g.observation == pb::Observation::ObservedWrite as i32)
            .collect();
        assert_eq!(rows.len(), 6);
        for (field, expected) in [
            (pb::StateField::LidoTotalShares, &p.total_shares),
            (pb::StateField::LidoExternalShares, &p.external_shares),
            (pb::StateField::LidoBufferedEther, &p.buffered_ether),
            (pb::StateField::LidoDepositedPostReport, &p.deposited_post_report),
            (pb::StateField::LidoClValidatorsBalance, &p.cl_validators_balance),
            (pb::StateField::LidoClPendingBalance, &p.cl_pending_balance),
        ] {
            assert_eq!(rows.iter().find(|g| g.field == field as i32).unwrap().value, expected.to_string());
        }
        let derived: Vec<_> = output
            .global_state
            .iter()
            .filter(|g| g.observation == pb::Observation::Derived as i32)
            .collect();
        match call(&p, "37cfdaca", &[], &n(0)) {
            OracleExit::Return(bytes) => {
                assert_eq!(derived.len(), 1);
                assert_eq!(derived[0].value, BigUint::from_bytes_be(&bytes).to_string());
            }
            OracleExit::Revert(payload) => {
                assert_eq!(payload, error("MATH_ADD_OVERFLOW"));
                assert!(derived.is_empty());
            }
            OracleExit::Invalid => panic!("unexpected source INVALID for projector inputs"),
        }
    }
}

#[test]
fn source_zero_denominators_are_invalid_and_never_an_empty_revert_or_successful_zero() {
    let empty_revert = call(&pool(0, 0, 0, 0, 0, 0), "deadbeef", &[], &n(0));
    assert_eq!(
        empty_revert,
        OracleExit::Revert(Vec::new()),
        "wrong-selector control is not an arithmetic INVALID"
    );
    let mut calls = 1;
    for p in [pool(5, 5, 1, 0, 0, 0), pool(0, 0, 1, 0, 0, 0), pool(5, 0, 0, 0, 0, 0), pool(0, 0, 0, 0, 0, 0)] {
        let no_shares = p.internal_shares().unwrap().is_zero();
        let no_ether = p.internal_ether().unwrap().is_zero();
        let forward = if no_shares {
            OracleExit::Invalid
        } else {
            OracleExit::Return(word(&n(0)).to_vec())
        };
        let inverse = if no_ether {
            OracleExit::Invalid
        } else {
            OracleExit::Return(word(&n(0)).to_vec())
        };
        for amount in [n(0), n(1)] {
            assert_eq!(call(&p, "7a28fb88", std::slice::from_ref(&amount), &n(0)), forward);
            assert_eq!(call(&p, "70a08231", &[n(1)], &amount), forward);
            assert_eq!(call(&p, "19208451", std::slice::from_ref(&amount), &n(0)), inverse);
            let expected = if no_shares { Err(Unknown::Invalid("zero internal shares")) } else { Ok(n(0)) };
            assert_eq!(p.pooled_eth_by_shares(&amount), expected);
            assert_eq!(balance_of(Some(&amount), &p), expected);
            assert_eq!(
                p.shares_by_pooled_eth(&amount),
                if no_ether { Err(Unknown::Invalid("zero internal ether")) } else { Ok(n(0)) }
            );
            calls += 3;
        }
        assert_eq!(call(&p, "d87db880", &[], &n(0)), forward);
        assert_eq!(call(&p, "37cfdaca", &[], &n(0)), forward);
        let expected = if no_shares { Err(Unknown::Invalid("zero internal shares")) } else { Ok(n(0)) };
        assert_eq!(p.external_ether(), expected);
        assert_eq!(p.total_pooled_ether(), expected);
        calls += 2;
    }
    assert_eq!(calls, 33);
}
