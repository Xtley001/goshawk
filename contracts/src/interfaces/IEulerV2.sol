// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title IEulerV2 — Euler V2 Vault Liquidation Interface
/// @notice 05_PROTOCOLS_AND_ADDRESSES.md §5.6, 08_EXECUTION_CONTRACT.md §8.4
interface IEulerV2 {
    function liquidateVault(
        address collateral,
        address debt,
        address borrower,
        uint256 debtToCover
    ) external;
}
