// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

interface ISparkPool {
    function liquidationCall(
        address collateralAsset,
        address debtAsset,
        address user,
        uint256 debtToCover,
        bool receiveAToken
    ) external;
}
