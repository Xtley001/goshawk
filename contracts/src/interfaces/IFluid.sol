// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title IFluid — Fluid Lending Liquidation Interface
/// @notice 05_PROTOCOLS_AND_ADDRESSES.md §5.4, 08_EXECUTION_CONTRACT.md §8.4
interface IFluid {
    function liquidate(
        address collateral,
        address debt,
        address user,
        uint256 debtToCover
    ) external;
}
