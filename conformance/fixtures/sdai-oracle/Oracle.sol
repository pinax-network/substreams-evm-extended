// SPDX-License-Identifier: AGPL-3.0-or-later

/// SavingsDai.sol -- A tokenized representation DAI in the DSR (pot)

// Copyright (C) 2017, 2018, 2019 dbrock, rain, mrchico
// Copyright (C) 2021-2022 Dai Foundation
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.


pragma solidity 0.8.17;
// Controlled input substitutions only: Pot scalar calls and one holder
// mapping read. Source arithmetic, assembly and branch order stay intact.
contract Oracle {
    uint256 private constant RAY = 10 ** 27;
    uint256 boundChi;
    uint256 boundRho;
    uint256 boundDsr;
    uint256 boundShares;
function _rpow(uint256 x, uint256 n) internal pure returns (uint256 z) {
        assembly {
            switch x case 0 {switch n case 0 {z := RAY} default {z := 0}}
            default {
                switch mod(n, 2) case 0 { z := RAY } default { z := x }
                let half := div(RAY, 2)  // for rounding.
                for { n := div(n, 2) } n { n := div(n,2) } {
                    let xx := mul(x, x)
                    if iszero(eq(div(xx, x), x)) { revert(0,0) }
                    let xxRound := add(xx, half)
                    if lt(xxRound, xx) { revert(0,0) }
                    x := div(xxRound, RAY)
                    if mod(n,2) {
                        let zx := mul(z, x)
                        if and(iszero(iszero(x)), iszero(eq(div(zx, x), z))) { revert(0,0) }
                        let zxRound := add(zx, half)
                        if lt(zxRound, zx) { revert(0,0) }
                        z := div(zxRound, RAY)
                    }
                }
            }
        }
    }
function _divup(uint256 x, uint256 y) internal pure returns (uint256 z) {
        unchecked {
            z = x != 0 ? ((x - 1) / y) + 1 : 0;
        }
    }
function convertToAssets(uint256 shares) public view returns (uint256) {
        uint256 rho = boundRho;
        uint256 chi = (block.timestamp > rho) ? _rpow(boundDsr, block.timestamp - rho) * boundChi / RAY : boundChi;
        return shares * chi / RAY;
    }
function convertToShares(uint256 assets) public view returns (uint256) {
        uint256 rho = boundRho;
        uint256 chi = (block.timestamp > rho) ? _rpow(boundDsr, block.timestamp - rho) * boundChi / RAY : boundChi;
        return assets * RAY / chi;
    }
function previewDeposit(uint256 assets) external view returns (uint256) {
        return convertToShares(assets);
    }
function previewMint(uint256 shares) external view returns (uint256) {
        uint256 rho = boundRho;
        uint256 chi = (block.timestamp > rho) ? _rpow(boundDsr, block.timestamp - rho) * boundChi / RAY : boundChi;
        return _divup(shares * chi, RAY);
    }
function previewWithdraw(uint256 assets) external view returns (uint256) {
        uint256 rho = boundRho;
        uint256 chi = (block.timestamp > rho) ? _rpow(boundDsr, block.timestamp - rho) * boundChi / RAY : boundChi;
        return _divup(assets * RAY, chi);
    }
function previewRedeem(uint256 shares) external view returns (uint256) {
        return convertToAssets(shares);
    }
function maxWithdraw(address owner) external view returns (uint256) {
        return convertToAssets(boundShares);
    }

    function rpow(uint256 x, uint256 n) external pure returns (uint256) {
        return _rpow(x, n);
    }
    function divup(uint256 x, uint256 y) external pure returns (uint256) {
        return _divup(x, y);
    }
}
