// SPDX-License-Identifier: BUSL-1.1 AND MIT
// Extracted original Aave getter/index function bodies; see provenance.txt.
pragma solidity 0.8.27;
import {WadRayMath} from './sources/half-up/src/contracts/protocol/libraries/math/WadRayMath.sol';
import {MathUtils} from './sources/half-up/src/contracts/protocol/libraries/math/MathUtils.sol';
import {DataTypes} from './sources/half-up/src/contracts/protocol/libraries/types/DataTypes.sol';

contract HalfUpOracle {
    using WadRayMath for uint256;

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
    return boundShares.rayMul(getNormalizedIncome(boundReserve));
  }

    function normalizedIncome() public view returns (uint256) { return getNormalizedIncome(boundReserve); }
    function mulHalf(uint256 a, uint256 b) external pure returns (uint256) { return WadRayMath.rayMul(a, b); }
    function divHalf(uint256 a, uint256 b) external pure returns (uint256) { return WadRayMath.rayDiv(a, b); }
    function linear(uint256 rate, uint40 last) external view returns (uint256) { return MathUtils.calculateLinearInterest(rate, last); }
}
