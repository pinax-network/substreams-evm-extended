//! Aave V3 aToken balance model.
//!
//! Sources: `WadRayMath.sol`, `MathUtils.sol`, `ReserveLogic.sol` and
//! `TokenMath.sol` at aave-dao/aave-v3-origin
//! `8305565ae342f1773c42cd2e4593f175fe5968a0` (v3.7 codebase). The
//! pre-v3.5 `balanceOf` (aToken revision 3 and earlier) applied half-up
//! `rayMul`; v3.5 (revision 4) and later apply `rayMulFloor`
//! (`TokenMath.getATokenBalance`).
use crate::{Result, Unknown};
use num_bigint::BigUint;
use num_integer::Integer;
use num_traits::{One, Zero};

pub fn ray() -> BigUint {
    BigUint::from(10u8).pow(27)
}
pub fn half_ray() -> BigUint {
    BigUint::from(5u8) * BigUint::from(10u8).pow(26)
}
pub const SECONDS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
const U256_MAX_BITS: u64 = 256;

fn fits_uint256(v: &BigUint) -> bool {
    v.bits() <= U256_MAX_BITS
}

/// `WadRayMath.rayMul`: `(a * b + HALF_RAY) / RAY`, reverting on overflow.
pub fn ray_mul(a: &BigUint, b: &BigUint) -> Result<BigUint> {
    let product = a * b + half_ray();
    if !fits_uint256(&product) {
        return Err(Unknown::Invalid("rayMul overflow"));
    }
    Ok(product / ray())
}
/// `WadRayMath.rayMulFloor`: `(a * b) / RAY`.
pub fn ray_mul_floor(a: &BigUint, b: &BigUint) -> Result<BigUint> {
    let product = a * b;
    if !fits_uint256(&product) {
        return Err(Unknown::Invalid("rayMulFloor overflow"));
    }
    Ok(product / ray())
}
/// `WadRayMath.rayMulCeil`: `ceil(a * b / RAY)`.
pub fn ray_mul_ceil(a: &BigUint, b: &BigUint) -> Result<BigUint> {
    let product = a * b;
    if !fits_uint256(&product) {
        return Err(Unknown::Invalid("rayMulCeil overflow"));
    }
    Ok(product.div_ceil(&ray()))
}
/// `WadRayMath.rayDiv`: `(a * RAY + b / 2) / b`.
pub fn ray_div(a: &BigUint, b: &BigUint) -> Result<BigUint> {
    if b.is_zero() {
        return Err(Unknown::Invalid("rayDiv by zero"));
    }
    let numerator = a * ray() + b / BigUint::from(2u8);
    if !fits_uint256(&numerator) {
        return Err(Unknown::Invalid("rayDiv overflow"));
    }
    Ok(numerator / b)
}

/// `MathUtils.calculateLinearInterest(rate, lastUpdateTimestamp)` evaluated at
/// `current_timestamp`: `rate * (now - last) / SECONDS_PER_YEAR + RAY`.
pub fn linear_interest(rate: &BigUint, last_update: u64, current_timestamp: u64) -> Result<BigUint> {
    if current_timestamp < last_update {
        return Err(Unknown::Invalid("evaluation clock precedes last update"));
    }
    let elapsed = BigUint::from(current_timestamp - last_update);
    let product = rate * elapsed;
    if !fits_uint256(&product) {
        return Err(Unknown::Invalid("linear interest overflow"));
    }
    Ok(product / BigUint::from(SECONDS_PER_YEAR) + ray())
}

/// Stored reserve inputs read from Pool storage (`DataTypes.ReserveData`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reserve {
    pub liquidity_index: BigUint,
    pub current_liquidity_rate: BigUint,
    pub last_update_timestamp: u64,
}
impl Reserve {
    /// `ReserveLogic.getNormalizedIncome`: the stored index when the reserve
    /// was updated in this very second, else linear interest applied with
    /// half-up `rayMul`.
    pub fn normalized_income(&self, current_timestamp: u64) -> Result<BigUint> {
        if self.last_update_timestamp == current_timestamp {
            return Ok(self.liquidity_index.clone());
        }
        let interest = linear_interest(&self.current_liquidity_rate, self.last_update_timestamp, current_timestamp)?;
        ray_mul(&interest, &self.liquidity_index)
    }
}

/// Rounding era of `AToken.balanceOf`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Era {
    /// aToken revision 3 and earlier: `scaled.rayMul(index)` (half-up).
    HalfUp,
    /// aToken revision 4 (v3.5) and later: `scaled.rayMulFloor(index)`.
    Floor,
}

/// The observable ERC-20 `balanceOf` of an aToken holder at a block clock.
pub fn balance_of(scaled: &BigUint, reserve: &Reserve, current_timestamp: u64, era: Era) -> Result<BigUint> {
    if reserve.liquidity_index.is_zero() {
        return Err(Unknown::MissingInput("liquidity index"));
    }
    let index = reserve.normalized_income(current_timestamp)?;
    match era {
        Era::HalfUp => ray_mul(scaled, &index),
        Era::Floor => ray_mul_floor(scaled, &index),
    }
}

/// `ReserveLogic._updateIndexes` for the liquidity side: the next stored
/// index after an update at `current_timestamp`, `RAY`-scaled.
pub fn next_liquidity_index(reserve: &Reserve, current_timestamp: u64) -> Result<BigUint> {
    if reserve.current_liquidity_rate.is_zero() {
        return Ok(reserve.liquidity_index.clone());
    }
    let interest = linear_interest(&reserve.current_liquidity_rate, reserve.last_update_timestamp, current_timestamp)?;
    ray_mul(&interest, &reserve.liquidity_index)
}

pub fn one() -> BigUint {
    BigUint::one()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn b(s: &str) -> BigUint {
        s.parse().unwrap()
    }

    #[test]
    fn ray_math_matches_the_pinned_rounding_rules() {
        // 1.5 RAY * 1.5 RAY = 2.25 RAY exactly; half-up and floor agree.
        let x = b("1500000000000000000000000000");
        assert_eq!(ray_mul(&x, &x).unwrap(), b("2250000000000000000000000000"));
        assert_eq!(ray_mul_floor(&x, &x).unwrap(), b("2250000000000000000000000000"));
        // 1 wei * (RAY + 0.5 wei-ray) rounds differently per rule.
        let index = b("1000000000000000000000000000") + b("500000000000000000000000000");
        assert_eq!(ray_mul(&one(), &index).unwrap(), b("2"));
        assert_eq!(ray_mul_floor(&one(), &index).unwrap(), b("1"));
        assert_eq!(ray_mul_ceil(&one(), &index).unwrap(), b("2"));
        assert_eq!(ray_mul(&one(), &b("1499999999999999999999999999")).unwrap(), b("1"));
        assert_eq!(ray_div(&b("2"), &b("4")).unwrap(), b("500000000000000000000000000"));
        assert!(ray_div(&one(), &BigUint::zero()).is_err());
        let max = (BigUint::one() << 256u32) - BigUint::one();
        assert!(ray_mul(&max, &b("2")).is_err());
    }

    #[test]
    fn linear_interest_is_exact_integer_arithmetic() {
        let rate = b("18450000000000000000000000"); // 1.845% per year in ray
        assert_eq!(linear_interest(&rate, 100, 100).unwrap(), ray());
        // one year exactly adds the rate
        assert_eq!(linear_interest(&rate, 0, SECONDS_PER_YEAR).unwrap(), ray() + rate.clone());
        // 450 ms cadence: one second adds rate / SECONDS_PER_YEAR, truncated
        assert_eq!(linear_interest(&rate, 10, 11).unwrap(), ray() + &rate / BigUint::from(SECONDS_PER_YEAR));
        assert!(linear_interest(&rate, 11, 10).is_err());
    }

    #[test]
    fn normalized_income_returns_the_stored_index_within_the_same_second() {
        let reserve = Reserve {
            liquidity_index: b("1023456789012345678901234567"),
            current_liquidity_rate: b("18450000000000000000000000"),
            last_update_timestamp: 1789689612,
        };
        assert_eq!(reserve.normalized_income(1789689612).unwrap(), reserve.liquidity_index);
        let later = reserve.normalized_income(1789689612 + 3600).unwrap();
        assert!(later > reserve.liquidity_index);
        assert!(reserve.normalized_income(1789689611).is_err());
    }

    #[test]
    fn balance_of_distinguishes_rounding_eras_and_refuses_missing_state() {
        let reserve = Reserve {
            liquidity_index: b("1000000000000000000000000000") + b("500000000000000000000000000"),
            current_liquidity_rate: BigUint::zero(),
            last_update_timestamp: 1,
        };
        assert_eq!(balance_of(&one(), &reserve, 1, Era::HalfUp).unwrap(), b("2"));
        assert_eq!(balance_of(&one(), &reserve, 1, Era::Floor).unwrap(), b("1"));
        let uninitialized = Reserve {
            liquidity_index: BigUint::zero(),
            ..reserve
        };
        assert_eq!(balance_of(&one(), &uninitialized, 1, Era::Floor), Err(Unknown::MissingInput("liquidity index")));
    }
}

#[cfg(test)]
mod captured_oracles {
    //! Reserve index updates captured from Aave V3 BNB Pool storage in saved BSC
    //! Extended blocks (`aave/balance-state/tests/fixtures/cases.json`). The
    //! contract computed the new index from the stored rate, the stored clock
    //! and the block timestamp; reproducing it exactly is an independent check
    //! of `linear_interest` and half-up `ray_mul`, not a test that repeats the
    //! implementation.
    use super::*;
    fn b(s: &str) -> BigUint {
        s.parse().unwrap()
    }
    const CASES: [(&str, &str, &str, u64, u64, &str); 4] = [
        (
            "122288220 USDT",
            "1124514677823054194978648271",
            "28034908431682012299972417",
            1789591520,
            1789591698,
            "1124514855764725683677494165",
        ),
        (
            "122288734 USDT",
            "1124514957731538917391922460",
            "28035416077187211327497932",
            1789591800,
            1789591929,
            "1124515086691634348902975804",
        ),
        (
            "122288932 USDT",
            "1124515086691634348902975804",
            "28035604914884915624731350",
            1789591929,
            1789592018,
            "1124515175664712783928523189",
        ),
        (
            "122288932 USDC",
            "1124366161134373935820757143",
            "34985053082719390972448209",
            1789591264,
            1789592018,
            "1124367101626237618641033796",
        ),
    ];

    #[test]
    fn captured_reserve_updates_reproduce_the_stored_index_exactly() {
        for (name, index, rate, last, now, expected) in CASES {
            let reserve = Reserve {
                liquidity_index: b(index),
                current_liquidity_rate: b(rate),
                last_update_timestamp: last,
            };
            assert_eq!(next_liquidity_index(&reserve, now).unwrap(), b(expected), "{name}");
            // The projection a consumer makes one second before the update is
            // strictly between the stored and the next index.
            let projected = reserve.normalized_income(now - 1).unwrap();
            assert!(projected > reserve.liquidity_index && projected < b(expected), "{name}");
            // Floor instead of half-up would already miss the stored value for
            // some of these updates: the rounding chain is load-bearing.
            let floor = ray_mul_floor(&linear_interest(&reserve.current_liquidity_rate, last, now).unwrap(), &reserve.liquidity_index).unwrap();
            assert!(floor == b(expected) || floor + one() == b(expected), "{name}");
        }
    }

    #[test]
    fn captured_holder_balance_evaluates_from_the_emitted_inputs() {
        // Block 122288932: aBnbUSDT holder scaled balance after the block and the
        // USDT reserve state written in the same block, evaluated at that clock.
        let reserve = Reserve {
            liquidity_index: b("1124515175664712783928523189"),
            current_liquidity_rate: b("28015924093916359280074547"),
            last_update_timestamp: 1789592018,
        };
        let scaled = b("45954312872751152145397");
        let at_block = balance_of(&scaled, &reserve, 1789592018, Era::Floor).unwrap();
        assert_eq!(at_block, ray_mul_floor(&scaled, &reserve.liquidity_index).unwrap());
        // An idle block later changes the evaluated amount without any write.
        let later = balance_of(&scaled, &reserve, 1789592018 + 450, Era::Floor).unwrap();
        assert!(later > at_block);
        assert!(balance_of(&scaled, &reserve, 1789592017, Era::Floor).is_err());
    }
}
