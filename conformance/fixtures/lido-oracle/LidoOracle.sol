// SPDX-License-Identifier: GPL-3.0
// Extracted pinned-source getter harness; substitutions are listed in provenance.txt.
pragma solidity 0.4.24;
import './UnstructuredStorage.sol';
import './SafeMath.sol';
library PackedOracle {
using UnstructuredStorage for bytes32;
uint256 constant internal UINT128_LOW_MASK = ~uint128(0);
function getLowAndHighUint128(bytes32 position) internal view returns (uint256 low, uint256 high) {
        uint256 value = position.getStorageUint256();
        low = value & UINT128_LOW_MASK;
        high = value >> 128;
    }
}
contract LidoOracle {
using SafeMath for uint256;
using PackedOracle for bytes32;
uint256 constant internal UINT128_MAX = ~uint128(0);
bytes32 internal constant TOTAL_SHARES_POSITION_LOW128 =
        bytes32(0);
bytes32 internal constant BUFFERED_ETHER_AND_DEPOSITED_POST_REPORT_POSITION =
        bytes32(1);
bytes32 internal constant CL_VALIDATORS_BALANCE_AND_CL_PENDING_BALANCE_POSITION =
        bytes32(2);
bytes32 internal constant TOTAL_AND_EXTERNAL_SHARES_POSITION = TOTAL_SHARES_POSITION_LOW128;
function balanceOf(address _account) external view returns (uint256) {
        return getPooledEthByShares(_sharesOf(_account));
    }
function getSharesByPooledEth(uint256 _ethAmount) public view returns (uint256) {
        require(_ethAmount < UINT128_MAX, "ETH_TOO_LARGE");
        return (_ethAmount
            * _getShareRateDenominator()) // denominator in shares
            / _getShareRateNumerator(); // numerator in ether
    }
function getPooledEthByShares(uint256 _sharesAmount) public view returns (uint256) {
        require(_sharesAmount < UINT128_MAX, "SHARES_TOO_LARGE");
        return (_sharesAmount
            * _getShareRateNumerator()) // numerator in ether
            / _getShareRateDenominator(); // denominator in shares
    }
function getTotalPooledEther() external view returns (uint256) {
        return _getTotalPooledEther();
    }
function _getInternalEther() internal view returns (uint256) {
        (uint256 bufferedEther, uint256 depositedPostReport) = _getBufferedEtherAndDepositedPostReport();
        (uint256 clValidatorsBalance, uint256 clPendingBalance) = _getClValidatorsBalanceAndClPendingBalance();

        // With balance-based accounting, we don't need to calculate transientEther
        // as pending deposits are already included in clPendingBalance
        return bufferedEther.add(clValidatorsBalance).add(clPendingBalance).add(depositedPostReport);
    }
function _getExternalEther(uint256 _internalEther) internal view returns (uint256) {
        (uint256 totalShares, uint256 externalShares) = _getTotalAndExternalShares();
        uint256 internalShares = totalShares - externalShares;
        return (externalShares * _internalEther) / internalShares;
    }
function _getTotalPooledEther() internal view returns (uint256) {
        uint256 internalEther = _getInternalEther();
        return internalEther.add(_getExternalEther(internalEther));
    }
function _getShareRateNumerator() internal view returns (uint256) {
        return _getInternalEther();
    }
function _getShareRateDenominator() internal view returns (uint256) {
        (uint256 totalShares, uint256 externalShares) = _getTotalAndExternalShares();
        uint256 internalShares = totalShares - externalShares; // never 0 because of the stone in the elevator
        return internalShares;
    }
function _getTotalAndExternalShares() internal view returns (uint256, uint256) {
        return TOTAL_AND_EXTERNAL_SHARES_POSITION.getLowAndHighUint128();
    }
function _getBufferedEtherAndDepositedPostReport() internal view returns (uint256, uint256) {
        return BUFFERED_ETHER_AND_DEPOSITED_POST_REPORT_POSITION.getLowAndHighUint128();
    }
function _getClValidatorsBalanceAndClPendingBalance() internal view returns (uint256, uint256) {
        return CL_VALIDATORS_BALANCE_AND_CL_PENDING_BALANCE_POSITION.getLowAndHighUint128();
    }
function _sharesOf(address) internal view returns (uint256 value) { assembly { value := sload(3) } }
function internalEther() external view returns (uint256) { return _getInternalEther(); }
function internalShares() external view returns (uint256) { return _getShareRateDenominator(); }
function externalEther() external view returns (uint256) { return _getExternalEther(_getInternalEther()); }
}
