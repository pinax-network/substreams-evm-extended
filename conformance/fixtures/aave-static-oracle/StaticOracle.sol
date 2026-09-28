// SPDX-License-Identifier: MIT
// Original StaticATokenLM conversion/preview/max function bodies with explicit
// controlled external/storage reads. Aave rate composition is independently pinned.
pragma solidity 0.8.27;
import {AaveOracle} from './AaveOracle.sol';
import {DataTypes} from './sources/current/src/contracts/protocol/libraries/types/DataTypes.sol';
import {RayMathExplicitRounding, Rounding} from './sources/static/src/RayMathExplicitRounding.sol';
contract StaticOracle is AaveOracle {
    using RayMathExplicitRounding for uint256;
    address constant _aTokenUnderlying = address(0);
    uint256 internal boundLiquidity;
    bool internal boundActive;
    bool internal boundPaused;
    function rate() public view returns (uint256) {
    return getNormalizedIncome(boundReserve);
  }
function convertToShares(uint256 assets) external view returns (uint256) {
    return _convertToShares(assets, Rounding.DOWN);
  }
function convertToAssets(uint256 shares) external view returns (uint256) {
    return _convertToAssets(shares, Rounding.DOWN);
  }
function previewDeposit(uint256 assets) public view virtual returns (uint256) {
    return _convertToShares(assets, Rounding.DOWN);
  }
function previewMint(uint256 shares) public view virtual returns (uint256) {
    return _convertToAssets(shares, Rounding.UP);
  }
function previewWithdraw(uint256 assets) public view virtual returns (uint256) {
    return _convertToShares(assets, Rounding.UP);
  }
function previewRedeem(uint256 shares) public view virtual returns (uint256) {
    return _convertToAssets(shares, Rounding.DOWN);
  }
function maxRedeem(address owner) public view virtual returns (uint256) {
    address cachedATokenUnderlying = _aTokenUnderlying;
    DataTypes.ReserveData memory reserveData = boundReserve;

    // if paused or inactive users cannot withdraw underlying
    if (
      !boundActive ||
      boundPaused
    ) {
      return 0;
    }

    // otherwise users can withdraw up to the available amount
    uint256 underlyingTokenBalanceInShares = _convertToShares(
      boundLiquidity,
      Rounding.DOWN
    );
    uint256 cachedUserBalance = boundShares;
    return
      underlyingTokenBalanceInShares >= cachedUserBalance
        ? cachedUserBalance
        : underlyingTokenBalanceInShares;
  }
function maxWithdraw(address owner) public view virtual returns (uint256) {
    uint256 shares = maxRedeem(owner);
    return _convertToAssets(shares, Rounding.DOWN);
  }
function _convertToShares(uint256 assets, Rounding rounding) internal view returns (uint256) {
    if (rounding == Rounding.UP) return assets.rayDivRoundUp(rate());
    return assets.rayDivRoundDown(rate());
  }
function _convertToAssets(uint256 shares, Rounding rounding) internal view returns (uint256) {
    if (rounding == Rounding.UP) return shares.rayMulRoundUp(rate());
    return shares.rayMulRoundDown(rate());
  }

    function mulDown(uint256 a, uint256 b) external pure returns (uint256) { return RayMathExplicitRounding.rayMulRoundDown(a, b); }
    function mulUp(uint256 a, uint256 b) external pure returns (uint256) { return RayMathExplicitRounding.rayMulRoundUp(a, b); }
    function divDown(uint256 a, uint256 b) external pure returns (uint256) { return RayMathExplicitRounding.rayDivRoundDown(a, b); }
    function divUp(uint256 a, uint256 b) external pure returns (uint256) { return RayMathExplicitRounding.rayDivRoundUp(a, b); }
}
