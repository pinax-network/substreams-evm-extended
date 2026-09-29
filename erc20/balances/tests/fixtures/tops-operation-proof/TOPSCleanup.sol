// SPDX-License-Identifier: Unlicensed
pragma solidity ^0.8.19;
// Exact captured bodies; synthetic storage and entrypoint. Not the deployed runtime.
contract TOPSCleanup {
uint256[31] private reserved;
struct LPInfo {
        uint256 lpAmount;
        uint256 createdAt;
        uint256 expireAt;
    }
mapping(address => LPInfo[]) public lpInfos;
mapping(address => uint256) public lpAmount;
function _processExpiredLPInfo(address user) private {
        LPInfo[] storage userLPInfos = lpInfos[user];
        if (userLPInfos.length == 0) return;
        uint256 expiredAmount = 0;
        uint256 firstUnexpiredIndex = 0;
        uint256 currentTime = block.timestamp;
        uint256 arrayLength = userLPInfos.length;
        unchecked {
            for (uint256 i = 0; i < arrayLength; i++) {
                if (currentTime >= userLPInfos[i].expireAt) {
                    expiredAmount += userLPInfos[i].lpAmount;
                    firstUnexpiredIndex = i + 1;
                } else {
                    break;
                }
            }
        }
        if (expiredAmount > 0) {
            uint256 remainingCount = arrayLength - firstUnexpiredIndex;
            if (remainingCount > 0) {
                for (uint256 i = 0; i < remainingCount;) {
                    userLPInfos[i] = userLPInfos[firstUnexpiredIndex + i];
                    unchecked { i++; }
                }
            }
            while (userLPInfos.length > remainingCount) {
                userLPInfos.pop();
            }
            unchecked {
                lpAmount[user] += expiredAmount;
            }
        }
    }
function getWithdrawableLPAmount(address user) external view returns (uint256 totalWithdrawable) {
        totalWithdrawable = lpAmount[user];
        LPInfo[] storage userLPInfos = lpInfos[user];
        for (uint256 i = 0; i < userLPInfos.length; i++) {
            if (block.timestamp >= userLPInfos[i].expireAt) {
                totalWithdrawable += userLPInfos[i].lpAmount;
            } else {
                break;
            }
        }
        return totalWithdrawable;
    }
function getUserLPDetails(address user) external view returns (
        uint256 baseLPAmount,
        uint256 totalLocked,
        uint256 totalExpired,
        uint256 totalWithdrawable,
        uint256 lpInfoCount
    ) {
        baseLPAmount = lpAmount[user];
        lpInfoCount = lpInfos[user].length;
        LPInfo[] storage userLPInfos = lpInfos[user];
        for (uint256 i = 0; i < userLPInfos.length; i++) {
            if (block.timestamp >= userLPInfos[i].expireAt) {
                totalExpired += userLPInfos[i].lpAmount;
            } else {
                for (uint256 j = i; j < userLPInfos.length; j++) {
                    totalLocked += userLPInfos[j].lpAmount;
                }
                break;
            }
        }
        totalWithdrawable = baseLPAmount + totalExpired;
        return (baseLPAmount, totalLocked, totalExpired, totalWithdrawable, lpInfoCount);
    }
function process(address user) external { _processExpiredLPInfo(user); }
}
