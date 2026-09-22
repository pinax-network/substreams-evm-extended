//! Lido stETH balance model, contract version 4.
//!
//! Source: `contracts/0.4.24/StETH.sol` and `Lido.sol` at lidofinance/core
//! `2da0f48f1a2a103a394dcf8760810fe9165697fb` (v4.0.1). Since version 3 the
//! share rate is `internalEther / internalShares` with
//! `internalShares = totalShares - externalShares` and `internalEther =
//! bufferedEther + clValidatorsBalance + clPendingBalance +
//! depositedPostReport`; `totalPooledEther = internalEther + externalShares *
//! internalEther / internalShares`. Every packed field is uint128. Amounts are
//! wei; results are truncated as the EVM does. `getPooledEthByShares` and
//! `getSharesByPooledEth` require the argument to be below `UINT128_MAX`
//! (`~uint128(0)`, StETH.sol:58, 318, 330), so `2^128 - 1` itself reverts.
use crate::{Result, Unknown};
use num_bigint::BigUint;
use num_traits::Zero;

/// Global inputs of one evaluation, all at the same block clock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pool {
    pub total_shares: BigUint,
    pub external_shares: BigUint,
    pub buffered_ether: BigUint,
    pub deposited_post_report: BigUint,
    pub cl_validators_balance: BigUint,
    pub cl_pending_balance: BigUint,
}

fn fits_uint128(v: &BigUint) -> bool {
    v.bits() <= 128
}
/// `require(_amount < UINT128_MAX)`: strictly below `2^128 - 1`.
fn below_uint128_max(v: &BigUint) -> bool {
    v.bits() < 128 || (v.bits() == 128 && v.count_ones() < 128)
}

impl Pool {
    fn check(&self) -> Result<()> {
        for (name, v) in [
            ("totalShares", &self.total_shares),
            ("externalShares", &self.external_shares),
            ("bufferedEther", &self.buffered_ether),
            ("depositedPostReport", &self.deposited_post_report),
            ("clValidatorsBalance", &self.cl_validators_balance),
            ("clPendingBalance", &self.cl_pending_balance),
        ] {
            if !fits_uint128(v) {
                let _ = name;
                return Err(Unknown::Invalid("packed field exceeds uint128"));
            }
        }
        if self.external_shares > self.total_shares {
            return Err(Unknown::Invalid("external shares exceed total shares"));
        }
        Ok(())
    }
    /// `_getInternalEther()`.
    pub fn internal_ether(&self) -> Result<BigUint> {
        self.check()?;
        Ok(&self.buffered_ether + &self.cl_validators_balance + &self.cl_pending_balance + &self.deposited_post_report)
    }
    /// `_getShareRateDenominator()`: `totalShares - externalShares`.
    pub fn internal_shares(&self) -> Result<BigUint> {
        self.check()?;
        Ok(&self.total_shares - &self.external_shares)
    }
    /// `_getExternalEther(internalEther)`: `externalShares * internalEther / internalShares`.
    pub fn external_ether(&self) -> Result<BigUint> {
        let internal_shares = self.internal_shares()?;
        if internal_shares.is_zero() {
            return Err(Unknown::Invalid("zero internal shares"));
        }
        Ok(&self.external_shares * self.internal_ether()? / internal_shares)
    }
    /// `getTotalPooledEther()`.
    pub fn total_pooled_ether(&self) -> Result<BigUint> {
        Ok(self.internal_ether()? + self.external_ether()?)
    }
    /// `getPooledEthByShares(shares)`: `shares * internalEther / internalShares`.
    pub fn pooled_eth_by_shares(&self, shares: &BigUint) -> Result<BigUint> {
        if !below_uint128_max(shares) {
            return Err(Unknown::Invalid("SHARES_TOO_LARGE"));
        }
        let internal_shares = self.internal_shares()?;
        if internal_shares.is_zero() {
            return Err(Unknown::Invalid("zero internal shares"));
        }
        Ok(shares * self.internal_ether()? / internal_shares)
    }
    /// `getSharesByPooledEth(eth)`: `eth * internalShares / internalEther`.
    pub fn shares_by_pooled_eth(&self, eth: &BigUint) -> Result<BigUint> {
        if !below_uint128_max(eth) {
            return Err(Unknown::Invalid("ETH_TOO_LARGE"));
        }
        let internal_ether = self.internal_ether()?;
        if internal_ether.is_zero() {
            return Err(Unknown::Invalid("zero internal ether"));
        }
        Ok(eth * self.internal_shares()? / internal_ether)
    }
}

/// `balanceOf(holder)`: the ERC-20 amount from retained shares; `None`
/// shares are an uninitialized holder, never zero.
pub fn balance_of(shares: Option<&BigUint>, pool: &Pool) -> Result<BigUint> {
    let shares = shares.ok_or(Unknown::MissingInput("holder shares"))?;
    pool.pooled_eth_by_shares(shares)
}

/// Post-report share rate from `TokenRebased` fields, as the integration guide
/// documents it: `postTotalEther / postTotalShares` applied to shares. Equal to
/// `pooled_eth_by_shares` only when the ratios coincide exactly; it is evidence,
/// not the getter.
pub fn pooled_eth_from_report(shares: &BigUint, post_total_shares: &BigUint, post_total_ether: &BigUint) -> Result<BigUint> {
    if post_total_shares.is_zero() {
        return Err(Unknown::Invalid("zero post total shares"));
    }
    Ok(shares * post_total_ether / post_total_shares)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn n(v: u128) -> BigUint {
        BigUint::from(v)
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

    #[test]
    fn share_rate_uses_internal_ether_over_internal_shares_and_truncates() {
        // internalEther 200, internalShares 800 -> rate 0.25; external ether 50; total 250.
        let p = pool(1000, 200, 100, 20, 50, 30);
        assert_eq!(p.internal_ether().unwrap(), n(200));
        assert_eq!(p.internal_shares().unwrap(), n(800));
        assert_eq!(p.external_ether().unwrap(), n(50));
        assert_eq!(p.total_pooled_ether().unwrap(), n(250));
        assert_eq!(p.pooled_eth_by_shares(&n(800)).unwrap(), n(200));
        assert_eq!(p.pooled_eth_by_shares(&n(7)).unwrap(), n(1)); // 7 * 200 / 800 = 1.75 -> 1
        assert_eq!(p.pooled_eth_by_shares(&n(3)).unwrap(), n(0)); // dust
        assert_eq!(p.shares_by_pooled_eth(&n(1)).unwrap(), n(4));
        assert_eq!(balance_of(Some(&n(800)), &p).unwrap(), n(200));
        assert_eq!(balance_of(None, &p), Err(Unknown::MissingInput("holder shares")));
    }

    #[test]
    fn external_shares_do_not_move_the_share_rate_but_do_move_total_pooled_ether() {
        let without = pool(800, 0, 100, 20, 50, 30);
        let with = pool(1000, 200, 100, 20, 50, 30);
        assert_eq!(without.pooled_eth_by_shares(&n(123)).unwrap(), with.pooled_eth_by_shares(&n(123)).unwrap());
        assert_eq!(without.total_pooled_ether().unwrap(), n(200));
        assert_eq!(with.total_pooled_ether().unwrap(), n(250));
        // The report ratio postTotalEther/postTotalShares agrees here (250/1000 == 200/800)
        // but is not the getter in general.
        assert_eq!(pooled_eth_from_report(&n(800), &n(1000), &n(250)).unwrap(), n(200));
        assert_eq!(pooled_eth_from_report(&n(7), &n(1000), &n(251)).unwrap(), n(1));
        assert_eq!(with.pooled_eth_by_shares(&n(7)).unwrap(), n(1));
    }

    #[test]
    fn rebases_up_and_down_and_report_only_updates_change_every_holder() {
        let before = pool(1_000, 0, 500, 0, 500, 0);
        let up = pool(1_000, 0, 500, 0, 600, 0);
        let down = pool(1_000, 0, 500, 0, 400, 0);
        let shares = n(10);
        assert_eq!(before.pooled_eth_by_shares(&shares).unwrap(), n(10));
        assert_eq!(up.pooled_eth_by_shares(&shares).unwrap(), n(11));
        assert_eq!(down.pooled_eth_by_shares(&shares).unwrap(), n(9));
        // Fee minting: shares grow with the ether, rate moves less than the ether alone.
        let fees = pool(1_050, 0, 500, 0, 600, 0);
        assert_eq!(fees.pooled_eth_by_shares(&n(1_050)).unwrap(), n(1_100));
    }

    #[test]
    fn domain_errors_are_explicit() {
        let big = BigUint::from(1u8) << 128u32;
        assert_eq!(pool(1000, 0, 1, 0, 0, 0).pooled_eth_by_shares(&big), Err(Unknown::Invalid("SHARES_TOO_LARGE")));
        assert_eq!(pool(1000, 0, 1, 0, 0, 0).shares_by_pooled_eth(&big), Err(Unknown::Invalid("ETH_TOO_LARGE")));
        assert_eq!(
            pool(5, 5, 1, 0, 0, 0).pooled_eth_by_shares(&n(1)),
            Err(Unknown::Invalid("zero internal shares"))
        );
        assert_eq!(
            pool(5, 6, 1, 0, 0, 0).internal_shares(),
            Err(Unknown::Invalid("external shares exceed total shares"))
        );
        assert_eq!(
            pool(0, 0, 0, 0, 0, 0).pooled_eth_by_shares(&n(0)),
            Err(Unknown::Invalid("zero internal shares"))
        );
        assert_eq!(
            pool(10, 0, 0, 0, 0, 0).shares_by_pooled_eth(&n(1)),
            Err(Unknown::Invalid("zero internal ether"))
        );
        let mut p = pool(10, 0, 0, 0, 0, 0);
        p.buffered_ether = big;
        assert_eq!(p.internal_ether(), Err(Unknown::Invalid("packed field exceeds uint128")));
        assert_eq!(pooled_eth_from_report(&n(1), &n(0), &n(1)), Err(Unknown::Invalid("zero post total shares")));
        // Packed fields may hold the uint128 maximum; a getter argument may not:
        // `require(_sharesAmount < UINT128_MAX)` rejects 2^128 - 1 itself.
        let max = (BigUint::from(1u8) << 128u32) - 1u8;
        let p = Pool {
            total_shares: max.clone(),
            external_shares: n(0),
            buffered_ether: max.clone(),
            deposited_post_report: n(0),
            cl_validators_balance: n(0),
            cl_pending_balance: n(0),
        };
        assert_eq!(p.pooled_eth_by_shares(&max), Err(Unknown::Invalid("SHARES_TOO_LARGE")));
        assert_eq!(p.shares_by_pooled_eth(&max), Err(Unknown::Invalid("ETH_TOO_LARGE")));
        assert_eq!(balance_of(Some(&max), &p), Err(Unknown::Invalid("SHARES_TOO_LARGE")));
        let below = &max - 1u8;
        assert_eq!(p.pooled_eth_by_shares(&below).unwrap(), below);
        assert_eq!(p.shares_by_pooled_eth(&below).unwrap(), below);
    }

    #[test]
    fn share_burns_and_zero_share_holders_in_a_live_pool() {
        // burnShares: totalShares falls with no ether leaving, so every other
        // holder's balance rises; the burned holder's zero shares read as 0.
        let before = pool(1_000, 0, 500, 0, 500, 0);
        let after_burn = pool(800, 0, 500, 0, 500, 0);
        assert_eq!(before.pooled_eth_by_shares(&n(100)).unwrap(), n(100));
        assert_eq!(after_burn.pooled_eth_by_shares(&n(100)).unwrap(), n(125));
        assert_eq!(balance_of(Some(&n(0)), &after_burn).unwrap(), n(0));
        assert_eq!(after_burn.shares_by_pooled_eth(&n(0)).unwrap(), n(0));
        // Truncation at the boundary of one wei: 1 wei buys 0 shares when the
        // rate exceeds one, and dust shares round down to 0 wei.
        let rich = pool(1_000, 0, 1_500, 0, 1_500, 0);
        assert_eq!(rich.shares_by_pooled_eth(&n(1)).unwrap(), n(0));
        assert_eq!(rich.shares_by_pooled_eth(&n(3)).unwrap(), n(1));
        let poor = pool(1_000, 0, 1, 0, 0, 0);
        assert_eq!(poor.pooled_eth_by_shares(&n(999)).unwrap(), n(0));
    }
}
