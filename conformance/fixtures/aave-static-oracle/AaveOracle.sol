// SPDX-License-Identifier: BUSL-1.1 AND MIT
// Extracted original Aave getter/index function bodies; see provenance.txt.
pragma solidity 0.8.27;
import {WadRayMath} from './sources/current/src/contracts/protocol/libraries/math/WadRayMath.sol';
import {MathUtils} from './sources/current/src/contracts/protocol/libraries/math/MathUtils.sol';
import {DataTypes} from './sources/current/src/contracts/protocol/libraries/types/DataTypes.sol';
import {TokenMath} from './sources/current/src/contracts/protocol/libraries/helpers/TokenMath.sol';
import {SafeCast} from './sources/oz/contracts/utils/math/SafeCast.sol';
contract AaveOracle {
    using WadRayMath for uint256;
using TokenMath for uint256; using SafeCast for uint256;
    DataTypes.ReserveData internal boundReserve;
    uint256 internal boundShares;
    function getNormalizedIncome(
    DataTypes.ReserveData storage reserve
  ) internal view returns (uint256) {
    uint40 timestamp = reserve.lastUpdateTimestamp;

    //solium-disable-next-line
    if (timestamp == block.timestamp) {
      //if the index was updated in the same block, no need to perform any calculation
      return reserve.liquidityIndex;
    } else {
      return
        MathUtils.calculateLinearInterest(reserve.currentLiquidityRate, timestamp).rayMul(
          reserve.liquidityIndex
        );
    }
  }
    function balanceOf(
    address user
  ) public view virtual  returns (uint256) {
    return
      boundShares.getATokenBalance(getNormalizedIncome(boundReserve));
  }
function _updateIndexes(
    DataTypes.ReserveData storage reserve,
    DataTypes.ReserveCache memory reserveCache
  ) internal {
    // Only cumulating on the supply side if there is any income being produced
    // The case of Reserve Factor 100% is not a problem (currentLiquidityRate == 0),
    // as liquidity index should not be updated
    if (reserveCache.currLiquidityRate != 0) {
      uint256 cumulatedLiquidityInterest = MathUtils.calculateLinearInterest(
        reserveCache.currLiquidityRate,
        reserveCache.reserveLastUpdateTimestamp
      );
      reserveCache.nextLiquidityIndex = cumulatedLiquidityInterest.rayMul(
        reserveCache.currLiquidityIndex
      );
      reserve.liquidityIndex = reserveCache.nextLiquidityIndex.toUint128();
    }

    // Variable borrow index only gets updated if there is any variable debt.
    // reserveCache.currVariableBorrowRate != 0 is not a correct validation,
    // because a positive base variable rate can be stored on
    // reserveCache.currVariableBorrowRate, but the index should not increase
    if (reserveCache.currScaledVariableDebt != 0) {
      uint256 cumulatedVariableBorrowInterest = MathUtils.calculateCompoundedInterest(
        reserveCache.currVariableBorrowRate,
        reserveCache.reserveLastUpdateTimestamp
      );
      reserveCache.nextVariableBorrowIndex = cumulatedVariableBorrowInterest.rayMul(
        reserveCache.currVariableBorrowIndex
      );
      reserve.variableBorrowIndex = reserveCache.nextVariableBorrowIndex.toUint128();
    }
  }
    // Construct the liquidity cache from controlled packed storage; all debt
    // fields are zero. This calls _updateIndexes, not the outer updateState.
    function updatedIndex() external returns (uint256) {
        DataTypes.ReserveCache memory cache;
        cache.currLiquidityIndex = boundReserve.liquidityIndex;
        cache.nextLiquidityIndex = boundReserve.liquidityIndex;
        cache.currLiquidityRate = boundReserve.currentLiquidityRate;
        cache.reserveLastUpdateTimestamp = boundReserve.lastUpdateTimestamp;
        _updateIndexes(boundReserve, cache);
        require(cache.nextLiquidityIndex == boundReserve.liquidityIndex);
        return boundReserve.liquidityIndex;
    }
    function mulFloor(uint256 a, uint256 b) external pure returns (uint256) { return WadRayMath.rayMulFloor(a, b); }
    function mulCeil(uint256 a, uint256 b) external pure returns (uint256) { return WadRayMath.rayMulCeil(a, b); }

    function normalizedIncome() public view returns (uint256) { return getNormalizedIncome(boundReserve); }
    function mulHalf(uint256 a, uint256 b) external pure returns (uint256) { return WadRayMath.rayMul(a, b); }
    function divHalf(uint256 a, uint256 b) external pure returns (uint256) { return WadRayMath.rayDiv(a, b); }
    function linear(uint256 rate, uint40 last) external view returns (uint256) { return MathUtils.calculateLinearInterest(rate, last); }
}
