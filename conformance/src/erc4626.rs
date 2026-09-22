//! ERC-4626 conversion models, one per bound implementation. The standard
//! fixes interface semantics only (`convertTo*` round down and may be
//! inexact; `preview*` include fees; `max*` include limits), so each vault
//! binds to its own source:
//!
//! * `StataTokenLm`: bgd-labs/static-a-token-v3
//!   `101f5d977889254ca2d2711b9582b45f832d10a0` `StaticATokenLM.sol` and
//!   `RayMathExplicitRounding.sol`: `convertToAssets = previewRedeem =
//!   rayMulRoundDown(shares, POOL.getReserveNormalizedIncome(underlying))`,
//!   `previewMint` rounds up, `maxWithdraw` applies the same conversion to
//!   `maxRedeem`, which is 0 while the reserve is inactive or paused.
//! * `SavingsDai`: sky-ecosystem/sdai `665879762f8b5df5d234463f45d1d6a49bd4fbeb`
//!   `SavingsDai.sol`: `convertToAssets = shares * chi' / RAY` with
//!   `chi' = rpow(dsr, now - rho) * chi / RAY` when `now > rho`, from the Maker
//!   Pot; `previewRedeem == convertToAssets`; `previewWithdraw` rounds up.
//! * `OzVirtualOffset`: OpenZeppelin `ERC4626.sol` v5.0.0
//!   (`932fddf69a699a9a80fd2396fd1a2ab91cdda123`): `shares * (totalAssets + 1)
//!   / (totalSupply + 10^offset)`, floor for `convertToAssets` /
//!   `previewRedeem`, ceil for `previewWithdraw`.
//!
//! Share balances are the ERC-20 amount; these functions produce the
//! underlying claim, a different metric. Missing input is an error, never 0.
#[cfg(test)]
mod oz_evm_oracle;

use crate::aave;
use crate::{Result, Unknown};
use num_bigint::BigUint;
use num_traits::{One, Zero};

fn ray() -> BigUint {
    aave::ray()
}
fn max_uint256() -> BigUint {
    (BigUint::one() << 256u32) - BigUint::one()
}
fn checked(v: BigUint) -> Result<BigUint> {
    if v > max_uint256() {
        Err(Unknown::Invalid("uint256 overflow"))
    } else {
        Ok(v)
    }
}
fn div_up(x: &BigUint, y: &BigUint) -> Result<BigUint> {
    if y.is_zero() {
        return Err(Unknown::Invalid("division by zero"));
    }
    if x.is_zero() {
        return Ok(BigUint::zero());
    }
    Ok((x - BigUint::one()) / y + BigUint::one())
}

// ---------------------------------------------------------------------------
// Aave static aToken (StaticATokenLM revision 2)
// ---------------------------------------------------------------------------

/// `rayMulRoundDown(a, b)`: `a * b / RAY`, 0 when either is 0.
pub fn ray_mul_round_down(a: &BigUint, b: &BigUint) -> Result<BigUint> {
    if a.is_zero() || b.is_zero() {
        return Ok(BigUint::zero());
    }
    checked(a * b)?;
    Ok(a * b / ray())
}
/// `rayMulRoundUp(a, b)`: `(a * b + RAY - 1) / RAY`, 0 when either is 0.
pub fn ray_mul_round_up(a: &BigUint, b: &BigUint) -> Result<BigUint> {
    if a.is_zero() || b.is_zero() {
        return Ok(BigUint::zero());
    }
    let p = checked(a * b + ray() - BigUint::one())?;
    Ok(p / ray())
}
/// `rayDivRoundDown(a, b)`: `a * RAY / b`.
pub fn ray_div_round_down(a: &BigUint, b: &BigUint) -> Result<BigUint> {
    if b.is_zero() {
        return Err(Unknown::Invalid("division by zero"));
    }
    Ok(checked(a * ray())? / b)
}
/// `rayDivRoundUp(a, b)`: `(a * RAY + b - 1) / b`.
pub fn ray_div_round_up(a: &BigUint, b: &BigUint) -> Result<BigUint> {
    if b.is_zero() {
        return Err(Unknown::Invalid("division by zero"));
    }
    Ok(checked(a * ray() + b - BigUint::one())? / b)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StataTokenLm {
    /// The bound reserve of the vault's underlying, from the Aave Pool.
    pub reserve: aave::Reserve,
    /// `ReserveConfiguration` active and not paused; `None` when not carried.
    pub reserve_active_and_unpaused: Option<bool>,
}
impl StataTokenLm {
    /// `rate()` = `POOL.getReserveNormalizedIncome(underlying)`.
    pub fn rate(&self, timestamp: u64) -> Result<BigUint> {
        self.reserve.normalized_income(timestamp)
    }
    /// `convertToAssets` / `previewRedeem`.
    pub fn convert_to_assets(&self, shares: &BigUint, timestamp: u64) -> Result<BigUint> {
        ray_mul_round_down(shares, &self.rate(timestamp)?)
    }
    /// `previewMint` (assets needed for shares, rounded up).
    pub fn preview_mint(&self, shares: &BigUint, timestamp: u64) -> Result<BigUint> {
        ray_mul_round_up(shares, &self.rate(timestamp)?)
    }
    /// `convertToShares` / `previewDeposit`.
    pub fn convert_to_shares(&self, assets: &BigUint, timestamp: u64) -> Result<BigUint> {
        ray_div_round_down(assets, &self.rate(timestamp)?)
    }
    /// `previewWithdraw` (shares burned for assets, rounded up).
    pub fn preview_withdraw(&self, assets: &BigUint, timestamp: u64) -> Result<BigUint> {
        ray_div_round_up(assets, &self.rate(timestamp)?)
    }
    /// `maxRedeem(owner)`: 0 while the reserve is inactive or paused, else
    /// `min(shares, convertToShares(underlying.balanceOf(aToken)))`. The
    /// aToken's underlying balance is a further cross-contract input.
    pub fn max_redeem(&self, shares: &BigUint, atoken_underlying_balance: Option<&BigUint>, timestamp: u64) -> Result<BigUint> {
        match self.reserve_active_and_unpaused {
            None => return Err(Unknown::MissingInput("reserve configuration")),
            Some(false) => return Ok(BigUint::zero()),
            Some(true) => {}
        }
        let liquid = atoken_underlying_balance.ok_or(Unknown::MissingInput("aToken underlying balance"))?;
        let liquid_shares = self.convert_to_shares(liquid, timestamp)?;
        Ok(if liquid_shares >= *shares { shares.clone() } else { liquid_shares })
    }
    /// `maxWithdraw(owner)` = `convertToAssets(maxRedeem(owner))`.
    pub fn max_withdraw(&self, shares: &BigUint, atoken_underlying_balance: Option<&BigUint>, timestamp: u64) -> Result<BigUint> {
        let shares = self.max_redeem(shares, atoken_underlying_balance, timestamp)?;
        self.convert_to_assets(&shares, timestamp)
    }
}

// ---------------------------------------------------------------------------
// Savings DAI (Maker Pot)
// ---------------------------------------------------------------------------

/// DSS `rpow(x, n)` in RAY: half-up rounding at every squaring and multiply,
/// reverting on 256-bit overflow.
pub fn rpow(x: &BigUint, n: u64) -> Result<BigUint> {
    let ray = ray();
    if x.is_zero() {
        return Ok(if n == 0 { ray } else { BigUint::zero() });
    }
    let half = &ray / 2u8;
    let mut z = if n % 2 == 0 { ray.clone() } else { x.clone() };
    let mut x = x.clone();
    let mut n = n / 2;
    while n != 0 {
        let xx = checked(&x * &x)?;
        let xx_round = checked(xx + &half)?;
        x = xx_round / &ray;
        if n % 2 != 0 {
            let zx = checked(&z * &x)?;
            let zx_round = checked(zx + &half)?;
            z = zx_round / &ray;
        }
        n /= 2;
    }
    Ok(z)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavingsDai {
    /// Pot `chi` (ray), `rho` (seconds) and `dsr` (ray per second).
    pub chi: BigUint,
    pub rho: u64,
    pub dsr: BigUint,
}
impl SavingsDai {
    /// The `chi` `convertToAssets` uses at `timestamp`: the stored one when
    /// no time passed since `rho`, else the drip projection.
    pub fn chi_at(&self, timestamp: u64) -> Result<BigUint> {
        if self.chi.is_zero() {
            return Err(Unknown::MissingInput("Pot chi"));
        }
        if timestamp > self.rho {
            let growth = rpow(&self.dsr, timestamp - self.rho)?;
            Ok(checked(growth * &self.chi)? / ray())
        } else {
            Ok(self.chi.clone())
        }
    }
    /// `convertToAssets` / `previewRedeem`: `shares * chi / RAY`.
    pub fn convert_to_assets(&self, shares: &BigUint, timestamp: u64) -> Result<BigUint> {
        Ok(checked(shares * self.chi_at(timestamp)?)? / ray())
    }
    /// `convertToShares` / `previewDeposit`: `assets * RAY / chi`.
    pub fn convert_to_shares(&self, assets: &BigUint, timestamp: u64) -> Result<BigUint> {
        Ok(checked(assets * ray())? / self.chi_at(timestamp)?)
    }
    /// `previewWithdraw`: `_divup(assets * RAY, chi)`.
    pub fn preview_withdraw(&self, assets: &BigUint, timestamp: u64) -> Result<BigUint> {
        div_up(&checked(assets * ray())?, &self.chi_at(timestamp)?)
    }
    /// `previewMint`: `_divup(shares * chi, RAY)`.
    pub fn preview_mint(&self, shares: &BigUint, timestamp: u64) -> Result<BigUint> {
        div_up(&checked(shares * self.chi_at(timestamp)?)?, &ray())
    }
    /// `maxWithdraw(owner)` = `convertToAssets(balanceOf[owner])`; no limits.
    pub fn max_withdraw(&self, shares: &BigUint, timestamp: u64) -> Result<BigUint> {
        self.convert_to_assets(shares, timestamp)
    }
}

// ---------------------------------------------------------------------------
// OpenZeppelin virtual-offset vault
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OzVirtualOffset {
    pub total_assets: BigUint,
    pub total_supply: BigUint,
    pub decimals_offset: u8,
}
impl OzVirtualOffset {
    /// The two additions and exponentiation in `_convertTo*` are checked
    /// uint256 operations, even when the requested amount is zero.
    fn virtual_totals(&self) -> Result<(BigUint, BigUint)> {
        checked(self.total_assets.clone())?;
        checked(self.total_supply.clone())?;
        let virtual_shares = checked(BigUint::from(10u8).pow(self.decimals_offset as u32))?;
        Ok((checked(&self.total_assets + BigUint::one())?, checked(&self.total_supply + virtual_shares)?))
    }
    /// `convertToAssets` / `previewRedeem` (floor).
    pub fn convert_to_assets(&self, shares: &BigUint) -> Result<BigUint> {
        let (assets, supply) = self.virtual_totals()?;
        oz_mul_div(shares, &assets, &supply, false)
    }
    /// `previewMint` (ceil).
    pub fn preview_mint(&self, shares: &BigUint) -> Result<BigUint> {
        let (assets, supply) = self.virtual_totals()?;
        oz_mul_div(shares, &assets, &supply, true)
    }
    /// `convertToShares` / `previewDeposit` (floor).
    pub fn convert_to_shares(&self, assets: &BigUint) -> Result<BigUint> {
        let (total_assets, supply) = self.virtual_totals()?;
        oz_mul_div(assets, &supply, &total_assets, false)
    }
    /// `previewWithdraw` (ceil).
    pub fn preview_withdraw(&self, assets: &BigUint) -> Result<BigUint> {
        let (total_assets, supply) = self.virtual_totals()?;
        oz_mul_div(assets, &supply, &total_assets, true)
    }
    /// Base `maxWithdraw(owner)` = `convertToAssets(balanceOf(owner))`;
    /// derived vaults override it with limits.
    pub fn max_withdraw(&self, shares: &BigUint) -> Result<BigUint> {
        self.convert_to_assets(shares)
    }
}

/// Pinned OZ v5 `Math.mulDiv`: uint256 operands, a full 512-bit product,
/// and a checked uint256 result. The ceil overload checks its final +1 too.
fn oz_mul_div(x: &BigUint, y: &BigUint, denominator: &BigUint, round_up: bool) -> Result<BigUint> {
    checked(x.clone())?;
    checked(y.clone())?;
    checked(denominator.clone())?;
    if denominator.is_zero() {
        return Err(Unknown::Invalid("division by zero"));
    }
    let product = x * y;
    let quotient = checked(&product / denominator)?;
    if round_up && !(&product % denominator).is_zero() {
        checked(quotient + BigUint::one())
    } else {
        Ok(quotient)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn n(v: u128) -> BigUint {
        BigUint::from(v)
    }
    fn ray_n(v: u128) -> BigUint {
        n(v) * ray()
    }

    #[test]
    fn stata_conversion_is_ray_rounding_on_the_normalized_income() {
        let reserve = aave::Reserve {
            liquidity_index: ray() + ray() / 10u8,
            current_liquidity_rate: BigUint::zero(),
            last_update_timestamp: 100,
        };
        let v = StataTokenLm {
            reserve,
            reserve_active_and_unpaused: Some(true),
        };
        // rate 1.1 ray at any time without a rate.
        assert_eq!(v.convert_to_assets(&n(10), 100).unwrap(), n(11));
        assert_eq!(v.convert_to_assets(&n(1), 500).unwrap(), n(1)); // 1.1 -> 1
        assert_eq!(v.preview_mint(&n(1), 500).unwrap(), n(2)); // ceil
        assert_eq!(v.convert_to_shares(&n(11), 500).unwrap(), n(10));
        assert_eq!(v.convert_to_shares(&n(1), 500).unwrap(), n(0));
        assert_eq!(v.preview_withdraw(&n(1), 500).unwrap(), n(1));
        assert_eq!(v.convert_to_assets(&n(0), 500).unwrap(), n(0));
        // maxWithdraw: liquidity-limited and paused branches.
        assert_eq!(v.max_withdraw(&n(10), Some(&n(1_000)), 500).unwrap(), n(11));
        assert_eq!(v.max_redeem(&n(10), Some(&n(5)), 500).unwrap(), n(4)); // 5 / 1.1 = 4.54 -> 4
        assert_eq!(v.max_withdraw(&n(10), None, 500), Err(Unknown::MissingInput("aToken underlying balance")));
        let paused = StataTokenLm {
            reserve_active_and_unpaused: Some(false),
            ..v.clone()
        };
        assert_eq!(paused.max_withdraw(&n(10), Some(&n(1_000)), 500).unwrap(), n(0));
        assert_eq!(paused.convert_to_assets(&n(10), 500).unwrap(), n(11));
        let unknown = StataTokenLm {
            reserve_active_and_unpaused: None,
            ..v.clone()
        };
        assert_eq!(
            unknown.max_withdraw(&n(10), Some(&n(1)), 500),
            Err(Unknown::MissingInput("reserve configuration"))
        );
        // Time moves the rate through the reserve's linear interest.
        let accruing = StataTokenLm {
            reserve: aave::Reserve {
                liquidity_index: ray(),
                current_liquidity_rate: ray_n(1) / 10u8,
                last_update_timestamp: 0,
            },
            reserve_active_and_unpaused: Some(true),
        };
        let year = aave::SECONDS_PER_YEAR;
        assert_eq!(accruing.convert_to_assets(&n(1_000), 0).unwrap(), n(1_000));
        assert_eq!(accruing.convert_to_assets(&n(1_000), year).unwrap(), n(1_100));
        assert_eq!(accruing.convert_to_assets(&n(1_000), 100).unwrap(), n(1_000));
        // 100 s of 10% APY is dust
    }

    #[test]
    fn rpow_matches_dss_rounding_and_overflow_semantics() {
        assert_eq!(rpow(&n(0), 0).unwrap(), ray());
        assert_eq!(rpow(&n(0), 5).unwrap(), n(0));
        assert_eq!(rpow(&ray(), 1_000_000).unwrap(), ray());
        let two = ray() * 2u8;
        assert_eq!(rpow(&two, 10).unwrap(), ray() * 1024u32);
        // 1.5^3 = 3.375 exactly in ray.
        let one_and_half = ray() + ray() / 2u8;
        assert_eq!(rpow(&one_and_half, 3).unwrap(), ray() * 3375u32 / 1000u32);
        // A per-second DSR of 1.000000001547125957863212448 (5% APY) over one day.
        let dsr = BigUint::parse_bytes(b"1000000001547125957863212448", 10).unwrap();
        let day = rpow(&dsr, 86_400).unwrap();
        assert!(day > ray() && day < ray() + ray() / 1000u32);
        // Half-up: (x*x + half) / RAY rounds .5 up.
        let x = ray() + BigUint::one(); // 1 + 1e-27
        assert_eq!(rpow(&x, 2).unwrap(), ray() + n(2)); // (1+e)^2 = 1 + 2e + e^2 -> rounds to 1 + 2e
        assert_eq!(rpow(&(ray() * 1_000_000_000u64), 4), Err(Unknown::Invalid("uint256 overflow")));
    }

    #[test]
    fn sdai_projects_chi_between_drips_and_rounds_like_the_source() {
        let s = SavingsDai {
            chi: ray() + ray() / 20u8,
            rho: 1_000,
            dsr: ray(),
        }; // chi 1.05, zero rate
        assert_eq!(s.convert_to_assets(&n(100), 1_000).unwrap(), n(105));
        assert_eq!(s.convert_to_assets(&n(100), 5_000).unwrap(), n(105)); // dsr = 1: unchanged
        assert_eq!(s.convert_to_assets(&n(1), 5_000).unwrap(), n(1)); // 1.05 -> 1
        assert_eq!(s.preview_mint(&n(1), 5_000).unwrap(), n(2));
        assert_eq!(s.convert_to_shares(&n(105), 5_000).unwrap(), n(100));
        assert_eq!(s.convert_to_shares(&n(1), 5_000).unwrap(), n(0));
        assert_eq!(s.preview_withdraw(&n(1), 5_000).unwrap(), n(1));
        assert_eq!(s.preview_withdraw(&n(0), 5_000).unwrap(), n(0));
        assert_eq!(s.max_withdraw(&n(100), 5_000).unwrap(), n(105));
        // With a rate, chi grows after rho and is stored-only before/at rho.
        let growing = SavingsDai {
            chi: ray(),
            rho: 1_000,
            dsr: ray() * 2u8,
        }; // doubling per second (synthetic)
        assert_eq!(growing.chi_at(1_000).unwrap(), ray());
        assert_eq!(growing.chi_at(999).unwrap(), ray());
        assert_eq!(growing.chi_at(1_003).unwrap(), ray() * 8u8);
        assert_eq!(growing.convert_to_assets(&n(3), 1_003).unwrap(), n(24));
        assert_eq!(
            SavingsDai { chi: n(0), rho: 0, dsr: ray() }.convert_to_assets(&n(1), 1),
            Err(Unknown::MissingInput("Pot chi"))
        );
    }

    #[test]
    fn oz_virtual_offset_changes_the_naive_ratio_and_handles_zero_supply() {
        let v = OzVirtualOffset {
            total_assets: n(1_000),
            total_supply: n(1_000),
            decimals_offset: 0,
        };
        // Naive would be 1:1; virtual +1 asset makes it 1001/1001 = 1 exactly here.
        assert_eq!(v.convert_to_assets(&n(500),).unwrap(), n(500));
        let skewed = OzVirtualOffset {
            total_assets: n(1_000),
            total_supply: n(3),
            decimals_offset: 0,
        };
        assert_eq!(skewed.convert_to_assets(&n(1)).unwrap(), n(250)); // 1001 / 4
        assert_eq!(skewed.preview_mint(&n(1)).unwrap(), n(251)); // ceil(250.25)
        assert_eq!(skewed.convert_to_shares(&n(1_000)).unwrap(), n(3)); // 4000/1001 = 3.99 -> 3
        assert_eq!(skewed.preview_withdraw(&n(1_000)).unwrap(), n(4));
        // Empty vault: 1 share = 1 asset at offset 0; with offset 3, 1 share = 0 assets (floor 1/1000).
        let empty = OzVirtualOffset {
            total_assets: n(0),
            total_supply: n(0),
            decimals_offset: 0,
        };
        assert_eq!(empty.convert_to_assets(&n(7)).unwrap(), n(7));
        let offset = OzVirtualOffset {
            total_assets: n(0),
            total_supply: n(0),
            decimals_offset: 3,
        };
        assert_eq!(offset.convert_to_assets(&n(7)).unwrap(), n(0));
        assert_eq!(offset.convert_to_assets(&n(7_000)).unwrap(), n(7));
        assert_eq!(offset.convert_to_shares(&n(1)).unwrap(), n(1_000));
        // A donation moves every holder's claim without a share write.
        let donated = OzVirtualOffset {
            total_assets: n(2_000),
            ..v.clone()
        };
        assert_eq!(donated.convert_to_assets(&n(500)).unwrap(), n(999)); // 500 * 2001 / 1001
        let huge = OzVirtualOffset {
            total_assets: max_uint256(),
            total_supply: n(1),
            decimals_offset: 0,
        };
        assert_eq!(huge.convert_to_assets(&n(2)), Err(Unknown::Invalid("uint256 overflow")));
    }

    #[test]
    fn oz_uses_full_precision_and_checks_every_uint256_boundary() {
        let one = BigUint::one();
        let max = max_uint256();
        let overflow = Err(Unknown::Invalid("uint256 overflow"));
        let symmetric = OzVirtualOffset {
            total_assets: (&one << 100u32) - &one,
            total_supply: (&one << 100u32) - &one,
            decimals_offset: 0,
        };
        let large = &one << 200u32;
        for result in [
            symmetric.convert_to_assets(&large),
            symmetric.preview_mint(&large),
            symmetric.convert_to_shares(&large),
            symmetric.preview_withdraw(&large),
        ] {
            assert_eq!(result.unwrap(), large);
        }
        // Checked additions happen before mulDiv, including amount == 0.
        for v in [
            OzVirtualOffset {
                total_assets: max.clone(),
                ..symmetric.clone()
            },
            OzVirtualOffset {
                total_supply: max.clone(),
                ..symmetric.clone()
            },
            OzVirtualOffset {
                decimals_offset: 78,
                ..symmetric.clone()
            },
            OzVirtualOffset {
                decimals_offset: 255,
                ..symmetric.clone()
            },
        ] {
            for amount in [n(0), n(1)] {
                assert_eq!(v.convert_to_assets(&amount), overflow);
                assert_eq!(v.preview_mint(&amount), overflow);
                assert_eq!(v.convert_to_shares(&amount), overflow);
                assert_eq!(v.preview_withdraw(&amount), overflow);
            }
        }
        let excess = &max + &one;
        assert_eq!(symmetric.convert_to_assets(&excess), overflow);
        assert_eq!(symmetric.convert_to_shares(&excess), overflow);
        assert_eq!(symmetric.preview_mint(&excess), overflow);
        assert_eq!(symmetric.preview_withdraw(&excess), overflow);
        let offset_limit = OzVirtualOffset {
            total_assets: n(0),
            total_supply: n(0),
            decimals_offset: 77,
        };
        assert_eq!(offset_limit.convert_to_shares(&one).unwrap(), n(10).pow(77));
        assert_eq!(offset_limit.preview_mint(&one).unwrap(), one);
        // A fullprecision quotient can still exceed the return type.
        assert_eq!(oz_mul_div(&max, &max, &n(1), false), overflow);
        assert_eq!(oz_mul_div(&max, &max, &max, true).unwrap(), max);
        assert_eq!(oz_mul_div(&n(0), &n(0), &n(0), false), Err(Unknown::Invalid("division by zero")));
        // x*y/d = MAX + 1/(MAX-2): floor succeeds, checked ceil fails.
        let x = &max - &one;
        let d = &max - n(2);
        assert_eq!(oz_mul_div(&x, &x, &d, false).unwrap(), max);
        assert_eq!(oz_mul_div(&x, &x, &d, true), overflow);
        let ceil_limit = OzVirtualOffset {
            total_assets: &max - n(2),
            total_supply: &max - n(3),
            decimals_offset: 0,
        };
        assert_eq!(ceil_limit.convert_to_assets(&x).unwrap(), max);
        assert_eq!(ceil_limit.preview_mint(&x), overflow);
        let ceil_limit_inverse = OzVirtualOffset {
            total_assets: ceil_limit.total_supply.clone(),
            total_supply: ceil_limit.total_assets.clone(),
            decimals_offset: 0,
        };
        assert_eq!(ceil_limit_inverse.convert_to_shares(&x).unwrap(), max);
        assert_eq!(ceil_limit_inverse.preview_withdraw(&x), overflow);
    }
}
