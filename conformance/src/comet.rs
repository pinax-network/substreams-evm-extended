//! Compound III (Comet) supplied-balance model.
//!
//! Source: `CometWithExtendedAssetList.sol`, `CometCore.sol`, `CometMath.sol`
//! and `CometStorage.sol` at compound-finance/comet
//! `f766f51583c23acc33b2a7824654ef2029a96804` (identical `balanceOf` and index
//! math to the removed legacy `Comet.sol`). Principal is a signed int104;
//! indices are uint64 at `BASE_INDEX_SCALE = 1e15`; rates are per-second
//! factors at `FACTOR_SCALE = 1e18`. Casts revert on overflow.
use crate::{Result, Unknown};
use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{One, Zero};

pub const FACTOR_SCALE: u64 = 1_000_000_000_000_000_000;
pub const BASE_INDEX_SCALE: u64 = 1_000_000_000_000_000;
pub const SECONDS_PER_YEAR: u64 = 31_536_000;
pub const MAX_UINT64: u128 = u64::MAX as u128;

fn u(v: u64) -> BigUint {
    BigUint::from(v)
}
fn fits_uint64(v: &BigUint) -> bool {
    v.bits() <= 64
}
fn fits_uint104(v: &BigUint) -> bool {
    v.bits() <= 104
}
/// int104 range: [-2^103, 2^103 - 1].
pub fn fits_int104(v: &BigInt) -> bool {
    let bound = BigInt::one() << 103u32;
    v >= &-bound.clone() && v < &bound
}

/// `mulFactor(n, factor) = n * factor / FACTOR_SCALE`.
pub fn mul_factor(n: &BigUint, factor: &BigUint) -> BigUint {
    n * factor / u(FACTOR_SCALE)
}
/// `presentValueSupply(index, principal) = principal * index / BASE_INDEX_SCALE`.
pub fn present_value_supply(base_supply_index: u64, principal: &BigUint) -> BigUint {
    principal * u(base_supply_index) / u(BASE_INDEX_SCALE)
}
/// `presentValueBorrow`: same formula with the borrow index (floor).
pub fn present_value_borrow(base_borrow_index: u64, principal: &BigUint) -> BigUint {
    principal * u(base_borrow_index) / u(BASE_INDEX_SCALE)
}
/// `principalValueSupply = safe104(present * BASE_INDEX_SCALE / index)`.
pub fn principal_value_supply(base_supply_index: u64, present: &BigUint) -> Result<BigUint> {
    if base_supply_index == 0 {
        return Err(Unknown::Invalid("zero supply index"));
    }
    let v = present * u(BASE_INDEX_SCALE) / u(base_supply_index);
    if !fits_uint104(&v) {
        return Err(Unknown::Invalid("principal exceeds uint104"));
    }
    Ok(v)
}
/// `principalValueBorrow = safe104((present * BASE_INDEX_SCALE + index - 1) / index)`.
pub fn principal_value_borrow(base_borrow_index: u64, present: &BigUint) -> Result<BigUint> {
    if base_borrow_index == 0 {
        return Err(Unknown::Invalid("zero borrow index"));
    }
    let v = (present * u(BASE_INDEX_SCALE) + u(base_borrow_index) - BigUint::one()) / u(base_borrow_index);
    if !fits_uint104(&v) {
        return Err(Unknown::Invalid("principal exceeds uint104"));
    }
    Ok(v)
}

/// Per-implementation rate immutables (already divided by SECONDS_PER_YEAR
/// in the constructor). All are uint64 factors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RateModel {
    pub supply_kink: u64,
    pub supply_slope_low: u64,
    pub supply_slope_high: u64,
    pub supply_base: u64,
    pub borrow_kink: u64,
    pub borrow_slope_low: u64,
    pub borrow_slope_high: u64,
    pub borrow_base: u64,
}
impl RateModel {
    /// Constructor scaling: `perYear / SECONDS_PER_YEAR` (integer division).
    pub fn from_per_year(kinks: (u64, u64), supply: (u64, u64, u64), borrow: (u64, u64, u64)) -> Self {
        let s = |v: u64| v / SECONDS_PER_YEAR;
        RateModel {
            supply_kink: kinks.0,
            supply_slope_low: s(supply.0),
            supply_slope_high: s(supply.1),
            supply_base: s(supply.2),
            borrow_kink: kinks.1,
            borrow_slope_low: s(borrow.0),
            borrow_slope_high: s(borrow.1),
            borrow_base: s(borrow.2),
        }
    }
    fn kinked(utilization: &BigUint, kink: u64, low: u64, high: u64, base: u64) -> Result<u64> {
        let rate = if utilization <= &u(kink) {
            u(base) + mul_factor(&u(low), utilization)
        } else {
            u(base) + mul_factor(&u(low), &u(kink)) + mul_factor(&u(high), &(utilization - u(kink)))
        };
        if !fits_uint64(&rate) {
            return Err(Unknown::Invalid("rate exceeds uint64"));
        }
        Ok(rate.try_into().unwrap())
    }
    /// `getSupplyRate(utilization)`.
    pub fn supply_rate(&self, utilization: &BigUint) -> Result<u64> {
        Self::kinked(utilization, self.supply_kink, self.supply_slope_low, self.supply_slope_high, self.supply_base)
    }
    /// `getBorrowRate(utilization)`.
    pub fn borrow_rate(&self, utilization: &BigUint) -> Result<u64> {
        Self::kinked(utilization, self.borrow_kink, self.borrow_slope_low, self.borrow_slope_high, self.borrow_base)
    }
}

/// Stored market state (`CometStorage`) plus the rate immutables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Market {
    pub base_supply_index: u64,
    pub base_borrow_index: u64,
    pub total_supply_base: BigUint,
    pub total_borrow_base: BigUint,
    pub last_accrual_time: u64,
    pub rates: RateModel,
}
impl Market {
    /// `getUtilization()` with the STORED indices.
    pub fn utilization(&self) -> BigUint {
        let total_supply = present_value_supply(self.base_supply_index, &self.total_supply_base);
        let total_borrow = present_value_borrow(self.base_borrow_index, &self.total_borrow_base);
        if total_supply.is_zero() {
            BigUint::zero()
        } else {
            total_borrow * u(FACTOR_SCALE) / total_supply
        }
    }
    /// `accruedInterestIndices(timeElapsed)`: stored indices projected to the
    /// evaluation clock.
    pub fn accrued_indices(&self, now: u64) -> Result<(u64, u64)> {
        if now < self.last_accrual_time {
            return Err(Unknown::Invalid("evaluation clock precedes last accrual"));
        }
        let elapsed = now - self.last_accrual_time;
        if elapsed == 0 {
            return Ok((self.base_supply_index, self.base_borrow_index));
        }
        let utilization = self.utilization();
        let supply_rate = self.rates.supply_rate(&utilization)?;
        let borrow_rate = self.rates.borrow_rate(&utilization)?;
        let bump = |index: u64, rate: u64| -> Result<u64> {
            let delta = mul_factor(&u(index), &(u(rate) * u(elapsed)));
            if !fits_uint64(&delta) {
                return Err(Unknown::Invalid("index delta exceeds uint64"));
            }
            let next = index as u128 + u128::try_from(delta).unwrap();
            if next > MAX_UINT64 {
                return Err(Unknown::Invalid("index exceeds uint64"));
            }
            Ok(next as u64)
        };
        Ok((bump(self.base_supply_index, supply_rate)?, bump(self.base_borrow_index, borrow_rate)?))
    }
}

/// `balanceOf(account)` at `now`: supplied base balance, 0 for a borrower.
pub fn balance_of(principal: &BigInt, market: &Market, now: u64) -> Result<BigUint> {
    if !fits_int104(principal) {
        return Err(Unknown::Invalid("principal outside int104"));
    }
    if market.base_supply_index == 0 {
        return Err(Unknown::MissingInput("base supply index"));
    }
    let (supply_index, _) = market.accrued_indices(now)?;
    if principal.sign() != Sign::Plus {
        return Ok(BigUint::zero());
    }
    Ok(present_value_supply(supply_index, principal.magnitude()))
}
/// `borrowBalanceOf(account)` at `now`: debt for a negative principal, else 0.
/// Debt is reported separately and never as an unsigned ERC-20 balance.
pub fn borrow_balance_of(principal: &BigInt, market: &Market, now: u64) -> Result<BigUint> {
    if !fits_int104(principal) {
        return Err(Unknown::Invalid("principal outside int104"));
    }
    let (_, borrow_index) = market.accrued_indices(now)?;
    if principal.sign() != Sign::Minus {
        return Ok(BigUint::zero());
    }
    // `unsigned104(-principal)`: checked negation of int104 min reverts.
    if *principal == -(BigInt::one() << 103u32) {
        return Err(Unknown::Invalid("int104 negation overflow"));
    }
    Ok(present_value_borrow(borrow_index, principal.magnitude()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn market() -> Market {
        // Mainnet cUSDCv3 genesis configuration (deployments/mainnet/usdc/configuration.json)
        // scaled by the constructor; indices and totals are synthetic.
        Market {
            base_supply_index: 1_058_123_456_789_012,
            base_borrow_index: 1_074_987_654_321_098,
            total_supply_base: BigUint::from(1_234_567_890_123_456u64),
            total_borrow_base: BigUint::from(987_654_321_098_765u64),
            last_accrual_time: 1_789_689_600,
            rates: RateModel::from_per_year(
                (800_000_000_000_000_000, 800_000_000_000_000_000),
                (32_500_000_000_000_000, 400_000_000_000_000_000, 0),
                (35_000_000_000_000_000, 250_000_000_000_000_000, 15_000_000_000_000_000),
            ),
        }
    }

    #[test]
    fn constructor_scaling_matches_the_pinned_immutables() {
        let m = market();
        assert_eq!(
            (m.rates.supply_slope_low, m.rates.supply_slope_high, m.rates.supply_base),
            (1_030_568_239, 12_683_916_793, 0)
        );
        assert_eq!(
            (m.rates.borrow_slope_low, m.rates.borrow_slope_high, m.rates.borrow_base),
            (1_109_842_719, 7_927_447_995, 475_646_879)
        );
    }

    #[test]
    fn present_and_principal_values_round_in_the_protocols_directions() {
        let p = BigUint::from(1_000_000u64);
        assert_eq!(present_value_supply(BASE_INDEX_SCALE, &p), p);
        assert_eq!(present_value_supply(1_500_000_000_000_000, &p), BigUint::from(1_500_000u64));
        // Supply principal floors, borrow principal ceils.
        let present = BigUint::from(1_000_001u64);
        assert_eq!(principal_value_supply(1_500_000_000_000_000, &present).unwrap(), BigUint::from(666_667u64));
        assert_eq!(principal_value_borrow(1_500_000_000_000_000, &present).unwrap(), BigUint::from(666_668u64));
        assert!(principal_value_supply(0, &present).is_err());
        let huge = BigUint::one() << 120u32;
        assert!(principal_value_supply(BASE_INDEX_SCALE, &huge).is_err());
    }

    #[test]
    fn kinked_rates_and_utilization_follow_the_stored_indices() {
        let m = market();
        let util = m.utilization();
        assert!(util > BigUint::zero() && util < BigUint::from(FACTOR_SCALE));
        // Exact utilization of the synthetic market with the STORED indices:
        // presentValueBorrow * 1e18 / presentValueSupply.
        assert_eq!(util, BigUint::from(812_750_275_760_136_302u64));
        let below = m.rates.supply_rate(&BigUint::from(400_000_000_000_000_000u64)).unwrap();
        let at_kink = m.rates.supply_rate(&BigUint::from(800_000_000_000_000_000u64)).unwrap();
        let just_above = m.rates.supply_rate(&BigUint::from(800_000_000_000_000_001u64)).unwrap();
        let above = m.rates.supply_rate(&BigUint::from(900_000_000_000_000_000u64)).unwrap();
        assert!(below < at_kink && at_kink < above);
        // At the kink the `<=` branch applies: base + slopeLow * kink.
        assert_eq!(at_kink, 824_454_591);
        // One unit above: the high slope contributes floor(12683916793 * 1 / 1e18) = 0.
        assert_eq!(just_above, 824_454_591);
        // 90% utilization: 0 + 1030568239 * 0.8 + 12683916793 * 0.1, each term floored.
        assert_eq!(above, 2_092_846_270);
        assert_eq!(m.rates.borrow_rate(&BigUint::from(900_000_000_000_000_000u64)).unwrap(), 2_156_265_853);
        assert_eq!(m.rates.borrow_rate(&BigUint::from(800_000_000_000_000_000u64)).unwrap(), 1_363_521_054);
        assert_eq!(m.rates.borrow_rate(&BigUint::zero()).unwrap(), 475_646_879);
        // The market's own rates at its stored utilization.
        assert_eq!(
            (m.rates.supply_rate(&util).unwrap(), m.rates.borrow_rate(&util).unwrap()),
            (986_178_027, 1_464_598_202)
        );
        // Checked casts: a rate above uint64 and slopes that overflow are refusals.
        let saturated = RateModel {
            supply_base: u64::MAX,
            supply_slope_low: 1,
            ..m.rates.clone()
        };
        assert_eq!(
            saturated.supply_rate(&BigUint::from(FACTOR_SCALE)),
            Err(Unknown::Invalid("rate exceeds uint64"))
        );
        assert_eq!(saturated.supply_rate(&BigUint::zero()).unwrap(), u64::MAX);
        let empty = Market {
            total_supply_base: BigUint::zero(),
            ..market()
        };
        assert_eq!(empty.utilization(), BigUint::zero());
    }

    #[test]
    fn indices_project_with_elapsed_time_and_balances_keep_signedness() {
        let m = market();
        let (s0, b0) = m.accrued_indices(m.last_accrual_time).unwrap();
        assert_eq!((s0, b0), (m.base_supply_index, m.base_borrow_index));
        // index += mulFactor(index, rate * elapsed), floored at each step.
        let (s1, b1) = m.accrued_indices(m.last_accrual_time + 3600).unwrap();
        assert_eq!((s1, b1), (1_058_127_213_382_182, 1_074_993_322_251_046));
        let (sy, by) = m.accrued_indices(m.last_accrual_time + SECONDS_PER_YEAR).unwrap();
        assert_eq!((sy, by), (1_091_031_212_963_283, 1_124_638_720_669_845));
        assert!(m.accrued_indices(m.last_accrual_time - 1).is_err());
        // uint64 limits of the projection.
        let at_max = Market {
            base_supply_index: u64::MAX,
            ..market()
        };
        assert_eq!(at_max.accrued_indices(m.last_accrual_time + 1), Err(Unknown::Invalid("index exceeds uint64")));
        let saturated_rate = Market {
            rates: RateModel {
                supply_base: u64::MAX,
                supply_slope_low: 0,
                supply_slope_high: 0,
                ..m.rates.clone()
            },
            ..market()
        };
        assert_eq!(
            saturated_rate.accrued_indices(m.last_accrual_time + 2_000),
            Err(Unknown::Invalid("index delta exceeds uint64"))
        );
        let principal = BigInt::from(5_000_000u64);
        let at_accrual = balance_of(&principal, &m, m.last_accrual_time).unwrap();
        assert_eq!(at_accrual, present_value_supply(m.base_supply_index, &BigUint::from(5_000_000u64)));
        assert!(balance_of(&principal, &m, m.last_accrual_time + 3600).unwrap() > at_accrual);
        // Negative principal: supplied balance is zero, debt is positive.
        let debt = BigInt::from(-5_000_000i64);
        assert_eq!(balance_of(&debt, &m, m.last_accrual_time).unwrap(), BigUint::zero());
        assert!(borrow_balance_of(&debt, &m, m.last_accrual_time).unwrap() > BigUint::zero());
        assert_eq!(borrow_balance_of(&principal, &m, m.last_accrual_time).unwrap(), BigUint::zero());
        // int104 bounds and zero supply/borrow crossings.
        let too_big = BigInt::one() << 103u32;
        assert!(balance_of(&too_big, &m, m.last_accrual_time).is_err());
        let int104_min = -(BigInt::one() << 103u32);
        assert_eq!(balance_of(&int104_min, &m, m.last_accrual_time).unwrap(), BigUint::zero());
        // `unsigned104(-principal)` reverts for int104 min in the pinned source.
        assert_eq!(
            borrow_balance_of(&int104_min, &m, m.last_accrual_time),
            Err(Unknown::Invalid("int104 negation overflow"))
        );
        let int104_min_plus_one = int104_min + BigInt::one();
        assert!(borrow_balance_of(&int104_min_plus_one, &m, m.last_accrual_time).is_ok());
        assert_eq!(balance_of(&BigInt::zero(), &m, m.last_accrual_time).unwrap(), BigUint::zero());
        let uninitialized = Market {
            base_supply_index: 0,
            ..market()
        };
        assert_eq!(
            balance_of(&principal, &uninitialized, m.last_accrual_time),
            Err(Unknown::MissingInput("base supply index"))
        );
    }
}
