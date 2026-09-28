//! Boundaries from the pinned Solidity operation order, including intermediates
//! which overflow even though an unbounded final quotient would fit uint256.
use super::*;

fn max() -> BigUint {
    (u(1) << 256u32) - u(1)
}
fn flat(rate: BigUint) -> JumpRateModelV2 {
    JumpRateModelV2 {
        base_rate_per_block: rate,
        multiplier_per_block: u(0),
        jump_multiplier_per_block: u(0),
        kink: u(EXP_SCALE),
    }
}
fn market() -> Market {
    Market {
        total_cash: u(0),
        total_borrows: u(1),
        total_reserves: u(0),
        total_supply: u(1),
        borrow_index: u(EXP_SCALE),
        accrual_block_number: 1,
        reserve_factor_mantissa: u(0),
        initial_exchange_rate_mantissa: u(EXP_SCALE),
    }
}

#[test]
fn exchange_rate_checks_addition_before_reserve_subtraction() {
    let mut m = market();
    m.total_cash = max();
    m.total_reserves = max();
    assert!(m.exchange_rate_stored().is_err(), "MAX + 1 - MAX must fail before subtraction");
    m.total_cash -= u(1);
    assert_eq!(m.exchange_rate_stored().unwrap(), u(0));
    m.total_supply = u(0);
    m.total_cash = max();
    assert_eq!(m.exchange_rate_stored().unwrap(), u(EXP_SCALE), "zero supply skips total arithmetic");
}

#[test]
fn utilization_checks_scaled_numerator_and_denominator_addition() {
    assert!(JumpRateModelV2::utilization(&u(0), &(max() / u(EXP_SCALE) + u(1)), &u(0)).is_err());
    assert!(JumpRateModelV2::utilization(&max(), &u(1), &max()).is_err());
    assert_eq!(JumpRateModelV2::utilization(&max(), &u(0), &max()).unwrap(), u(0));
    assert_eq!(JumpRateModelV2::utilization(&u(0), &(max() / u(EXP_SCALE)), &u(0)).unwrap(), u(EXP_SCALE));
}

#[test]
fn annual_parameter_scaling_checks_both_products_before_division() {
    assert!(JumpRateModelV2::from_per_year(&u(0), &(max() / u(EXP_SCALE) + u(1)), &u(0), &u(EXP_SCALE)).is_err());
    assert!(JumpRateModelV2::from_per_year(&u(0), &u(0), &u(0), &(max() / u(BLOCKS_PER_YEAR) + u(1))).is_err());
    assert!(JumpRateModelV2::from_per_year(&max(), &(max() / u(EXP_SCALE)), &max(), &(max() / u(BLOCKS_PER_YEAR))).is_ok());
}

#[test]
fn borrow_rate_checks_normal_and_jump_products_and_additions() {
    let mut m = flat(u(0));
    m.multiplier_per_block = max() / u(EXP_SCALE) + u(1);
    assert!(m.borrow_rate(&u(0), &u(1), &u(0)).is_err(), "normal product");
    m.multiplier_per_block = u(1);
    m.base_rate_per_block = max();
    assert!(m.borrow_rate(&u(0), &u(1), &u(0)).is_err(), "normal addition");
    m.kink = u(0);
    m.multiplier_per_block = u(0);
    m.base_rate_per_block = u(0);
    m.jump_multiplier_per_block = max() / u(EXP_SCALE) + u(1);
    assert!(m.borrow_rate(&u(0), &u(1), &u(0)).is_err(), "jump product");
    m.base_rate_per_block = max();
    m.jump_multiplier_per_block = u(1);
    assert!(m.borrow_rate(&u(0), &u(1), &u(0)).is_err(), "jump addition");
    m.jump_multiplier_per_block = u(0);
    assert_eq!(m.borrow_rate(&u(0), &u(1), &u(0)).unwrap(), max());
}

#[test]
fn supply_rate_checks_pool_and_utilization_products_before_division() {
    let m = flat(max() / u(EXP_SCALE) + u(1));
    assert!(m.supply_rate(&u(0), &u(1), &u(0), &u(0)).is_err(), "borrow rate times reserve remainder");
    assert!(
        m.supply_rate(&u(0), &u(1), &u(0), &u(EXP_SCALE)).is_ok(),
        "zero pool share skips no required arithmetic but multiplies by zero"
    );
    let m = flat(max() / u(EXP_SCALE));
    assert!(
        m.supply_rate(&u(0), &u(2), &u(1), &u(0)).is_err(),
        "utilization can exceed one when reserves reduce denominator"
    );
}

#[test]
fn accrual_checks_products_even_when_the_truncated_result_would_fit() {
    let model = RateModel::WhitePaper2019(WhitePaper2019 {
        base_rate_per_year: u(BLOCKS_PER_YEAR) * u(2),
        multiplier_per_year: u(0),
    });
    // WhitePaper's utilization also scales borrows, so isolate the accrual
    // product with a large block delta and an otherwise valid utilization.
    for revision in [CTokenRevision::Current, CTokenRevision::Legacy2019] {
        let mut m = market();
        m.total_borrows = max() / u(EXP_SCALE);
        assert!(accrue_with(&m, &model, revision, 1 + EXP_SCALE).is_err(), "interest product");
        m.total_borrows = u(EXP_SCALE);
        m.reserve_factor_mantissa = max();
        assert!(accrue_with(&m, &model, revision, 2).is_err(), "reserve product");
        m.reserve_factor_mantissa = u(0);
        m.borrow_index = max();
        assert!(accrue_with(&m, &model, revision, 2).is_err(), "index product");
    }
}

#[test]
fn accrual_checks_final_reserve_and_index_additions() {
    // Zero borrows lets utilization skip the otherwise invalid cash/reserves
    // denominator. A zero reserve factor still preserves MAX reserves safely.
    let mut m = market();
    m.total_borrows = u(0);
    m.total_reserves = max();
    m.borrow_index = max();
    for revision in [CTokenRevision::Current, CTokenRevision::Legacy2019] {
        assert!(accrue_with(&m, &RateModel::Jump(flat(u(1))), revision, 2).is_err(), "index final addition");
        assert_eq!(accrue_with(&m, &RateModel::Jump(flat(u(0))), revision, 2).unwrap().total_reserves, max());
    }
    m.total_borrows = u(EXP_SCALE);
    m.borrow_index = u(EXP_SCALE);
    m.reserve_factor_mantissa = u(EXP_SCALE);
    let white = RateModel::WhitePaper2019(WhitePaper2019 {
        base_rate_per_year: u(BLOCKS_PER_YEAR),
        multiplier_per_year: u(0),
    });
    assert_eq!(
        accrue_with(&m, &white, CTokenRevision::Legacy2019, 2),
        Err(Unknown::Invalid("total reserves addition overflow"))
    );
    // This isolates CToken's arithmetic stage with a controlled rate. The
    // supplied Jump/WhitePaper models reject MAX borrows earlier at utilization.
    m.total_borrows = max();
    assert_eq!(accrue_at_rate(&m, &u(1), 2), Err(Unknown::Invalid("total borrows addition overflow")));
}

#[test]
fn underlying_conversion_checks_product_and_preserves_revision_short_circuit() {
    let mut m = market();
    m.total_supply = u(0);
    m.initial_exchange_rate_mantissa = max();
    assert!(underlying_balance(&u(2), &m, &flat(u(0)), 1).is_err());
    assert_eq!(underlying_balance(&u(0), &m, &flat(u(0)), 1).unwrap(), u(0));
    m.total_cash = max();
    let model = RateModel::Jump(flat(u(0)));
    assert!(accrue_with(&m, &model, CTokenRevision::Current, 1).is_ok());
    assert!(
        accrue_with(&m, &model, CTokenRevision::Legacy2019, 1).is_err(),
        "legacy evaluates the invalid rate denominator even at zero delta"
    );
    m.total_supply = u(1);
    assert!(
        underlying_balance(&u(0), &m, &flat(u(0)), 1).is_err(),
        "zero shares still evaluates exchange rate"
    );
}
