// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title IERC3156FlashBorrower — ERC-3156 Flash Loan Borrower Interface
/// @notice 07_FLASH_LOANS_AND_DEX.md §7.1, 08_EXECUTION_CONTRACT.md §8.2
interface IERC3156FlashBorrower {
    function onFlashLoan(
        address initiator,
        address token,
        uint256 amount,
        uint256 fee,
        bytes calldata data
    ) external returns (bytes32);
}

/// @title IERC3156FlashLender — ERC-3156 Flash Loan Lender Interface
/// @notice 07_FLASH_LOANS_AND_DEX.md §7.1, 08_EXECUTION_CONTRACT.md §8.2
interface IERC3156FlashLender {
    function flashLoan(
        IERC3156FlashBorrower receiver,
        address token,
        uint256 amount,
        bytes calldata data
    ) external returns (bool);
}
