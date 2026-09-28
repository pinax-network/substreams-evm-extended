// SPDX-License-Identifier: BUSL-1.1
pragma solidity 0.8.15;
import './CometCore.sol';
// Pinned function bodies below: only override modifiers and the two holder
// mapping reads are substituted. Rate immutables are explicit input slots.
// Original Core, Storage, Math and Configuration are inherited unchanged.
contract Oracle is CometCore {
    uint supplyKink;
    uint supplyPerSecondInterestRateSlopeLow;
    uint supplyPerSecondInterestRateSlopeHigh;
    uint supplyPerSecondInterestRateBase;
    uint borrowKink;
    uint borrowPerSecondInterestRateSlopeLow;
    uint borrowPerSecondInterestRateSlopeHigh;
    uint borrowPerSecondInterestRateBase;
    int104 boundPrincipal;
    error TimestampTooLarge();
function getNowInternal() virtual internal view returns (uint40) {
        if (block.timestamp >= 2**40) revert TimestampTooLarge();
        return uint40(block.timestamp);
    }
function accruedInterestIndices(uint timeElapsed) internal view returns (uint64, uint64) {
        uint64 baseSupplyIndex_ = baseSupplyIndex;
        uint64 baseBorrowIndex_ = baseBorrowIndex;
        if (timeElapsed > 0) {
            uint utilization = getUtilization();
            uint supplyRate = getSupplyRate(utilization);
            uint borrowRate = getBorrowRate(utilization);
            baseSupplyIndex_ += safe64(mulFactor(baseSupplyIndex_, supplyRate * timeElapsed));
            baseBorrowIndex_ += safe64(mulFactor(baseBorrowIndex_, borrowRate * timeElapsed));
        }
        return (baseSupplyIndex_, baseBorrowIndex_);
    }
function getSupplyRate(uint utilization) public view returns (uint64) {
        if (utilization <= supplyKink) {
            // interestRateBase + interestRateSlopeLow * utilization
            return safe64(supplyPerSecondInterestRateBase + mulFactor(supplyPerSecondInterestRateSlopeLow, utilization));
        } else {
            // interestRateBase + interestRateSlopeLow * kink + interestRateSlopeHigh * (utilization - kink)
            return safe64(supplyPerSecondInterestRateBase + mulFactor(supplyPerSecondInterestRateSlopeLow, supplyKink) + mulFactor(supplyPerSecondInterestRateSlopeHigh, (utilization - supplyKink)));
        }
    }
function getBorrowRate(uint utilization) public view returns (uint64) {
        if (utilization <= borrowKink) {
            // interestRateBase + interestRateSlopeLow * utilization
            return safe64(borrowPerSecondInterestRateBase + mulFactor(borrowPerSecondInterestRateSlopeLow, utilization));
        } else {
            // interestRateBase + interestRateSlopeLow * kink + interestRateSlopeHigh * (utilization - kink)
            return safe64(borrowPerSecondInterestRateBase + mulFactor(borrowPerSecondInterestRateSlopeLow, borrowKink) + mulFactor(borrowPerSecondInterestRateSlopeHigh, (utilization - borrowKink)));
        }
    }
function getUtilization() public view returns (uint) {
        uint totalSupply_ = presentValueSupply(baseSupplyIndex, totalSupplyBase);
        uint totalBorrow_ = presentValueBorrow(baseBorrowIndex, totalBorrowBase);
        if (totalSupply_ == 0) {
            return 0;
        } else {
            return totalBorrow_ * FACTOR_SCALE / totalSupply_;
        }
    }
function mulFactor(uint n, uint factor) internal pure returns (uint) {
        return n * factor / FACTOR_SCALE;
    }
function balanceOf(address account) public view returns (uint256) {
        (uint64 baseSupplyIndex_, ) = accruedInterestIndices(getNowInternal() - lastAccrualTime);
        int104 principal = boundPrincipal;
        return principal > 0 ? presentValueSupply(baseSupplyIndex_, unsigned104(principal)) : 0;
    }
function borrowBalanceOf(address account) public view returns (uint256) {
        (, uint64 baseBorrowIndex_) = accruedInterestIndices(getNowInternal() - lastAccrualTime);
        int104 principal = boundPrincipal;
        return principal < 0 ? presentValueBorrow(baseBorrowIndex_, unsigned104(-principal)) : 0;
    }

    function indices() external view returns (uint64, uint64) {
        return accruedInterestIndices(getNowInternal() - lastAccrualTime);
    }
    function presentSupply(uint64 index, uint104 principal) external pure returns (uint) {
        return presentValueSupply(index, principal);
    }
    function presentBorrow(uint64 index, uint104 principal) external pure returns (uint) {
        return presentValueBorrow(index, principal);
    }
    function principalSupply(uint64 index, uint present) external pure returns (uint104) {
        return principalValueSupply(index, present);
    }
    function principalBorrow(uint64 index, uint present) external pure returns (uint104) {
        return principalValueBorrow(index, present);
    }
}
