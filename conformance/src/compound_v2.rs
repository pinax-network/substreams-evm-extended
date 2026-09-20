//! Compound v2 cToken model.
//!
//! Source: `CToken.sol`, `CErc20.sol`, `CTokenInterfaces.sol`,
//! `ExponentialNoError.sol` and `BaseJumpRateModelV2.sol` at
//! compound-finance/compound-protocol
//! `a3214f67b73310d547e00fc578e8355911c9d376`. Mantissas are 1e18-scaled;
//! accrual is block-number based; `Exp` arithmetic truncates.
use crate::{Result, Unknown};
use num_bigint::BigUint;
use num_traits::Zero;

pub const EXP_SCALE: u64 = 1_000_000_000_000_000_000;
pub const BLOCKS_PER_YEAR: u64 = 2_102_400;
/// `borrowRateMaxMantissa = 0.0005e16`.
pub const BORROW_RATE_MAX_MANTISSA: u64 = 5_000_000_000_000;

fn u(v: u64) -> BigUint {
    BigUint::from(v)
}
fn fits_uint256(v: &BigUint) -> bool {
    v.bits() <= 256
}

/// Stored market state (`CTokenStorage`) plus the cross-contract cash input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Market {
    /// `getCashPrior()`: underlying balance of the cToken (or Pot-derived for
    /// a DSR delegate, or the native balance for CEther). Cross-contract.
    pub total_cash: BigUint,
    pub total_borrows: BigUint,
    pub total_reserves: BigUint,
    pub total_supply: BigUint,
    pub borrow_index: BigUint,
    pub accrual_block_number: u64,
    pub reserve_factor_mantissa: BigUint,
    pub initial_exchange_rate_mantissa: BigUint,
}
impl Market {
    /// `exchangeRateStoredInternal()`.
    pub fn exchange_rate_stored(&self) -> Result<BigUint> {
        if self.total_supply.is_zero() {
            return Ok(self.initial_exchange_rate_mantissa.clone());
        }
        let cash_plus_borrows = &self.total_cash + &self.total_borrows;
        if cash_plus_borrows < self.total_reserves {
            return Err(Unknown::Invalid("reserves exceed cash plus borrows"));
        }
        let numerator = (cash_plus_borrows - &self.total_reserves) * u(EXP_SCALE);
        if !fits_uint256(&numerator) {
            return Err(Unknown::Invalid("exchange rate overflow"));
        }
        Ok(numerator / &self.total_supply)
    }
}

/// `BaseJumpRateModelV2` storage parameters (per-block mantissas).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JumpRateModelV2 {
    pub base_rate_per_block: BigUint,
    pub multiplier_per_block: BigUint,
    pub jump_multiplier_per_block: BigUint,
    pub kink: BigUint,
}
impl JumpRateModelV2 {
    /// `updateJumpRateModelInternal` scaling from per-year parameters.
    pub fn from_per_year(base_rate_per_year: &BigUint, multiplier_per_year: &BigUint, jump_multiplier_per_year: &BigUint, kink: &BigUint) -> Result<Self> {
        if kink.is_zero() {
            return Err(Unknown::Invalid("zero kink"));
        }
        Ok(JumpRateModelV2 {
            base_rate_per_block: base_rate_per_year / u(BLOCKS_PER_YEAR),
            multiplier_per_block: multiplier_per_year * u(EXP_SCALE) / (u(BLOCKS_PER_YEAR) * kink),
            jump_multiplier_per_block: jump_multiplier_per_year / u(BLOCKS_PER_YEAR),
            kink: kink.clone(),
        })
    }
    /// `utilizationRate(cash, borrows, reserves)`.
    pub fn utilization(cash: &BigUint, borrows: &BigUint, reserves: &BigUint) -> Result<BigUint> {
        if borrows.is_zero() {
            return Ok(BigUint::zero());
        }
        let denominator = cash + borrows;
        if denominator < *reserves {
            return Err(Unknown::Invalid("reserves exceed cash plus borrows"));
        }
        let denominator = denominator - reserves;
        if denominator.is_zero() {
            return Err(Unknown::Invalid("zero utilization denominator"));
        }
        Ok(borrows * u(EXP_SCALE) / denominator)
    }
    /// `getBorrowRateInternal`.
    pub fn borrow_rate(&self, cash: &BigUint, borrows: &BigUint, reserves: &BigUint) -> Result<BigUint> {
        let util = Self::utilization(cash, borrows, reserves)?;
        if util <= self.kink {
            Ok(util * &self.multiplier_per_block / u(EXP_SCALE) + &self.base_rate_per_block)
        } else {
            let normal = &self.kink * &self.multiplier_per_block / u(EXP_SCALE) + &self.base_rate_per_block;
            let excess = util - &self.kink;
            Ok(excess * &self.jump_multiplier_per_block / u(EXP_SCALE) + normal)
        }
    }
    /// `getSupplyRate`.
    pub fn supply_rate(&self, cash: &BigUint, borrows: &BigUint, reserves: &BigUint, reserve_factor_mantissa: &BigUint) -> Result<BigUint> {
        if reserve_factor_mantissa > &u(EXP_SCALE) {
            return Err(Unknown::Invalid("reserve factor exceeds one"));
        }
        let one_minus_reserve_factor = u(EXP_SCALE) - reserve_factor_mantissa;
        let borrow_rate = self.borrow_rate(cash, borrows, reserves)?;
        let rate_to_pool = borrow_rate * one_minus_reserve_factor / u(EXP_SCALE);
        Ok(Self::utilization(cash, borrows, reserves)? * rate_to_pool / u(EXP_SCALE))
    }
}

/// `accrueInterest()` projected to `current_block`: the market state a
/// state-changing call would observe, without executing one. Returns the
/// stored state unchanged when the block equals the accrual block.
pub fn accrue(market: &Market, model: &JumpRateModelV2, current_block: u64) -> Result<Market> {
    if current_block < market.accrual_block_number {
        return Err(Unknown::Invalid("evaluation block precedes accrual block"));
    }
    if current_block == market.accrual_block_number {
        return Ok(market.clone());
    }
    let borrow_rate = model.borrow_rate(&market.total_cash, &market.total_borrows, &market.total_reserves)?;
    if borrow_rate > u(BORROW_RATE_MAX_MANTISSA) {
        return Err(Unknown::Invalid("borrow rate is absurdly high"));
    }
    let block_delta = u(current_block - market.accrual_block_number);
    let simple_interest_factor = borrow_rate * block_delta;
    let interest_accumulated = &simple_interest_factor * &market.total_borrows / u(EXP_SCALE);
    let total_borrows_new = &interest_accumulated + &market.total_borrows;
    let total_reserves_new = &market.reserve_factor_mantissa * &interest_accumulated / u(EXP_SCALE) + &market.total_reserves;
    let borrow_index_new = &simple_interest_factor * &market.borrow_index / u(EXP_SCALE) + &market.borrow_index;
    Ok(Market {
        total_borrows: total_borrows_new,
        total_reserves: total_reserves_new,
        borrow_index: borrow_index_new,
        accrual_block_number: current_block,
        ..market.clone()
    })
}

/// `balanceOfUnderlying` semantics without execution: shares times the
/// projected exchange rate, truncated. The share count itself is the ERC-20
/// `balanceOf`.
pub fn underlying_balance(shares: &BigUint, market: &Market, model: &JumpRateModelV2, current_block: u64) -> Result<BigUint> {
    let accrued = accrue(market, model, current_block)?;
    Ok(shares * accrued.exchange_rate_stored()? / u(EXP_SCALE))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn b(s: &str) -> BigUint {
        s.parse().unwrap()
    }
    fn market() -> Market {
        Market {
            total_cash: b("310457889201347"),
            total_borrows: b("250000000000000"),
            total_reserves: b("1000000000000"),
            total_supply: b("2800000000000000"),
            borrow_index: b("1100000000000000000"),
            accrual_block_number: 20_000_000,
            reserve_factor_mantissa: b("75000000000000000"),
            initial_exchange_rate_mantissa: b("200000000000000"),
        }
    }
    fn model() -> JumpRateModelV2 {
        // IRM_USDC_Updateable deployment record: base 0, multiplier 4e16, jump 1.09e18, kink 8e17.
        JumpRateModelV2::from_per_year(&BigUint::zero(), &b("40000000000000000"), &b("1090000000000000000"), &b("800000000000000000")).unwrap()
    }

    #[test]
    fn exchange_rate_uses_cash_borrows_reserves_and_supply_or_the_initial_rate() {
        let m = market();
        assert_eq!(
            m.exchange_rate_stored().unwrap(),
            (b("310457889201347") + b("250000000000000") - b("1000000000000")) * b("1000000000000000000") / b("2800000000000000")
        );
        let empty = Market {
            total_supply: BigUint::zero(),
            ..market()
        };
        assert_eq!(empty.exchange_rate_stored().unwrap(), b("200000000000000"));
        let insolvent = Market {
            total_reserves: b("999999999999999999"),
            ..market()
        };
        assert!(insolvent.exchange_rate_stored().is_err());
    }

    #[test]
    fn donation_only_cash_change_moves_the_exchange_rate_without_any_ctoken_write() {
        let before = market().exchange_rate_stored().unwrap();
        let donated = Market {
            total_cash: b("310457889201347") + b("1000000"),
            ..market()
        };
        assert!(donated.exchange_rate_stored().unwrap() > before);
    }

    #[test]
    fn jump_rate_model_scaling_and_kink_follow_the_pinned_source() {
        let m = model();
        assert_eq!(m.base_rate_per_block, BigUint::zero());
        assert_eq!(
            m.multiplier_per_block,
            b("40000000000000000") * b("1000000000000000000") / (b("2102400") * b("800000000000000000"))
        );
        assert_eq!(m.jump_multiplier_per_block, b("1090000000000000000") / b("2102400"));
        let cash = b("100");
        let low = m.borrow_rate(&cash, &b("50"), &BigUint::zero()).unwrap();
        let high = m.borrow_rate(&cash, &b("900"), &BigUint::zero()).unwrap();
        assert!(high > low);
        assert_eq!(
            JumpRateModelV2::utilization(&cash, &BigUint::zero(), &BigUint::zero()).unwrap(),
            BigUint::zero()
        );
        assert!(m.supply_rate(&cash, &b("50"), &BigUint::zero(), &b("75000000000000000")).unwrap() < low);
        assert!(JumpRateModelV2::from_per_year(&BigUint::zero(), &b("1"), &b("1"), &BigUint::zero()).is_err());
    }

    #[test]
    fn accrual_is_block_based_truncating_and_idempotent_within_a_block() {
        let m = market();
        assert_eq!(accrue(&m, &model(), 20_000_000).unwrap(), m);
        let later = accrue(&m, &model(), 20_000_010).unwrap();
        assert!(later.total_borrows > m.total_borrows && later.borrow_index > m.borrow_index && later.total_reserves >= m.total_reserves);
        assert_eq!(later.accrual_block_number, 20_000_010);
        assert!(accrue(&m, &model(), 19_999_999).is_err());
        // Stored versus projected conversion differ once blocks elapse; the
        // share count itself is unchanged.
        let shares = b("123456789");
        let stored = &shares * m.exchange_rate_stored().unwrap() / b("1000000000000000000");
        let projected = underlying_balance(&shares, &m, &model(), 20_000_010).unwrap();
        assert!(projected >= stored);
        assert_eq!(underlying_balance(&shares, &m, &model(), 20_000_000).unwrap(), stored);
    }
}
