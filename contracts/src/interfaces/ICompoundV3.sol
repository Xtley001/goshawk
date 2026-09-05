// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title ICompoundV3 — Compound V3 Comet Absorption Interface
/// @notice 05_PROTOCOLS_AND_ADDRESSES.md §5.5, 08_EXECUTION_CONTRACT.md §8.4
interface ICompoundV3 {
    function absorb(address absorber, address[] calldata accounts) external;
}
